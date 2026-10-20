use super::*;

pub(super) fn builtin_member(ty: &Type, name: &str) -> bool {
    match ty {
        Type::Array(_) => matches!(
            name,
            "size" | "push" | "sorted" | "map" | "filter" | "forEach"
        ),
        Type::FixedArray(_, _) | Type::Text => name == "size",
        Type::Baddie(_) => matches!(name, "isSome" | "copium"),
        Type::Result(_) => matches!(name, "isOk" | "cope"),
        Type::ConversionError => name == "message",
        _ => false,
    }
}

impl Builder<'_> {
    pub(super) fn coerce_role(
        &mut self,
        value: Value,
        expected: &Type,
        span: Span,
    ) -> Result<Value> {
        if let (Some(class), Some(role)) = (value.ty.class_id(), expected.role_id())
            && self.tables.classes[class]
                .implementations
                .iter()
                .any(|implementation| implementation.role == role)
        {
            let slot = self.local(expected.clone());
            self.emit(
                Inst::IntoRole {
                    destination: slot,
                    receiver: value.slot,
                    class,
                    role,
                },
                span,
            );
            return Ok(Value {
                slot,
                ty: expected.clone(),
                constant: None,
            });
        }
        Ok(value)
    }

    pub(super) fn extension_id(&self, ty: &Type, name: &Name) -> Result<usize> {
        self.tables
            .extensions
            .iter()
            .find(|extension| {
                extension.receiver == *ty
                    && extension.name == modules::display_name(name, self.tables.project)
            })
            .map(|extension| extension.function)
            .ok_or_else(|| {
                error(
                    "LARP-T001",
                    name.span,
                    format!("Unknown member '{}' on {ty}", name.text),
                )
            })
    }

    pub(super) fn extension_call(
        &mut self,
        receiver: Value,
        name: &Name,
        arguments: &[Expr],
        span: Span,
        evaluate: bool,
    ) -> Result<Value> {
        let id = self.extension_id(&receiver.ty, name)?;
        self.receiver_call(receiver, id, arguments, span, evaluate)
    }

    pub(super) fn receiver_call(
        &mut self,
        receiver: Value,
        method: usize,
        arguments: &[Expr],
        span: Span,
        evaluate: bool,
    ) -> Result<Value> {
        let header = self.tables.headers[method].clone();
        if arguments.len() + 1 != header.parameters.len() {
            return Err(error("LARP-T006", span, "Incorrect method argument count"));
        }
        self.require(header.effects, span)?;
        let mut values = vec![receiver.slot];
        for (argument, ty) in arguments.iter().zip(&header.parameters[1..]) {
            values.push(self.expression(argument, Some(ty.clone()), evaluate)?.slot);
        }
        let slot = self.local(header.result.clone());
        self.emit(
            Inst::Call {
                destination: slot,
                function: method,
                arguments: values,
            },
            span,
        );
        Ok(Value {
            slot,
            ty: header.result,
            constant: None,
        })
    }

    fn role_method(&self, role: usize, name: &Name) -> Option<usize> {
        self.tables.roles[role]
            .methods
            .iter()
            .position(|method| method.name == name.text)
    }

    pub(super) fn role_call(
        &mut self,
        receiver: Value,
        name: &Name,
        arguments: &[Expr],
        span: Span,
        evaluate: bool,
    ) -> Result<Value> {
        let role = receiver.ty.role_id().unwrap();
        let Some(method) = self.role_method(role, name) else {
            return self.extension_call(receiver, name, arguments, span, evaluate);
        };
        let signature = self.tables.roles[role].methods[method].clone();
        if arguments.len() != signature.parameters.len() {
            return Err(error(
                "LARP-T006",
                span,
                "Incorrect role method argument count",
            ));
        }
        self.require(signature.effects, span)?;
        let arguments = arguments
            .iter()
            .zip(&signature.parameters)
            .map(|(argument, ty)| {
                self.expression(argument, Some(ty.clone()), evaluate)
                    .map(|value| value.slot)
            })
            .collect::<Result<Vec<_>>>()?;
        let slot = self.local(signature.result.clone());
        self.emit(
            Inst::CallRole {
                destination: slot,
                receiver: receiver.slot,
                role,
                method,
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

    pub(super) fn role_member(
        &mut self,
        receiver: Value,
        name: &Name,
        span: Span,
    ) -> Result<Value> {
        let role = receiver.ty.role_id().unwrap();
        let Some(method) = self.role_method(role, name) else {
            let id = self.extension_id(&receiver.ty, name)?;
            return self.bound_method(receiver, id, span);
        };
        let signature = self.tables.roles[role].methods[method].clone();
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
                    self.tables.roles[role].name,
                    name.text,
                    span.range().start
                ),
                span,
            },
            parameters: signature.parameters,
            result: signature.result,
            effects: signature.effects,
            owner: self.function.owner,
            constructor: None,
            scenario: self.function.scenario,
        };
        let mut child = Builder::new(&header, self.tables, span, id);
        let captured = child.local(receiver.ty.clone());
        child.function.captures.push(receiver.ty);
        let result = child.local(header.result.clone());
        child.emit(
            Inst::CallRole {
                destination: result,
                receiver: captured,
                role,
                method,
                arguments: (0..header.parameters.len()).collect(),
            },
            span,
        );
        child.terminate(Terminator::Return(Some(result)), span);
        self.generated.push(child.function);
        let ty = Type::function(header.parameters, header.result, header.effects);
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

    pub(super) fn extra_member_hint(&self, ty: &Type, name: &Name) -> Option<Type> {
        if let Some(role) = ty.role_id()
            && let Some(method) = self.role_method(role, name)
        {
            let signature = &self.tables.roles[role].methods[method];
            return Some(Type::function(
                signature.parameters.clone(),
                signature.result.clone(),
                signature.effects,
            ));
        }
        if builtin_member(ty, &name.text) {
            return self.builtin_member_hint(ty, name);
        }
        let id = self.extension_id(ty, name).ok()?;
        let header = &self.tables.headers[id];
        Some(Type::function(
            header.parameters[1..].to_vec(),
            header.result.clone(),
            header.effects,
        ))
    }
}
