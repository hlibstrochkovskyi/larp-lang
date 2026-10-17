use larp_diagnostics::{Diagnostic, Label};
use larp_ir::{
    Block, BlockId, Function, Instruction, InstructionKind as Inst, Local, Program, Terminator,
};
use larp_source::Span;
use larp_syntax::{self as ast, Expr, ExprKind, Name, StatementKind};
use larp_types::{Binary, Effects, Intrinsic, Scalar, Type, Unary, binary_type, unary_type};
mod arrays;
mod calls;
mod closures;
mod objects;
use std::collections::HashMap;

type Result<T> = std::result::Result<T, Box<Diagnostic>>;

#[derive(Clone)]
struct Header {
    name: Name,
    parameters: Vec<Type>,
    result: Type,
    effects: Effects,
    owner: Option<usize>,
    constructor: Option<usize>,
}

fn error(code: &str, span: Span, message: impl Into<String>) -> Box<Diagnostic> {
    Box::new(Diagnostic::error(code, span, message))
}

fn effects(declared: &Option<Vec<Name>>) -> Result<Effects> {
    let Some(declared) = declared else {
        return Ok(Effects::ABORT);
    };
    let mut effects = Effects::NONE;
    for effect in declared {
        effects = effects.union(Effects::from_name(&effect.text).ok_or_else(|| {
            error(
                "LARP-E003",
                effect.span,
                format!("Unknown effect '{}'", effect.text),
            )
        })?);
    }
    Ok(effects)
}

fn ty(expression: &ast::TypeExpr, class_types: &HashMap<String, Type>) -> Result<Type> {
    let result = match &expression.kind {
        ast::TypeExprKind::Integer(_) => Err(error(
            "LARP-T001",
            expression.span,
            "An integer literal is only valid as a fixed-array length",
        )),
        ast::TypeExprKind::Named { name, arguments } => match name.text.as_str() {
            "baddie" if arguments.len() == 1 => Ok(Type::option(ty(&arguments[0], class_types)?)),
            "result" if arguments.len() == 2 => Ok(Type::result(
                ty(&arguments[0], class_types)?,
                ty(&arguments[1], class_types)?,
            )),
            "mfs" if arguments.len() == 1 => Ok(Type::array(ty(&arguments[0], class_types)?)),
            "hediJeans" if arguments.len() == 2 => {
                let ast::TypeExprKind::Integer(digits) = &arguments[1].kind else {
                    return Err(error(
                        "LARP-T001",
                        arguments[1].span,
                        "Fixed-array length must be a nonnegative integer literal",
                    ));
                };
                let length = digits
                    .replace('_', "")
                    .parse::<usize>()
                    .ok()
                    .filter(|length| *length <= i32::MAX as usize)
                    .ok_or_else(|| {
                        error(
                            "LARP-T004",
                            arguments[1].span,
                            "Fixed-array length exceeds the collection length limit",
                        )
                    })?;
                Ok(Type::fixed_array(ty(&arguments[0], class_types)?, length))
            }
            "baddie" | "result" | "mfs" | "hediJeans" => Err(error(
                "LARP-T001",
                expression.span,
                "Incorrect number of generic type arguments",
            )),
            _ if !arguments.is_empty() => Err(error(
                "LARP-T005",
                expression.span,
                "This type does not accept generic arguments",
            )),
            _ => Type::from_name(&name.text)
                .or_else(|| class_types.get(&name.text).cloned())
                .ok_or_else(|| {
                    error(
                        "LARP-T005",
                        name.span,
                        format!("Unknown type '{}'", name.text),
                    )
                }),
        },
        ast::TypeExprKind::Function {
            parameters,
            result,
            effects: bound,
        } => {
            let parameters = parameters
                .iter()
                .map(|parameter| ty(parameter, class_types))
                .collect::<Result<Vec<_>>>()?;
            if parameters.contains(&Type::Unit) {
                return Err(error(
                    "LARP-T001",
                    expression.span,
                    "Function parameters cannot have type unit",
                ));
            }
            Ok(Type::function(
                parameters,
                ty(result, class_types)?,
                effects(bound)?,
            ))
        }
    }?;
    if !result.within_limits() {
        return Err(error(
            "LARP-P003",
            expression.span,
            "Type complexity limit exceeded",
        ));
    }
    if !result.is_well_formed() {
        return Err(error(
            "LARP-T001",
            expression.span,
            "Invalid type: array elements and function parameters cannot have type unit",
        ));
    }
    Ok(result)
}

pub fn check(ast: &ast::Program) -> std::result::Result<Program, Vec<Diagnostic>> {
    objects::check_program(ast)
}

#[derive(Clone)]
struct Binding {
    slot: Local,
    ty: Type,
    writable: bool,
    span: Span,
}

#[derive(Clone)]
struct Value {
    slot: Local,
    ty: Type,
    constant: Option<Scalar>,
}

struct Builder<'a> {
    function: Function,
    current: BlockId,
    scopes: Vec<HashMap<String, Binding>>,
    breaks: Vec<BlockId>,
    headers: &'a [Header],
    names: &'a HashMap<String, usize>,
    classes: &'a [larp_bytecode::Class],
    class_types: &'a HashMap<String, Type>,
    constructor_fields: Vec<Binding>,
    errors: Vec<Diagnostic>,
    generated: Vec<Function>,
    generated_base: usize,
    required: Effects,
    inferred_result: Option<Option<Type>>,
}

impl<'a> Builder<'a> {
    fn new(
        header: &Header,
        headers: &'a [Header],
        names: &'a HashMap<String, usize>,
        classes: &'a [larp_bytecode::Class],
        class_types: &'a HashMap<String, Type>,
        span: Span,
        generated_base: usize,
    ) -> Self {
        Self {
            function: Function {
                owner: header.owner,
                constructor: header.constructor,
                name: header.name.text.clone(),
                parameters: header.parameters.clone(),
                captures: Vec::new(),
                result: header.result.clone(),
                effects: header.effects,
                locals: header.parameters.clone(),
                blocks: vec![Block {
                    parameter: None,
                    instructions: Vec::new(),
                    terminator: Terminator::Unreachable,
                    span,
                }],
                span,
            },
            current: 0,
            scopes: vec![HashMap::new()],
            breaks: Vec::new(),
            headers,
            names,
            classes,
            class_types,
            constructor_fields: Vec::new(),
            errors: Vec::new(),
            generated: Vec::new(),
            generated_base,
            required: Effects::NONE,
            inferred_result: None,
        }
    }
    fn finish(&mut self) {
        if !self.errors.is_empty() {
            return;
        }
        let reachable = self.function.reachable();
        for (block, live) in self.function.blocks.iter_mut().zip(reachable) {
            if live && matches!(block.terminator, Terminator::Unreachable) {
                if let Some(class) = self.function.constructor {
                    block.terminator = Terminator::ConstructReturn {
                        class,
                        fields: self
                            .constructor_fields
                            .iter()
                            .map(|field| field.slot)
                            .collect(),
                    };
                } else if self.function.result == Type::Unit {
                    block.terminator = Terminator::Return(None);
                } else {
                    self.errors.push(Diagnostic::error(
                        "LARP-T007",
                        self.function.span,
                        format!(
                            "Function '{}' can finish without returning {}",
                            self.function.name, self.function.result
                        ),
                    ));
                    break;
                }
            }
        }
    }
    fn resolve_type(&self, expression: &ast::TypeExpr) -> Result<Type> {
        ty(expression, self.class_types)
    }
    fn local(&mut self, ty: Type) -> Local {
        let slot = self.function.locals.len();
        self.function.locals.push(ty);
        slot
    }
    fn emit(&mut self, kind: Inst, span: Span) {
        self.function.blocks[self.current]
            .instructions
            .push(Instruction { kind, span });
    }
    fn new_block(&mut self, span: Span) -> BlockId {
        let id = self.function.blocks.len();
        self.function.blocks.push(Block {
            parameter: None,
            instructions: Vec::new(),
            terminator: Terminator::Unreachable,
            span,
        });
        id
    }
    fn terminate(&mut self, terminator: Terminator, span: Span) {
        self.function.blocks[self.current].terminator = terminator;
        self.function.blocks[self.current].span = span;
    }
    fn literal(&mut self, value: Scalar, span: Span) -> Value {
        let ty = value.ty();
        let slot = self.local(ty.clone());
        self.emit(
            Inst::Constant {
                destination: slot,
                value: value.clone(),
            },
            span,
        );
        Value {
            slot,
            ty,
            constant: Some(value),
        }
    }
    fn resolve(&self, name: &str) -> Option<Binding> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }
    fn declare(&mut self, name: &Name, binding: Binding) -> Result<()> {
        let scope = self.scopes.last_mut().unwrap();
        if let Some(previous) = scope.get(&name.text) {
            let mut diagnostic = error(
                "LARP-N002",
                name.span,
                format!("Duplicate binding '{}'", name.text),
            );
            diagnostic.related.push(Label {
                span: previous.span,
                message: "First declaration".into(),
            });
            return Err(diagnostic);
        }
        scope.insert(name.text.clone(), binding);
        Ok(())
    }
    fn require(&mut self, effects: Effects, span: Span) -> Result<()> {
        if self.function.constructor.is_some() && effects.contains(Effects::WRITE) {
            return Err(error(
                "LARP-T012",
                span,
                "A constructor cannot mutate existing objects or call an operation with write effects",
            ));
        }
        self.required = self.required.union(effects);
        if self.function.effects.contains(effects) {
            Ok(())
        } else {
            let mut diagnostic = error(
                "LARP-E001",
                span,
                format!(
                    "Operation requires {effects}, outside declared {}",
                    self.function.effects
                ),
            );
            diagnostic.help = Some("Declare the required effects or remove the operation".into());
            Err(diagnostic)
        }
    }
    fn matches(&self, expected: Type, found: Type, span: Span) -> Result<()> {
        if expected.accepts(&found) {
            return Ok(());
        }
        let narrowing = expected == Type::Mf && found == Type::Unc;
        let mut diagnostic = error(
            if narrowing { "LARP-T041" } else { "LARP-T001" },
            span,
            format!("Expected {expected}, found {found}"),
        );
        diagnostic.expected_type = Some(expected.to_string());
        diagnostic.found_type = Some(found.to_string());
        if narrowing {
            diagnostic.help = Some("Use a matching integer type, or ascend<mf>(value) and handle the result with .cope(handler)".into());
        }
        Err(diagnostic)
    }
    fn block(&mut self, block: &ast::Block, scoped: bool) {
        if scoped {
            self.scopes.push(HashMap::new());
        }
        for statement in &block.statements {
            if let Err(error) = self.statement(statement) {
                self.errors.push(*error);
            }
        }
        if scoped {
            self.scopes.pop();
        }
    }
    fn condition(&mut self, expression: &Expr) -> Result<Value> {
        let value = self.expression_value(expression, Some(Type::Verdict), true)?;
        if value.ty != Type::Verdict {
            let mut diagnostic = error(
                "LARP-T002",
                expression.span,
                "A condition must have type verdict; there is no implicit truthiness",
            );
            diagnostic.expected_type = Some("verdict".into());
            diagnostic.found_type = Some(value.ty.to_string());
            diagnostic.help = Some("Use an explicit comparison, such as value mogg 0".into());
            return Err(diagnostic);
        }
        Ok(value)
    }
    fn statement(&mut self, statement: &ast::Statement) -> Result<()> {
        let span = statement.span;
        match &statement.kind {
            StatementKind::Binding {
                ty: declared,
                name,
                value,
            } => {
                let declared = declared
                    .as_ref()
                    .map(|ty| self.resolve_type(ty))
                    .transpose()?;
                let value = self.expression(value, declared.clone(), true)?;
                if value.ty == Type::Unit {
                    return Err(error(
                        "LARP-T001",
                        name.span,
                        "A local binding cannot have type unit",
                    ));
                }
                let binding_type = declared.unwrap_or(value.ty.clone());
                let slot = self.local(binding_type.clone());
                self.emit(
                    Inst::Copy {
                        destination: slot,
                        source: value.slot,
                    },
                    span,
                );
                self.declare(
                    name,
                    Binding {
                        slot,
                        ty: binding_type,
                        writable: true,
                        span: name.span,
                    },
                )?;
            }
            StatementKind::Assignment { target, op, value } => {
                self.assignment(target, *op, value, span)?
            }
            StatementKind::Expression(expression) => {
                self.expression(expression, None, true)?;
            }
            StatementKind::Block(block) => self.block(block, true),
            StatementKind::If {
                condition,
                then_block,
                otherwise,
            } => {
                let condition = self.condition(condition)?;
                let when_true = self.new_block(then_block.span);
                let when_false = self.new_block(span);
                let merge = self.new_block(span);
                self.terminate(
                    Terminator::Branch {
                        condition: condition.slot,
                        when_true,
                        when_false,
                    },
                    span,
                );
                self.current = when_true;
                self.block(then_block, true);
                self.terminate(Terminator::Goto(merge), span);
                self.current = when_false;
                if let Some(otherwise) = otherwise {
                    self.statement(otherwise)?;
                }
                self.terminate(Terminator::Goto(merge), span);
                self.current = merge;
            }
            StatementKind::While { condition, body } => {
                let test = self.new_block(condition.span);
                let run = self.new_block(body.span);
                let done = self.new_block(span);
                self.terminate(Terminator::Goto(test), span);
                self.current = test;
                let condition = self.condition(condition)?;
                self.terminate(
                    Terminator::Branch {
                        condition: condition.slot,
                        when_true: run,
                        when_false: done,
                    },
                    span,
                );
                self.current = run;
                self.breaks.push(done);
                self.block(body, true);
                self.breaks.pop();
                self.terminate(Terminator::Goto(test), span);
                self.current = done;
            }
            StatementKind::Range {
                ty: index_type,
                name,
                start,
                end,
                body,
            } => {
                let index_type = self.resolve_type(index_type)?;
                if !index_type.is_integer() {
                    return Err(error(
                        "LARP-T001",
                        name.span,
                        "Range-loop index must be mf or unc",
                    ));
                }
                let start = self.expression(start, Some(index_type.clone()), true)?;
                let end = self.expression(end, Some(index_type.clone()), true)?;
                self.scopes.push(HashMap::new());
                let slot = self.local(index_type.clone());
                self.emit(
                    Inst::Copy {
                        destination: slot,
                        source: start.slot,
                    },
                    span,
                );
                let binding = Binding {
                    slot,
                    ty: index_type.clone(),
                    writable: false,
                    span: name.span,
                };
                self.declare(name, binding.clone())?;
                let test = self.new_block(span);
                let run = self.new_block(body.span);
                let step = self.new_block(span);
                let done = self.new_block(span);
                self.terminate(Terminator::Goto(test), span);
                self.current = test;
                let current = self.read(&binding, span);
                let condition = self.binary(Binary::Less, current, end, span, true)?;
                self.terminate(
                    Terminator::Branch {
                        condition: condition.slot,
                        when_true: run,
                        when_false: done,
                    },
                    span,
                );
                self.current = run;
                self.breaks.push(done);
                self.block(body, true);
                self.breaks.pop();
                self.terminate(Terminator::Goto(step), span);
                self.current = step;
                let current = self.read(&binding, span);
                let one = self.literal(
                    if index_type == Type::Mf {
                        Scalar::Mf(1)
                    } else {
                        Scalar::Unc(1)
                    },
                    span,
                );
                let next = self.binary(Binary::Add, current, one, span, true)?;
                self.emit(
                    Inst::Copy {
                        destination: slot,
                        source: next.slot,
                    },
                    span,
                );
                self.terminate(Terminator::Goto(test), span);
                self.current = done;
                self.scopes.pop();
            }
            StatementKind::Forever(body) => {
                let run = self.new_block(body.span);
                let done = self.new_block(span);
                self.terminate(Terminator::Goto(run), span);
                self.current = run;
                self.breaks.push(done);
                self.block(body, true);
                self.breaks.pop();
                self.terminate(Terminator::Goto(run), span);
                self.current = done;
            }
            StatementKind::Return(expression) => {
                if let Some(class) = self.function.constructor {
                    if expression.is_some() {
                        return Err(error(
                            "LARP-T012",
                            span,
                            "A constructor accepts return; but cannot return an explicit value",
                        ));
                    }
                    self.terminate(
                        Terminator::ConstructReturn {
                            class,
                            fields: self
                                .constructor_fields
                                .iter()
                                .map(|field| field.slot)
                                .collect(),
                        },
                        span,
                    );
                    self.current = self.new_block(span);
                    return Ok(());
                }
                let expected = match &self.inferred_result {
                    None => Some(self.function.result.clone()),
                    Some(inferred) => inferred.clone(),
                };
                let (slot, result_type) = if let Some(expression) = expression {
                    let value = self.expression(expression, expected.clone(), true)?;
                    (Some(value.slot), value.ty)
                } else {
                    (None, Type::Unit)
                };
                if let Some(expected) = expected {
                    self.matches(expected, result_type.clone(), span)?;
                }
                if matches!(self.inferred_result, Some(None))
                    && self.function.reachable()[self.current]
                {
                    self.function.result = result_type.clone();
                    self.inferred_result = Some(Some(result_type));
                }
                self.terminate(Terminator::Return(slot), span);
                self.current = self.new_block(span);
            }
            StatementKind::Break => {
                let target = self.breaks.last().copied().ok_or_else(|| {
                    error("LARP-T009", span, "touch_grass requires an enclosing loop")
                })?;
                self.terminate(Terminator::Goto(target), span);
                self.current = self.new_block(span);
            }
        }
        Ok(())
    }
    fn read(&mut self, binding: &Binding, span: Span) -> Value {
        let slot = self.local(binding.ty.clone());
        self.emit(
            Inst::Copy {
                destination: slot,
                source: binding.slot,
            },
            span,
        );
        Value {
            slot,
            ty: binding.ty.clone(),
            constant: None,
        }
    }
    fn hint(&self, expression: &Expr) -> Option<Type> {
        match &expression.kind {
            ExprKind::SelfValue => self.resolve("self").map(|binding| binding.ty).or_else(|| {
                self.function
                    .constructor
                    .map(|class| Type::class(class, self.classes[class].name.as_str()))
            }),
            ExprKind::New { class, .. } => self.class_types.get(&class.text).cloned(),
            ExprKind::Name(name) => {
                self.resolve(&name.text)
                    .map(|binding| binding.ty)
                    .or_else(|| {
                        if let Some(index) = self.names.get(&name.text) {
                            let header = &self.headers[*index];
                            Some(Type::function(
                                header.parameters.clone(),
                                header.result.clone(),
                                header.effects,
                            ))
                        } else if name.text == "fortnine" {
                            Some(Type::Unc)
                        } else if name.text == "parseScore" {
                            Some(Type::function(
                                vec![Type::Text],
                                Type::result(Type::Mf, Type::Text),
                                Effects::NONE,
                            ))
                        } else if name.text == "no_cap" {
                            Some(Type::function(
                                vec![Type::Verdict],
                                Type::Unit,
                                Effects::ABORT,
                            ))
                        } else {
                            None
                        }
                    })
            }
            ExprKind::Float(_) => Some(Type::Geeked),
            ExprKind::Text(_) => Some(Type::Text),
            ExprKind::Verdict(_) => Some(Type::Verdict),
            ExprKind::Unary { operand, .. } => self.hint(operand),
            ExprKind::Binary { left, right, op } => {
                if matches!(
                    op,
                    Binary::Equal
                        | Binary::NotEqual
                        | Binary::Less
                        | Binary::LessEqual
                        | Binary::Greater
                        | Binary::GreaterEqual
                        | Binary::And
                        | Binary::Or
                ) {
                    Some(Type::Verdict)
                } else {
                    self.hint(left).or_else(|| self.hint(right))
                }
            }
            ExprKind::Call { callee, .. } => {
                if let ExprKind::Member { receiver, name } = &callee.kind {
                    match (self.hint(receiver), name.text.as_str()) {
                        (Some(Type::Baddie(payload)), "copium") => Some(payload.as_ref().clone()),
                        (Some(Type::Result(payloads)), "cope") => Some(payloads.0.clone()),
                        (Some(Type::Baddie(_)), "isSome") | (Some(Type::Result(_)), "isOk") => {
                            Some(Type::Verdict)
                        }
                        (Some(Type::Array(_) | Type::FixedArray(_, _) | Type::Text), "size") => {
                            Some(Type::Mf)
                        }
                        _ => {
                            if let Some(Type::Function(signature)) = self.hint(callee) {
                                Some(signature.result.clone())
                            } else {
                                None
                            }
                        }
                    }
                } else if let Some(Type::Function(signature)) = self.hint(callee) {
                    Some(signature.result.clone())
                } else {
                    None
                }
            }
            ExprKind::Member { receiver, name }
                if name.text == "message" && self.hint(receiver) == Some(Type::ConversionError) =>
            {
                Some(Type::Text)
            }
            ExprKind::Index { receiver, .. } => {
                self.hint(receiver).and_then(|ty| ty.element().cloned())
            }
            ExprKind::Array(elements) => elements
                .first()
                .and_then(|value| self.hint(value))
                .map(Type::array),
            ExprKind::Member { receiver, name } => self
                .hint(receiver)
                .and_then(|ty| self.member_hint(&ty, name)),
            _ => None,
        }
    }
    fn expression(
        &mut self,
        expression: &Expr,
        expected: Option<Type>,
        evaluate: bool,
    ) -> Result<Value> {
        let value = self.expression_value(expression, expected.clone(), evaluate)?;
        if let Some(expected) = expected {
            self.matches(expected, value.ty.clone(), expression.span)?;
        }
        Ok(value)
    }
    fn expression_value(
        &mut self,
        expression: &Expr,
        expected: Option<Type>,
        evaluate: bool,
    ) -> Result<Value> {
        let span = expression.span;
        let value = match &expression.kind {
            ExprKind::Integer(digits) => self.integer(digits, false, expected.clone(), span)?,
            ExprKind::Array(elements) => {
                self.array_literal(elements, expected.clone(), span, evaluate)?
            }
            ExprKind::Index { receiver, index } => {
                let receiver = self.expression(receiver, None, evaluate)?;
                if receiver.ty.element().is_none() {
                    return Err(error(
                        "LARP-T001",
                        span,
                        "Indexing requires a dynamic or fixed array",
                    ));
                }
                let index = self.expression(index, Some(Type::Mf), evaluate)?;
                self.intrinsic(
                    Intrinsic::Index(receiver.ty),
                    vec![receiver.slot, index.slot],
                    span,
                )?
            }
            ExprKind::Float(raw) => {
                let value = raw
                    .parse::<f64>()
                    .map_err(|_| error("LARP-T004", span, "Invalid floating-point literal"))?;
                if !value.is_finite() {
                    return Err(error(
                        "LARP-T004",
                        span,
                        "Floating-point literals must be finite",
                    ));
                }
                self.literal(Scalar::Geeked(value), span)
            }
            ExprKind::Text(value) => self.literal(Scalar::Text(value.clone()), span),
            ExprKind::Verdict(value) => self.literal(Scalar::Verdict(*value), span),
            ExprKind::SelfValue => self.self_value(span)?,
            ExprKind::New { class, arguments } => {
                self.construct(class, arguments, span, evaluate)?
            }
            ExprKind::Name(name) => {
                if let Some(binding) = self.resolve(&name.text) {
                    self.read(&binding, span)
                } else if name.text == "fortnine" && !self.names.contains_key(&name.text) {
                    self.literal(Scalar::Unc(49), span)
                } else if let Some(index) = self.names.get(&name.text).copied() {
                    let header = &self.headers[index];
                    let function_type = Type::function(
                        header.parameters.clone(),
                        header.result.clone(),
                        header.effects,
                    );
                    let slot = self.local(function_type.clone());
                    self.emit(
                        Inst::Closure {
                            destination: slot,
                            function: index,
                            captures: Vec::new(),
                        },
                        span,
                    );
                    Value {
                        slot,
                        ty: function_type,
                        constant: None,
                    }
                } else if calls::is_builtin(&name.text) {
                    self.builtin_reference(name, expected.clone(), span)?
                } else {
                    return Err(error(
                        "LARP-N001",
                        span,
                        format!("Unknown name '{}'", name.text),
                    ));
                }
            }
            ExprKind::Unary {
                op: Unary::Negate,
                operand,
            } if matches!(operand.kind, ExprKind::Integer(_)) => {
                let ExprKind::Integer(digits) = &operand.kind else {
                    unreachable!()
                };
                self.integer(digits, true, expected.clone(), span)?
            }
            ExprKind::Unary { op, operand } => {
                let context = if *op == Unary::Not {
                    Some(Type::Verdict)
                } else {
                    expected
                        .clone()
                        .filter(|ty| ty.is_numeric())
                        .or_else(|| self.hint(operand))
                };
                let operand = self.expression(operand, context, evaluate)?;
                let result = unary_type(*op, &operand.ty).ok_or_else(|| {
                    error(
                        "LARP-T001",
                        span,
                        format!("Operator {op:?} does not accept {}", operand.ty),
                    )
                })?;
                if evaluate && let Some(constant) = &operand.constant {
                    let constant = larp_types::unary(*op, constant).map_err(|_| {
                        error("LARP-T004", span, "Invalid constant unary operation")
                    })?;
                    self.literal(constant, span)
                } else {
                    if *op == Unary::Negate && operand.ty.is_integer() {
                        self.require(Effects::ABORT, span)?;
                    }
                    let slot = self.local(result.clone());
                    self.emit(
                        Inst::Unary {
                            destination: slot,
                            op: *op,
                            operand: operand.slot,
                        },
                        span,
                    );
                    Value {
                        slot,
                        ty: result,
                        constant: None,
                    }
                }
            }
            ExprKind::Binary {
                op: Binary::And | Binary::Or,
                left,
                right,
            } => {
                let ExprKind::Binary { op, .. } = &expression.kind else {
                    unreachable!()
                };
                self.short_circuit(*op, left, right, span, evaluate)?
            }
            ExprKind::Binary { op, left, right } => {
                if let (Some(left_type), Some(right_type)) = (self.hint(left), self.hint(right))
                    && left_type != right_type
                {
                    return Err(error(
                        "LARP-T001",
                        span,
                        format!(
                            "Operator {op:?} requires matching operand types, found {left_type} and {right_type}"
                        ),
                    ));
                }
                let comparisons = matches!(
                    op,
                    Binary::Equal
                        | Binary::NotEqual
                        | Binary::Less
                        | Binary::LessEqual
                        | Binary::Greater
                        | Binary::GreaterEqual
                );
                let context = if comparisons {
                    None
                } else {
                    expected
                        .clone()
                        .filter(|ty| ty.is_numeric() || *ty == Type::Text)
                };
                let context = context
                    .or_else(|| self.hint(left))
                    .or_else(|| self.hint(right));
                let left = self.expression(left, context, evaluate)?;
                let right = self.expression(right, Some(left.ty.clone()), evaluate)?;
                self.binary(*op, left, right, span, evaluate)?
            }
            ExprKind::Call {
                callee,
                arguments,
                type_arguments,
            } => self.call(
                callee,
                arguments,
                type_arguments,
                expected.clone(),
                span,
                evaluate,
            )?,
            ExprKind::None => {
                let Some(option @ Type::Baddie(_)) = expected.clone() else {
                    return Err(error(
                        "LARP-T003",
                        span,
                        "no_bitches requires an expected baddie<T> type",
                    ));
                };
                self.intrinsic(Intrinsic::None(option), Vec::new(), span)?
            }
            ExprKind::Member { receiver, name } => {
                if self.function.constructor.is_some()
                    && matches!(receiver.kind, ExprKind::SelfValue)
                {
                    return self.constructor_field_read(name, span);
                }
                let receiver = self.expression(receiver, None, evaluate)?;
                if receiver.ty == Type::ConversionError && name.text == "message" {
                    self.intrinsic(Intrinsic::ConversionMessage, vec![receiver.slot], span)?
                } else {
                    self.member_value(receiver, name, span)?
                }
            }
            ExprKind::Lambda { parameters, body } => {
                self.lambda(parameters, body, expected.clone(), span)?
            }
        };
        Ok(value)
    }
    fn integer(
        &mut self,
        digits: &str,
        negative: bool,
        expected: Option<Type>,
        span: Span,
    ) -> Result<Value> {
        let ty = expected
            .clone()
            .filter(|ty| ty.is_integer())
            .unwrap_or(Type::Mf);
        let invalid = || {
            error(
                "LARP-T004",
                span,
                format!("Integer literal does not fit {ty}"),
            )
        };
        let magnitude = digits.parse::<i128>().map_err(|_| invalid())?;
        let value = if negative { -magnitude } else { magnitude };
        let value = match ty {
            Type::Mf => Scalar::Mf(i32::try_from(value).map_err(|_| invalid())?),
            Type::Unc => Scalar::Unc(i64::try_from(value).map_err(|_| invalid())?),
            _ => unreachable!(),
        };
        Ok(self.literal(value, span))
    }
    fn binary(
        &mut self,
        op: Binary,
        left: Value,
        right: Value,
        span: Span,
        evaluate: bool,
    ) -> Result<Value> {
        let result = binary_type(op, &left.ty, &right.ty).ok_or_else(|| {
            error(
                "LARP-T001",
                span,
                format!(
                    "Operator {op:?} does not accept {} and {}",
                    left.ty, right.ty
                ),
            )
        })?;
        if evaluate && let (Some(a), Some(b)) = (&left.constant, &right.constant) {
            let value = larp_types::binary(op, a, b).map_err(|_| {
                error(
                    "LARP-T004",
                    span,
                    "Invalid constant arithmetic (overflow or division by zero)",
                )
            })?;
            return Ok(self.literal(value, span));
        }
        if left.ty.is_integer()
            && matches!(
                op,
                Binary::Add
                    | Binary::Subtract
                    | Binary::Multiply
                    | Binary::Divide
                    | Binary::Remainder
            )
        {
            self.require(Effects::ABORT, span)?;
        }
        let slot = self.local(result.clone());
        self.emit(
            Inst::Binary {
                destination: slot,
                op,
                left: left.slot,
                right: right.slot,
            },
            span,
        );
        Ok(Value {
            slot,
            ty: result,
            constant: None,
        })
    }
    fn short_circuit(
        &mut self,
        op: Binary,
        left: &Expr,
        right: &Expr,
        span: Span,
        evaluate: bool,
    ) -> Result<Value> {
        let left = self.expression(left, Some(Type::Verdict), evaluate)?;
        let skip = matches!(
            (&left.constant, op),
            (Some(Scalar::Verdict(false)), Binary::And) | (Some(Scalar::Verdict(true)), Binary::Or)
        );
        let result = self.local(Type::Verdict);
        self.emit(
            Inst::Copy {
                destination: result,
                source: left.slot,
            },
            span,
        );
        let run = self.new_block(right.span);
        let done = self.new_block(span);
        let (when_true, when_false) = if op == Binary::And {
            (run, done)
        } else {
            (done, run)
        };
        self.terminate(
            Terminator::Branch {
                condition: left.slot,
                when_true,
                when_false,
            },
            span,
        );
        self.current = run;
        let right = self.expression(right, Some(Type::Verdict), evaluate && !skip)?;
        self.emit(
            Inst::Copy {
                destination: result,
                source: right.slot,
            },
            span,
        );
        self.terminate(Terminator::Goto(done), span);
        self.current = done;
        let constant = if skip {
            left.constant
        } else if evaluate {
            match (&left.constant, &right.constant) {
                (Some(a), Some(b)) => larp_types::binary(op, a, b).ok(),
                _ => None,
            }
        } else {
            None
        };
        Ok(Value {
            slot: result,
            ty: Type::Verdict,
            constant,
        })
    }
}
