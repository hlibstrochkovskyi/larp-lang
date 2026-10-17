//! In-memory typed stack bytecode and mandatory control-flow verification.

use larp_source::Span;
use larp_types::{Binary, Effects, Intrinsic, Scalar, Type, Unary, binary_type, unary_type};
use std::collections::VecDeque;
use std::fmt::Write;

mod classes;

#[derive(Clone, Debug)]
pub struct Module {
    pub classes: Vec<Class>,
    pub functions: Vec<Function>,
    pub entry: usize,
}

#[derive(Clone, Debug)]
pub struct Class {
    pub name: String,
    pub fields: Vec<Field>,
    pub constructor: usize,
    pub methods: Vec<Method>,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub ty: Type,
    pub public: bool,
}

#[derive(Clone, Debug)]
pub struct Method {
    pub name: String,
    pub function: usize,
    pub public: bool,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub owner: Option<usize>,
    pub constructor: Option<usize>,
    pub name: String,
    pub parameters: Vec<Type>,
    pub captures: Vec<Type>,
    pub result: Type,
    pub effects: Effects,
    pub locals: Vec<Type>,
    pub code: Vec<Instruction>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Instruction {
    pub op: Op,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Op {
    Constant(Scalar),
    Load(usize),
    Store(usize),
    Unary(Unary, Type),
    Binary(Binary, Type),
    Jump(usize),
    JumpIfFalse(usize),
    Call(usize),
    MakeClosure(usize),
    Construct(usize),
    NewObject(usize),
    GetField {
        class: usize,
        field: usize,
    },
    SetField {
        class: usize,
        field: usize,
    },
    CallIndirect(Type),
    Intrinsic(Intrinsic),
    MatchResult {
        ty: Type,
        success: usize,
        error: usize,
    },
    Return,
    Print,
    Assert,
    Crash,
}

#[derive(Debug)]
pub struct VerifyError {
    pub kind: VerifyErrorKind,
    pub function: usize,
    pub instruction: Option<usize>,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerifyErrorKind {
    Invalid,
    ResourceLimit,
}

impl VerifyError {
    fn resource(mut self) -> Self {
        self.kind = VerifyErrorKind::ResourceLimit;
        self
    }
}

#[derive(Debug)]
pub struct VerifiedModule(Module);

impl VerifiedModule {
    pub fn module(&self) -> &Module {
        &self.0
    }

    pub fn disassemble(&self) -> String {
        let mut result = String::new();
        for (index, class) in self.0.classes.iter().enumerate() {
            let _ = writeln!(
                result,
                "class {index} {} constructor={}",
                class.name, class.constructor
            );
            for (field, layout) in class.fields.iter().enumerate() {
                let _ = writeln!(
                    result,
                    "  field {field} {}: {} public={}",
                    layout.name, layout.ty, layout.public
                );
            }
            for method in &class.methods {
                let _ = writeln!(
                    result,
                    "  method {} function={} public={}",
                    method.name, method.function, method.public
                );
            }
        }
        for (index, function) in self.0.functions.iter().enumerate() {
            let _ = writeln!(
                result,
                "function {index} {}({:?}) -> {} {}",
                function.name, function.parameters, function.result, function.effects
            );
            for (pc, instruction) in function.code.iter().enumerate() {
                let _ = writeln!(
                    result,
                    "  {pc:04} {:?} @ {:?}",
                    instruction.op,
                    instruction.span.range()
                );
            }
        }
        result
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct State {
    stack: Vec<Type>,
    initialized: Vec<bool>,
}

pub fn operation_effect(op: &Op) -> Effects {
    match op {
        Op::Print => Effects::OBSERVE,
        Op::GetField { .. } => Effects::READ,
        Op::SetField { .. } => Effects::WRITE,
        Op::Assert | Op::Crash => Effects::ABORT,
        Op::Unary(Unary::Negate, ty) if ty.is_integer() => Effects::ABORT,
        Op::Binary(
            Binary::Add | Binary::Subtract | Binary::Multiply | Binary::Divide | Binary::Remainder,
            ty,
        ) if ty.is_integer() => Effects::ABORT,
        _ => Effects::NONE,
    }
}

pub fn verify(module: Module) -> Result<VerifiedModule, VerifyError> {
    let invalid = |message: &str| VerifyError {
        kind: VerifyErrorKind::Invalid,
        function: module.entry,
        instruction: None,
        message: message.into(),
    };
    if module.functions.len() > 4096 {
        return Err(invalid("Function limit exceeded").resource());
    }
    if module.functions.is_empty() {
        return Err(invalid("Invalid function count"));
    }
    let Some(entry) = module.functions.get(module.entry) else {
        return Err(invalid("Entry function is out of bounds"));
    };
    if !entry.parameters.is_empty() || !entry.captures.is_empty() || entry.result != Type::Unit {
        return Err(invalid("Entry must take no parameters and return unit"));
    }
    if entry.owner.is_some() || entry.constructor.is_some() {
        return Err(invalid("Entry cannot belong to a class"));
    }
    classes::preflight_types(&module)?;
    // A forward call/closure can expose another function's types during
    // dataflow. Validate every type before any structural type comparison.
    for (index, function) in module.functions.iter().enumerate() {
        let validate = |ty: &Type, instruction| {
            let resource = !ty.within_limits();
            if resource || !ty.is_well_formed() {
                Err(VerifyError {
                    kind: if resource {
                        VerifyErrorKind::ResourceLimit
                    } else {
                        VerifyErrorKind::Invalid
                    },
                    function: index,
                    instruction,
                    message: if resource {
                        "Type complexity limit exceeded"
                    } else {
                        "Malformed type"
                    }
                    .into(),
                })
            } else {
                Ok(())
            }
        };
        for ty in function
            .locals
            .iter()
            .chain(&function.parameters)
            .chain(&function.captures)
            .chain(std::iter::once(&function.result))
        {
            validate(ty, None)?;
        }
        for (pc, instruction) in function.code.iter().enumerate() {
            match &instruction.op {
                Op::Unary(_, ty)
                | Op::Binary(_, ty)
                | Op::CallIndirect(ty)
                | Op::MatchResult { ty, .. } => validate(ty, Some(pc))?,
                Op::Intrinsic(operation) => {
                    for ty in operation.types() {
                        validate(ty, Some(pc))?;
                    }
                    if let Some(signature) = operation.signature() {
                        validate(&signature.result, Some(pc))?;
                        for ty in &signature.parameters {
                            validate(ty, Some(pc))?;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    classes::validate_layouts(&module)?;
    for index in 0..module.functions.len() {
        verify_function(&module, index)?;
    }
    Ok(VerifiedModule(module))
}

fn verify_function(module: &Module, index: usize) -> Result<(), VerifyError> {
    let function = &module.functions[index];
    let error = |pc: Option<usize>, message: String| VerifyError {
        kind: VerifyErrorKind::Invalid,
        function: index,
        instruction: pc,
        message,
    };
    if function.code.len() > 1_000_000 || function.locals.len() > 65_536 {
        return Err(error(None, "Code or local limit exceeded".into()).resource());
    }
    if function.code.is_empty() {
        return Err(error(None, "Function code is empty".into()));
    }
    if function.locals.get(..function.parameters.len()) != Some(function.parameters.as_slice())
        || function.parameters.contains(&Type::Unit)
    {
        return Err(error(
            None,
            "Parameter slots do not match the signature".into(),
        ));
    }
    let capture_start = function.parameters.len();
    let capture_end = capture_start + function.captures.len();
    if function.locals.get(capture_start..capture_end) != Some(function.captures.as_slice()) {
        return Err(error(
            None,
            "Capture slots do not match the closure signature".into(),
        ));
    }
    let receiver_readonly = module
        .classes
        .iter()
        .any(|class| class.methods.iter().any(|method| method.function == index));
    // Validate structural indices and capabilities even in unreachable code.
    for (pc, instruction) in function.code.iter().enumerate() {
        let mut effects = operation_effect(&instruction.op);
        match &instruction.op {
            Op::Load(local) | Op::Store(local) if *local >= function.locals.len() => {
                return Err(error(Some(pc), "Local index out of bounds".into()));
            }
            Op::Store(0) if receiver_readonly => {
                return Err(error(Some(pc), "Method receiver is read-only".into()));
            }
            Op::Return if function.constructor.is_some() => {
                return Err(error(
                    Some(pc),
                    "Constructor requires terminal object completion".into(),
                ));
            }
            Op::Store(local) if (capture_start..capture_end).contains(local) => {
                return Err(error(Some(pc), "Captured bindings are read-only".into()));
            }
            Op::Jump(target) | Op::JumpIfFalse(target) if *target >= function.code.len() => {
                return Err(error(Some(pc), "Jump target out of bounds".into()));
            }
            Op::Call(target) => {
                let Some(callee) = module.functions.get(*target) else {
                    return Err(error(Some(pc), "Call target out of bounds".into()));
                };
                classes::call_access(module, function, *target)
                    .map_err(|message| error(Some(pc), message))?;
                effects = effects.union(callee.effects);
                if !callee.captures.is_empty() {
                    return Err(error(
                        Some(pc),
                        "Direct calls cannot supply closure captures".into(),
                    ));
                }
            }
            Op::MakeClosure(target) => {
                if *target >= module.functions.len() {
                    return Err(error(Some(pc), "Closure target out of bounds".into()));
                }
                classes::call_access(module, function, *target)
                    .map_err(|message| error(Some(pc), message))?;
            }
            Op::Construct(class) => {
                let layout = module
                    .classes
                    .get(*class)
                    .ok_or_else(|| error(Some(pc), "Class index out of bounds".into()))?;
                effects = effects.union(module.functions[layout.constructor].effects);
            }
            Op::NewObject(class) => {
                if *class >= module.classes.len() {
                    return Err(error(Some(pc), "Class index out of bounds".into()));
                }
                if function.constructor != Some(*class) {
                    return Err(error(
                        Some(pc),
                        "Object allocation requires its constructor".into(),
                    ));
                }
            }
            Op::GetField { class, field } | Op::SetField { class, field } => {
                classes::field_access(module, function, *class, *field)
                    .map_err(|message| error(Some(pc), message))?;
            }
            Op::CallIndirect(Type::Function(signature)) => {
                effects = effects.union(signature.effects)
            }
            Op::CallIndirect(_) => {
                return Err(error(
                    Some(pc),
                    "Indirect call requires a function type".into(),
                ));
            }
            Op::Intrinsic(operation) => {
                let Some(signature) = operation.signature() else {
                    return Err(error(Some(pc), "Invalid intrinsic signature".into()));
                };
                effects = effects.union(signature.effects);
            }
            Op::MatchResult {
                ty,
                success,
                error: failure,
            } => {
                if !matches!(ty, Type::Result(_))
                    || *success >= function.code.len()
                    || *failure >= function.code.len()
                {
                    return Err(error(
                        Some(pc),
                        "Invalid result dispatch type or target".into(),
                    ));
                }
            }
            Op::Unary(op, ty) if unary_type(*op, ty).is_none() => {
                return Err(error(Some(pc), "Invalid unary operand type".into()));
            }
            Op::Binary(op, ty) if binary_type(*op, ty, ty).is_none() => {
                return Err(error(Some(pc), "Invalid binary operand type".into()));
            }
            _ => {}
        }
        if function.constructor.is_some()
            && effects.contains(Effects::WRITE)
            && !matches!(instruction.op, Op::Construct(_))
        {
            return Err(error(
                Some(pc),
                "Constructor cannot mutate existing heap state".into(),
            ));
        }
        if !function.effects.contains(effects) {
            return Err(error(
                Some(pc),
                format!(
                    "Instruction requires {effects}, outside {}",
                    function.effects
                ),
            ));
        }
    }
    let mut initial = State {
        stack: Vec::new(),
        initialized: vec![false; function.locals.len()],
    };
    initial.initialized[..capture_end].fill(true);
    let mut states: Vec<Option<State>> = vec![None; function.code.len()];
    let mut analysis_cells = initial.initialized.len();
    states[0] = Some(initial);
    let mut queue = VecDeque::from([0]);
    while let Some(pc) = queue.pop_front() {
        let mut state = states[pc].clone().unwrap();
        let failure = |message: String| error(Some(pc), message);
        let pop = |state: &mut State, ty: Type| -> Result<(), VerifyError> {
            let found = state
                .stack
                .pop()
                .ok_or_else(|| failure("Operand stack underflow".into()))?;
            if ty.accepts(&found) {
                Ok(())
            } else {
                Err(failure(format!("Expected {ty}, found {found}")))
            }
        };
        let mut successors = Vec::with_capacity(2);
        match &function.code[pc].op {
            Op::Constant(value) => state.stack.push(value.ty()),
            Op::Load(local) => {
                if !state.initialized[*local] {
                    return Err(failure("Read of an uninitialized local".into()));
                }
                state.stack.push(function.locals[*local].clone());
            }
            Op::Store(local) => {
                pop(&mut state, function.locals[*local].clone())?;
                state.initialized[*local] = true;
            }
            Op::Unary(op, ty) => {
                pop(&mut state, ty.clone())?;
                state.stack.push(unary_type(*op, ty).unwrap());
            }
            Op::Binary(op, ty) => {
                pop(&mut state, ty.clone())?;
                pop(&mut state, ty.clone())?;
                state.stack.push(binary_type(*op, ty, ty).unwrap());
            }
            Op::Jump(target) => successors.push((*target, None)),
            Op::JumpIfFalse(target) => {
                pop(&mut state, Type::Verdict)?;
                successors.push((*target, None));
                successors.push((pc + 1, None));
            }
            Op::Call(target) => {
                let callee = &module.functions[*target];
                for ty in callee.parameters.iter().rev() {
                    pop(&mut state, ty.clone())?;
                }
                state.stack.push(callee.result.clone());
            }
            Op::Construct(class) => {
                let constructor = &module.functions[module.classes[*class].constructor];
                for ty in constructor.parameters.iter().rev() {
                    pop(&mut state, ty.clone())?;
                }
                state.stack.push(constructor.result.clone());
            }
            Op::NewObject(class) => {
                let layout = &module.classes[*class];
                for field in layout.fields.iter().rev() {
                    pop(&mut state, field.ty.clone())?;
                }
                if !state.stack.is_empty() {
                    return Err(failure(
                        "Object completion leaves extra operands on the stack".into(),
                    ));
                }
                continue;
            }
            Op::GetField { class, field } => {
                let layout = &module.classes[*class];
                pop(&mut state, Type::class(*class, layout.name.as_str()))?;
                state.stack.push(layout.fields[*field].ty.clone());
            }
            Op::SetField { class, field } => {
                let layout = &module.classes[*class];
                pop(&mut state, layout.fields[*field].ty.clone())?;
                pop(&mut state, Type::class(*class, layout.name.as_str()))?;
                state.stack.push(Type::Unit);
            }
            Op::MakeClosure(target) => {
                let callee = &module.functions[*target];
                for capture in callee.captures.iter().rev() {
                    pop(&mut state, capture.clone())?;
                }
                state.stack.push(Type::function(
                    callee.parameters.clone(),
                    callee.result.clone(),
                    callee.effects,
                ));
            }
            Op::CallIndirect(ty @ Type::Function(signature)) => {
                for parameter in signature.parameters.iter().rev() {
                    pop(&mut state, parameter.clone())?;
                }
                pop(&mut state, ty.clone())?;
                state.stack.push(signature.result.clone());
            }
            Op::CallIndirect(_) => unreachable!("validated function signature"),
            Op::Intrinsic(operation) => {
                let signature = operation.signature().unwrap();
                for parameter in signature.parameters.iter().rev() {
                    pop(&mut state, parameter.clone())?;
                }
                state.stack.push(signature.result);
            }
            Op::MatchResult {
                ty: ty @ Type::Result(payloads),
                success,
                error,
            } => {
                pop(&mut state, ty.clone())?;
                successors.push((*success, Some(payloads.0.clone())));
                successors.push((*error, Some(payloads.1.clone())));
            }
            Op::MatchResult { .. } => unreachable!("validated result type"),
            Op::Print => {
                let ty = state
                    .stack
                    .pop()
                    .ok_or_else(|| failure("Operand stack underflow".into()))?;
                if !ty.is_printable() {
                    return Err(failure("Print requires a scalar value".into()));
                }
            }
            Op::Assert => {
                pop(&mut state, Type::Verdict)?;
            }
            Op::Crash => {
                pop(&mut state, Type::Text)?;
                continue;
            }
            Op::Return => {
                pop(&mut state, function.result.clone())?;
                if !state.stack.is_empty() {
                    return Err(failure("Return leaves extra operands on the stack".into()));
                }
                continue;
            }
        }
        if state.stack.len() > 65_536 {
            return Err(failure("Operand stack limit exceeded".into()).resource());
        }
        if successors.is_empty() {
            successors.push((pc + 1, None));
        }
        for (next, payload) in successors {
            let mut state = state.clone();
            if let Some(payload) = payload {
                state.stack.push(payload);
            }
            if next >= function.code.len() {
                return Err(failure("Execution falls outside the function".into()));
            }
            match &mut states[next] {
                None => {
                    analysis_cells += state.initialized.len() + state.stack.len();
                    if analysis_cells > 4_000_000 {
                        return Err(
                            failure("Verification analysis budget exceeded".into()).resource()
                        );
                    }
                    states[next] = Some(state.clone());
                    queue.push_back(next);
                }
                Some(previous) => {
                    if previous.stack != state.stack {
                        return Err(failure(
                            "Incompatible operand stacks at control-flow merge".into(),
                        ));
                    }
                    let mut changed = false;
                    for (old, incoming) in previous.initialized.iter_mut().zip(&state.initialized) {
                        if *old && !incoming {
                            *old = false;
                            changed = true;
                        }
                    }
                    if changed {
                        queue.push_back(next);
                    }
                }
            }
        }
    }
    Ok(())
}
