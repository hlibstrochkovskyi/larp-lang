use larp_bytecode::{Function, Instruction, Module, Op, verify};
use larp_source::{SourceFile, SourceId};
use larp_types::{Effects, EnumType, EnumVariant, Intrinsic, Scalar, Type};
use larp_vm::{Limits, execute_with_stats};
use std::sync::Arc;

fn entry(enums: Vec<Arc<EnumType>>, locals: Vec<Type>, code: Vec<Op>) -> Module {
    let source = SourceFile::new(SourceId(0), "enum-vm", "");
    let span = source.span(0..0).unwrap();
    Module {
        enums,
        roles: vec![],
        extensions: vec![],
        classes: vec![],
        entry: 0,
        functions: vec![Function {
            scenario: None,
            owner: None,
            constructor: None,
            name: "entry".into(),
            parameters: vec![],
            captures: vec![],
            result: Type::Unit,
            effects: Effects::ALL,
            locals,
            code: code
                .into_iter()
                .map(|op| Instruction { op, span })
                .collect(),
            span,
        }],
    }
}

fn run(module: Module, expected: &str) {
    let module = verify(module).unwrap();
    for gc_stress in [false, true] {
        let mut output = vec![];
        let stats = execute_with_stats(
            &module,
            &mut output,
            Limits {
                gc_stress,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(output, expected.as_bytes());
        assert_eq!(stats.allocations, stats.reclaimed);
    }
}
fn unit() -> Op {
    Op::Constant(Scalar::Unit)
}
fn mf(value: i32) -> Op {
    Op::Constant(Scalar::Mf(value))
}

#[test]
fn enum_payloads_arrive_in_order_and_only_selected_branch_executes() {
    let descriptor = Arc::new(EnumType {
        id: 0,
        name: Arc::from("Packet"),
        variants: vec![
            EnumVariant {
                name: Arc::from("Empty"),
                payload: vec![],
            },
            EnumVariant {
                name: Arc::from("Data"),
                payload: vec![Type::Text, Type::Mf],
            },
        ],
    });
    let ty = Type::Enum(descriptor.clone());
    run(
        entry(
            vec![descriptor],
            vec![Type::Mf, Type::Text],
            vec![
                Op::Constant(Scalar::Text("aura".into())),
                mf(49),
                Op::MakeEnum {
                    ty: ty.clone(),
                    variant: 1,
                },
                Op::MatchSum {
                    ty,
                    targets: vec![4, 7],
                },
                Op::Constant(Scalar::Text("empty".into())),
                Op::Print,
                Op::Jump(13),
                Op::Store(0),
                Op::Store(1),
                Op::Load(1),
                Op::Print,
                Op::Load(0),
                Op::Print,
                unit(),
                Op::Return,
            ],
        ),
        "aura\n49\n",
    );
}

#[test]
fn managed_enum_payload_remains_rooted_across_other_array_allocations() {
    let array = Type::array(Type::Mf);
    let descriptor = Arc::new(EnumType {
        id: 0,
        name: Arc::from("Box"),
        variants: vec![EnumVariant {
            name: Arc::from("Items"),
            payload: vec![array.clone()],
        }],
    });
    let ty = Type::Enum(descriptor.clone());
    let mut code = vec![
        mf(49),
        Op::Intrinsic(Intrinsic::ArrayNew {
            ty: array.clone(),
            count: 1,
        }),
        Op::MakeEnum {
            ty: ty.clone(),
            variant: 0,
        },
        Op::Store(0),
    ];
    for _ in 0..32 {
        code.extend([
            mf(20),
            Op::Intrinsic(Intrinsic::ArrayNew {
                ty: array.clone(),
                count: 1,
            }),
            Op::Store(1),
        ]);
    }
    code.push(Op::Load(0));
    let target = code.len() + 1;
    code.push(Op::MatchSum {
        ty: ty.clone(),
        targets: vec![target],
    });
    code.extend([
        mf(0),
        Op::Intrinsic(Intrinsic::Index(array.clone())),
        Op::Print,
        unit(),
        Op::Return,
    ]);
    run(entry(vec![descriptor], vec![ty, array], code), "49\n");
}

#[test]
fn builtin_sum_dispatch_preserves_none_some_win_fumble_and_verdict_order() {
    for value in [false, true] {
        run(
            entry(
                vec![],
                vec![],
                vec![
                    Op::Constant(Scalar::Verdict(value)),
                    Op::MatchSum {
                        ty: Type::Verdict,
                        targets: vec![2, 5],
                    },
                    Op::Constant(Scalar::Text("cap".into())),
                    Op::Print,
                    Op::Jump(7),
                    Op::Constant(Scalar::Text("fr".into())),
                    Op::Print,
                    unit(),
                    Op::Return,
                ],
            ),
            if value { "fr\n" } else { "cap\n" },
        );
    }
    let option = Type::option(Type::Mf);
    for present in [false, true] {
        let mut code = if present {
            vec![mf(49), Op::Intrinsic(Intrinsic::Some(option.clone()))]
        } else {
            vec![Op::Intrinsic(Intrinsic::None(option.clone()))]
        };
        let none = code.len() + 1;
        let some = none + 3;
        let done = some + 2;
        code.extend([
            Op::MatchSum {
                ty: option.clone(),
                targets: vec![none, some],
            },
            Op::Constant(Scalar::Text("none".into())),
            Op::Print,
            Op::Jump(done),
            Op::Print,
            Op::Jump(done),
            unit(),
            Op::Return,
        ]);
        run(
            entry(vec![], vec![], code),
            if present { "49\n" } else { "none\n" },
        );
    }
    let result = Type::result(Type::Mf, Type::Text);
    for success in [false, true] {
        let mut code = if success {
            vec![mf(49), Op::Intrinsic(Intrinsic::Ok(result.clone()))]
        } else {
            vec![
                Op::Constant(Scalar::Text("no".into())),
                Op::Intrinsic(Intrinsic::Err(result.clone())),
            ]
        };
        let win = code.len() + 1;
        let error = win + 2;
        let done = error + 2;
        code.extend([
            Op::MatchSum {
                ty: result.clone(),
                targets: vec![win, error],
            },
            Op::Print,
            Op::Jump(done),
            Op::Print,
            Op::Jump(done),
            unit(),
            Op::Return,
        ]);
        run(
            entry(vec![], vec![], code),
            if success { "49\n" } else { "no\n" },
        );
    }
}
