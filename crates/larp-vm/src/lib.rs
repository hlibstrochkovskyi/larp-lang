//! Iterative execution of verified bytecode with checked scalar operations.

use larp_bytecode::{Op, VerifiedModule};
use larp_diagnostics::Diagnostic;
use larp_source::Span;
use larp_types::{ArithmeticError, Scalar};
use std::io::Write;

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub instructions: u64,
    pub call_frames: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            instructions: 1_000_000,
            call_frames: 1024,
        }
    }
}

struct Frame {
    function: usize,
    pc: usize,
    locals: Vec<Option<Scalar>>,
    stack_base: usize,
}

pub fn execute(
    module: &VerifiedModule,
    output: &mut impl Write,
    limits: Limits,
) -> Result<(), Box<Diagnostic>> {
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
    let mut stack: Vec<Scalar> = Vec::new();
    let mut fuel = limits.instructions;
    while let Some(frame) = frames.last_mut() {
        let function = &module.functions[frame.function];
        let instruction = &function.code[frame.pc];
        let span = instruction.span;
        let error = |code: &str, message: String| Box::new(Diagnostic::error(code, span, message));
        if fuel == 0 {
            return Err(error("LARP-R006", "Instruction budget exhausted".into()));
        }
        fuel -= 1;
        frame.pc += 1;
        match &instruction.op {
            Op::Constant(value) => stack.push(value.clone()),
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
                let operand = stack.pop().expect("verified operand stack");
                stack.push(
                    larp_types::unary(*op, &operand).map_err(|error| arithmetic(error, span))?,
                );
            }
            Op::Binary(op, _) => {
                let right = stack.pop().expect("verified operand stack");
                let left = stack.pop().expect("verified operand stack");
                stack.push(
                    larp_types::binary(*op, &left, &right)
                        .map_err(|error| arithmetic(error, span))?,
                );
            }
            Op::Jump(target) => frame.pc = *target,
            Op::JumpIfFalse(target) => {
                let Scalar::Verdict(condition) = stack.pop().expect("verified operand stack")
                else {
                    unreachable!("verified condition type")
                };
                if !condition {
                    frame.pc = *target;
                }
            }
            Op::Call(target) => {
                if frames.len() >= limits.call_frames {
                    return Err(error("LARP-R006", "Call-frame limit exceeded".into()));
                }
                let callee = &module.functions[*target];
                let mut locals = vec![None; callee.locals.len()];
                for local in locals[..callee.parameters.len()].iter_mut().rev() {
                    *local = Some(stack.pop().expect("verified arguments"));
                }
                frames.push(Frame {
                    function: *target,
                    pc: 0,
                    locals,
                    stack_base: stack.len(),
                });
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
                let value = stack.pop().expect("verified print value");
                writeln!(output, "{value}")
                    .map_err(|failure| error("LARP-R007", format!("Output failed: {failure}")))?;
            }
            Op::Assert => {
                if stack.pop() != Some(Scalar::Verdict(true)) {
                    return Err(error("LARP-R003", "no_cap assertion failed".into()));
                }
            }
            Op::Crash => {
                let value = stack.pop().expect("verified crash message");
                return Err(error("LARP-R003", value.to_string()));
            }
        }
    }
    Ok(())
}

fn arithmetic(error: ArithmeticError, span: Span) -> Box<Diagnostic> {
    let (code, message) = match error {
        ArithmeticError::Overflow => ("LARP-R001", "Integer arithmetic overflow"),
        ArithmeticError::DivisionByZero => ("LARP-R002", "Integer division or remainder by zero"),
        ArithmeticError::InvalidTypes => unreachable!("verified operation types"),
    };
    Box::new(Diagnostic::error(code, span, message))
}
