use super::*;
use std::collections::VecDeque;

impl Builder<'_> {
    pub(super) fn self_value(&mut self, span: Span) -> Result<Value> {
        if self.function.constructor.is_some() {
            return Err(error(
                "LARP-T012",
                span,
                "A partially initialized self cannot be used as a value, called, or captured",
            ));
        }
        if self.scenario_forbidden.contains("self") && self.resolve("self").is_none() {
            return Err(error(
                "LARP-S002",
                span,
                "An outer self reference cannot be read inside a scenario",
            ));
        }
        let binding = self.resolve("self").ok_or_else(|| {
            error(
                "LARP-T001",
                span,
                "self is available only in an instance method or its lexical closures",
            )
        })?;
        self.read(&binding, span)
    }

    pub(super) fn class_field(&self, class: usize, name: &Name) -> Result<usize> {
        let field = self.tables.classes[class]
            .fields
            .iter()
            .position(|field| field.name == name.text)
            .ok_or_else(|| {
                error(
                    "LARP-T001",
                    name.span,
                    format!(
                        "{} has no field '{}'",
                        self.tables.classes[class].name, name.text
                    ),
                )
            })?;
        if !self.tables.classes[class].fields[field].public && self.function.owner != Some(class) {
            return Err(error(
                "LARP-T010",
                name.span,
                "A lowkey field is accessible only inside its declaring class",
            ));
        }
        Ok(field)
    }

    pub(super) fn constructor_field(&self, name: &Name) -> Result<Binding> {
        let class = self.function.constructor.unwrap();
        if self.tables.classes[class]
            .methods
            .iter()
            .any(|method| method.name == name.text)
        {
            return Err(error(
                "LARP-T012",
                name.span,
                "A constructor cannot extract a method from a partially initialized self",
            ));
        }
        let field = self.class_field(class, name)?;
        Ok(self.constructor_fields[field].clone())
    }

    pub(super) fn constructor_field_read(&mut self, name: &Name, span: Span) -> Result<Value> {
        let field = self.constructor_field(name)?;
        self.require(Effects::READ, span)?;
        self.read(&field, span)
    }

    pub(super) fn construct(
        &mut self,
        name: &Name,
        arguments: &[Expr],
        span: Span,
        evaluate: bool,
    ) -> Result<Value> {
        if self.function.scenario.is_some() {
            return Err(error(
                "LARP-E002",
                span,
                "Reference allocation is forbidden inside a scenario",
            ));
        }
        let ty = self.tables.types.get(&name.text).cloned().ok_or_else(|| {
            error(
                "LARP-T005",
                name.span,
                "new requires a declared archetype name",
            )
        })?;
        let class = ty.class_id().ok_or_else(|| {
            error(
                "LARP-T005",
                name.span,
                "new requires an archetype, not a role",
            )
        })?;
        let header = self.tables.headers[self.tables.classes[class].constructor].clone();
        if arguments.len() != header.parameters.len() {
            return Err(error(
                "LARP-T006",
                span,
                format!(
                    "Expected {} constructor arguments, found {}",
                    header.parameters.len(),
                    arguments.len()
                ),
            ));
        }
        // Constructor writes initialize fresh storage and are the sole permitted
        // write-bound call from another constructor.
        if self.function.constructor.is_some() {
            self.required = self.required.union(header.effects);
        } else {
            self.require(header.effects, span)?;
        }
        let arguments = arguments
            .iter()
            .zip(&header.parameters)
            .map(|(argument, ty)| {
                self.expression(argument, Some(ty.clone()), evaluate)
                    .map(|value| value.slot)
            })
            .collect::<Result<Vec<_>>>()?;
        let slot = self.local(ty.clone());
        self.emit(
            Inst::Construct {
                destination: slot,
                class,
                arguments,
            },
            span,
        );
        Ok(Value {
            slot,
            ty,
            constant: None,
        })
    }

    pub(super) fn get_field(
        &mut self,
        receiver: Value,
        class: usize,
        field: usize,
        span: Span,
    ) -> Result<Value> {
        self.require(Effects::READ, span)?;
        let ty = self.tables.classes[class].fields[field].ty.clone();
        let slot = self.local(ty.clone());
        self.emit(
            Inst::GetField {
                destination: slot,
                receiver: receiver.slot,
                class,
                field,
            },
            span,
        );
        Ok(Value {
            slot,
            ty,
            constant: None,
        })
    }

    pub(super) fn set_field(
        &mut self,
        receiver: Value,
        class: usize,
        field: usize,
        value: Value,
        span: Span,
    ) -> Result<()> {
        self.require(Effects::WRITE, span)?;
        let unit = self.local(Type::Unit);
        self.emit(
            Inst::SetField {
                destination: unit,
                receiver: receiver.slot,
                class,
                field,
                value: value.slot,
            },
            span,
        );
        Ok(())
    }

    fn method_id(&self, class: usize, name: &Name) -> Result<Option<usize>> {
        let Some(method) = self.tables.classes[class]
            .methods
            .iter()
            .find(|method| method.name == name.text)
        else {
            return Ok(None);
        };
        if !method.public && self.function.owner != Some(class) {
            return Err(error(
                "LARP-T010",
                name.span,
                "A lowkey method is accessible only inside its declaring class",
            ));
        }
        Ok(Some(method.function))
    }

    pub(super) fn member_value(
        &mut self,
        receiver: Value,
        name: &Name,
        expected: Option<Type>,
        span: Span,
    ) -> Result<Value> {
        if receiver.ty.role_id().is_some() {
            return self.role_member(receiver, name, span);
        }
        let Some(class) = receiver.ty.class_id() else {
            if roles::builtin_member(&receiver.ty, &name.text) {
                return self.bound_builtin(receiver, name, expected, span);
            }
            let id = self.extension_id(&receiver.ty, name)?;
            return self.bound_method(receiver, id, span);
        };
        if self.tables.classes[class]
            .fields
            .iter()
            .any(|field| field.name == name.text)
        {
            let field = self.class_field(class, name)?;
            return self.get_field(receiver, class, field, span);
        }
        let method = match self.method_id(class, name)? {
            Some(id) => id,
            None => self.extension_id(&receiver.ty, name)?,
        };
        if self.function.scenario.is_some() {
            return Err(error(
                "LARP-S004",
                span,
                "A scenario root method cannot be extracted as a bound function",
            ));
        }
        self.bound_method(receiver, method, span)
    }

    pub(super) fn object_call(
        &mut self,
        receiver: Value,
        name: &Name,
        arguments: &[Expr],
        span: Span,
        evaluate: bool,
    ) -> Result<Value> {
        let class = receiver.ty.class_id().unwrap();
        if self.tables.classes[class]
            .fields
            .iter()
            .any(|field| field.name == name.text)
        {
            let field = self.class_field(class, name)?;
            let callee = self.get_field(receiver, class, field, span)?;
            return self.call_value(callee, arguments, span, evaluate);
        }
        let method = match self.method_id(class, name)? {
            Some(id) => id,
            None => self.extension_id(&receiver.ty, name)?,
        };
        self.receiver_call(receiver, method, arguments, span, evaluate)
    }

    pub(super) fn bound_method(
        &mut self,
        receiver: Value,
        method: usize,
        span: Span,
    ) -> Result<Value> {
        let target = self.tables.headers[method].clone();
        let header = Header {
            name: Name {
                text: format!("bound:{}@{}", target.name.text, span.range().start),
                span,
            },
            parameters: target.parameters[1..].to_vec(),
            result: target.result.clone(),
            effects: target.effects,
            owner: self.function.owner,
            constructor: None,
            scenario: self.function.scenario,
        };
        let mut child = Builder::new(
            &header,
            self.tables,
            span,
            self.generated_base + self.generated.len(),
        );
        let receiver_slot = child.local(receiver.ty.clone());
        child.function.captures.push(receiver.ty);
        let slot = child.local(header.result.clone());
        let mut arguments = vec![receiver_slot];
        arguments.extend(0..header.parameters.len());
        child.emit(
            Inst::Call {
                destination: slot,
                function: method,
                arguments,
            },
            span,
        );
        child.terminate(Terminator::Return(Some(slot)), span);
        let id = self.generated_base + self.generated.len();
        if id >= 4096 {
            return Err(error(
                "LARP-P003",
                span,
                "Generated function limit exceeded",
            ));
        }
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

    pub(super) fn member_hint(&self, ty: &Type, name: &Name) -> Option<Type> {
        let Some(class) = ty.class_id() else {
            return self.extra_member_hint(ty, name);
        };
        let layout = &self.tables.classes[class];
        if let Some(field) = layout.fields.iter().find(|field| field.name == name.text) {
            return Some(field.ty.clone());
        }
        let Some(method) = layout
            .methods
            .iter()
            .find(|method| method.name == name.text)
        else {
            return self.extra_member_hint(ty, name);
        };
        let header = &self.tables.headers[method.function];
        Some(Type::function(
            header.parameters[1..].to_vec(),
            header.result.clone(),
            header.effects,
        ))
    }

    pub(super) fn check_initialization(&mut self) {
        let count = self.constructor_fields.len();
        if self.function.blocks.len().saturating_mul(count) > 4_000_000 {
            self.errors.push(*error(
                "LARP-P003",
                self.function.span,
                "Constructor initialization analysis budget exceeded",
            ));
            return;
        }
        let field_ids: HashMap<_, _> = self
            .constructor_fields
            .iter()
            .enumerate()
            .map(|(index, field)| (field.slot, index))
            .collect();
        let mut states: Vec<Option<Vec<bool>>> = vec![None; self.function.blocks.len()];
        states[0] = Some(vec![false; count]);
        let mut queue = VecDeque::from([0]);
        while let Some(id) = queue.pop_front() {
            let mut state = states[id].clone().unwrap();
            for instruction in &self.function.blocks[id].instructions {
                if let Inst::Copy { destination, .. } = instruction.kind
                    && let Some(field) = field_ids.get(&destination)
                {
                    state[*field] = true;
                }
            }
            let targets = match self.function.blocks[id].terminator {
                Terminator::Goto(target) => vec![target],
                Terminator::Branch {
                    when_true,
                    when_false,
                    ..
                } => vec![when_true, when_false],
                Terminator::MatchResult { success, error, .. } => vec![success, error],
                _ => vec![],
            };
            for next in targets {
                let changed = if let Some(previous) = &mut states[next] {
                    let mut changed = false;
                    for (old, incoming) in previous.iter_mut().zip(&state) {
                        if *old && !incoming {
                            *old = false;
                            changed = true;
                        }
                    }
                    changed
                } else {
                    states[next] = Some(state.clone());
                    true
                };
                if changed {
                    queue.push_back(next);
                }
            }
        }
        for (block, state) in self.function.blocks.iter().zip(states) {
            let Some(mut state) = state else {
                continue;
            };
            for instruction in &block.instructions {
                if let Inst::Copy {
                    destination,
                    source,
                } = instruction.kind
                {
                    if let Some(field) = field_ids.get(&source)
                        && !state[*field]
                    {
                        self.errors.push(*error(
                            "LARP-T011",
                            instruction.span,
                            "A field must be assigned on every path before it is read",
                        ));
                        return;
                    }
                    if let Some(field) = field_ids.get(&destination) {
                        state[*field] = true;
                    }
                }
            }
            if matches!(block.terminator, Terminator::ConstructReturn { .. })
                && state.contains(&false)
            {
                let field = state.iter().position(|initialized| !initialized).unwrap();
                let mut diagnostic = error(
                    "LARP-T011",
                    block.span,
                    "Every field must be assigned on every successful constructor path",
                );
                diagnostic.related.push(Label {
                    span: self.constructor_fields[field].span,
                    message: "This field is not definitely initialized".into(),
                });
                self.errors.push(*diagnostic);
                return;
            }
        }
    }
}
