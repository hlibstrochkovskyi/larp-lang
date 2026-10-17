use larp_diagnostics::{Diagnostic, Label};
use larp_ir::{
    Block, BlockId, Function, Instruction, InstructionKind as Inst, Local, Program, Terminator,
};
use larp_source::Span;
use larp_syntax::{self as ast, Expr, ExprKind, Name, StatementKind};
use larp_types::{Binary, Effects, Scalar, Type, Unary, binary_type, unary_type};
use std::collections::HashMap;

type Result<T> = std::result::Result<T, Box<Diagnostic>>;

#[derive(Clone)]
struct Header {
    name: Name,
    parameters: Vec<Type>,
    result: Type,
    effects: Effects,
}

fn error(code: &str, span: Span, message: impl Into<String>) -> Box<Diagnostic> {
    Box::new(Diagnostic::error(code, span, message))
}

fn ty(name: &Name) -> Result<Type> {
    Type::from_name(&name.text).ok_or_else(|| {
        error(
            "LARP-T005",
            name.span,
            format!("Unknown type '{}'", name.text),
        )
    })
}

pub fn check(ast: &ast::Program) -> std::result::Result<Program, Vec<Diagnostic>> {
    let mut errors = Vec::new();
    let mut headers: Vec<Header> = Vec::new();
    let mut names: HashMap<String, usize> = HashMap::new();
    if ast.functions.len() >= 4096 {
        return Err(vec![Diagnostic::error(
            "LARP-P003",
            ast.main.span,
            "Too many functions (limit: 4096 including main)",
        )]);
    }
    for function in &ast.functions {
        let result = (|| -> Result<Header> {
            if let Some(previous) = names.get(&function.name.text) {
                let mut diagnostic = error(
                    "LARP-N002",
                    function.name.span,
                    format!("Duplicate function '{}'", function.name.text),
                );
                diagnostic.related.push(Label {
                    span: headers[*previous].name.span,
                    message: "First declaration".into(),
                });
                return Err(diagnostic);
            }
            let mut parameters = Vec::new();
            for (parameter_type, _) in &function.parameters {
                let parameter_type = ty(parameter_type)?;
                if parameter_type == Type::Unit {
                    return Err(error(
                        "LARP-T001",
                        function.name.span,
                        "Function parameters cannot have type unit",
                    ));
                }
                parameters.push(parameter_type);
            }
            let mut effects = Effects::ABORT;
            if let Some(declared) = &function.effects {
                effects = Effects::NONE;
                for effect in declared {
                    effects = effects.union(Effects::from_name(&effect.text).ok_or_else(|| {
                        error(
                            "LARP-E003",
                            effect.span,
                            format!("Unknown effect '{}'", effect.text),
                        )
                    })?);
                }
            }
            Ok(Header {
                name: function.name.clone(),
                parameters,
                result: ty(&function.result)?,
                effects,
            })
        })();
        match result {
            Ok(header) => {
                names.insert(header.name.text.clone(), headers.len());
                headers.push(header);
            }
            Err(error) => errors.push(*error),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
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
    });
    let mut functions = Vec::new();
    for (index, header) in headers.iter().enumerate() {
        let body = if index == entry {
            &ast.main
        } else {
            &ast.functions[index].body
        };
        let mut builder = Builder::new(header, &headers, &names, body.span);
        if index != entry {
            for (slot, (_, name)) in ast.functions[index].parameters.iter().enumerate() {
                if let Err(error) = builder.declare(
                    name,
                    Binding {
                        slot,
                        ty: header.parameters[slot],
                        writable: true,
                        span: name.span,
                    },
                ) {
                    builder.errors.push(*error);
                }
            }
        }
        builder.block(body, false);
        let reachable = builder.function.reachable();
        if builder.errors.is_empty() {
            for (block, is_reachable) in builder.function.blocks.iter_mut().zip(reachable) {
                if is_reachable && matches!(block.terminator, Terminator::Unreachable) {
                    if header.result == Type::Unit {
                        block.terminator = Terminator::Return(None);
                    } else {
                        builder.errors.push(Diagnostic::error(
                            "LARP-T007",
                            body.span,
                            format!(
                                "Function '{}' can finish without returning {}",
                                header.name.text, header.result
                            ),
                        ));
                        break;
                    }
                }
            }
        }
        functions.push(builder.function);
        errors.extend(builder.errors);
    }
    if errors.is_empty() {
        Ok(Program { functions, entry })
    } else {
        Err(errors)
    }
}

#[derive(Clone, Copy)]
struct Binding {
    slot: Local,
    ty: Type,
    writable: bool,
    span: Span,
}

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
    errors: Vec<Diagnostic>,
}

impl<'a> Builder<'a> {
    fn new(
        header: &Header,
        headers: &'a [Header],
        names: &'a HashMap<String, usize>,
        span: Span,
    ) -> Self {
        Self {
            function: Function {
                name: header.name.text.clone(),
                parameters: header.parameters.clone(),
                result: header.result,
                effects: header.effects,
                locals: header.parameters.clone(),
                blocks: vec![Block {
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
            errors: Vec::new(),
        }
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
        let slot = self.local(ty);
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
            .find_map(|scope| scope.get(name).copied())
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
    fn require(&self, effects: Effects, span: Span) -> Result<()> {
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
        if expected == found {
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
            diagnostic.help = Some("Use a matching integer type; checked conversions will be available with result<T,E> support".into());
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
        let value = self.expression(expression, None, true)?;
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
                let declared = declared.as_ref().map(ty).transpose()?;
                let value = self.expression(value, declared, true)?;
                if value.ty == Type::Unit {
                    return Err(error(
                        "LARP-T001",
                        name.span,
                        "A local binding cannot have type unit",
                    ));
                }
                let slot = self.local(value.ty);
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
                        ty: value.ty,
                        writable: true,
                        span: name.span,
                    },
                )?;
            }
            StatementKind::Assignment { name, op, value } => {
                let binding = self.resolve(&name.text).ok_or_else(|| {
                    error(
                        "LARP-N001",
                        name.span,
                        format!("Unknown binding '{}'", name.text),
                    )
                })?;
                if !binding.writable {
                    return Err(error(
                        "LARP-T008",
                        name.span,
                        "A range-loop index is read-only",
                    ));
                }
                let previous = if op.is_some() {
                    Some(self.read(binding, name.span))
                } else {
                    None
                };
                let value = self.expression(value, Some(binding.ty), true)?;
                let value = if let Some(op) = op {
                    self.binary(*op, previous.unwrap(), value, span, true)?
                } else {
                    value
                };
                self.emit(
                    Inst::Copy {
                        destination: binding.slot,
                        source: value.slot,
                    },
                    span,
                );
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
                let index_type = ty(index_type)?;
                if !index_type.is_integer() {
                    return Err(error(
                        "LARP-T001",
                        name.span,
                        "Range-loop index must be mf or unc",
                    ));
                }
                let start = self.expression(start, Some(index_type), true)?;
                let end = self.expression(end, Some(index_type), true)?;
                self.scopes.push(HashMap::new());
                let slot = self.local(index_type);
                self.emit(
                    Inst::Copy {
                        destination: slot,
                        source: start.slot,
                    },
                    span,
                );
                let binding = Binding {
                    slot,
                    ty: index_type,
                    writable: false,
                    span: name.span,
                };
                self.declare(name, binding)?;
                let test = self.new_block(span);
                let run = self.new_block(body.span);
                let step = self.new_block(span);
                let done = self.new_block(span);
                self.terminate(Terminator::Goto(test), span);
                self.current = test;
                let current = self.read(binding, span);
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
                let current = self.read(binding, span);
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
                let value = if let Some(expression) = expression {
                    Some(
                        self.expression(expression, Some(self.function.result), true)?
                            .slot,
                    )
                } else {
                    self.matches(self.function.result, Type::Unit, span)?;
                    None
                };
                self.terminate(Terminator::Return(value), span);
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
    fn read(&mut self, binding: Binding, span: Span) -> Value {
        let slot = self.local(binding.ty);
        self.emit(
            Inst::Copy {
                destination: slot,
                source: binding.slot,
            },
            span,
        );
        Value {
            slot,
            ty: binding.ty,
            constant: None,
        }
    }
    fn hint(&self, expression: &Expr) -> Option<Type> {
        match &expression.kind {
            ExprKind::Name(name) => {
                self.resolve(&name.text)
                    .map(|binding| binding.ty)
                    .or_else(|| {
                        if name.text == "fortnine" {
                            Some(Type::Unc)
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
                if let ExprKind::Name(name) = &callee.kind {
                    self.names
                        .get(&name.text)
                        .map(|index| self.headers[*index].result)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
    fn expression(
        &mut self,
        expression: &Expr,
        expected: Option<Type>,
        evaluate: bool,
    ) -> Result<Value> {
        let span = expression.span;
        let value = match &expression.kind {
            ExprKind::Integer(digits) => self.integer(digits, false, expected, span)?,
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
            ExprKind::Name(name) => {
                if let Some(binding) = self.resolve(&name.text) {
                    self.read(binding, span)
                } else if name.text == "fortnine" && !self.names.contains_key(&name.text) {
                    self.literal(Scalar::Unc(49), span)
                } else if self.names.contains_key(&name.text) {
                    return Err(error(
                        "LARP-P002",
                        span,
                        "Functions as values are not implemented yet; call the function directly",
                    ));
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
                self.integer(digits, true, expected, span)?
            }
            ExprKind::Unary { op, operand } => {
                let context = if *op == Unary::Not {
                    Some(Type::Verdict)
                } else {
                    expected
                        .filter(|ty| ty.is_numeric())
                        .or_else(|| self.hint(operand))
                };
                let operand = self.expression(operand, context, evaluate)?;
                let result = unary_type(*op, operand.ty).ok_or_else(|| {
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
                    let slot = self.local(result);
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
                    expected.filter(|ty| ty.is_numeric() || *ty == Type::Text)
                };
                let context = context
                    .or_else(|| self.hint(left))
                    .or_else(|| self.hint(right));
                let left = self.expression(left, context, evaluate)?;
                let right = self.expression(right, Some(left.ty), evaluate)?;
                self.binary(*op, left, right, span, evaluate)?
            }
            ExprKind::Call { callee, arguments } => {
                self.call(callee, arguments, expected, span, evaluate)?
            }
        };
        if let Some(expected) = expected {
            self.matches(expected, value.ty, span)?;
        }
        Ok(value)
    }
    fn integer(
        &mut self,
        digits: &str,
        negative: bool,
        expected: Option<Type>,
        span: Span,
    ) -> Result<Value> {
        let ty = expected.filter(|ty| ty.is_integer()).unwrap_or(Type::Mf);
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
        let result = binary_type(op, left.ty, right.ty).ok_or_else(|| {
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
        let slot = self.local(result);
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
    fn call(
        &mut self,
        callee: &Expr,
        arguments: &[Expr],
        expected: Option<Type>,
        span: Span,
        evaluate: bool,
    ) -> Result<Value> {
        let ExprKind::Name(name) = &callee.kind else {
            return Err(error(
                "LARP-P002",
                callee.span,
                "Indirect function calls are not implemented yet",
            ));
        };
        if let Some(binding) = self.resolve(&name.text) {
            return Err(error(
                "LARP-T001",
                name.span,
                format!("{} is not callable", binding.ty),
            ));
        }
        if let Some(index) = self.names.get(&name.text).copied() {
            let header = self.headers[index].clone();
            if arguments.len() != header.parameters.len() {
                return Err(error(
                    "LARP-T006",
                    span,
                    format!(
                        "Expected {} arguments, found {}",
                        header.parameters.len(),
                        arguments.len()
                    ),
                ));
            }
            self.require(header.effects, span)?;
            let arguments = arguments
                .iter()
                .zip(&header.parameters)
                .map(|(expression, ty)| {
                    self.expression(expression, Some(*ty), evaluate)
                        .map(|value| value.slot)
                })
                .collect::<Result<Vec<_>>>()?;
            let slot = self.local(header.result);
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
        if !matches!(name.text.as_str(), "yap" | "no_cap" | "crashout") {
            return Err(error(
                "LARP-N001",
                name.span,
                format!("Unknown function '{}'", name.text),
            ));
        }
        if arguments.len() != 1 {
            return Err(error(
                "LARP-T006",
                span,
                format!("{} requires exactly one argument", name.text),
            ));
        }
        let context = match name.text.as_str() {
            "no_cap" => Some(Type::Verdict),
            "crashout" => Some(Type::Text),
            _ => None,
        };
        let value = self.expression(&arguments[0], context, evaluate)?;
        match name.text.as_str() {
            "yap" => {
                if value.ty == Type::Unit {
                    return Err(error("LARP-T001", span, "yap cannot print unit"));
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
                let dummy = match ty {
                    Type::Mf => Scalar::Mf(0),
                    Type::Unc => Scalar::Unc(0),
                    Type::Geeked => Scalar::Geeked(0.0),
                    Type::Verdict => Scalar::Verdict(false),
                    Type::Text => Scalar::Text(String::new()),
                    Type::Unit => Scalar::Unit,
                };
                let mut value = self.literal(dummy, span);
                value.constant = None;
                return Ok(value);
            }
            _ => unreachable!(),
        }
        Ok(self.literal(Scalar::Unit, span))
    }
}
