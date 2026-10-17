//! In-memory typed stack bytecode and mandatory control-flow verification.

use larp_source::Span;
use larp_types::{Binary, Effects, Scalar, Type, Unary, binary_type, unary_type};
use std::collections::VecDeque;
use std::fmt::Write;

#[derive(Clone, Debug)]
pub struct Module {
    pub functions: Vec<Function>,
    pub entry: usize,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<Type>,
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
    if !entry.parameters.is_empty() || entry.result != Type::Unit {
        return Err(invalid("Entry must take no parameters and return unit"));
    }
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
    // Validate structural indices and capabilities even in unreachable code.
    for (pc, instruction) in function.code.iter().enumerate() {
        let mut effects = operation_effect(&instruction.op);
        match &instruction.op {
            Op::Load(local) | Op::Store(local) if *local >= function.locals.len() => {
                return Err(error(Some(pc), "Local index out of bounds".into()));
            }
            Op::Jump(target) | Op::JumpIfFalse(target) if *target >= function.code.len() => {
                return Err(error(Some(pc), "Jump target out of bounds".into()));
            }
            Op::Call(target) => {
                let Some(callee) = module.functions.get(*target) else {
                    return Err(error(Some(pc), "Call target out of bounds".into()));
                };
                effects = effects.union(callee.effects);
            }
            Op::Unary(op, ty) if unary_type(*op, *ty).is_none() => {
                return Err(error(Some(pc), "Invalid unary operand type".into()));
            }
            Op::Binary(op, ty) if binary_type(*op, *ty, *ty).is_none() => {
                return Err(error(Some(pc), "Invalid binary operand type".into()));
            }
            _ => {}
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
    initial.initialized[..function.parameters.len()].fill(true);
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
            if found == ty {
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
                state.stack.push(function.locals[*local]);
            }
            Op::Store(local) => {
                pop(&mut state, function.locals[*local])?;
                state.initialized[*local] = true;
            }
            Op::Unary(op, ty) => {
                pop(&mut state, *ty)?;
                state.stack.push(unary_type(*op, *ty).unwrap());
            }
            Op::Binary(op, ty) => {
                pop(&mut state, *ty)?;
                pop(&mut state, *ty)?;
                state.stack.push(binary_type(*op, *ty, *ty).unwrap());
            }
            Op::Jump(target) => successors.push(*target),
            Op::JumpIfFalse(target) => {
                pop(&mut state, Type::Verdict)?;
                successors.push(*target);
                successors.push(pc + 1);
            }
            Op::Call(target) => {
                let callee = &module.functions[*target];
                for ty in callee.parameters.iter().rev() {
                    pop(&mut state, *ty)?;
                }
                state.stack.push(callee.result);
            }
            Op::Print => {
                let ty = state
                    .stack
                    .pop()
                    .ok_or_else(|| failure("Operand stack underflow".into()))?;
                if ty == Type::Unit {
                    return Err(failure("Cannot print unit".into()));
                }
            }
            Op::Assert => pop(&mut state, Type::Verdict)?,
            Op::Crash => {
                pop(&mut state, Type::Text)?;
                continue;
            }
            Op::Return => {
                pop(&mut state, function.result)?;
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
            successors.push(pc + 1);
        }
        for next in successors {
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
