//! Iterative execution of verified bytecode with checked values and a traced heap.

mod heap;
mod value;

use heap::{Array, Closure, Heap};
use larp_bytecode::{Op, VerifiedModule};
use larp_diagnostics::Diagnostic;
use larp_source::Span;
use larp_types::{ArithmeticError, Binary, Intrinsic, Scalar, Type};
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
    iteration: Option<Iteration>,
}

#[derive(Clone, Copy)]
enum IterationKind {
    Map,
    Filter,
    ForEach,
}

struct Iteration {
    kind: IterationKind,
    receiver: Value,
    callback: Value,
    current: Option<Value>,
    output: Vec<Value>,
    index: usize,
    waiting: bool,
    span: Span,
}

fn collect(heap: &mut Heap, stack: &[Value], frames: &[Frame]) {
    let mut roots: Vec<&Value> = stack.iter().collect();
    for frame in frames {
        roots.extend(frame.locals.iter().filter_map(Option::as_ref));
        if let Some(iteration) = &frame.iteration {
            roots.push(&iteration.receiver);
            roots.push(&iteration.callback);
            roots.extend(iteration.current.iter());
            roots.extend(&iteration.output);
        }
    }
    heap.collect(roots.into_iter());
}

fn trap(code: &str, span: Span, message: impl Into<String>) -> Box<Diagnostic> {
    Box::new(Diagnostic::error(code, span, message))
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
        iteration: None,
    }];
    let mut stack: Vec<Value> = Vec::new();
    let mut heap = Heap::default();
    let mut fuel = limits.instructions;
    while !frames.is_empty() {
        // A suspended collection resumes only after its callback frame returns.
        // This keeps arbitrarily nested higher-order calls off the Rust stack.
        if frames.last().unwrap().iteration.is_some() {
            let iteration = frames.last_mut().unwrap().iteration.as_mut().unwrap();
            let span = iteration.span;
            let Value::Array(receiver) = iteration.receiver else {
                unreachable!("verified array receiver")
            };
            if iteration.waiting {
                let returned = stack.pop().expect("collection callback return");
                match iteration.kind {
                    IterationKind::Map => iteration.output.push(returned),
                    IterationKind::Filter => {
                        if matches!(returned, Value::Scalar(Scalar::Verdict(true))) {
                            iteration.output.push(iteration.current.take().unwrap());
                        }
                    }
                    IterationKind::ForEach => {}
                }
                iteration.current = None;
                iteration.waiting = false;
            }
            let array = heap.array(receiver).expect("rooted live array");
            if iteration.index < array.values.len() {
                if frames.len() >= limits.call_frames {
                    return Err(trap("LARP-R006", span, "Call-frame limit exceeded"));
                }
                let iteration = frames.last_mut().unwrap().iteration.as_mut().unwrap();
                let argument = array.values[iteration.index].clone();
                iteration.index += 1;
                if matches!(iteration.kind, IterationKind::Filter) {
                    iteration.current = Some(argument.clone());
                }
                iteration.waiting = true;
                let Value::Closure(callback) = iteration.callback else {
                    unreachable!("verified callback")
                };
                let closure = heap.get(callback).expect("rooted live callback");
                let callee = &module.functions[closure.function];
                let mut locals = vec![None; callee.locals.len()];
                for (slot, value) in locals
                    .iter_mut()
                    .zip(std::iter::once(argument).chain(closure.captures.clone()))
                {
                    *slot = Some(value);
                }
                frames.push(Frame {
                    function: closure.function,
                    pc: 0,
                    locals,
                    stack_base: stack.len(),
                    iteration: None,
                });
            } else {
                // The unfinished output is still an explicit root at this safepoint.
                if heap.should_collect(limits.gc_stress) {
                    collect(&mut heap, &stack, &frames);
                }
                let iteration = frames.last_mut().unwrap().iteration.take().unwrap();
                heap.array_mut(receiver)
                    .expect("rooted live array")
                    .iterations -= 1;
                let result = match iteration.kind {
                    IterationKind::Map | IterationKind::Filter => {
                        let handle = heap
                            .allocate(Array {
                                values: iteration.output,
                                iterations: 0,
                            })
                            .map_err(|message| trap("LARP-R006", span, message))?;
                        Value::Array(handle)
                    }
                    IterationKind::ForEach => Value::Scalar(Scalar::Unit),
                };
                stack.push(result);
            }
            continue;
        }
        let frame = frames.last().unwrap();
        let instruction = &module.functions[frame.function].code[frame.pc];
        let span = instruction.span;
        let error = |code: &str, message: String| Box::new(Diagnostic::error(code, span, message));
        if fuel == 0 {
            return Err(error("LARP-R006", "Instruction budget exhausted".into()));
        }
        fuel -= 1;
        // All source operands remain on the stack at allocation safepoints.
        if matches!(
            instruction.op,
            Op::MakeClosure(_) | Op::Intrinsic(Intrinsic::ArrayNew { .. } | Intrinsic::Sorted(_))
        ) && heap.should_collect(limits.gc_stress)
        {
            collect(&mut heap, &stack, &frames);
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
                    iteration: None,
                });
            }
            Op::Intrinsic(operation) => {
                let count = operation.signature().unwrap().parameters.len();
                let mut arguments = stack.split_off(stack.len() - count);
                if matches!(
                    operation,
                    Intrinsic::Map { .. } | Intrinsic::Filter { .. } | Intrinsic::ForEach { .. }
                ) {
                    let callback = arguments.pop().unwrap();
                    let receiver = arguments.pop().unwrap();
                    let Value::Array(handle) = receiver else {
                        unreachable!("verified array receiver")
                    };
                    let array = heap.array_mut(handle).expect("rooted live array");
                    array.iterations = array
                        .iterations
                        .checked_add(1)
                        .ok_or_else(|| error("LARP-R006", "Iteration nesting exhausted".into()))?;
                    frame.iteration = Some(Iteration {
                        kind: match operation {
                            Intrinsic::Map { .. } => IterationKind::Map,
                            Intrinsic::Filter { .. } => IterationKind::Filter,
                            _ => IterationKind::ForEach,
                        },
                        receiver,
                        callback,
                        current: None,
                        output: Vec::new(),
                        index: 0,
                        waiting: false,
                        span,
                    });
                } else {
                    stack.push(
                        heap_intrinsic(operation, arguments, &mut heap)
                            .map_err(|(code, message)| error(code, message))?,
                    );
                }
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

fn heap_intrinsic(
    operation: &Intrinsic,
    mut arguments: Vec<Value>,
    heap: &mut Heap,
) -> Result<Value, (&'static str, String)> {
    let result = match operation {
        Intrinsic::ArrayNew { ty, .. } => {
            check_length(arguments.len())?;
            if matches!(ty, Type::FixedArray(_, _)) {
                Value::FixedArray(arguments)
            } else {
                Value::Array(
                    heap.allocate(Array {
                        values: arguments,
                        iterations: 0,
                    })
                    .map_err(|message| ("LARP-R006", message.into()))?,
                )
            }
        }
        Intrinsic::Size(_) => {
            let count = match &arguments[0] {
                Value::Array(handle) => {
                    heap.array(*handle).expect("rooted live array").values.len()
                }
                Value::FixedArray(values) => values.len(),
                Value::Scalar(Scalar::Text(text)) => text.chars().count(),
                _ => unreachable!("verified size receiver"),
            };
            Value::Scalar(Scalar::Mf(check_length(count)?))
        }
        Intrinsic::CheckIndex(_) => {
            let index = arguments.pop().unwrap();
            let length = match &arguments[0] {
                Value::Array(handle) => {
                    heap.array(*handle).expect("rooted live array").values.len()
                }
                Value::FixedArray(values) => values.len(),
                _ => unreachable!("verified index receiver"),
            };
            index_checked(index, length)?;
            Value::Scalar(Scalar::Unit)
        }
        Intrinsic::Index(_) => {
            let index = arguments.pop().unwrap();
            let values = match &arguments[0] {
                Value::Array(handle) => &heap.array(*handle).expect("rooted live array").values,
                Value::FixedArray(values) => values,
                _ => unreachable!("verified index receiver"),
            };
            values[index_checked(index, values.len())?].clone()
        }
        Intrinsic::SetIndex(_) => {
            let value = arguments.pop().unwrap();
            let index = arguments.pop().unwrap();
            match arguments.pop().unwrap() {
                Value::Array(handle) => {
                    let array = heap.array_mut(handle).expect("rooted live array");
                    let index = index_checked(index, array.values.len())?;
                    array.values[index] = value;
                    Value::Scalar(Scalar::Unit)
                }
                Value::FixedArray(mut values) => {
                    let index = index_checked(index, values.len())?;
                    values[index] = value;
                    Value::FixedArray(values)
                }
                _ => unreachable!("verified index receiver"),
            }
        }
        Intrinsic::Push(_) => {
            let value = arguments.pop().unwrap();
            let Value::Array(handle) = arguments.pop().unwrap() else {
                unreachable!("verified push receiver")
            };
            let array = heap.array_mut(handle).expect("rooted live array");
            if array.iterations != 0 {
                return Err((
                    "LARP-R005",
                    "Cannot structurally mutate an array during callback iteration".into(),
                ));
            }
            check_length(
                array
                    .values
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| ("LARP-R004", "Array length exceeds mf range".into()))?,
            )?;
            array.values.push(value);
            Value::Scalar(Scalar::Unit)
        }
        Intrinsic::Sorted(_) => {
            let Value::Array(handle) = arguments.pop().unwrap() else {
                unreachable!("verified sort receiver")
            };
            let mut values = heap
                .array(handle)
                .expect("rooted live array")
                .values
                .clone();
            values.sort_by(|a, b| match (a, b) {
                (Value::Scalar(Scalar::Mf(a)), Value::Scalar(Scalar::Mf(b))) => a.cmp(b),
                (Value::Scalar(Scalar::Unc(a)), Value::Scalar(Scalar::Unc(b))) => a.cmp(b),
                (Value::Scalar(Scalar::Text(a)), Value::Scalar(Scalar::Text(b))) => a.cmp(b),
                _ => unreachable!("verified sortable element"),
            });
            Value::Array(
                heap.allocate(Array {
                    values,
                    iterations: 0,
                })
                .map_err(|message| ("LARP-R006", message.into()))?,
            )
        }
        _ => return value::intrinsic(operation, arguments),
    };
    Ok(result)
}

fn check_length(length: usize) -> Result<i32, (&'static str, String)> {
    i32::try_from(length).map_err(|_| ("LARP-R004", "Collection length exceeds mf range".into()))
}

fn index_checked(index: Value, length: usize) -> Result<usize, (&'static str, String)> {
    let Value::Scalar(Scalar::Mf(index)) = index else {
        unreachable!("verified mf index")
    };
    usize::try_from(index)
        .ok()
        .filter(|index| *index < length)
        .ok_or_else(|| {
            (
                "LARP-R004",
                format!("Array index {index} is outside length {length}"),
            )
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
