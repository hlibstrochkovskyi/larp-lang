use super::*;

enum Place {
    Local(Binding),
    Field {
        receiver: Value,
        class: usize,
        field: usize,
        ty: Type,
    },
    Element {
        container: Value,
        index: Value,
        parent: Box<Place>,
    },
}

impl Place {
    fn ty(&self) -> Type {
        match self {
            Self::Local(binding) => binding.ty.clone(),
            Self::Field { ty, .. } => ty.clone(),
            Self::Element { container, .. } => container.ty.element().unwrap().clone(),
        }
    }
    fn writable(&self) -> bool {
        match self {
            Self::Local(binding) => binding.writable,
            Self::Field { .. } => true,
            Self::Element {
                container, parent, ..
            } => matches!(container.ty, Type::Array(_)) || parent.writable(),
        }
    }
}

impl Builder<'_> {
    pub(super) fn array_literal(
        &mut self,
        elements: &[Expr],
        expected: Option<Type>,
        span: Span,
        evaluate: bool,
    ) -> Result<Value> {
        if elements.len() > 65_536 {
            return Err(error(
                "LARP-P003",
                span,
                "Array literal exceeds the operand limit",
            ));
        }
        let mut array_type = match expected {
            Some(ty @ (Type::Array(_) | Type::FixedArray(_, _))) => Some(ty),
            Some(other) => {
                return Err(error(
                    "LARP-T001",
                    span,
                    format!("An array cannot initialize {other}"),
                ));
            }
            None => None,
        };
        if let Some(Type::FixedArray(_, length)) = &array_type
            && *length != elements.len()
        {
            return Err(error(
                "LARP-T001",
                span,
                format!("Expected {length} array elements, found {}", elements.len()),
            ));
        }
        let mut values = Vec::new();
        for element in elements {
            let context = array_type.as_ref().and_then(|ty| ty.element().cloned());
            let value = self.expression(element, context, evaluate)?;
            if array_type.is_none() {
                array_type = Some(Type::array(value.ty.clone()));
            }
            values.push(value.slot);
        }
        let array_type = array_type.ok_or_else(|| {
            error(
                "LARP-T003",
                span,
                "An empty array requires an expected element type",
            )
        })?;
        if !array_type.within_limits() {
            return Err(error(
                "LARP-P003",
                span,
                "Array element type exceeds complexity limits",
            ));
        }
        if !array_type.is_well_formed() {
            return Err(error(
                "LARP-T001",
                span,
                "Array elements cannot have type unit",
            ));
        }
        self.intrinsic(
            Intrinsic::ArrayNew {
                ty: array_type,
                count: elements.len(),
            },
            values,
            span,
        )
    }

    pub(super) fn array_method(
        &mut self,
        receiver: Value,
        name: &Name,
        arguments: &[Expr],
        expected: Option<Type>,
        span: Span,
        evaluate: bool,
    ) -> Result<Value> {
        let wrong_arity = |count| {
            error(
                "LARP-T006",
                span,
                format!("Expected {count} arguments, found {}", arguments.len()),
            )
        };
        match name.text.as_str() {
            "size" => {
                if !arguments.is_empty() {
                    return Err(wrong_arity(0));
                }
                self.intrinsic(Intrinsic::Size(receiver.ty), vec![receiver.slot], span)
            }
            "push" if matches!(receiver.ty, Type::Array(_)) => {
                if arguments.len() != 1 {
                    return Err(wrong_arity(1));
                }
                let value =
                    self.expression(&arguments[0], receiver.ty.element().cloned(), evaluate)?;
                self.intrinsic(
                    Intrinsic::Push(receiver.ty),
                    vec![receiver.slot, value.slot],
                    span,
                )
            }
            "sorted" if matches!(receiver.ty, Type::Array(_)) => {
                if !arguments.is_empty() {
                    return Err(wrong_arity(0));
                }
                let operation = Intrinsic::Sorted(receiver.ty);
                if operation.signature().is_none() {
                    return Err(error(
                        "LARP-T001",
                        span,
                        "sorted requires integer or text elements",
                    ));
                }
                self.intrinsic(operation, vec![receiver.slot], span)
            }
            "map" | "filter" | "forEach" if matches!(receiver.ty, Type::Array(_)) => {
                if arguments.len() != 1 {
                    return Err(wrong_arity(1));
                }
                let result_context = match name.text.as_str() {
                    "filter" => Some(Type::Verdict),
                    "forEach" => Some(Type::Unit),
                    _ => match expected {
                        Some(Type::Array(element)) => Some(element.as_ref().clone()),
                        _ => None,
                    },
                };
                let parameter = receiver.ty.element().unwrap().clone();
                let callback_type = result_context.as_ref().map(|result| {
                    Type::function(vec![parameter.clone()], result.clone(), Effects::ALL)
                });
                let callback = if callback_type.is_none()
                    && let ExprKind::Lambda { parameters, body } = &arguments[0].kind
                {
                    self.lambda_context(
                        parameters,
                        body,
                        Some(Type::function(
                            vec![parameter.clone()],
                            Type::Unit,
                            Effects::ALL,
                        )),
                        arguments[0].span,
                        true,
                    )?
                } else {
                    self.expression(&arguments[0], callback_type, evaluate)?
                };
                let Type::Function(signature) = &callback.ty else {
                    return Err(error(
                        "LARP-T001",
                        arguments[0].span,
                        "A collection callback must be a function",
                    ));
                };
                if signature.parameters.as_slice() != [parameter]
                    || signature.result == Type::Unit && name.text == "map"
                {
                    return Err(error(
                        "LARP-T001",
                        arguments[0].span,
                        "Collection callback has incompatible parameter or result types",
                    ));
                }
                let operation = match name.text.as_str() {
                    "map" => Intrinsic::Map {
                        array: receiver.ty,
                        callback: callback.ty,
                    },
                    "filter" => Intrinsic::Filter {
                        array: receiver.ty,
                        callback: callback.ty,
                    },
                    _ => Intrinsic::ForEach {
                        array: receiver.ty,
                        callback: callback.ty,
                    },
                };
                self.intrinsic(operation, vec![receiver.slot, callback.slot], span)
            }
            _ => Err(error(
                "LARP-T001",
                name.span,
                format!("{} has no method '{}'", receiver.ty, name.text),
            )),
        }
    }

    fn prepare_place(&mut self, target: &Expr) -> Result<Place> {
        match &target.kind {
            ExprKind::SelfValue => {
                if self.function.constructor.is_some() {
                    return Err(error(
                        "LARP-T012",
                        target.span,
                        "A partially initialized self is not an assignable value",
                    ));
                }
                self.resolve("self").map(Place::Local).ok_or_else(|| {
                    error(
                        "LARP-T001",
                        target.span,
                        "self is available only in an instance method",
                    )
                })
            }
            ExprKind::Member { receiver, name } => {
                if self.function.constructor.is_some()
                    && matches!(receiver.kind, ExprKind::SelfValue)
                {
                    return Ok(Place::Local(self.constructor_field(name)?));
                }
                let receiver = self.expression(receiver, None, true)?;
                let class = receiver.ty.class_id().ok_or_else(|| {
                    error(
                        "LARP-T001",
                        name.span,
                        "Only archetype fields can be assigned through member access",
                    )
                })?;
                let field = self.class_field(class, name)?;
                let ty = self.tables.classes[class].fields[field].ty.clone();
                Ok(Place::Field {
                    receiver,
                    class,
                    field,
                    ty,
                })
            }
            ExprKind::Name(name) => self.resolve(&name.text).map(Place::Local).ok_or_else(|| {
                error(
                    "LARP-N001",
                    name.span,
                    format!("Unknown binding '{}'", name.text),
                )
            }),
            ExprKind::Index { receiver, index } => {
                let parent = self.prepare_place(receiver)?;
                let container = self.read_place(&parent, receiver.span)?;
                if container.ty.element().is_none() {
                    return Err(error(
                        "LARP-T001",
                        receiver.span,
                        "An indexed assignment requires an array",
                    ));
                }
                let index = self.expression(index, Some(Type::Mf), true)?;
                self.intrinsic(
                    Intrinsic::CheckIndex(container.ty.clone()),
                    vec![container.slot, index.slot],
                    target.span,
                )?;
                Ok(Place::Element {
                    container,
                    index,
                    parent: Box::new(parent),
                })
            }
            _ => Err(error(
                "LARP-P002",
                target.span,
                "Object member assignment is not implemented yet",
            )),
        }
    }

    fn place_container(&mut self, container: &Value, parent: &Place, span: Span) -> Result<Value> {
        if matches!(container.ty, Type::FixedArray(_, _)) {
            self.read_place(parent, span)
        } else {
            Ok(container.clone())
        }
    }

    fn read_place(&mut self, place: &Place, span: Span) -> Result<Value> {
        match place {
            Place::Local(binding) => Ok(self.read(binding, span)),
            Place::Field {
                receiver,
                class,
                field,
                ..
            } => self.get_field(receiver.clone(), *class, *field, span),
            Place::Element {
                container,
                index,
                parent,
            } => {
                let container = self.place_container(container, parent, span)?;
                self.intrinsic(
                    Intrinsic::Index(container.ty),
                    vec![container.slot, index.slot],
                    span,
                )
            }
        }
    }

    fn write_place(&mut self, place: &Place, value: Value, span: Span) -> Result<()> {
        match place {
            Place::Field {
                receiver,
                class,
                field,
                ..
            } => self.set_field(receiver.clone(), *class, *field, value, span)?,
            Place::Local(binding) => self.emit(
                Inst::Copy {
                    destination: binding.slot,
                    source: value.slot,
                },
                span,
            ),
            Place::Element {
                container,
                index,
                parent,
            } => {
                let container = self.place_container(container, parent, span)?;
                let fixed = matches!(container.ty, Type::FixedArray(_, _));
                let updated = self.intrinsic(
                    Intrinsic::SetIndex(container.ty),
                    vec![container.slot, index.slot, value.slot],
                    span,
                )?;
                if fixed {
                    self.write_place(parent, updated, span)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn assignment(
        &mut self,
        target: &Expr,
        op: Option<Binary>,
        value: &Expr,
        span: Span,
    ) -> Result<()> {
        let place = self.prepare_place(target)?;
        if !place.writable() {
            return Err(error(
                "LARP-T008",
                target.span,
                "Captured value bindings and range-loop indices are read-only",
            ));
        }
        let previous = if op.is_some() {
            Some(self.read_place(&place, target.span)?)
        } else {
            None
        };
        let value = self.expression(value, Some(place.ty()), true)?;
        let value = if let Some(op) = op {
            self.binary(op, previous.unwrap(), value, span, true)?
        } else {
            value
        };
        self.write_place(&place, value, span)
    }
}
