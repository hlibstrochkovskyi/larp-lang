use larp_bytecode::{Function, Instruction, Module, Op, verify};
use larp_source::{SourceFile, SourceId};
use larp_types::{Effects, Intrinsic, Scalar, Type};
use larp_vm::{Limits, execute_with_stats};

fn function(
    parameters: Vec<Type>,
    captures: Vec<Type>,
    temporaries: Vec<Type>,
    result: Type,
    effects: Effects,
    code: Vec<Op>,
) -> Function {
    let source = SourceFile::new(SourceId(0), "arrays.larp", "");
    let span = source.span(0..0).unwrap();
    let locals = parameters
        .iter()
        .chain(&captures)
        .chain(&temporaries)
        .cloned()
        .collect();
    Function {
        owner: None,
        constructor: None,
        name: "test".into(),
        parameters,
        captures,
        result,
        effects,
        locals,
        code: code
            .into_iter()
            .map(|op| Instruction { op, span })
            .collect(),
        span,
    }
}

fn entry(locals: Vec<Type>, code: Vec<Op>) -> Function {
    function(vec![], vec![], locals, Type::Unit, Effects::ALL, code)
}

fn mf(value: i32) -> Op {
    Op::Constant(Scalar::Mf(value))
}
fn intrinsic(operation: Intrinsic) -> Op {
    Op::Intrinsic(operation)
}
fn unit() -> Op {
    Op::Constant(Scalar::Unit)
}

fn run(functions: Vec<Function>, expected: &str) {
    let module = verify(Module {
        roles: vec![],
        extensions: vec![],
        classes: vec![],
        functions,
        entry: 0,
    })
    .unwrap();
    for gc_stress in [false, true] {
        let mut output = Vec::new();
        let stats = execute_with_stats(
            &module,
            &mut output,
            Limits {
                gc_stress,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), expected);
        assert_eq!(stats.allocations, stats.reclaimed);
    }
}

#[test]
fn map_output_closures_remain_rooted_across_callback_allocations() {
    let array = Type::array(Type::Mf);
    let produced = Type::function(vec![], Type::Mf, Effects::NONE);
    let callback = Type::function(vec![Type::Mf], produced.clone(), Effects::NONE);
    let output = Type::array(produced.clone());
    run(
        vec![
            entry(
                vec![output.clone()],
                vec![
                    mf(11),
                    mf(22),
                    mf(33),
                    intrinsic(Intrinsic::ArrayNew {
                        ty: array.clone(),
                        count: 3,
                    }),
                    Op::MakeClosure(1),
                    intrinsic(Intrinsic::Map { array, callback }),
                    Op::Store(0),
                    Op::Load(0),
                    mf(0),
                    intrinsic(Intrinsic::Index(output.clone())),
                    Op::CallIndirect(produced.clone()),
                    Op::Print,
                    Op::Load(0),
                    mf(1),
                    intrinsic(Intrinsic::Index(output.clone())),
                    Op::CallIndirect(produced.clone()),
                    Op::Print,
                    Op::Load(0),
                    mf(2),
                    intrinsic(Intrinsic::Index(output)),
                    Op::CallIndirect(produced.clone()),
                    Op::Print,
                    unit(),
                    Op::Return,
                ],
            ),
            function(
                vec![Type::Mf],
                vec![],
                vec![],
                produced,
                Effects::NONE,
                vec![Op::Load(0), Op::MakeClosure(2), Op::Return],
            ),
            function(
                vec![],
                vec![Type::Mf],
                vec![],
                Type::Mf,
                Effects::NONE,
                vec![Op::Load(0), Op::Return],
            ),
        ],
        "11\n22\n33\n",
    );
}

#[test]
fn array_identity_and_fixed_value_copying_are_distinct() {
    let dynamic = Type::array(Type::Mf);
    let fixed = Type::fixed_array(dynamic.clone(), 1);
    run(
        vec![entry(
            vec![
                dynamic.clone(),
                dynamic.clone(),
                fixed.clone(),
                fixed.clone(),
                Type::Unit,
            ],
            vec![
                mf(1),
                intrinsic(Intrinsic::ArrayNew {
                    ty: dynamic.clone(),
                    count: 1,
                }),
                Op::Store(0),
                Op::Load(0),
                Op::Store(1),
                Op::Load(0),
                intrinsic(Intrinsic::ArrayNew {
                    ty: fixed.clone(),
                    count: 1,
                }),
                Op::Store(2),
                Op::Load(2),
                Op::Store(3),
                Op::Load(1),
                mf(2),
                intrinsic(Intrinsic::Push(dynamic.clone())),
                Op::Store(4),
                Op::Load(3),
                mf(0),
                intrinsic(Intrinsic::Index(fixed.clone())),
                intrinsic(Intrinsic::Size(dynamic.clone())),
                Op::Print,
                Op::Load(2),
                mf(0),
                intrinsic(Intrinsic::ArrayNew {
                    ty: dynamic.clone(),
                    count: 0,
                }),
                intrinsic(Intrinsic::SetIndex(fixed.clone())),
                Op::Store(2),
                Op::Load(2),
                mf(0),
                intrinsic(Intrinsic::Index(fixed.clone())),
                intrinsic(Intrinsic::Size(dynamic.clone())),
                Op::Print,
                Op::Load(3),
                mf(0),
                intrinsic(Intrinsic::Index(fixed)),
                intrinsic(Intrinsic::Size(dynamic)),
                Op::Print,
                unit(),
                Op::Return,
            ],
        )],
        "2\n0\n2\n",
    );
}

#[test]
fn nested_iteration_on_same_array_is_allowed_and_releases_every_guard() {
    let array = Type::array(Type::Mf);
    let nested_callback = Type::function(vec![Type::Mf], Type::Unit, Effects::NONE);
    let callback = Type::function(vec![Type::Mf], Type::Mf, Effects::READ);
    run(
        vec![
            entry(
                vec![array.clone(), array.clone(), Type::Unit],
                vec![
                    mf(3),
                    mf(4),
                    intrinsic(Intrinsic::ArrayNew {
                        ty: array.clone(),
                        count: 2,
                    }),
                    Op::Store(0),
                    Op::Load(0),
                    Op::Load(0),
                    Op::MakeClosure(1),
                    intrinsic(Intrinsic::Map {
                        array: array.clone(),
                        callback,
                    }),
                    Op::Store(1),
                    Op::Load(0),
                    mf(5),
                    intrinsic(Intrinsic::Push(array.clone())),
                    Op::Store(2),
                    Op::Load(0),
                    intrinsic(Intrinsic::Size(array.clone())),
                    Op::Print,
                    unit(),
                    Op::Return,
                ],
            ),
            function(
                vec![Type::Mf],
                vec![array.clone()],
                vec![Type::Unit],
                Type::Mf,
                Effects::READ,
                vec![
                    Op::Load(1),
                    Op::MakeClosure(2),
                    intrinsic(Intrinsic::ForEach {
                        array,
                        callback: nested_callback,
                    }),
                    Op::Store(2),
                    Op::Load(0),
                    Op::Return,
                ],
            ),
            function(
                vec![Type::Mf],
                vec![],
                vec![],
                Type::Unit,
                Effects::NONE,
                vec![unit(), Op::Return],
            ),
        ],
        "3\n",
    );
}

#[test]
fn aliased_push_is_rejected_while_callback_is_running() {
    let array = Type::array(Type::Mf);
    let effects = Effects::WRITE.union(Effects::ABORT);
    let callback = Type::function(vec![Type::Mf], Type::Unit, effects);
    let module = verify(Module {
        roles: vec![],
        extensions: vec![],
        classes: vec![],
        entry: 0,
        functions: vec![
            entry(
                vec![array.clone()],
                vec![
                    mf(7),
                    intrinsic(Intrinsic::ArrayNew {
                        ty: array.clone(),
                        count: 1,
                    }),
                    Op::Store(0),
                    Op::Load(0),
                    Op::Load(0),
                    Op::MakeClosure(1),
                    intrinsic(Intrinsic::ForEach {
                        array: array.clone(),
                        callback,
                    }),
                    Op::Return,
                ],
            ),
            function(
                vec![Type::Mf],
                vec![array.clone()],
                vec![],
                Type::Unit,
                effects,
                vec![
                    Op::Load(1),
                    mf(8),
                    intrinsic(Intrinsic::Push(array)),
                    Op::Return,
                ],
            ),
        ],
    })
    .unwrap();
    for gc_stress in [false, true] {
        let error = execute_with_stats(
            &module,
            &mut Vec::new(),
            Limits {
                gc_stress,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.code.id, "LARP-R005");
    }
}

#[test]
fn nested_collection_callbacks_obey_frame_limits_without_host_recursion() {
    let array = Type::array(Type::Mf);
    let callback = Type::function(vec![Type::Mf], Type::Unit, Effects::READ);
    let module = verify(Module {
        roles: vec![],
        extensions: vec![],
        classes: vec![],
        entry: 0,
        functions: vec![
            entry(
                vec![],
                vec![
                    mf(1),
                    intrinsic(Intrinsic::ArrayNew {
                        ty: array.clone(),
                        count: 1,
                    }),
                    Op::Call(1),
                    Op::Return,
                ],
            ),
            function(
                vec![array.clone()],
                vec![],
                vec![],
                Type::Unit,
                Effects::READ,
                vec![
                    Op::Load(0),
                    Op::Load(0),
                    Op::MakeClosure(2),
                    intrinsic(Intrinsic::ForEach {
                        array: array.clone(),
                        callback,
                    }),
                    Op::Return,
                ],
            ),
            function(
                vec![Type::Mf],
                vec![array],
                vec![],
                Type::Unit,
                Effects::READ,
                vec![Op::Load(1), Op::Call(1), Op::Return],
            ),
        ],
    })
    .unwrap();
    let error = execute_with_stats(
        &module,
        &mut Vec::new(),
        Limits {
            call_frames: 4096,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.code.id, "LARP-R006");
    assert!(error.message.contains("Call-frame"));
}

#[test]
fn negative_and_end_indices_abort_for_fixed_and_dynamic_arrays() {
    for array in [Type::array(Type::Mf), Type::fixed_array(Type::Mf, 1)] {
        for index in [-1, 1, i32::MAX] {
            let module = verify(Module {
                roles: vec![],
                extensions: vec![],
                classes: vec![],
                entry: 0,
                functions: vec![entry(
                    vec![Type::Mf],
                    vec![
                        mf(7),
                        intrinsic(Intrinsic::ArrayNew {
                            ty: array.clone(),
                            count: 1,
                        }),
                        mf(index),
                        intrinsic(Intrinsic::Index(array.clone())),
                        Op::Store(0),
                        unit(),
                        Op::Return,
                    ],
                )],
            })
            .unwrap();
            let error =
                execute_with_stats(&module, &mut Vec::new(), Limits::default()).unwrap_err();
            assert_eq!(error.code.id, "LARP-R004");
        }
    }
}

#[test]
fn element_replacement_is_allowed_and_later_callbacks_observe_it() {
    let array = Type::array(Type::Mf);
    let effects = Effects::OBSERVE.union(Effects::WRITE).union(Effects::ABORT);
    let callback = Type::function(vec![Type::Mf], Type::Unit, effects);
    run(
        vec![
            entry(
                vec![array.clone()],
                vec![
                    mf(1),
                    mf(2),
                    mf(3),
                    intrinsic(Intrinsic::ArrayNew {
                        ty: array.clone(),
                        count: 3,
                    }),
                    Op::Store(0),
                    Op::Load(0),
                    Op::Load(0),
                    Op::MakeClosure(1),
                    intrinsic(Intrinsic::ForEach {
                        array: array.clone(),
                        callback,
                    }),
                    Op::Return,
                ],
            ),
            function(
                vec![Type::Mf],
                vec![array.clone()],
                vec![],
                Type::Unit,
                effects,
                vec![
                    Op::Load(0),
                    Op::Print,
                    Op::Load(1),
                    mf(1),
                    mf(99),
                    intrinsic(Intrinsic::SetIndex(array)),
                    Op::Return,
                ],
            ),
        ],
        "1\n99\n3\n",
    );
}

#[test]
fn checked_assignment_place_aborts_before_later_side_effects() {
    for array in [Type::array(Type::Mf), Type::fixed_array(Type::Mf, 1)] {
        for index in [-1, 1, i32::MAX] {
            let module = verify(Module {
                roles: vec![],
                extensions: vec![],
                classes: vec![],
                entry: 0,
                functions: vec![entry(
                    vec![Type::Unit],
                    vec![
                        mf(7),
                        intrinsic(Intrinsic::ArrayNew {
                            ty: array.clone(),
                            count: 1,
                        }),
                        mf(index),
                        intrinsic(Intrinsic::CheckIndex(array.clone())),
                        Op::Store(0),
                        Op::Constant(Scalar::Text("RHS evaluated".into())),
                        Op::Print,
                        unit(),
                        Op::Return,
                    ],
                )],
            })
            .unwrap();
            let mut output = Vec::new();
            let error = execute_with_stats(&module, &mut output, Limits::default()).unwrap_err();
            assert_eq!(error.code.id, "LARP-R004");
            assert!(output.is_empty());
        }
    }
}
