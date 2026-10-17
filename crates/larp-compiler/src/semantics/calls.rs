use super::*;

pub(super) fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        "yap"
            | "no_cap"
            | "crashout"
            | "some"
            | "win"
            | "fumble"
            | "parseScore"
            | "ascend"
            | "fraud"
    )
}

impl Builder<'_> {
    pub(super) fn intrinsic(
        &mut self,
        operation: Intrinsic,
        arguments: Vec<Local>,
        span: Span,
    ) -> Result<Value> {
        let signature = operation
            .signature()
            .expect("compiler selects valid intrinsic types");
        self.require(signature.effects, span)?;
        let slot = self.local(signature.result.clone());
        self.emit(
            Inst::Intrinsic {
                destination: slot,
                operation,
                arguments,
            },
            span,
        );
        Ok(Value {
            slot,
            ty: signature.result,
            constant: None,
        })
    }

    fn arity(&self, arguments: &[Expr], count: usize, span: Span) -> Result<()> {
        if arguments.len() == count {
            Ok(())
        } else {
            Err(error(
                "LARP-T006",
                span,
                format!("Expected {count} arguments, found {}", arguments.len()),
            ))
        }
    }

    fn indirect_value(
        &mut self,
        callee: Value,
        arguments: Vec<Value>,
        span: Span,
    ) -> Result<Value> {
        let Type::Function(signature) = &callee.ty else {
            return Err(error(
                "LARP-T001",
                span,
                format!("{} is not callable", callee.ty),
            ));
        };
        self.require(signature.effects, span)?;
        let slot = self.local(signature.result.clone());
        self.emit(
            Inst::CallIndirect {
                destination: slot,
                callee: callee.slot,
                arguments: arguments.into_iter().map(|value| value.slot).collect(),
            },
            span,
        );
        Ok(Value {
            slot,
            ty: signature.result.clone(),
            constant: None,
        })
    }

    pub(super) fn call(
        &mut self,
        callee: &Expr,
        arguments: &[Expr],
        type_arguments: &[ast::TypeExpr],
        expected: Option<Type>,
        span: Span,
        evaluate: bool,
    ) -> Result<Value> {
        if let ExprKind::Member { receiver, name } = &callee.kind {
            if !type_arguments.is_empty() {
                return Err(error(
                    "LARP-P002",
                    span,
                    "Generic method calls are not implemented",
                ));
            }
            return self.method(receiver, name, arguments, span, evaluate);
        }
        if let ExprKind::Name(name) = &callee.kind
            && self.resolve(&name.text).is_none()
        {
            if let Some(index) = self.names.get(&name.text).copied() {
                if !type_arguments.is_empty() {
                    return Err(error(
                        "LARP-P002",
                        span,
                        "User-defined generic functions are not implemented",
                    ));
                }
                let header = self.headers[index].clone();
                self.arity(arguments, header.parameters.len(), span)?;
                self.require(header.effects, span)?;
                let arguments = arguments
                    .iter()
                    .zip(&header.parameters)
                    .map(|(argument, ty)| {
                        self.expression(argument, Some(ty.clone()), evaluate)
                            .map(|value| value.slot)
                    })
                    .collect::<Result<Vec<_>>>()?;
                let slot = self.local(header.result.clone());
                self.emit(
                    Inst::Call {
                        destination: slot,
                        function: index,
                        arguments,
                    },
                    span,
                );
                return Ok(Value {
                    slot,
                    ty: header.result,
                    constant: None,
                });
            }
            if is_builtin(&name.text) {
                return self.builtin(name, arguments, type_arguments, expected, span, evaluate);
            }
        }
        if !type_arguments.is_empty() {
            return Err(error(
                "LARP-P002",
                span,
                "Only ascend and fraud accept explicit call type arguments",
            ));
        }
        let callee = self.expression(callee, None, evaluate)?;
        let Type::Function(signature) = &callee.ty else {
            return Err(error(
                "LARP-T001",
                span,
                format!("{} is not callable", callee.ty),
            ));
        };
        self.arity(arguments, signature.parameters.len(), span)?;
        let arguments = arguments
            .iter()
            .zip(&signature.parameters)
            .map(|(argument, ty)| self.expression(argument, Some(ty.clone()), evaluate))
            .collect::<Result<Vec<_>>>()?;
        self.indirect_value(callee, arguments, span)
    }

    pub(super) fn builtin_reference(
        &mut self,
        name: &Name,
        expected: Option<Type>,
        span: Span,
    ) -> Result<Value> {
        let context = match &expected {
            Some(Type::Function(signature)) => Some(signature.clone()),
            Some(other) => {
                return Err(error(
                    "LARP-T001",
                    span,
                    format!("A function cannot initialize {other}"),
                ));
            }
            None => None,
        };
        let inferred = || {
            error(
                "LARP-T003",
                span,
                "This polymorphic builtin needs an expected function type",
            )
        };
        let operation = match name.text.as_str() {
            "parseScore" => Some(Intrinsic::ParseScore),
            "some" | "win" | "fumble" => {
                let signature = context.as_ref().ok_or_else(inferred)?;
                if signature.parameters.len() != 1 {
                    return Err(error(
                        "LARP-T006",
                        span,
                        "Builtin function requires one parameter",
                    ));
                }
                let result = signature.result.clone();
                let operation = match (name.text.as_str(), &result) {
                    ("some", Type::Baddie(_)) => Intrinsic::Some(result),
                    ("win", Type::Result(_)) => Intrinsic::Ok(result),
                    ("fumble", Type::Result(_)) => Intrinsic::Err(result),
                    _ => {
                        return Err(error(
                            "LARP-T001",
                            span,
                            "Expected function result does not match this constructor",
                        ));
                    }
                };
                Some(operation)
            }
            "ascend" | "fraud" => {
                let signature = context.as_ref().ok_or_else(inferred)?;
                if signature.parameters.len() != 1 {
                    return Err(error(
                        "LARP-T006",
                        span,
                        "Conversion function requires one parameter",
                    ));
                }
                let to = if name.text == "ascend" {
                    let Type::Result(payloads) = &signature.result else {
                        return Err(error(
                            "LARP-T001",
                            span,
                            "ascend returns result<T, ConversionError>",
                        ));
                    };
                    if payloads.1 != Type::ConversionError {
                        return Err(error(
                            "LARP-T001",
                            span,
                            "ascend errors have type ConversionError",
                        ));
                    }
                    payloads.0.clone()
                } else {
                    signature.result.clone()
                };
                let operation = Intrinsic::Convert {
                    from: signature.parameters[0].clone(),
                    to,
                    fallible: name.text == "ascend",
                };
                if operation.signature().is_none() {
                    return Err(error(
                        "LARP-T001",
                        span,
                        "Only mf/unc integer conversions are supported",
                    ));
                }
                Some(operation)
            }
            _ => None,
        };
        let (parameters, result, effects) = if let Some(operation) = &operation {
            let signature = operation.signature().unwrap();
            (signature.parameters, signature.result, signature.effects)
        } else {
            match name.text.as_str() {
                "no_cap" => (vec![Type::Verdict], Type::Unit, Effects::ABORT),
                "crashout" => (
                    vec![Type::Text],
                    context
                        .as_ref()
                        .map_or(Type::Unit, |signature| signature.result.clone()),
                    Effects::ABORT,
                ),
                "yap" => {
                    let signature = context.as_ref().ok_or_else(inferred)?;
                    if signature.parameters.len() != 1 || !signature.parameters[0].is_printable() {
                        return Err(error(
                            "LARP-T001",
                            span,
                            "yap requires one printable scalar parameter",
                        ));
                    }
                    (signature.parameters.clone(), Type::Unit, Effects::OBSERVE)
                }
                _ => unreachable!("known builtin reference"),
            }
        };
        let header = Header {
            name: Name {
                text: format!("{}@{}", name.text, span.range().start),
                span,
            },
            parameters,
            result,
            effects,
        };
        let mut child = Builder::new(
            &header,
            self.headers,
            self.names,
            span,
            self.generated_base + self.generated.len(),
        );
        if let Some(operation) = operation {
            let value = child.intrinsic(operation, vec![0], span)?;
            child.terminate(Terminator::Return(Some(value.slot)), span);
        } else {
            match name.text.as_str() {
                "yap" => child.emit(Inst::Print(0), span),
                "no_cap" => child.emit(Inst::Assert(0), span),
                "crashout" => child.terminate(Terminator::Crash(0), span),
                _ => unreachable!(),
            }
            child.finish();
        }
        let function_type = Type::function(header.parameters, header.result, header.effects);
        let function = self.generated_base + self.generated.len();
        if function >= 4096 {
            return Err(error(
                "LARP-P003",
                span,
                "Generated function limit exceeded",
            ));
        }
        self.generated.push(child.function);
        let slot = self.local(function_type.clone());
        self.emit(
            Inst::Closure {
                destination: slot,
                function,
                captures: Vec::new(),
            },
            span,
        );
        Ok(Value {
            slot,
            ty: function_type,
            constant: None,
        })
    }

    fn builtin(
        &mut self,
        name: &Name,
        arguments: &[Expr],
        type_arguments: &[ast::TypeExpr],
        expected: Option<Type>,
        span: Span,
        evaluate: bool,
    ) -> Result<Value> {
        self.arity(arguments, 1, span)?;
        if matches!(name.text.as_str(), "ascend" | "fraud") {
            if type_arguments.len() != 1 {
                return Err(error(
                    "LARP-T003",
                    span,
                    "Conversion requires one explicit target type",
                ));
            }
            let target = ty(&type_arguments[0])?;
            let source = self.expression(&arguments[0], None, evaluate)?;
            if !target.is_integer() || !source.ty.is_integer() {
                return Err(error(
                    "LARP-T001",
                    span,
                    "Only mf/unc integer conversions are supported",
                ));
            }
            return self.intrinsic(
                Intrinsic::Convert {
                    from: source.ty,
                    to: target,
                    fallible: name.text == "ascend",
                },
                vec![source.slot],
                span,
            );
        }
        if !type_arguments.is_empty() {
            return Err(error(
                "LARP-P002",
                span,
                "Only ascend and fraud accept explicit call type arguments",
            ));
        }
        if name.text == "some" {
            let context = match &expected {
                Some(Type::Baddie(payload)) => Some(payload.as_ref().clone()),
                Some(other) => {
                    return Err(error(
                        "LARP-T001",
                        span,
                        format!("some produces a baddie<T>, not {other}"),
                    ));
                }
                None => None,
            };
            let payload = self.expression(&arguments[0], context, evaluate)?;
            let option = expected.unwrap_or_else(|| Type::option(payload.ty.clone()));
            return self.intrinsic(Intrinsic::Some(option), vec![payload.slot], span);
        }
        if name.text == "win" || name.text == "fumble" {
            let result = match expected {
                Some(result @ Type::Result(_)) => result,
                Some(other) => {
                    return Err(error(
                        "LARP-T001",
                        span,
                        format!("{} produces a result<T,E>, not {other}", name.text),
                    ));
                }
                None => {
                    return Err(error(
                        "LARP-T003",
                        span,
                        "Result constructors require both expected payload types",
                    ));
                }
            };
            let Type::Result(payloads) = &result else {
                unreachable!()
            };
            let success = name.text == "win";
            let payload = self.expression(
                &arguments[0],
                Some(if success {
                    payloads.0.clone()
                } else {
                    payloads.1.clone()
                }),
                evaluate,
            )?;
            return self.intrinsic(
                if success {
                    Intrinsic::Ok(result)
                } else {
                    Intrinsic::Err(result)
                },
                vec![payload.slot],
                span,
            );
        }
        let context = match name.text.as_str() {
            "no_cap" => Some(Type::Verdict),
            "crashout" | "parseScore" => Some(Type::Text),
            _ => None,
        };
        let value = self.expression(&arguments[0], context, evaluate)?;
        match name.text.as_str() {
            "parseScore" => return self.intrinsic(Intrinsic::ParseScore, vec![value.slot], span),
            "yap" => {
                if !value.ty.is_printable() {
                    return Err(error(
                        "LARP-T001",
                        span,
                        "yap requires a scalar or text value",
                    ));
                }
                self.require(Effects::OBSERVE, span)?;
                self.emit(Inst::Print(value.slot), span);
            }
            "no_cap" => {
                self.require(Effects::ABORT, span)?;
                self.emit(Inst::Assert(value.slot), span);
            }
            "crashout" => {
                self.require(Effects::ABORT, span)?;
                self.terminate(Terminator::Crash(value.slot), span);
                self.current = self.new_block(span);
                let ty = expected.unwrap_or(Type::Unit);
                let slot = self.local(ty.clone());
                return Ok(Value {
                    slot,
                    ty,
                    constant: None,
                });
            }
            _ => unreachable!("known intrinsic name"),
        }
        Ok(self.literal(Scalar::Unit, span))
    }

    fn method(
        &mut self,
        receiver: &Expr,
        name: &Name,
        arguments: &[Expr],
        span: Span,
        evaluate: bool,
    ) -> Result<Value> {
        let receiver = self.expression(receiver, None, evaluate)?;
        match (&receiver.ty, name.text.as_str()) {
            (Type::Baddie(_), "isSome") => {
                self.arity(arguments, 0, span)?;
                self.intrinsic(Intrinsic::IsSome(receiver.ty), vec![receiver.slot], span)
            }
            (Type::Result(_), "isOk") => {
                self.arity(arguments, 0, span)?;
                self.intrinsic(Intrinsic::IsOk(receiver.ty), vec![receiver.slot], span)
            }
            (Type::Baddie(payload), "copium") => {
                self.arity(arguments, 1, span)?;
                let fallback =
                    self.expression(&arguments[0], Some(payload.as_ref().clone()), evaluate)?;
                self.intrinsic(
                    Intrinsic::Copium(receiver.ty),
                    vec![receiver.slot, fallback.slot],
                    span,
                )
            }
            (Type::Result(payloads), "cope") => {
                self.arity(arguments, 1, span)?;
                let (success_type, error_type) = (payloads.0.clone(), payloads.1.clone());
                let expected_handler =
                    Type::function(vec![error_type.clone()], success_type.clone(), Effects::ALL);
                // Evaluate the handler expression eagerly, then dispatch the result tag.
                let handler = self.expression(&arguments[0], Some(expected_handler), evaluate)?;
                let success = self.new_block(span);
                let failure = self.new_block(span);
                let done = self.new_block(span);
                let success_payload = self.local(success_type.clone());
                let error_payload = self.local(error_type.clone());
                let result = self.local(success_type.clone());
                self.function.blocks[success].parameter = Some(success_payload);
                self.function.blocks[failure].parameter = Some(error_payload);
                self.terminate(
                    Terminator::MatchResult {
                        value: receiver.slot,
                        success,
                        error: failure,
                    },
                    span,
                );
                self.current = success;
                self.emit(
                    Inst::Copy {
                        destination: result,
                        source: success_payload,
                    },
                    span,
                );
                self.terminate(Terminator::Goto(done), span);
                self.current = failure;
                let recovered = self.indirect_value(
                    handler,
                    vec![Value {
                        slot: error_payload,
                        ty: error_type,
                        constant: None,
                    }],
                    span,
                )?;
                self.emit(
                    Inst::Copy {
                        destination: result,
                        source: recovered.slot,
                    },
                    span,
                );
                self.terminate(Terminator::Goto(done), span);
                self.current = done;
                Ok(Value {
                    slot: result,
                    ty: success_type,
                    constant: None,
                })
            }
            _ => Err(error(
                "LARP-T001",
                name.span,
                format!("Unknown method '{}' on {}", name.text, receiver.ty),
            )),
        }
    }
}
