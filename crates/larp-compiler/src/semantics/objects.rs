use super::*;
use std::collections::{HashSet, VecDeque};

fn constructor_effects() -> Effects {
    Effects::READ.union(Effects::WRITE).union(Effects::ABORT)
}

fn parameters(
    parameters: &[(ast::TypeExpr, Name)],
    class_types: &HashMap<String, Type>,
) -> Result<Vec<Type>> {
    parameters
        .iter()
        .map(|(expression, name)| {
            let result = ty(expression, class_types)?;
            if result == Type::Unit {
                return Err(error(
                    "LARP-T001",
                    name.span,
                    "Function parameters cannot have type unit",
                ));
            }
            Ok(result)
        })
        .collect()
}

pub(super) fn check_program(ast: &ast::Program) -> std::result::Result<Program, Vec<Diagnostic>> {
    let mut body_errors = Vec::new();
    let result = (|| -> Result<Program> {
        if ast.classes.len() > 4096 {
            return Err(error(
                "LARP-P003",
                ast.main.span,
                "Class count limit exceeded",
            ));
        }
        let mut class_types = HashMap::new();
        for (index, class) in ast.classes.iter().enumerate() {
            if Type::from_name(&class.name.text).is_some()
                || matches!(
                    class.name.text.as_str(),
                    "mfs" | "hediJeans" | "baddie" | "result"
                )
                || class_types
                    .insert(
                        class.name.text.clone(),
                        Type::class(index, class.name.text.as_str()),
                    )
                    .is_some()
            {
                return Err(error(
                    "LARP-N002",
                    class.name.span,
                    format!("Duplicate or reserved type name '{}'", class.name.text),
                ));
            }
        }
        let mut classes = Vec::new();
        let mut members = 0usize;
        for class in &ast.classes {
            if class.constructors.len() != 1 {
                return Err(error(
                    "LARP-T011",
                    class.span,
                    "An archetype must declare exactly one init constructor",
                ));
            }
            members += class.fields.len() + class.methods.len();
            if members > 65_536 {
                return Err(error(
                    "LARP-P003",
                    class.span,
                    "Class member limit exceeded",
                ));
            }
            let mut seen = HashSet::new();
            let mut fields = Vec::new();
            for field in &class.fields {
                if !seen.insert(field.name.text.clone()) {
                    return Err(error(
                        "LARP-N002",
                        field.name.span,
                        "Duplicate class member name",
                    ));
                }
                let field_type = ty(&field.ty, &class_types)?;
                if field_type == Type::Unit {
                    return Err(error(
                        "LARP-T001",
                        field.name.span,
                        "A field cannot have type unit",
                    ));
                }
                fields.push(larp_bytecode::Field {
                    name: field.name.text.clone(),
                    ty: field_type,
                    public: field.public,
                });
            }
            for method in &class.methods {
                if !seen.insert(method.function.name.text.clone()) {
                    return Err(error(
                        "LARP-N002",
                        method.function.name.span,
                        "Fields and methods must have distinct names",
                    ));
                }
            }
            classes.push(larp_bytecode::Class {
                name: class.name.text.clone(),
                fields,
                methods: Vec::new(),
                constructor: usize::MAX,
            });
        }
        let mut headers = Vec::new();
        let mut bodies = Vec::new();
        let mut parameter_names = Vec::new();
        let mut names = HashMap::new();
        for function in &ast.functions {
            if let Some(previous) = names.get(&function.name.text).copied() {
                let mut diagnostic =
                    error("LARP-N002", function.name.span, "Duplicate function name");
                let previous: usize = previous;
                diagnostic.related.push(Label {
                    span: ast.functions[previous].name.span,
                    message: "First declaration".into(),
                });
                return Err(diagnostic);
            }
            names.insert(function.name.text.clone(), headers.len());
            headers.push(Header {
                name: function.name.clone(),
                parameters: parameters(&function.parameters, &class_types)?,
                result: ty(&function.result, &class_types)?,
                effects: effects(&function.effects)?,
                owner: None,
                constructor: None,
            });
            bodies.push(&function.body);
            parameter_names.push(
                function
                    .parameters
                    .iter()
                    .map(|(_, name)| name.clone())
                    .collect::<Vec<_>>(),
            );
        }
        for (index, class) in ast.classes.iter().enumerate() {
            let constructor = &class.constructors[0];
            classes[index].constructor = headers.len();
            headers.push(Header {
                name: Name {
                    text: format!("{}::init", class.name.text),
                    span: constructor.span,
                },
                parameters: parameters(&constructor.parameters, &class_types)?,
                result: class_types[&class.name.text].clone(),
                effects: constructor_effects(),
                owner: Some(index),
                constructor: Some(index),
            });
            bodies.push(&constructor.body);
            parameter_names.push(
                constructor
                    .parameters
                    .iter()
                    .map(|(_, name)| name.clone())
                    .collect(),
            );
            for declaration in &class.methods {
                let method = &declaration.function;
                let mut method_parameters = vec![class_types[&class.name.text].clone()];
                method_parameters.extend(parameters(&method.parameters, &class_types)?);
                classes[index].methods.push(larp_bytecode::Method {
                    name: method.name.text.clone(),
                    function: headers.len(),
                    public: declaration.public,
                });
                headers.push(Header {
                    name: Name {
                        text: format!("{}::{}", class.name.text, method.name.text),
                        span: method.name.span,
                    },
                    parameters: method_parameters,
                    result: ty(&method.result, &class_types)?,
                    effects: effects(&method.effects)?,
                    owner: Some(index),
                    constructor: None,
                });
                bodies.push(&method.body);
                let mut names = vec![Name {
                    text: "self".into(),
                    span: method.name.span,
                }];
                names.extend(method.parameters.iter().map(|(_, name)| name.clone()));
                parameter_names.push(names);
            }
        }
        if headers.len() >= 4096 {
            return Err(error(
                "LARP-P003",
                ast.main.span,
                "Too many functions including methods and constructors",
            ));
        }
        let entry = headers.len();
        headers.push(Header {
            name: Name {
                text: "main_character".into(),
                span: ast.main.span,
            },
            parameters: Vec::new(),
            result: Type::Unit,
            effects: Effects::ALL,
            owner: None,
            constructor: None,
        });
        bodies.push(&ast.main);
        parameter_names.push(Vec::new());
        let mut functions = Vec::new();
        let mut generated = Vec::new();
        let mut errors = Vec::new();
        for (index, header) in headers.iter().enumerate() {
            let mut builder = Builder::new(
                header,
                &headers,
                &names,
                &classes,
                &class_types,
                bodies[index].span,
                headers.len() + generated.len(),
            );
            for (slot, name) in parameter_names[index].iter().enumerate() {
                if let Err(error) = builder.declare(
                    name,
                    Binding {
                        slot,
                        ty: header.parameters[slot].clone(),
                        writable: name.text != "self",
                        span: name.span,
                    },
                ) {
                    builder.errors.push(*error);
                }
            }
            if let Some(class) = header.constructor {
                for (field, declaration) in
                    classes[class].fields.iter().zip(&ast.classes[class].fields)
                {
                    let slot = builder.local(field.ty.clone());
                    builder.constructor_fields.push(Binding {
                        slot,
                        ty: field.ty.clone(),
                        writable: true,
                        span: declaration.name.span,
                    });
                }
            }
            builder.block(bodies[index], false);
            builder.finish();
            if builder.function.constructor.is_some() && builder.errors.is_empty() {
                builder.check_initialization();
            }
            generated.extend(builder.generated);
            functions.push(builder.function);
            errors.extend(builder.errors);
        }
        if !errors.is_empty() {
            let first = errors[0].clone();
            body_errors = errors;
            return Err(Box::new(first));
        }
        functions.extend(generated);
        Ok(Program {
            classes,
            functions,
            entry,
        })
    })();
    result.map_err(|error| {
        if body_errors.is_empty() {
            vec![*error]
        } else {
            body_errors
        }
    })
}

impl Builder<'_> {
    pub(super) fn self_value(&mut self, span: Span) -> Result<Value> {
        if self.function.constructor.is_some() {
            return Err(error(
                "LARP-T012",
                span,
                "A partially initialized self cannot be used as a value, called, or captured",
            ));
        }
        let binding = self.resolve("self").ok_or_else(|| {
            error(
                "LARP-T001",
                span,
                "self is available only in an instance method or its lexical closures",
            )
        })?;
        Ok(self.read(&binding, span))
    }

    pub(super) fn class_field(&self, class: usize, name: &Name) -> Result<usize> {
        let field = self.classes[class]
            .fields
            .iter()
            .position(|field| field.name == name.text)
            .ok_or_else(|| {
                error(
                    "LARP-T001",
                    name.span,
                    format!("{} has no field '{}'", self.classes[class].name, name.text),
                )
            })?;
        if !self.classes[class].fields[field].public && self.function.owner != Some(class) {
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
        if self.classes[class]
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
        Ok(self.read(&field, span))
    }

    pub(super) fn construct(
        &mut self,
        name: &Name,
        arguments: &[Expr],
        span: Span,
        evaluate: bool,
    ) -> Result<Value> {
        let ty = self.class_types.get(&name.text).cloned().ok_or_else(|| {
            error(
                "LARP-T005",
                name.span,
                "new requires a declared archetype name",
            )
        })?;
        let class = ty.class_id().unwrap();
        let header = self.headers[self.classes[class].constructor].clone();
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
        let ty = self.classes[class].fields[field].ty.clone();
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
        let Some(method) = self.classes[class]
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
        span: Span,
    ) -> Result<Value> {
        let Some(class) = receiver.ty.class_id() else {
            return Err(error(
                "LARP-T001",
                name.span,
                format!("Unknown member '{}' on {}", name.text, receiver.ty),
            ));
        };
        if self.classes[class]
            .fields
            .iter()
            .any(|field| field.name == name.text)
        {
            let field = self.class_field(class, name)?;
            return self.get_field(receiver, class, field, span);
        }
        let method = self
            .method_id(class, name)?
            .ok_or_else(|| error("LARP-T001", name.span, "Unknown class member"))?;
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
        if self.classes[class]
            .fields
            .iter()
            .any(|field| field.name == name.text)
        {
            let field = self.class_field(class, name)?;
            let callee = self.get_field(receiver, class, field, span)?;
            return self.call_value(callee, arguments, span, evaluate);
        }
        let method = self
            .method_id(class, name)?
            .ok_or_else(|| error("LARP-T001", name.span, "Unknown class method"))?;
        let header = self.headers[method].clone();
        if arguments.len() + 1 != header.parameters.len() {
            return Err(error(
                "LARP-T006",
                span,
                "Incorrect instance method argument count",
            ));
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

    fn bound_method(&mut self, receiver: Value, method: usize, span: Span) -> Result<Value> {
        let target = self.headers[method].clone();
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
        };
        let mut child = Builder::new(
            &header,
            self.headers,
            self.names,
            self.classes,
            self.class_types,
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
        let class = ty.class_id()?;
        let layout = &self.classes[class];
        if let Some(field) = layout.fields.iter().find(|field| field.name == name.text) {
            return Some(field.ty.clone());
        }
        let method = layout
            .methods
            .iter()
            .find(|method| method.name == name.text)?;
        let header = &self.headers[method.function];
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
