//! Lexical scenario outlining and transitive regional call specialization.
use super::*;
use larp_bytecode::ScenarioContext;
use std::collections::{BTreeMap, HashSet, VecDeque};

impl Builder<'_> {
    pub(super) fn view_expression(&mut self, expression: &Expr, evaluate: bool) -> Result<Value> {
        self.view_access += 1;
        let result = self.expression(expression, None, evaluate);
        self.view_access -= 1;
        result
    }

    pub(super) fn scenario(
        &mut self,
        root: &Name,
        label: &Name,
        body: &ast::Block,
        span: Span,
    ) -> Result<()> {
        if self.function.scenario.is_some() {
            return Err(error("LARP-S003", span, "Scenario blocks cannot be nested"));
        }
        let binding = self.resolve(&root.text).ok_or_else(|| {
            error(
                "LARP-S001",
                root.span,
                "A scenario root must be a local concrete class binding",
            )
        })?;
        let class = binding.ty.class_id().ok_or_else(|| {
            error(
                "LARP-S001",
                root.span,
                "A scenario root must have concrete archetype type",
            )
        })?;
        if self.tables.classes[class]
            .fields
            .iter()
            .any(|field| !field.ty.is_snapshot())
        {
            return Err(error(
                "LARP-S001",
                root.span,
                "Every scenario root field must be a SnapshotValue",
            ));
        }
        let mut visible = BTreeMap::new();
        for scope in &self.scopes {
            for (name, binding) in scope {
                visible.insert(name.clone(), binding.clone());
            }
        }
        visible.remove(&root.text);
        let mut imports = Vec::new();
        let mut forbidden = HashSet::new();
        for (name, binding) in visible {
            if binding.lazy.is_none() && binding.ty.is_snapshot() {
                imports.push((name, binding));
            } else {
                forbidden.insert(name);
            }
        }
        let mut parameters = vec![binding.ty.clone()];
        parameters.extend(imports.iter().map(|(_, binding)| binding.ty.clone()));
        if !Type::function(parameters.clone(), Type::Unit, Effects::ALL).within_limits() {
            return Err(error(
                "LARP-P003",
                span,
                "Scenario import signature complexity limit exceeded",
            ));
        }
        let header = Header {
            name: Name {
                text: format!("scenario:{}@{}", label.text, span.range().start),
                span,
            },
            parameters,
            result: Type::Unit,
            effects: Effects::ALL,
            owner: self.function.owner,
            constructor: None,
            scenario: Some(ScenarioContext {
                entry: usize::MAX,
                class,
                origin: None,
            }),
        };
        let mut child = Builder::new(
            &header,
            self.tables,
            span,
            self.generated_base + self.generated.len(),
        );
        child.scenario_boundary = true;
        child.scenario_forbidden = forbidden;
        child.declare(
            root,
            Binding {
                lazy: None,
                slot: 0,
                ty: binding.ty.clone(),
                writable: false,
                span: root.span,
            },
        )?;
        child.scenario_outer.insert(0);
        for (index, (name, binding)) in imports.iter().enumerate() {
            child.declare(
                &Name {
                    text: name.clone(),
                    span: binding.span,
                },
                Binding {
                    lazy: None,
                    slot: index + 1,
                    ty: binding.ty.clone(),
                    writable: false,
                    span: binding.span,
                },
            )?;
            child.scenario_outer.insert(index + 1);
        }
        child.block(body, true);
        child.finish();
        if !child.errors.is_empty() {
            let first = child.errors.remove(0);
            self.errors.extend(child.errors);
            return Err(Box::new(first));
        }
        child.function.effects = child.required;
        let entry = self.generated_base + self.generated.len() + child.generated.len();
        if entry >= 4096 {
            return Err(error("LARP-P003", span, "Scenario function limit exceeded"));
        }
        for function in child
            .generated
            .iter_mut()
            .chain(std::iter::once(&mut child.function))
        {
            if let Some(context) = &mut function.scenario {
                context.entry = entry;
            }
        }
        self.require(
            Effects::READ
                .union(Effects::WRITE)
                .union(Effects::ABORT)
                .union(child.required),
            span,
        )?;
        let mut arguments = vec![self.read(&binding, root.span)?.slot];
        for (_, binding) in imports {
            arguments.push(self.read(&binding, span)?.slot);
        }
        self.generated.extend(child.generated);
        self.generated.push(child.function);
        let destination = self.local(Type::Unit);
        self.emit(
            Inst::Scenario {
                destination,
                entry,
                label: label.text.clone(),
                arguments,
            },
            span,
        );
        Ok(())
    }

    fn active_field(
        &self,
        root: &Name,
        field: &Name,
        span: Span,
    ) -> Result<(usize, usize, Binding)> {
        let context = self.function.scenario.ok_or_else(|| {
            error(
                "LARP-S006",
                span,
                "This operation requires an active scenario",
            )
        })?;
        // Explicit selection/claims apply only to the lexical root, never an alias.
        let binding = self
            .resolve(&root.text)
            .filter(|binding| self.scenario_boundary && binding.slot == 0)
            .ok_or_else(|| {
                error(
                    "LARP-S006",
                    root.span,
                    "Expected the active scenario root binding",
                )
            })?;
        let field = self.class_field(context.class, field)?;
        Ok((context.class, field, binding))
    }

    pub(super) fn flex(
        &mut self,
        root: &Name,
        field: &Name,
        expression: &Expr,
        span: Span,
    ) -> Result<()> {
        let (class, field, binding) = self.active_field(root, field, span)?;
        let receiver = self.read(&binding, span)?;
        let value = self.expression(
            expression,
            Some(self.tables.classes[class].fields[field].ty.clone()),
            true,
        )?;
        self.require(Effects::WRITE, span)?;
        let destination = self.local(Type::Unit);
        self.emit(
            Inst::Flex {
                destination,
                receiver: receiver.slot,
                class,
                field,
                value: value.slot,
            },
            span,
        );
        Ok(())
    }

    pub(super) fn canon(&mut self, root: &Name, field: &Name, span: Span) -> Result<()> {
        let (class, field, binding) = self.active_field(root, field, span)?;
        self.require(Effects::WRITE, span)?;
        let receiver = self.read(&binding, span)?;
        let destination = self.local(Type::Unit);
        self.emit(
            Inst::Canon {
                destination,
                receiver: receiver.slot,
                class,
                field,
            },
            span,
        );
        Ok(())
    }

    pub(super) fn palantir(
        &mut self,
        arguments: &[Expr],
        span: Span,
        evaluate: bool,
    ) -> Result<Value> {
        let context = self
            .function
            .scenario
            .ok_or_else(|| error("LARP-S006", span, "palantir requires an active scenario"))?;
        if arguments.len() != 1 {
            return Err(error(
                "LARP-T006",
                span,
                "palantir requires the active scenario root",
            ));
        }
        let receiver = self.view_expression(&arguments[0], evaluate)?;
        if receiver.ty.class_id() != Some(context.class) {
            return Err(error(
                "LARP-S006",
                arguments[0].span,
                "palantir requires the active scenario view",
            ));
        }
        self.require(Effects::OBSERVE, span)?;
        let fields = self.tables.classes[context.class]
            .fields
            .iter()
            .enumerate()
            .filter(|(_, field)| field.public || self.function.owner == Some(context.class))
            .map(|(index, _)| index)
            .collect();
        let slot = self.local(Type::Unit);
        self.emit(
            Inst::Palantir {
                destination: slot,
                receiver: receiver.slot,
                class: context.class,
                fields,
            },
            span,
        );
        Ok(Value {
            slot,
            ty: Type::Unit,
            constant: None,
        })
    }
}

fn callable(ty: &Type) -> bool {
    let Type::Function(signature) = ty else {
        return false;
    };
    signature.parameters.iter().all(Type::is_snapshot)
        && (signature.result.is_snapshot() || signature.result == Type::Unit)
        && !signature.effects.contains(Effects::IO)
}

fn regional_local(ty: &Type, class: usize) -> bool {
    ty.is_snapshot()
        || *ty == Type::Unit
        || ty.class_id() == Some(class)
        || callable(ty)
        || matches!(ty, Type::Lazy(_, _))
}

/// Each lexical region owns a closed graph of checked function instances.
/// Snapshot locals implicitly carry S_entry(T); the sole class is View_entry(C).
/// No ordinary caller can consume these values except the dedicated unit exit.
pub(super) fn specialize(program: &mut Program) -> Result<()> {
    let entries: Vec<_> = program
        .functions
        .iter()
        .enumerate()
        .filter_map(|(id, function)| {
            function
                .scenario
                .filter(|context| context.entry == id)
                .map(|context| (id, context.class))
        })
        .collect();
    for (entry, class) in entries {
        let mut mapping = HashMap::new();
        let mut pending = VecDeque::from([entry]);
        let mut visited = HashSet::new();
        while let Some(id) = pending.pop_front() {
            if !visited.insert(id) {
                continue;
            }
            let function = &program.functions[id];
            let invalid = |message| error("LARP-E002", function.span, message);
            if function.constructor.is_some()
                || function.effects.contains(Effects::IO)
                || function
                    .parameters
                    .iter()
                    .filter(|ty| ty.class_id().is_some())
                    .count()
                    > 1
                || function
                    .parameters
                    .iter()
                    .any(|ty| !regional_local(ty, class) || *ty == Type::Unit)
                || function.captures.iter().any(|ty| !ty.is_snapshot())
                || !(function.result.is_snapshot() || function.result == Type::Unit)
                || function.locals.iter().any(|ty| !regional_local(ty, class))
            {
                return Err(invalid(
                    "This call graph is not scenario-eligible: it contains reference state, captures, results, or I/O",
                ));
            }
            let mut replacements = Vec::new();
            for (block_id, block) in function.blocks.iter().enumerate() {
                for (instruction_id, instruction) in block.instructions.iter().enumerate() {
                    match &instruction.kind {
                        Inst::Scenario { .. }
                        | Inst::Construct { .. }
                        | Inst::IntoRole { .. }
                        | Inst::CallRole { .. } => {
                            return Err(error(
                                "LARP-E002",
                                instruction.span,
                                "Scenario calls cannot allocate references, dispatch roles, or enter another scenario",
                            ));
                        }
                        Inst::Closure {
                            function: target, ..
                        }
                        | Inst::Lazy {
                            function: target, ..
                        }
                        | Inst::Call {
                            function: target, ..
                        } => {
                            replacements.push((block_id, instruction_id, *target));
                        }
                        Inst::Intrinsic { operation, .. } => {
                            let signature = operation
                                .signature()
                                .ok_or_else(|| invalid("Invalid scenario intrinsic"))?;
                            if operation.types().iter().any(|ty| !ty.is_snapshot())
                                || signature.parameters.iter().any(|ty| !ty.is_snapshot())
                                || !(signature.result.is_snapshot()
                                    || signature.result == Type::Unit)
                                || signature.effects.contains(Effects::IO)
                            {
                                return Err(error(
                                    "LARP-E002",
                                    instruction.span,
                                    "This intrinsic reaches reference state or an unverified callback",
                                ));
                            }
                        }
                        Inst::Binary { left, .. }
                            if function.locals[*left].class_id().is_some() =>
                        {
                            return Err(error(
                                "LARP-S004",
                                instruction.span,
                                "A scenario view cannot be compared as a first-class identity",
                            ));
                        }
                        _ => {}
                    }
                }
            }
            // Rewrite edges only after the immutable body scan, preserving stable IDs.
            for (block, instruction, target) in replacements {
                let replacement = if let Some(context) = program.functions[target].scenario {
                    if context.entry != entry {
                        return Err(error(
                            "LARP-E002",
                            program.functions[id].span,
                            "A call cannot cross scenario regions",
                        ));
                    }
                    target
                } else if let Some(cloned) = mapping.get(&target) {
                    *cloned
                } else {
                    let cloned = program.functions.len();
                    if cloned >= 4096 {
                        return Err(error(
                            "LARP-P003",
                            program.functions[id].span,
                            "Regional specialization function limit exceeded",
                        ));
                    }
                    let mut function = program.functions[target].clone();
                    function.name = format!("{}@scenario:{entry}", function.name);
                    function.scenario = Some(ScenarioContext {
                        entry,
                        class,
                        origin: Some(target),
                    });
                    program.functions.push(function);
                    mapping.insert(target, cloned);
                    cloned
                };
                match &mut program.functions[id].blocks[block].instructions[instruction].kind {
                    Inst::Call { function, .. }
                    | Inst::Closure { function, .. }
                    | Inst::Lazy { function, .. } => *function = replacement,
                    _ => unreachable!(),
                }
                pending.push_back(replacement);
            }
        }
    }
    Ok(())
}
