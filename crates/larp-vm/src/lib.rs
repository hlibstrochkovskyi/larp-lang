//! Iterative execution of verified bytecode with checked values and a traced heap.

mod heap;
mod host;
mod scenario;
mod value;
pub use host::{Host, MAX_ARGUMENT_BYTES, MAX_ARGUMENTS, MAX_INPUT_BYTES, MAX_LINE_BYTES};

use heap::{Array, Closure, Handle, Heap, Instance, LazyCell, LazyState};
use larp_bytecode::{Op, VerifiedModule};
use larp_diagnostics::Diagnostic;
use larp_source::Span;
use larp_types::{ArithmeticError, Binary, Intrinsic, Scalar, Type};
use scenario::Scenario;
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
    return_to_lazy: Option<Handle>,
    silence_entry: usize,
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

fn collect(heap: &mut Heap, stack: &[Value], frames: &[Frame], scenario: Option<&Scenario>) {
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
    let forcing: Vec<_> = frames
        .iter()
        .filter_map(|frame| frame.return_to_lazy)
        .map(Value::Lazy)
        .collect();
    roots.extend(&forcing);
    let root = scenario.map(|scenario| Value::Object(scenario.root));
    roots.extend(root.iter());
    if let Some(scenario) = scenario {
        roots.extend(&scenario.baseline);
        roots.extend(&scenario.performed);
        roots.extend(scenario.claims.iter().filter_map(Option::as_ref));
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
    execute_with_observers(module, output, &mut std::io::sink(), limits)
}

/// Execute with independent ordinary-output and state-inspection sinks.
pub fn execute_with_observers(
    module: &VerifiedModule,
    output: &mut dyn Write,
    inspection: &mut dyn Write,
    limits: Limits,
) -> Result<ExecutionStats, Box<Diagnostic>> {
    execute_with_host(module, output, inspection, &mut Host::default(), limits)
}

/// Execute with explicitly granted argument/stdin capabilities; legacy APIs deny them.
pub fn execute_with_host(
    module: &VerifiedModule,
    output: &mut dyn Write,
    inspection: &mut dyn Write,
    host: &mut Host<'_>,
    limits: Limits,
) -> Result<ExecutionStats, Box<Diagnostic>> {
    execute_host_runtime(
        module,
        output,
        inspection,
        host,
        limits,
        &mut Heap::default(),
    )
}

#[cfg(test)]
fn execute_runtime(
    module: &VerifiedModule,
    output: &mut dyn Write,
    inspection: &mut dyn Write,
    limits: Limits,
    heap: &mut Heap,
) -> Result<ExecutionStats, Box<Diagnostic>> {
    execute_host_runtime(
        module,
        output,
        inspection,
        &mut Host::default(),
        limits,
        heap,
    )
}

fn execute_host_runtime(
    module: &VerifiedModule,
    output: &mut dyn Write,
    inspection: &mut dyn Write,
    host: &mut Host<'_>,
    limits: Limits,
    heap: &mut Heap,
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
        return_to_lazy: None,
        silence_entry: 0,
    }];
    let mut stack: Vec<Value> = Vec::new();
    let mut scenario: Option<Scenario> = None;
    let mut fuel = limits.instructions;
    let mut silence_depth = 0usize;
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
                    return_to_lazy: None,
                    silence_entry: silence_depth,
                });
            } else {
                // The unfinished output is still an explicit root at this safepoint.
                if heap.should_collect(limits.gc_stress) {
                    collect(heap, &stack, &frames, scenario.as_ref());
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
            Op::MakeClosure(_)
                | Op::MakeLazy(_)
                | Op::NewObject(_)
                | Op::Intrinsic(
                    Intrinsic::ArrayNew { .. } | Intrinsic::Sorted(_) | Intrinsic::Arguments
                )
        ) && heap.should_collect(limits.gc_stress)
        {
            collect(heap, &stack, &frames, scenario.as_ref());
        }
        let frame = frames.last_mut().unwrap();
        frame.pc += 1;
        match &instruction.op {
            Op::SilenceEnter => {
                silence_depth = silence_depth.checked_add(1).ok_or_else(|| {
                    error(
                        "LARP-R006",
                        "Observation suppression depth exhausted".into(),
                    )
                })?;
            }
            Op::SilenceExit => {
                silence_depth = silence_depth
                    .checked_sub(1)
                    .expect("verified observation suppression depth");
            }
            Op::MakeLazy(target) => {
                let count = module.functions[*target].captures.len();
                let captures = stack.split_off(stack.len() - count);
                let handle = heap
                    .allocate(LazyCell {
                        function: *target,
                        captures,
                        state: LazyState::Pending,
                    })
                    .map_err(|message| error("LARP-R006", message.into()))?;
                stack.push(Value::Lazy(handle));
            }
            Op::Force(_) => {
                let Value::Lazy(handle) = stack.pop().unwrap() else {
                    unreachable!("verified lazy cell")
                };
                match force_lazy(heap, handle, frames.len() < limits.call_frames, span)? {
                    Forced::Ready(value) => stack.push(value),
                    Forced::Thunk { function, captures } => {
                        let callee = &module.functions[function];
                        let mut locals = vec![None; callee.locals.len()];
                        for (slot, value) in locals.iter_mut().zip(captures) {
                            *slot = Some(value);
                        }
                        frames.push(Frame {
                            function,
                            pc: 0,
                            locals,
                            stack_base: stack.len(),
                            iteration: None,
                            return_to_lazy: Some(handle),
                            silence_entry: silence_depth,
                        });
                    }
                }
            }

            Op::Scenario { entry, label } => {
                if frames.len() >= limits.call_frames {
                    return Err(error("LARP-R006", "Call-frame limit exceeded".into()));
                }
                let callee = &module.functions[*entry];
                let mut arguments = stack.split_off(stack.len() - callee.parameters.len());
                let Value::Object(root) = arguments[0] else {
                    unreachable!("verified ordinary scenario root")
                };
                debug_assert!(scenario.is_none());
                let instance = heap.instance(root).expect("rooted live scenario root");
                scenario = Some(Scenario::new(
                    *entry,
                    instance.class,
                    label.clone(),
                    root,
                    instance.fields.clone(),
                ));
                arguments[0] = Value::View { entry: *entry };
                let mut locals = vec![None; callee.locals.len()];
                for (slot, value) in locals.iter_mut().zip(arguments) {
                    *slot = Some(value);
                }
                frames.push(Frame {
                    function: *entry,
                    pc: 0,
                    locals,
                    stack_base: stack.len(),
                    iteration: None,
                    return_to_lazy: None,
                    silence_entry: silence_depth,
                });
            }
            Op::Canon { class, field } => {
                let receiver = stack.pop().unwrap();
                let state = scenario.as_mut().expect("verified active scenario");
                state.check_receiver(receiver, *class);
                state.selected[*field] = true;
                stack.push(Value::Scalar(Scalar::Unit));
            }
            Op::Flex { class, field } => {
                let value = stack.pop().unwrap();
                let receiver = stack.pop().unwrap();
                let state = scenario.as_mut().expect("verified active scenario");
                state.check_receiver(receiver, *class);
                state.claims[*field] = Some(value.clone());
                state.performed[*field] = value;
                stack.push(Value::Scalar(Scalar::Unit));
            }
            Op::Palantir { class, fields } => {
                let receiver = stack.pop().unwrap();
                let state = scenario.as_ref().expect("verified active scenario");
                state.check_receiver(receiver, *class);
                state
                    .inspect(&module.classes[*class], fields, inspection)
                    .map_err(|failure| {
                        error("LARP-R007", format!("Inspection output failed: {failure}"))
                    })?;
                stack.push(Value::Scalar(Scalar::Unit));
            }

            Op::IntoRole { role, .. } => {
                let Value::Object(object) = stack.pop().unwrap() else {
                    unreachable!("verified class-to-role coercion")
                };
                stack.push(Value::Role {
                    object,
                    role: *role,
                });
            }
            Op::CallRole { role, method } => {
                if frames.len() >= limits.call_frames {
                    return Err(error("LARP-R006", "Call-frame limit exceeded".into()));
                }
                let count = module.roles[*role].methods[*method].parameters.len();
                let arguments = stack.split_off(stack.len() - count);
                let Value::Role {
                    object,
                    role: actual,
                } = stack.pop().unwrap()
                else {
                    unreachable!("verified role receiver")
                };
                debug_assert_eq!(*role, actual);
                let instance = heap.instance(object).expect("rooted live role receiver");
                let implementation = module.classes[instance.class]
                    .implementations
                    .iter()
                    .find(|implementation| implementation.role == *role)
                    .expect("verified role implementation");
                let target = implementation.methods[*method];
                let mut locals = vec![None; module.functions[target].locals.len()];
                for (slot, value) in locals
                    .iter_mut()
                    .zip(std::iter::once(Value::Object(object)).chain(arguments))
                {
                    *slot = Some(value);
                }
                frames.push(Frame {
                    function: target,
                    pc: 0,
                    locals,
                    stack_base: stack.len(),
                    iteration: None,
                    return_to_lazy: None,
                    silence_entry: silence_depth,
                });
            }
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
            Op::NewObject(class) => {
                let count = module.classes[*class].fields.len();
                let fields = stack.split_off(stack.len() - count);
                let handle = heap
                    .allocate(Instance {
                        class: *class,
                        fields,
                    })
                    .map_err(|message| error("LARP-R006", message.into()))?;
                stack.truncate(frame.stack_base);
                silence_depth = frame.silence_entry;
                frames.pop();
                stack.push(Value::Object(handle));
            }
            Op::GetField { class, field } => {
                let receiver = stack.pop().unwrap();
                let value = match receiver {
                    Value::Object(handle) => {
                        let instance = heap.instance(handle).expect("rooted live object");
                        debug_assert_eq!(instance.class, *class);
                        instance.fields[*field].clone()
                    }
                    Value::View { .. } => {
                        let state = scenario.as_ref().expect("verified active scenario");
                        state.check_receiver(receiver, *class);
                        state.performed[*field].clone()
                    }
                    _ => unreachable!("verified object receiver"),
                };
                stack.push(value);
            }
            Op::SetField { class, field } => {
                let value = stack.pop().unwrap();
                let receiver = stack.pop().unwrap();
                match receiver {
                    Value::Object(handle) => {
                        let instance = heap.instance_mut(handle).expect("rooted live object");
                        debug_assert_eq!(instance.class, *class);
                        instance.fields[*field] = value;
                    }
                    Value::View { .. } => {
                        let state = scenario.as_mut().expect("verified active scenario");
                        state.check_receiver(receiver, *class);
                        state.performed[*field] = value;
                    }
                    _ => unreachable!("verified object receiver"),
                }
                stack.push(Value::Scalar(Scalar::Unit));
            }
            Op::Call(_) | Op::CallIndirect(_) | Op::Construct(_) => {
                if frames.len() >= limits.call_frames {
                    return Err(error("LARP-R006", "Call-frame limit exceeded".into()));
                }
                let count = match &instruction.op {
                    Op::Call(target) => module.functions[*target].parameters.len(),
                    Op::Construct(class) => module.functions[module.classes[*class].constructor]
                        .parameters
                        .len(),
                    Op::CallIndirect(larp_types::Type::Function(signature)) => {
                        signature.parameters.len()
                    }
                    _ => unreachable!("verified call signature"),
                };
                let arguments = stack.split_off(stack.len() - count);
                let (target, captures) = match instruction.op {
                    Op::Call(target) => (target, Vec::new()),
                    Op::Construct(class) => (module.classes[class].constructor, Vec::new()),
                    _ => {
                        let Value::Closure(handle) = stack.pop().unwrap() else {
                            unreachable!("verified closure")
                        };
                        let closure = heap.get(handle).expect("rooted live closure handle");
                        (closure.function, closure.captures.clone())
                    }
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
                    return_to_lazy: None,
                    silence_entry: silence_depth,
                });
            }
            Op::Intrinsic(operation) => {
                if matches!(operation, Intrinsic::Arguments | Intrinsic::ReadLine) {
                    stack.push(
                        host_intrinsic(operation, host, heap, scenario.is_some())
                            .map_err(|(code, message)| error(code, message))?,
                    );
                    continue;
                }
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
                        heap_intrinsic(operation, arguments, heap)
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
                if let Some(handle) = frame.return_to_lazy {
                    let cell = heap.lazy_mut(handle).expect("rooted evaluating lazy cell");
                    cell.state = LazyState::Ready(value.clone());
                    cell.captures.clear();
                }
                if scenario
                    .as_ref()
                    .is_some_and(|state| state.entry == frame.function)
                {
                    // Prepare the complete state before one publication with no
                    // intervening allocation, observation, callbacks, or GC.
                    scenario.take().unwrap().commit(heap);
                }
                stack.truncate(frame.stack_base);
                silence_depth = frame.silence_entry;
                frames.pop();
                if !frames.is_empty() {
                    stack.push(value);
                }
            }
            Op::Print => {
                let Value::Scalar(value) = stack.pop().unwrap() else {
                    unreachable!("verified printable scalar")
                };
                if silence_depth == 0 {
                    writeln!(output, "{value}").map_err(|failure| {
                        error("LARP-R007", format!("Output failed: {failure}"))
                    })?;
                }
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

enum Forced {
    Ready(Value),
    Thunk {
        function: usize,
        captures: Vec<Value>,
    },
}

fn force_lazy(
    heap: &mut Heap,
    handle: Handle,
    can_enter: bool,
    span: Span,
) -> Result<Forced, Box<Diagnostic>> {
    let cell = heap.lazy_mut(handle).expect("rooted live lazy cell");
    match &cell.state {
        LazyState::Ready(value) => Ok(Forced::Ready(value.clone())),
        LazyState::Evaluating => Err(trap(
            "LARP-R009",
            span,
            "Lazy initializer is already being evaluated",
        )),
        LazyState::Pending => {
            if !can_enter {
                return Err(trap("LARP-R006", span, "Call-frame limit exceeded"));
            }
            cell.state = LazyState::Evaluating;
            Ok(Forced::Thunk {
                function: cell.function,
                captures: cell.captures.clone(),
            })
        }
    }
}

fn host_intrinsic(
    operation: &Intrinsic,
    host: &mut Host<'_>,
    heap: &mut Heap,
    scenario_active: bool,
) -> Result<Value, (&'static str, String)> {
    if scenario_active {
        return Err((
            "LARP-R010",
            "Host input is unavailable in an active scenario".into(),
        ));
    }
    match operation {
        Intrinsic::Arguments => {
            let arguments = host.arguments().map_err(host::Failure::diagnostic)?;
            let values = arguments
                .iter()
                .map(|value| Value::Scalar(Scalar::Text(value.clone())))
                .collect();
            let handle = heap
                .allocate(Array {
                    values,
                    iterations: 0,
                })
                .map_err(|message| ("LARP-R006", message.into()))?;
            Ok(Value::Array(handle))
        }
        Intrinsic::ReadLine => Ok(Value::Result(
            match host.read_line().map_err(host::Failure::diagnostic)? {
                Ok(line) => Ok(Box::new(Value::Option(
                    line.map(|text| Box::new(Value::Scalar(Scalar::Text(text)))),
                ))),
                Err(message) => Err(Box::new(Value::Scalar(Scalar::Text(message.into())))),
            },
        )),
        _ => unreachable!("host input intrinsic"),
    }
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

#[cfg(test)]
mod scenario_tests;

#[cfg(test)]
mod lazy_tests;
