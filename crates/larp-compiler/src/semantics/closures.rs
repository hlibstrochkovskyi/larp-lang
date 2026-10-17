use super::*;
use std::collections::{BTreeSet, HashSet};

impl Builder<'_> {
    pub(super) fn lambda(
        &mut self,
        parameters: &[ast::LambdaParameter],
        body: &ast::LambdaBody,
        expected: Option<Type>,
        span: Span,
    ) -> Result<Value> {
        let signature = match &expected {
            Some(Type::Function(signature)) => Some(signature.clone()),
            Some(other) => {
                return Err(error(
                    "LARP-T001",
                    span,
                    format!("A lambda cannot initialize {other}"),
                ));
            }
            None => None,
        };
        if signature
            .as_ref()
            .is_some_and(|signature| signature.parameters.len() != parameters.len())
        {
            return Err(error(
                "LARP-T006",
                span,
                "Lambda parameter count does not match its expected function type",
            ));
        }
        let mut parameter_types = Vec::new();
        for (index, parameter) in parameters.iter().enumerate() {
            let context = signature
                .as_ref()
                .map(|signature| signature.parameters[index].clone());
            let declared = parameter.ty.as_ref().map(ty).transpose()?;
            if let (Some(expected), Some(actual)) = (&context, &declared) {
                self.matches(expected.clone(), actual.clone(), parameter.name.span)?;
            }
            let ty = declared.or(context).ok_or_else(|| {
                error(
                    "LARP-T003",
                    parameter.name.span,
                    "Lambda parameters need annotations or an expected function type",
                )
            })?;
            if ty == Type::Unit {
                return Err(error(
                    "LARP-T001",
                    parameter.name.span,
                    "Function parameters cannot have type unit",
                ));
            }
            parameter_types.push(ty);
        }
        let mut captures = Vec::new();
        for name in free_names(parameters, body) {
            if let Some(binding) = self.resolve(&name) {
                let copied = self.read(&binding, span);
                captures.push((name, binding, copied.slot));
            }
        }
        let header = Header {
            name: Name {
                text: format!("lambda@{}", span.range().start),
                span,
            },
            parameters: parameter_types,
            result: signature
                .as_ref()
                .map_or(Type::Unit, |signature| signature.result.clone()),
            effects: signature
                .as_ref()
                .map_or(Effects::ALL, |signature| signature.effects),
        };
        let mut child = Builder::new(
            &header,
            self.headers,
            self.names,
            span,
            self.generated_base + self.generated.len(),
        );
        if signature.is_none() {
            child.inferred_result = Some(None);
        }
        child.scopes.insert(0, HashMap::new());
        for (index, parameter) in parameters.iter().enumerate() {
            child.declare(
                &parameter.name,
                Binding {
                    slot: index,
                    ty: header.parameters[index].clone(),
                    writable: true,
                    span: parameter.name.span,
                },
            )?;
        }
        for (name, binding, _) in &captures {
            let slot = child.local(binding.ty.clone());
            child.function.captures.push(binding.ty.clone());
            child.scopes[0].insert(
                name.clone(),
                Binding {
                    slot,
                    ty: binding.ty.clone(),
                    writable: false,
                    span: binding.span,
                },
            );
        }
        match body {
            ast::LambdaBody::Block(block) => child.block(block, false),
            ast::LambdaBody::Expression(expression) => {
                let expected = signature.as_ref().map(|signature| signature.result.clone());
                let value = child.expression(expression, expected, true)?;
                child.function.result = signature
                    .as_ref()
                    .map_or(value.ty, |signature| signature.result.clone());
                child.terminate(Terminator::Return(Some(value.slot)), span);
            }
        }
        child.finish();
        if !child.errors.is_empty() {
            let first = child.errors.remove(0);
            self.errors.extend(child.errors);
            return Err(Box::new(first));
        }
        child.function.effects = child.required;
        let function_type = Type::function(
            child.function.parameters.clone(),
            child.function.result.clone(),
            child.required,
        );
        self.generated.extend(child.generated);
        let id = self.generated_base + self.generated.len();
        if id >= 4096 {
            return Err(error(
                "LARP-P003",
                span,
                "Too many generated functions (limit: 4096 including main)",
            ));
        }
        self.generated.push(child.function);
        let slot = self.local(function_type.clone());
        self.emit(
            Inst::Closure {
                destination: slot,
                function: id,
                captures: captures.into_iter().map(|(_, _, slot)| slot).collect(),
            },
            span,
        );
        Ok(Value {
            slot,
            ty: function_type,
            constant: None,
        })
    }
}

fn free_names(parameters: &[ast::LambdaParameter], body: &ast::LambdaBody) -> BTreeSet<String> {
    struct Walker {
        bound: Vec<HashSet<String>>,
        free: BTreeSet<String>,
    }
    impl Walker {
        fn name(&mut self, name: &Name) {
            if !self
                .bound
                .iter()
                .rev()
                .any(|scope| scope.contains(&name.text))
            {
                self.free.insert(name.text.clone());
            }
        }
        fn lambda(&mut self, parameters: &[ast::LambdaParameter], body: &ast::LambdaBody) {
            self.bound.push(
                parameters
                    .iter()
                    .map(|parameter| parameter.name.text.clone())
                    .collect(),
            );
            match body {
                ast::LambdaBody::Expression(value) => self.expr(value),
                ast::LambdaBody::Block(block) => self.block(block),
            }
            self.bound.pop();
        }
        fn expr(&mut self, expr: &Expr) {
            match &expr.kind {
                ExprKind::Name(name) => self.name(name),
                ExprKind::Unary { operand, .. } => self.expr(operand),
                ExprKind::Binary { left, right, .. } => {
                    self.expr(left);
                    self.expr(right);
                }
                ExprKind::Call {
                    callee, arguments, ..
                } => {
                    self.expr(callee);
                    for argument in arguments {
                        self.expr(argument);
                    }
                }
                ExprKind::Member { receiver, .. } => self.expr(receiver),
                ExprKind::Lambda { parameters, body } => self.lambda(parameters, body),
                _ => {}
            }
        }
        fn block(&mut self, block: &ast::Block) {
            self.bound.push(HashSet::new());
            for statement in &block.statements {
                self.statement(statement);
            }
            self.bound.pop();
        }
        fn statement(&mut self, statement: &ast::Statement) {
            match &statement.kind {
                StatementKind::Binding { name, value, .. } => {
                    self.expr(value);
                    self.bound.last_mut().unwrap().insert(name.text.clone());
                }
                StatementKind::Assignment { name, value, .. } => {
                    self.name(name);
                    self.expr(value);
                }
                StatementKind::Expression(value) => self.expr(value),
                StatementKind::Block(block) | StatementKind::Forever(block) => self.block(block),
                StatementKind::If {
                    condition,
                    then_block,
                    otherwise,
                } => {
                    self.expr(condition);
                    self.block(then_block);
                    if let Some(otherwise) = otherwise {
                        self.statement(otherwise);
                    }
                }
                StatementKind::While { condition, body } => {
                    self.expr(condition);
                    self.block(body);
                }
                StatementKind::Range {
                    name,
                    start,
                    end,
                    body,
                    ..
                } => {
                    self.expr(start);
                    self.expr(end);
                    self.bound.push(HashSet::from([name.text.clone()]));
                    self.block(body);
                    self.bound.pop();
                }
                StatementKind::Return(Some(value)) => self.expr(value),
                _ => {}
            }
        }
    }
    let mut walker = Walker {
        bound: Vec::new(),
        free: BTreeSet::new(),
    };
    walker.lambda(parameters, body);
    walker.free
}
