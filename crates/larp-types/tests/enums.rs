use larp_types::{Effects, EnumType, EnumVariant, MAX_ENUM_PAYLOAD, MAX_ENUM_VARIANTS, Type};
use std::sync::Arc;

fn enumeration(id: usize, payloads: Vec<Vec<Type>>) -> Type {
    Type::Enum(Arc::new(EnumType {
        id,
        name: "Funding".into(),
        variants: payloads
            .into_iter()
            .enumerate()
            .map(|(index, payload)| EnumVariant {
                name: format!("V{index}").into(),
                payload,
            })
            .collect(),
    }))
}

#[test]
fn nominal_identity_and_alternative_order_are_preserved() {
    let payload = vec![vec![], vec![Type::Text, Type::Unc]];
    let ty = enumeration(0, payload.clone());
    assert_eq!(ty.alternatives(), Some(payload));
    assert_eq!(ty.to_string(), "Funding");
    assert_ne!(
        ty,
        enumeration(1, vec![vec![], vec![Type::Text, Type::Unc]])
    );
    assert_eq!(Type::Verdict.alternatives(), Some(vec![vec![], vec![]]));
    assert_eq!(
        Type::option(Type::Mf).alternatives(),
        Some(vec![vec![], vec![Type::Mf]])
    );
    assert_eq!(
        Type::result(Type::Mf, Type::Text).alternatives(),
        Some(vec![vec![Type::Mf], vec![Type::Text]])
    );
    assert!(Type::Mf.alternatives().is_none());
}

#[test]
fn every_variant_controls_snapshot_and_equality_eligibility() {
    let value = enumeration(
        0,
        vec![
            vec![],
            vec![Type::option(Type::Text), Type::fixed_array(Type::Mf, 2)],
        ],
    );
    assert!(value.is_snapshot());
    assert!(value.supports_equality());
    assert!(Type::result(Type::Mf, value.clone()).is_snapshot());
    assert!(Type::lazy(value, Effects::NONE).is_well_formed());
    for payload in [
        Type::class(0, "Founder"),
        Type::role(0, "Scored"),
        Type::array(Type::Mf),
        Type::function(vec![], Type::Mf, Effects::NONE),
    ] {
        let ty = enumeration(0, vec![vec![], vec![payload.clone()]]);
        assert!(ty.is_well_formed());
        assert!(!ty.is_snapshot());
        assert_eq!(
            ty.supports_equality(),
            matches!(payload, Type::Class(..) | Type::Role(..))
        );
        assert!(!Type::lazy(ty, Effects::NONE).is_well_formed());
    }
}

#[test]
fn malformed_layouts_and_internal_payloads_are_rejected() {
    for ty in [
        enumeration(0, vec![]),
        enumeration(0, vec![vec![Type::Unit]]),
        enumeration(0, vec![vec![Type::lazy(Type::Mf, Effects::NONE)]]),
        Type::Enum(Arc::new(EnumType {
            id: 0,
            name: "".into(),
            variants: vec![EnumVariant {
                name: "A".into(),
                payload: vec![],
            }],
        })),
        Type::Enum(Arc::new(EnumType {
            id: 0,
            name: "E".into(),
            variants: vec![EnumVariant {
                name: "".into(),
                payload: vec![],
            }],
        })),
        Type::Enum(Arc::new(EnumType {
            id: 0,
            name: "E".into(),
            variants: vec![
                EnumVariant {
                    name: "A".into(),
                    payload: vec![],
                },
                EnumVariant {
                    name: "A".into(),
                    payload: vec![],
                },
            ],
        })),
    ] {
        assert!(!ty.is_well_formed());
        assert!(!ty.is_snapshot());
        assert!(!ty.supports_equality());
        assert!(ty.alternatives().is_none());
    }
}

#[test]
fn layout_arity_depth_and_shared_expansion_have_separate_bounds() {
    assert!(enumeration(0, vec![vec![]; MAX_ENUM_VARIANTS]).is_well_formed());
    assert!(enumeration(0, vec![vec![Type::Mf; MAX_ENUM_PAYLOAD]]).is_well_formed());
    for ty in [
        enumeration(0, vec![vec![]; MAX_ENUM_VARIANTS + 1]),
        enumeration(0, vec![vec![Type::Mf; MAX_ENUM_PAYLOAD + 1]]),
        enumeration(0, vec![vec![Type::Mf; MAX_ENUM_PAYLOAD]; MAX_ENUM_VARIANTS]),
    ] {
        assert!(!ty.within_limits());
        assert!(!ty.is_well_formed());
    }
    let mut ty = Type::Mf;
    for id in 0..129 {
        ty = enumeration(id, vec![vec![ty]]);
    }
    assert!(!ty.within_limits());
    let mut shared = Type::Mf;
    for id in 0..20 {
        shared = enumeration(id, vec![vec![shared.clone(), shared]]);
    }
    assert!(!shared.within_limits());
}
