//! Iterative execution of verified bytecode with checked values and traced closures.

mod heap;
mod value;

use heap::{Closure, Heap};
use larp_bytecode::{Op, VerifiedModule};
use larp_diagnostics::Diagnostic;
use larp_source::Span;
use larp_types::{ArithmeticError, Binary, Scalar};
use std::io::Write;
use value::Value;

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub instructions: u64,
    pub call_frames: usize,
    pub gc_stress: bool,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            instructions: 1_000_000,
            call_frames: 1024,
            gc_stress: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ExecutionStats {
    pub instructions: u64,
    pub allocations: u64,
    pub collections: u64,
    pub reclaimed: u64,
    pub peak_live_objects: usize,
}

struct Frame {
    function: usize,
    pc: usize,
    locals: Vec<Option<Value>>,
    stack_base: usize,
}

pub fn execute(
    module: &VerifiedModule,
    output: &mut impl Write,
    limits: Limits,
) -> Result<(), Box<Diagnostic>> {
    execute_with_stats(module, output, limits).map(|_| ())
}

pub fn execute_with_stats(
    module: &VerifiedModule,
    output: &mut impl Write,
    limits: Limits,
) -> Result<ExecutionStats, Box<Diagnostic>> {
    let module = module.module();
    let entry = &module.functions[module.entry];
    if limits.call_frames == 0 {
        return Err(Box::new(Diagnostic::error(
            "LARP-R006",
            entry.span,
            "Call-frame limit is zero",
        )));
    }
    let mut frames = vec![Frame {
        function: module.entry,
        pc: 0,
        locals: vec![None; entry.locals.len()],
        stack_base: 0,
    }];
    let mut stack: Vec<Value> = Vec::new();
    let mut heap = Heap::default();
    let mut fuel = limits.instructions;
    while !frames.is_empty() {
        let frame = frames.last().unwrap();
        let instruction = &module.functions[frame.function].code[frame.pc];
        let span = instruction.span;
        let error = |code: &str, message: String| Box::new(Diagnostic::error(code, span, message));
        if fuel == 0 {
            return Err(error("LARP-R006", "Instruction budget exhausted".into()));
        }
        fuel -= 1;
        // Captures remain in the operand stack until after this allocation safepoint.
        if matches!(instruction.op, Op::MakeClosure(_)) && heap.should_collect(limits.gc_stress) {
            heap.collect(
                stack.iter().chain(
                    frames
                        .iter()
                        .flat_map(|frame| frame.locals.iter().filter_map(Option::as_ref)),
                ),
            );
        }
        let frame = frames.last_mut().unwrap();
        frame.pc += 1;
        match &instruction.op {
            Op::Constant(value) => stack.push(Value::Scalar(value.clone())),
            Op::Load(local) => stack.push(
                frame.locals[*local]
                    .as_ref()
                    .expect("verified local initialization")
                    .clone(),
            ),
            Op::Store(local) => {
                frame.locals[*local] = Some(stack.pop().expect("verified operand stack"))
            }
            Op::Unary(op, _) => {
                let Value::Scalar(operand) = stack.pop().unwrap() else {
                    unreachable!("verified scalar unary operand")
                };
                stack.push(Value::Scalar(
                    larp_types::unary(*op, &operand).map_err(|error| arithmetic(error, span))?,
                ));
            }
            Op::Binary(op, _) => {
                let right = stack.pop().unwrap();
                let left = stack.pop().unwrap();
                if matches!(op, Binary::Equal | Binary::NotEqual) {
                    let equal = left.equals(&right);
                    stack.push(Value::Scalar(Scalar::Verdict(if *op == Binary::Equal {
                        equal
                    } else {
                        !equal
                    })));
                } else {
                    let (Value::Scalar(left), Value::Scalar(right)) = (left, right) else {
                        unreachable!("verified scalar binary operands")
                    };
                    stack.push(Value::Scalar(
                        larp_types::binary(*op, &left, &right)
                            .map_err(|error| arithmetic(error, span))?,
                    ));
                }
            }
            Op::Jump(target) => frame.pc = *target,
            Op::JumpIfFalse(target) => {
                let Value::Scalar(Scalar::Verdict(condition)) = stack.pop().unwrap() else {
                    unreachable!("verified condition")
                };
                if !condition {
                    frame.pc = *target;
                }
            }
            Op::MakeClosure(function) => {
                let count = module.functions[*function].captures.len();
                let captures = stack.split_off(stack.len() - count);
                let handle = heap
                    .allocate(Closure {
                        function: *function,
                        captures,
                    })
                    .map_err(|message| error("LARP-R006", message.into()))?;
                stack.push(Value::Closure(handle));
            }
            Op::Call(_) | Op::CallIndirect(_) => {
                if frames.len() >= limits.call_frames {
                    return Err(error("LARP-R006", "Call-frame limit exceeded".into()));
                }
                let count = match &instruction.op {
                    Op::Call(target) => module.functions[*target].parameters.len(),
                    Op::CallIndirect(larp_types::Type::Function(signature)) => {
                        signature.parameters.len()
                    }
                    _ => unreachable!("verified call signature"),
                };
                let arguments = stack.split_off(stack.len() - count);
                let (target, captures) = if let Op::Call(target) = instruction.op {
                    (target, Vec::new())
                } else {
                    let Value::Closure(handle) = stack.pop().unwrap() else {
                        unreachable!("verified closure")
                    };
                    let closure = heap.get(handle).expect("rooted live closure handle");
                    (closure.function, closure.captures.clone())
                };
                let callee = &module.functions[target];
                let mut locals = vec![None; callee.locals.len()];
                for (slot, value) in locals.iter_mut().zip(arguments.into_iter().chain(captures)) {
                    *slot = Some(value);
                }
                frames.push(Frame {
                    function: target,
                    pc: 0,
                    locals,
                    stack_base: stack.len(),
                });
            }
            Op::Intrinsic(operation) => {
                let count = operation.signature().unwrap().parameters.len();
                let arguments = stack.split_off(stack.len() - count);
                stack.push(
                    value::intrinsic(operation, arguments)
                        .map_err(|(code, message)| error(code, message))?,
                );
            }
            Op::MatchResult { success, error, .. } => {
                let Value::Result(result) = stack.pop().unwrap() else {
                    unreachable!("verified result")
                };
                match result {
                    Ok(value) => {
                        stack.push(*value);
                        frame.pc = *success;
                    }
                    Err(value) => {
                        stack.push(*value);
                        frame.pc = *error;
                    }
                }
            }
            Op::Return => {
                let value = stack.pop().expect("verified return value");
                stack.truncate(frame.stack_base);
                frames.pop();
                if !frames.is_empty() {
                    stack.push(value);
                }
            }
            Op::Print => {
                let Value::Scalar(value) = stack.pop().unwrap() else {
                    unreachable!("verified printable scalar")
                };
                writeln!(output, "{value}")
                    .map_err(|failure| error("LARP-R007", format!("Output failed: {failure}")))?;
            }
            Op::Assert => {
                if !matches!(stack.pop(), Some(Value::Scalar(Scalar::Verdict(true)))) {
                    return Err(error("LARP-R003", "no_cap assertion failed".into()));
                }
            }
            Op::Crash => {
                let Value::Scalar(Scalar::Text(message)) = stack.pop().unwrap() else {
                    unreachable!("verified crash message")
                };
                return Err(error("LARP-R003", message));
            }
        }
    }
    heap.collect(std::iter::empty());
    Ok(ExecutionStats {
        instructions: limits.instructions - fuel,
        allocations: heap.allocated,
        collections: heap.collections,
        reclaimed: heap.reclaimed,
        peak_live_objects: heap.peak_live,
    })
}

fn arithmetic(error: ArithmeticError, span: Span) -> Box<Diagnostic> {
    let (code, message) = match error {
        ArithmeticError::Overflow => ("LARP-R001", "Integer arithmetic overflow"),
        ArithmeticError::DivisionByZero => ("LARP-R002", "Integer division or remainder by zero"),
        ArithmeticError::InvalidTypes => unreachable!("verified operation types"),
    };
    Box::new(Diagnostic::error(code, span, message))
}
