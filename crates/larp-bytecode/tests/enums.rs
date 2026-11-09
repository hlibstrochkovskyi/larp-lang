use larp_bytecode::{Function, Instruction, Module, Op, verify};
use larp_source::{SourceFile, SourceId};
use larp_types::{Effects, EnumType, EnumVariant, Intrinsic, Scalar, Type};
use std::sync::Arc;

fn layout(payload: Vec<Type>) -> Arc<EnumType> {
    Arc::new(EnumType {
        id: 0,
        name: Arc::from("Packet"),
        variants: vec![
            EnumVariant {
                name: Arc::from("Empty"),
                payload: vec![],
            },
            EnumVariant {
                name: Arc::from("Data"),
                payload,
            },
        ],
    })
}

fn module(descriptor: Arc<EnumType>, locals: Vec<Type>, code: Vec<Op>) -> Module {
    let source = SourceFile::new(SourceId(0), "enum-bytecode", "");
    let span = source.span(0..0).unwrap();
    Module {
        enums: vec![descriptor],
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

fn unit() -> Op {
    Op::Constant(Scalar::Unit)
}

#[test]
fn all_enum_branches_receive_exact_ordered_payloads() {
    let descriptor = layout(vec![Type::Text, Type::Mf]);
    let ty = Type::Enum(descriptor.clone());
    let program = module(
        descriptor,
        vec![Type::Mf, Type::Text],
        vec![
            Op::Constant(Scalar::Text("aura".into())),
            Op::Constant(Scalar::Mf(49)),
            Op::MakeEnum {
                ty: ty.clone(),
                variant: 1,
            },
            Op::MatchSum {
                ty,
                targets: vec![4, 6],
            },
            unit(),
            Op::Return,
            Op::Store(0),
            Op::Store(1),
            unit(),
            Op::Return,
        ],
    );
    verify(program).unwrap();
}

#[test]
fn every_nested_enum_descriptor_must_match_its_canonical_nominal_definition() {
    let canonical = layout(vec![Type::array(Type::Mf)]);
    let forged = Type::Enum(layout(vec![Type::Mf]));
    for ty in [
        forged.clone(),
        Type::option(forged.clone()),
        Type::result(Type::Mf, forged.clone()),
        Type::array(forged.clone()),
        Type::fixed_array(forged.clone(), 1),
        Type::function(vec![], forged.clone(), Effects::NONE),
        Type::lazy(forged, Effects::NONE),
    ] {
        let error = verify(module(
            canonical.clone(),
            vec![ty],
            vec![unit(), Op::Return],
        ))
        .unwrap_err();
        assert!(
            error.message.contains("enum identity or descriptor"),
            "{error:?}"
        );
    }
}

#[test]
fn unused_and_unreachable_enum_descriptors_and_tags_are_still_checked() {
    let descriptor = layout(vec![Type::Mf]);
    let ty = Type::Enum(descriptor.clone());
    let bad_ops = [
        Op::MakeEnum {
            ty: ty.clone(),
            variant: 2,
        },
        Op::MakeEnum {
            ty: Type::Mf,
            variant: 0,
        },
        Op::MatchSum {
            ty: Type::Mf,
            targets: vec![0],
        },
        Op::MatchSum {
            ty: ty.clone(),
            targets: vec![0],
        },
        Op::MatchSum {
            ty,
            targets: vec![0, 99],
        },
    ];
    for op in bad_ops {
        assert!(
            verify(module(
                descriptor.clone(),
                vec![],
                vec![unit(), Op::Return, op]
            ))
            .is_err()
        );
    }
    let bad = Arc::new(EnumType {
        id: 1,
        name: Arc::from("Bad"),
        variants: descriptor.variants.clone(),
    });
    assert!(verify(module(bad, vec![], vec![unit(), Op::Return])).is_err());
}

#[test]
fn malformed_unused_enum_layouts_are_rejected_before_dataflow() {
    for variants in [
        vec![],
        vec![EnumVariant {
            name: Arc::from("Bad"),
            payload: vec![Type::Unit],
        }],
        vec![
            EnumVariant {
                name: Arc::from("Same"),
                payload: vec![]
            };
            2
        ],
        vec![EnumVariant {
            name: Arc::from("Wide"),
            payload: vec![Type::Mf; 65],
        }],
    ] {
        let descriptor = Arc::new(EnumType {
            id: 0,
            name: Arc::from("Invalid"),
            variants,
        });
        assert!(verify(module(descriptor, vec![], vec![unit(), Op::Return])).is_err());
    }
}

#[test]
fn construction_and_dispatch_cannot_forge_or_erase_payload_stack_types() {
    let descriptor = layout(vec![Type::Mf]);
    let ty = Type::Enum(descriptor.clone());
    for code in [
        vec![
            Op::MakeEnum {
                ty: ty.clone(),
                variant: 1,
            },
            Op::Return,
        ],
        vec![
            Op::Constant(Scalar::Text("wrong".into())),
            Op::MakeEnum {
                ty: ty.clone(),
                variant: 1,
            },
            Op::Return,
        ],
        vec![
            Op::Constant(Scalar::Mf(49)),
            Op::MakeEnum {
                ty: ty.clone(),
                variant: 1,
            },
            Op::MatchSum {
                ty,
                targets: vec![3, 3],
            },
            unit(),
            Op::Return,
        ],
    ] {
        assert!(verify(module(descriptor.clone(), vec![], code)).is_err());
    }
    let result = Type::result(Type::Mf, Type::Text);
    let code = vec![
        Op::Constant(Scalar::Mf(49)),
        Op::Intrinsic(Intrinsic::Ok(result.clone())),
        Op::MatchSum {
            ty: result,
            targets: vec![3, 3],
        },
        Op::Store(0),
        unit(),
        Op::Return,
    ];
    assert!(verify(module(descriptor, vec![Type::Mf], code)).is_err());
}
