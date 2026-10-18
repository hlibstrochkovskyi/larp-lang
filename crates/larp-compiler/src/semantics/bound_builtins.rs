use super::*;

fn operation(ty: &Type, name: &str, callback: Option<Type>) -> Option<Intrinsic> {
    Some(match (ty, name) {
        (Type::Array(_) | Type::FixedArray(_, _) | Type::Text, "size") => {
            Intrinsic::Size(ty.clone())
        }
        (Type::Array(_), "push") => Intrinsic::Push(ty.clone()),
        (Type::Array(_), "sorted") => Intrinsic::Sorted(ty.clone()),
        (Type::Array(_), "map") => Intrinsic::Map {
            array: ty.clone(),
            callback: callback?,
        },
        (Type::Array(_), "filter") => Intrinsic::Filter {
            array: ty.clone(),
            callback: callback?,
        },
        (Type::Array(_), "forEach") => Intrinsic::ForEach {
            array: ty.clone(),
            callback: callback?,
        },
        (Type::Baddie(_), "isSome") => Intrinsic::IsSome(ty.clone()),
        (Type::Baddie(_), "copium") => Intrinsic::Copium(ty.clone()),
        (Type::Result(_), "isOk") => Intrinsic::IsOk(ty.clone()),
        _ => return None,
    })
}

impl Builder<'_> {
    pub(super) fn builtin_member_hint(&self, ty: &Type, name: &Name) -> Option<Type> {
        let signature = operation(ty, &name.text, None)?.signature()?;
        Some(Type::function(
            signature.parameters[1..].to_vec(),
            signature.result,
            signature.effects,
        ))
    }

    pub(super) fn bound_builtin(
        &mut self,
        receiver: Value,
        name: &Name,
        expected: Option<Type>,
        span: Span,
    ) -> Result<Value> {
        let polymorphic = matches!(name.text.as_str(), "map" | "filter" | "forEach" | "cope");
        let callback = if polymorphic {
            let Some(Type::Function(context)) = expected else {
                return Err(error(
                    "LARP-T003",
                    span,
                    "This polymorphic method needs an expected concrete function type",
                ));
            };
            if context.parameters.len() != 1 {
                return Err(error(
                    "LARP-T001",
                    span,
                    "A bound callback method takes one function argument",
                ));
            }
            Some(context.parameters[0].clone())
        } else {
            None
        };
        let intrinsic = operation(&receiver.ty, &name.text, callback.clone());
        let (parameters, result, effects) = if let Some(operation) = &intrinsic {
            let signature = operation
                .signature()
                .ok_or_else(|| error("LARP-T001", span, "Invalid bound method signature"))?;
            (
                signature.parameters[1..].to_vec(),
                signature.result,
                signature.effects,
            )
        } else if let (Type::Result(payload), "cope", Some(Type::Function(handler))) =
            (&receiver.ty, name.text.as_str(), &callback)
        {
            if handler.parameters.as_slice() != [payload.1.clone()] || handler.result != payload.0 {
                return Err(error(
                    "LARP-T001",
                    span,
                    "Recovery handler must map the error type to the success type",
                ));
            }
            (
                vec![callback.clone().unwrap()],
                payload.0.clone(),
                handler.effects,
            )
        } else {
            return Err(error(
                "LARP-T001",
                span,
                "This member cannot be extracted as a function",
            ));
        };
        let id = self.generated_base + self.generated.len();
        if id >= 4096 {
            return Err(error(
                "LARP-P003",
                span,
                "Generated function limit exceeded",
            ));
        }
        let header = Header {
            name: Name {
                text: format!(
                    "bound:{}::{}@{}",
                    receiver.ty,
                    name.text,
                    span.range().start
                ),
                span,
            },
            parameters,
            result,
            effects,
            owner: self.function.owner,
            constructor: None,
            scenario: self.function.scenario,
        };
        let mut child = Builder::new(&header, self.tables, span, id);
        let capture = child.local(receiver.ty.clone());
        child.function.captures.push(receiver.ty.clone());
        let result = if let Some(operation) = intrinsic {
            let mut arguments = vec![capture];
            arguments.extend(0..header.parameters.len());
            child.intrinsic(operation, arguments, span)?
        } else {
            let Type::Result(payload) = &receiver.ty else {
                unreachable!()
            };
            child.cope_values(
                Value {
                    slot: capture,
                    ty: receiver.ty.clone(),
                    constant: None,
                },
                Value {
                    slot: 0,
                    ty: callback.unwrap(),
                    constant: None,
                },
                payload.0.clone(),
                payload.1.clone(),
                span,
            )?
        };
        child.terminate(Terminator::Return(Some(result.slot)), span);
        let ty = Type::function(header.parameters, header.result, header.effects);
        if !ty.within_limits() {
            return Err(error(
                "LARP-P003",
                span,
                "Bound method type complexity limit exceeded",
            ));
        }
        self.generated.push(child.function);
        let slot = self.local(ty.clone());
        self.emit(
            Inst::Closure {
                destination: slot,
                function: id,
                captures: vec![receiver.slot],
            },
            span,
        );
        Ok(Value {
            slot,
            ty,
            constant: None,
        })
    }
}
