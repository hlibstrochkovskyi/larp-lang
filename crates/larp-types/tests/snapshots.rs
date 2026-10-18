use larp_types::{Effects, Type};

#[test]
fn snapshots_are_closed_under_value_containers_but_never_reference_containers() {
    for ty in [
        Type::Mf,
        Type::Unc,
        Type::Geeked,
        Type::Verdict,
        Type::Text,
        Type::ConversionError,
    ] {
        assert!(ty.is_snapshot());
        assert!(Type::option(ty.clone()).is_snapshot());
        assert!(Type::result(ty.clone(), Type::fixed_array(ty, 2)).is_snapshot());
    }
    for ty in [
        Type::Unit,
        Type::class(0, "Founder"),
        Type::role(0, "Scored"),
        Type::array(Type::Mf),
        Type::function(vec![Type::Mf], Type::Mf, Effects::NONE),
    ] {
        assert!(!ty.is_snapshot());
        assert!(!Type::option(ty.clone()).is_snapshot());
        assert!(!Type::fixed_array(ty.clone(), 0).is_snapshot());
        assert!(!Type::result(Type::Mf, ty).is_snapshot());
    }
}

#[test]
fn snapshot_checks_reject_malformed_or_excessively_complex_value_shapes() {
    assert!(!Type::fixed_array(Type::Mf, usize::MAX).is_snapshot());
    let mut dag = Type::Mf;
    for _ in 0..30 {
        dag = Type::result(dag.clone(), dag.clone());
    }
    assert!(!dag.is_snapshot());
    let mut deep = Type::Text;
    for _ in 0..129 {
        deep = Type::option(deep);
    }
    assert!(!deep.is_snapshot());
}
