use super::*;
use larp_bytecode::{Function, Instruction, Module, verify};
use larp_source::{SourceFile, SourceId};
use larp_types::Effects;

fn span() -> Span {
    SourceFile::new(SourceId(0), "lazy.larp", "force lazy")
        .span(0..5)
        .unwrap()
}
fn function(locals: Vec<Type>, result: Type, effects: Effects, code: Vec<Op>) -> Function {
    Function {
        scenario: None,
        owner: None,
        constructor: None,
        name: "test".into(),
        parameters: vec![],
        captures: vec![],
        result,
        effects,
        locals,
        code: code
            .into_iter()
            .map(|op| Instruction { op, span: span() })
            .collect(),
        span: span(),
    }
}
fn module(main: Function, helpers: Vec<Function>) -> VerifiedModule {
    verify(Module {
        entry: 0,
        roles: vec![],
        extensions: vec![],
        classes: vec![],
        functions: std::iter::once(main).chain(helpers).collect(),
    })
    .unwrap()
}
fn mf(value: i32) -> Op {
    Op::Constant(Scalar::Mf(value))
}
fn text(value: &str) -> Op {
    Op::Constant(Scalar::Text(value.into()))
}
fn unit() -> Op {
    Op::Constant(Scalar::Unit)
}
fn cell(effects: Effects) -> Type {
    Type::lazy(Type::Mf, effects)
}

#[test]
fn repeated_forcing_runs_the_initializer_once_with_an_exact_instruction_budget() {
    let mut thunk = function(
        vec![Type::Mf],
        Type::Mf,
        Effects::NONE,
        vec![Op::Load(0), Op::Return],
    );
    thunk.captures = vec![Type::Mf];
    let compiled = module(
        function(
            vec![cell(Effects::NONE)],
            Type::Unit,
            Effects::ALL,
            vec![
                mf(7),
                Op::MakeLazy(1),
                Op::Store(0),
                Op::Load(0),
                Op::Force(cell(Effects::NONE)),
                Op::Print,
                Op::Load(0),
                Op::Force(cell(Effects::NONE)),
                Op::Print,
                unit(),
                Op::Return,
            ],
        ),
        vec![thunk],
    );
    for gc_stress in [false, true] {
        let mut output = vec![];
        let stats = execute_with_stats(
            &compiled,
            &mut output,
            Limits {
                instructions: 13,
                gc_stress,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(output, b"7\n7\n");
        assert_eq!(stats.instructions, 13);
        assert_eq!(stats.allocations, 1);
        assert_eq!(stats.reclaimed, 1);
    }
}

#[test]
fn an_unused_aborting_initializer_is_never_entered() {
    let compiled = module(
        function(
            vec![cell(Effects::ABORT)],
            Type::Unit,
            Effects::ALL,
            vec![
                Op::MakeLazy(1),
                Op::Store(0),
                text("unused"),
                Op::Print,
                unit(),
                Op::Return,
            ],
        ),
        vec![function(
            vec![],
            Type::Mf,
            Effects::ABORT,
            vec![text("must not run"), Op::Crash],
        )],
    );
    let mut output = vec![];
    let stats = execute_with_stats(
        &compiled,
        &mut output,
        Limits {
            gc_stress: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(output, b"unused\n");
    assert_eq!(stats.instructions, 6);
    assert_eq!(stats.allocations, stats.reclaimed);
}

#[test]
fn a_temporary_forcing_cell_is_rooted_while_its_initializer_allocates() {
    let signature = Type::function(vec![], Type::Mf, Effects::NONE);
    let compiled = module(
        function(
            vec![],
            Type::Unit,
            Effects::ALL,
            vec![
                Op::MakeLazy(1),
                Op::Force(cell(Effects::NONE)),
                Op::Print,
                unit(),
                Op::Return,
            ],
        ),
        vec![
            function(
                vec![signature],
                Type::Mf,
                Effects::NONE,
                vec![Op::MakeClosure(2), Op::Store(0), mf(9), Op::Return],
            ),
            function(vec![], Type::Mf, Effects::NONE, vec![mf(0), Op::Return]),
        ],
    );
    let mut output = vec![];
    let stats = execute_with_stats(
        &compiled,
        &mut output,
        Limits {
            gc_stress: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(output, b"9\n");
    assert_eq!(stats.allocations, 2);
    assert_eq!(stats.reclaimed, 2);
    assert!(stats.collections >= 3);
}

#[test]
fn forcing_state_rejects_reentry_and_checks_the_frame_limit_before_entering() {
    let mut heap = Heap::default();
    let handle = heap
        .allocate(LazyCell {
            function: 49,
            captures: vec![Value::Scalar(Scalar::Mf(7))],
            state: LazyState::Pending,
        })
        .unwrap();
    let failure = force_lazy(&mut heap, handle, false, span()).err().unwrap();
    assert_eq!(failure.code.id, "LARP-R006");
    assert!(matches!(
        heap.lazy(handle).unwrap().state,
        LazyState::Pending
    ));
    assert!(matches!(
        force_lazy(&mut heap, handle, true, span()).unwrap(),
        Forced::Thunk { function: 49, .. }
    ));
    let failure = force_lazy(&mut heap, handle, true, span()).err().unwrap();
    assert_eq!(failure.code.id, "LARP-R009");
    assert_eq!(failure.primary.span, span());
    heap.lazy_mut(handle).unwrap().state = LazyState::Ready(Value::Scalar(Scalar::Mf(49)));
    assert!(matches!(
        force_lazy(&mut heap, handle, false, span()).unwrap(),
        Forced::Ready(Value::Scalar(Scalar::Mf(49)))
    ));
}

#[test]
fn successful_forcing_discards_captured_inputs_and_keeps_the_cached_result() {
    let mut thunk = function(
        vec![Type::Text],
        Type::Mf,
        Effects::NONE,
        vec![mf(49), Op::Return],
    );
    thunk.captures = vec![Type::Text];
    let compiled = module(
        function(
            vec![cell(Effects::NONE), Type::Mf],
            Type::Unit,
            Effects::ALL,
            vec![
                text("captured input"),
                Op::MakeLazy(1),
                Op::Store(0),
                Op::Load(0),
                Op::Force(cell(Effects::NONE)),
                Op::Store(1),
                text("stop after caching"),
                Op::Crash,
            ],
        ),
        vec![thunk],
    );
    let mut heap = Heap::default();
    let failure = execute_runtime(
        &compiled,
        &mut vec![],
        &mut vec![],
        Limits {
            gc_stress: true,
            ..Default::default()
        },
        &mut heap,
    )
    .unwrap_err();
    assert_eq!(failure.code.id, "LARP-R003");
    // The private executor leaves the heap available after fatal termination.
    let mut found = false;
    for candidate in heap.lazy_cells() {
        assert!(candidate.captures.is_empty());
        assert!(matches!(
            candidate.state,
            LazyState::Ready(Value::Scalar(Scalar::Mf(49)))
        ));
        found = true;
    }
    assert!(found);
}

struct FailingOutput;
impl Write for FailingOutput {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("output unavailable"))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn nested_silence_applies_to_callees_and_restores_the_callers_dynamic_depth() {
    let compiled = module(
        function(
            vec![Type::Unit],
            Type::Unit,
            Effects::ALL,
            vec![
                Op::SilenceEnter,
                Op::Call(1),
                Op::Store(0),
                text("still hidden"),
                Op::Print,
                Op::SilenceExit,
                text("visible"),
                Op::Print,
                unit(),
                Op::Return,
            ],
        ),
        vec![function(
            vec![],
            Type::Unit,
            Effects::OBSERVE,
            vec![
                Op::SilenceEnter,
                text("inner"),
                Op::Print,
                Op::SilenceExit,
                text("callee"),
                Op::Print,
                unit(),
                Op::Return,
            ],
        )],
    );
    let mut output = vec![];
    execute(&compiled, &mut output, Limits::default()).unwrap();
    assert_eq!(output, b"visible\n");
}

#[test]
fn suppressed_print_does_not_write_to_the_sink_but_fatal_errors_are_not_suppressed() {
    let success = module(
        function(
            vec![],
            Type::Unit,
            Effects::ALL,
            vec![
                Op::SilenceEnter,
                text("hidden"),
                Op::Print,
                Op::SilenceExit,
                unit(),
                Op::Return,
            ],
        ),
        vec![],
    );
    execute(&success, &mut FailingOutput, Limits::default()).unwrap();
    let failure = module(
        function(
            vec![],
            Type::Unit,
            Effects::ALL,
            vec![
                Op::SilenceEnter,
                text("hidden"),
                Op::Print,
                text("fatal"),
                Op::Crash,
            ],
        ),
        vec![],
    );
    let error = execute(&failure, &mut FailingOutput, Limits::default()).unwrap_err();
    assert_eq!(error.code.id, "LARP-R003");
    assert_eq!(error.message, "fatal");
    // Suppression belongs to one execution; a later execution starts unmuted.
    let visible = module(
        function(
            vec![],
            Type::Unit,
            Effects::ALL,
            vec![text("visible"), Op::Print, unit(), Op::Return],
        ),
        vec![],
    );
    assert_eq!(
        execute(&visible, &mut FailingOutput, Limits::default())
            .unwrap_err()
            .code
            .id,
        "LARP-R007"
    );
}
