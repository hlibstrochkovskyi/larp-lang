use larp_types::{Effects, Type};

#[test]
fn lazy_storage_is_well_formed_only_for_snapshot_results_and_pure_abort_bounds() {
    for result in [
        Type::Mf,
        Type::Text,
        Type::option(Type::Unc),
        Type::fixed_array(Type::Verdict, 2),
        Type::result(Type::Mf, Type::ConversionError),
    ] {
        for effects in [Effects::NONE, Effects::ABORT] {
            let cell = Type::lazy(result.clone(), effects);
            assert!(cell.is_well_formed());
            assert!(!cell.is_snapshot());
            assert!(!cell.supports_equality());
            assert!(!Type::option(cell.clone()).is_well_formed());
            assert!(!Type::function(vec![cell.clone()], Type::Mf, Effects::NONE).is_well_formed());
            assert!(!Type::function(vec![], cell, Effects::NONE).is_well_formed());
        }
    }
    for result in [
        Type::Unit,
        Type::array(Type::Mf),
        Type::class(0, "Root"),
        Type::role(0, "Role"),
        Type::function(vec![], Type::Mf, Effects::NONE),
    ] {
        assert!(!Type::lazy(result, Effects::NONE).is_well_formed());
    }
    for effects in [Effects::READ, Effects::WRITE, Effects::OBSERVE, Effects::IO] {
        assert!(!Type::lazy(Type::Mf, effects).is_well_formed());
    }
    assert!(!Type::lazy(Type::Mf, Effects::NONE).accepts(&Type::lazy(Type::Mf, Effects::ABORT)));
}
