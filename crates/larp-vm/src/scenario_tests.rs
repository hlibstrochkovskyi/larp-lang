use super::*;
use larp_bytecode::{Class, Field, Function, Instruction, Module, ScenarioContext, verify};
use larp_source::{SourceFile, SourceId};
use larp_types::Effects;

fn root_type() -> Type {
    Type::class(0, "Account")
}
fn context() -> ScenarioContext {
    ScenarioContext {
        entry: 2,
        class: 0,
        origin: None,
    }
}
fn effects() -> Effects {
    Effects::READ
        .union(Effects::WRITE)
        .union(Effects::OBSERVE)
        .union(Effects::ABORT)
}
fn unit() -> Op {
    Op::Constant(Scalar::Unit)
}
fn mf(value: i32) -> Op {
    Op::Constant(Scalar::Mf(value))
}
fn text(value: &str) -> Op {
    Op::Constant(Scalar::Text(value.into()))
}
fn field(index: usize) -> Op {
    Op::GetField {
        class: 0,
        field: index,
    }
}
fn set(index: usize) -> Op {
    Op::SetField {
        class: 0,
        field: index,
    }
}
fn canon(index: usize) -> Op {
    Op::Canon {
        class: 0,
        field: index,
    }
}
fn flex(index: usize) -> Op {
    Op::Flex {
        class: 0,
        field: index,
    }
}
fn function(
    parameters: Vec<Type>,
    extra: Vec<Type>,
    result: Type,
    effects: Effects,
    code: Vec<Op>,
) -> Function {
    let source = SourceFile::new(SourceId(0), "scenarios.larp", "scenario execution");
    let span = source.span(0..source.text().len()).unwrap();
    Function {
        scenario: None,
        owner: None,
        constructor: None,
        name: "test".into(),
        locals: parameters.iter().chain(&extra).cloned().collect(),
        parameters,
        captures: vec![],
        result,
        effects,
        code: code
            .into_iter()
            .map(|op| Instruction { op, span })
            .collect(),
        span,
    }
}
fn code(function: &mut Function, code: Vec<Op>) {
    function.code = code
        .into_iter()
        .map(|op| Instruction {
            op,
            span: function.span,
        })
        .collect();
}
fn module(scenario_code: Vec<Op>) -> Module {
    let main = function(
        vec![],
        vec![root_type(), Type::Unit],
        Type::Unit,
        Effects::ALL,
        vec![
            mf(10),
            mf(20),
            mf(30),
            Op::Construct(0),
            Op::Store(0),
            Op::Load(0),
            Op::Scenario {
                entry: 2,
                label: "WhatIf".into(),
            },
            Op::Store(1),
            Op::Load(0),
            field(0),
            Op::Print,
            Op::Load(0),
            field(1),
            Op::Print,
            unit(),
            Op::Return,
        ],
    );
    let mut constructor = function(
        vec![Type::Mf, Type::Mf, Type::Mf],
        vec![],
        root_type(),
        Effects::READ.union(Effects::WRITE).union(Effects::ABORT),
        vec![Op::Load(0), Op::Load(1), Op::Load(2), Op::NewObject(0)],
    );
    constructor.owner = Some(0);
    constructor.constructor = Some(0);
    let mut scenario = function(
        vec![root_type()],
        vec![Type::Unit],
        Type::Unit,
        effects(),
        scenario_code,
    );
    scenario.scenario = Some(context());
    Module {
        roles: vec![],
        extensions: vec![],
        entry: 0,
        classes: vec![Class {
            implementations: vec![],
            name: "Account".into(),
            constructor: 1,
            methods: vec![],
            fields: vec![
                Field {
                    name: "a".into(),
                    ty: Type::Mf,
                    public: true,
                },
                Field {
                    name: "b".into(),
                    ty: Type::Mf,
                    public: true,
                },
                Field {
                    name: "hidden".into(),
                    ty: Type::Mf,
                    public: false,
                },
            ],
        }],
        functions: vec![main, constructor, scenario],
    }
}
fn changes() -> Vec<Op> {
    vec![
        Op::Load(0),
        mf(100),
        flex(0),
        Op::Store(1),
        Op::Load(0),
        canon(0),
        Op::Store(1),
        Op::Load(0),
        mf(200),
        set(1),
        Op::Store(1),
    ]
}
fn assert_baseline(heap: &Heap) {
    let instances: Vec<_> = heap.instances().collect();
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].class, 0);
    for (actual, expected) in instances[0].fields.iter().zip([10, 20, 30]) {
        assert!(actual.equals(&Value::Scalar(Scalar::Mf(expected))));
    }
}

#[test]
fn selected_final_fields_commit_and_claims_survive_ordinary_writes() {
    let module = verify(module(vec![
        Op::Load(0),
        canon(0),
        Op::Store(1),
        Op::Load(0),
        mf(100),
        flex(0),
        Op::Store(1),
        Op::Load(0),
        mf(120),
        set(0),
        Op::Store(1),
        Op::Load(0),
        canon(0),
        Op::Store(1),
        Op::Load(0),
        mf(200),
        flex(1),
        Op::Store(1),
        Op::Load(0),
        Op::Palantir {
            class: 0,
            fields: vec![0, 1],
        },
        Op::Store(1),
        unit(),
        Op::Return,
    ]))
    .unwrap();
    for gc_stress in [false, true] {
        let mut output = Vec::new();
        let mut inspection = Vec::new();
        let stats = execute_with_observers(
            &module,
            &mut output,
            &mut inspection,
            Limits {
                gc_stress,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(output, b"120\n20\n");
        assert_eq!(
            String::from_utf8(inspection).unwrap(),
            "PALANTIR [WhatIf]\nPROPERTY | REALITY | PERFORMED | LAST CLAIM | SELECTED\na | 10 | 120 | 100 | true\nb | 20 | 200 | 200 | false\n"
        );
        assert_eq!(stats.allocations, stats.reclaimed);
    }
}

#[test]
fn every_selected_field_is_published_from_the_complete_prepared_state() {
    let mut ops = changes();
    ops.extend([Op::Load(0), canon(1), Op::Store(1), unit(), Op::Return]);
    let module = verify(module(ops)).unwrap();
    let mut output = Vec::new();
    execute(
        &module,
        &mut output,
        Limits {
            gc_stress: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(output, b"100\n200\n");
}

#[test]
fn traps_discard_all_selected_changes_but_preserve_emitted_observations() {
    for (fault, expected) in [
        (vec![text("aborted"), Op::Crash], "LARP-R003"),
        (
            vec![
                Op::Constant(Scalar::Verdict(false)),
                Op::Assert,
                unit(),
                Op::Return,
            ],
            "LARP-R003",
        ),
        (
            vec![
                mf(i32::MAX),
                mf(1),
                Op::Binary(larp_types::Binary::Add, Type::Mf),
                Op::Store(2),
                unit(),
                Op::Return,
            ],
            "LARP-R001",
        ),
    ] {
        let mut ops = changes();
        ops.extend([
            text("before"),
            Op::Print,
            Op::Load(0),
            Op::Palantir {
                class: 0,
                fields: vec![0, 1],
            },
            Op::Store(1),
        ]);
        ops.extend(fault);
        let mut module = module(ops);
        module.functions[2].locals.push(Type::Mf);
        let module = verify(module).unwrap();
        let mut heap = Heap::default();
        let mut output = Vec::new();
        let mut inspection = Vec::new();
        let error = execute_runtime(
            &module,
            &mut output,
            &mut inspection,
            Limits {
                gc_stress: true,
                ..Default::default()
            },
            &mut heap,
        )
        .unwrap_err();
        assert_eq!(error.code.id, expected);
        assert_eq!(output, b"before\n");
        assert!(
            String::from_utf8(inspection)
                .unwrap()
                .contains("a | 10 | 100 | 100 | true")
        );
        assert_baseline(&heap);
    }
}

#[test]
fn fuel_exhaustion_discards_performed_state() {
    let mut ops = changes();
    let forever = ops.len();
    ops.push(Op::Jump(forever));
    let module = verify(module(ops)).unwrap();
    let mut heap = Heap::default();
    let error = execute_runtime(
        &module,
        &mut Vec::new(),
        &mut Vec::new(),
        Limits {
            instructions: 100,
            ..Default::default()
        },
        &mut heap,
    )
    .unwrap_err();
    assert_eq!(error.code.id, "LARP-R006");
    assert_baseline(&heap);
}

#[test]
fn inspection_failure_aborts_before_commit() {
    struct Fail;
    impl Write for Fail {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("inspection unavailable"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut ops = changes();
    ops.extend([
        Op::Load(0),
        Op::Palantir {
            class: 0,
            fields: vec![0, 1],
        },
        Op::Store(1),
        unit(),
        Op::Return,
    ]);
    let module = verify(module(ops)).unwrap();
    let mut heap = Heap::default();
    let error = execute_runtime(
        &module,
        &mut Vec::new(),
        &mut Fail,
        Limits::default(),
        &mut heap,
    )
    .unwrap_err();
    assert_eq!(error.code.id, "LARP-R007");
    assert_baseline(&heap);
}

#[test]
fn original_root_is_a_gc_root_even_when_only_a_view_remains_in_frames() {
    let callback = Type::function(vec![], Type::Mf, Effects::NONE);
    let mut module = module(vec![
        mf(49),
        Op::MakeClosure(3),
        Op::Store(2),
        Op::Load(0),
        mf(100),
        set(0),
        Op::Store(1),
        Op::Load(0),
        canon(0),
        Op::Store(1),
        unit(),
        Op::Return,
    ]);
    module.functions[2].locals.push(callback);
    let mut helper = function(
        vec![],
        vec![Type::Mf],
        Type::Mf,
        Effects::NONE,
        vec![Op::Load(0), Op::Return],
    );
    helper.captures = vec![Type::Mf];
    helper.scenario = Some(context());
    module.functions.push(helper);
    code(
        &mut module.functions[0],
        vec![
            mf(10),
            mf(20),
            mf(30),
            Op::Construct(0),
            Op::Scenario {
                entry: 2,
                label: "TemporaryRoot".into(),
            },
            Op::Store(1),
            unit(),
            Op::Return,
        ],
    );
    let module = verify(module).unwrap();
    let stats = execute_with_observers(
        &module,
        &mut Vec::new(),
        &mut Vec::new(),
        Limits {
            gc_stress: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(stats.allocations, 2);
    assert_eq!(stats.allocations, stats.reclaimed);
}

#[test]
fn specialized_calls_preserve_the_view_and_return_without_early_commit() {
    let mut module = module(vec![
        Op::Load(0),
        mf(100),
        Op::Call(4),
        Op::Store(1),
        Op::Load(0),
        field(0),
        Op::Print,
        Op::Load(0),
        canon(0),
        Op::Store(1),
        Op::Load(0),
        mf(120),
        set(0),
        Op::Store(1),
        unit(),
        Op::Return,
    ]);
    let ordinary = function(
        vec![root_type(), Type::Mf],
        vec![],
        Type::Unit,
        Effects::WRITE,
        vec![Op::Load(0), Op::Load(1), set(0), Op::Return],
    );
    let mut specialized = ordinary.clone();
    specialized.scenario = Some(ScenarioContext {
        origin: Some(3),
        ..context()
    });
    module.functions.extend([ordinary, specialized]);
    let module = verify(module).unwrap();
    let mut output = Vec::new();
    execute_with_observers(
        &module,
        &mut output,
        &mut Vec::new(),
        Limits {
            gc_stress: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(output, b"100\n120\n20\n");
}
