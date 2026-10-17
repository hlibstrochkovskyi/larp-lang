use larp_bytecode::{Function, Instruction, Module, Op, VerifyError, VerifyErrorKind, verify};
use larp_source::{SourceFile, SourceId};
use larp_types::{Effects, Intrinsic, Scalar, Type};

fn function(parameters: Vec<Type>, result: Type, effects: Effects, code: Vec<Op>) -> Function {
    let source = SourceFile::new(SourceId(0), "arrays.larp", "");
    let span = source.span(0..0).unwrap();
    Function {
        owner: None,
        constructor: None,
        name: "array_test".into(),
        locals: parameters.clone(),
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

fn unit() -> Function {
    function(
        vec![],
        Type::Unit,
        Effects::ALL,
        vec![Op::Constant(Scalar::Unit), Op::Return],
    )
}

fn verify_target(target: Function) -> Result<(), VerifyError> {
    verify(Module {
        classes: vec![],
        functions: vec![unit(), target],
        entry: 0,
    })
    .map(|_| ())
}

fn intrinsic_function(operation: Intrinsic, effects: Effects) -> Function {
    let signature = operation
        .signature()
        .expect("Expected a valid intrinsic signature");
    let mut code: Vec<_> = (0..signature.parameters.len()).map(Op::Load).collect();
    code.extend([Op::Intrinsic(operation), Op::Return]);
    function(signature.parameters, signature.result, effects, code)
}

fn reject(target: Function, message: &str) -> VerifyError {
    let error = verify_target(target).unwrap_err();
    assert!(error.message.contains(message), "{}", error.message);
    error
}

fn unreachable(operation: Intrinsic, effects: Effects) -> Function {
    let mut target = unit();
    target.effects = effects;
    target.code.push(Instruction {
        op: Op::Intrinsic(operation),
        span: target.span,
    });
    target
}

#[test]
fn array_intrinsics_have_exact_effect_bounds_even_when_unreachable() {
    let dynamic = Type::array(Type::Mf);
    let fixed = Type::fixed_array(Type::Mf, 2);
    let pure_map = Type::function(vec![Type::Mf], Type::Mf, Effects::NONE);
    let observing_filter = Type::function(vec![Type::Mf], Type::Verdict, Effects::OBSERVE);
    let writing_visit = Type::function(vec![Type::Mf], Type::Unit, Effects::WRITE);
    let cases = [
        (
            Intrinsic::ArrayNew {
                ty: dynamic.clone(),
                count: 0,
            },
            Effects::NONE,
        ),
        (Intrinsic::Size(dynamic.clone()), Effects::READ),
        (Intrinsic::Size(fixed.clone()), Effects::NONE),
        (Intrinsic::Size(Type::Text), Effects::ABORT),
        (
            Intrinsic::Index(dynamic.clone()),
            Effects::READ.union(Effects::ABORT),
        ),
        (Intrinsic::Index(fixed.clone()), Effects::ABORT),
        (Intrinsic::CheckIndex(dynamic.clone()), Effects::ABORT),
        (Intrinsic::CheckIndex(fixed.clone()), Effects::ABORT),
        (
            Intrinsic::SetIndex(dynamic.clone()),
            Effects::WRITE.union(Effects::ABORT),
        ),
        (Intrinsic::SetIndex(fixed), Effects::ABORT),
        (
            Intrinsic::Push(dynamic.clone()),
            Effects::WRITE.union(Effects::ABORT),
        ),
        (Intrinsic::Sorted(dynamic.clone()), Effects::READ),
        (
            Intrinsic::Map {
                array: dynamic.clone(),
                callback: pure_map,
            },
            Effects::READ,
        ),
        (
            Intrinsic::Filter {
                array: dynamic.clone(),
                callback: observing_filter,
            },
            Effects::READ.union(Effects::OBSERVE),
        ),
        (
            Intrinsic::ForEach {
                array: dynamic,
                callback: writing_visit,
            },
            Effects::READ.union(Effects::WRITE),
        ),
    ];
    for (operation, expected) in cases {
        assert_eq!(operation.signature().unwrap().effects, expected);
        verify_target(intrinsic_function(operation.clone(), expected)).unwrap();
        for allowed in [
            Effects::NONE,
            Effects::READ,
            Effects::WRITE,
            Effects::ABORT,
            Effects::OBSERVE,
        ] {
            if !allowed.contains(expected) {
                reject(unreachable(operation.clone(), allowed), "outside");
            }
        }
    }
}

#[test]
fn malformed_array_shapes_counts_and_operation_receivers_are_rejected() {
    let dynamic = Type::array(Type::Mf);
    let fixed = Type::fixed_array(Type::Mf, 2);
    let invalid = [
        Intrinsic::ArrayNew {
            ty: Type::Mf,
            count: 0,
        },
        Intrinsic::ArrayNew {
            ty: Type::array(Type::Unit),
            count: 0,
        },
        Intrinsic::ArrayNew {
            ty: Type::fixed_array(Type::Unit, 0),
            count: 0,
        },
        Intrinsic::ArrayNew {
            ty: fixed.clone(),
            count: 1,
        },
        Intrinsic::ArrayNew {
            ty: fixed.clone(),
            count: 3,
        },
        Intrinsic::ArrayNew {
            ty: dynamic.clone(),
            count: 65_537,
        },
        Intrinsic::ArrayNew {
            ty: dynamic,
            count: usize::MAX,
        },
        Intrinsic::ArrayNew {
            ty: Type::fixed_array(Type::Mf, i32::MAX as usize + 1),
            count: 0,
        },
        Intrinsic::Size(Type::Mf),
        Intrinsic::Index(Type::Text),
        Intrinsic::CheckIndex(Type::Text),
        Intrinsic::SetIndex(Type::option(Type::Mf)),
        Intrinsic::Push(fixed.clone()),
        Intrinsic::Sorted(fixed),
    ];
    for operation in invalid {
        assert!(operation.signature().is_none());
        let error = verify_target(unreachable(operation, Effects::ALL)).unwrap_err();
        assert_eq!(error.kind, VerifyErrorKind::Invalid);
        assert_eq!(error.instruction, Some(2));
    }
    // Checking the length metadata must not allocate the represented array.
    verify_target(intrinsic_function(
        Intrinsic::Size(Type::fixed_array(Type::Mf, i32::MAX as usize)),
        Effects::NONE,
    ))
    .unwrap();
    assert_eq!(
        Intrinsic::ArrayNew {
            ty: Type::array(Type::Mf),
            count: 65_536
        }
        .signature()
        .unwrap()
        .parameters
        .len(),
        65_536
    );
}

#[test]
fn indexing_checks_integer_width_payload_type_and_fixed_value_result() {
    for array in [Type::array(Type::Mf), Type::fixed_array(Type::Mf, 2)] {
        let mut target = intrinsic_function(Intrinsic::CheckIndex(array.clone()), Effects::ALL);
        target.parameters[1] = Type::Unc;
        target.locals[1] = Type::Unc;
        reject(target, "Expected mf, found unc");
        let mut target = intrinsic_function(Intrinsic::Index(array.clone()), Effects::ALL);
        target.parameters[1] = Type::Unc;
        target.locals[1] = Type::Unc;
        reject(target, "Expected mf, found unc");
        let mut target = intrinsic_function(Intrinsic::SetIndex(array), Effects::ALL);
        target.parameters[2] = Type::Unc;
        target.locals[2] = Type::Unc;
        reject(target, "Expected mf, found unc");
    }
    let dynamic = Intrinsic::SetIndex(Type::array(Type::Mf));
    let fixed = Intrinsic::SetIndex(Type::fixed_array(Type::Mf, 2));
    assert_eq!(dynamic.signature().unwrap().result, Type::Unit);
    assert_eq!(
        fixed.signature().unwrap().result,
        Type::fixed_array(Type::Mf, 2)
    );
    let mut target = intrinsic_function(fixed, Effects::ALL);
    target.result = Type::Unit;
    reject(target, "Expected unit, found hediJeans");
    let mut target = intrinsic_function(dynamic, Effects::ALL);
    target.result = Type::array(Type::Mf);
    reject(target, "Expected mfs<mf>, found unit");
}

#[test]
fn nested_array_and_function_elements_are_invariant_at_storage_boundaries() {
    let pure = Type::function(vec![], Type::Mf, Effects::NONE);
    let observing = Type::function(vec![], Type::Mf, Effects::OBSERVE);
    for (actual, expected) in [
        (Type::array(pure.clone()), Type::array(observing.clone())),
        (Type::array(observing.clone()), Type::array(pure.clone())),
        (
            Type::array(Type::array(pure)),
            Type::array(Type::array(observing)),
        ),
        (Type::array(Type::Mf), Type::array(Type::Unc)),
        (
            Type::fixed_array(Type::Mf, 1),
            Type::fixed_array(Type::Mf, 2),
        ),
    ] {
        let mut target = function(
            vec![actual],
            Type::Unit,
            Effects::NONE,
            vec![
                Op::Load(0),
                Op::Store(1),
                Op::Constant(Scalar::Unit),
                Op::Return,
            ],
        );
        target.locals.push(expected);
        reject(target, "Expected");
    }
}

#[test]
fn callbacks_cannot_forge_element_types_arity_or_result_contracts() {
    let array = Type::array(Type::Mf);
    let callbacks = [
        Type::Mf,
        Type::function(vec![], Type::Mf, Effects::NONE),
        Type::function(vec![Type::Unc], Type::Mf, Effects::NONE),
        Type::function(vec![Type::Mf, Type::Mf], Type::Mf, Effects::NONE),
    ];
    for callback in callbacks {
        for operation in [
            Intrinsic::Map {
                array: array.clone(),
                callback: callback.clone(),
            },
            Intrinsic::Filter {
                array: array.clone(),
                callback: callback.clone(),
            },
            Intrinsic::ForEach {
                array: array.clone(),
                callback: callback.clone(),
            },
        ] {
            assert!(operation.signature().is_none());
            reject(unreachable(operation, Effects::ALL), "intrinsic");
        }
    }
    for operation in [
        Intrinsic::Map {
            array: array.clone(),
            callback: Type::function(vec![Type::Mf], Type::Unit, Effects::NONE),
        },
        Intrinsic::Filter {
            array: array.clone(),
            callback: Type::function(vec![Type::Mf], Type::Mf, Effects::NONE),
        },
        Intrinsic::ForEach {
            array: array.clone(),
            callback: Type::function(vec![Type::Mf], Type::Verdict, Effects::NONE),
        },
        Intrinsic::Map {
            array: Type::fixed_array(Type::Mf, 0),
            callback: Type::function(vec![Type::Mf], Type::Mf, Effects::NONE),
        },
    ] {
        reject(unreachable(operation, Effects::ALL), "intrinsic");
    }
    let pure = Type::function(vec![], Type::Mf, Effects::NONE);
    let observing = Type::function(vec![], Type::Mf, Effects::OBSERVE);
    let operation = Intrinsic::Map {
        array: Type::array(observing),
        callback: Type::function(vec![pure], Type::Mf, Effects::NONE),
    };
    reject(unreachable(operation, Effects::ALL), "intrinsic");
}

#[test]
fn forged_callback_effects_are_checked_against_operand_and_caller_bounds() {
    let array = Type::array(Type::Mf);
    let pure = Type::function(vec![Type::Mf], Type::Mf, Effects::NONE);
    let observing = Type::function(vec![Type::Mf], Type::Mf, Effects::OBSERVE);
    let mut target = intrinsic_function(
        Intrinsic::Map {
            array: array.clone(),
            callback: pure.clone(),
        },
        Effects::READ,
    );
    target.parameters[1] = observing.clone();
    target.locals[1] = observing.clone();
    reject(target, "Expected fn");
    let operation = Intrinsic::Map {
        array,
        callback: observing,
    };
    reject(unreachable(operation.clone(), Effects::READ), "outside");
    let mut target = intrinsic_function(operation, Effects::READ.union(Effects::OBSERVE));
    target.parameters[1] = pure.clone();
    target.locals[1] = pure;
    verify_target(target).unwrap();
}

#[test]
fn sorting_rejects_float_boolean_reference_and_function_elements() {
    for element in [Type::Mf, Type::Unc, Type::Text] {
        verify_target(intrinsic_function(
            Intrinsic::Sorted(Type::array(element)),
            Effects::READ,
        ))
        .unwrap();
    }
    for element in [
        Type::Geeked,
        Type::Verdict,
        Type::array(Type::Mf),
        Type::option(Type::Mf),
        Type::function(vec![], Type::Mf, Effects::NONE),
    ] {
        reject(
            unreachable(Intrinsic::Sorted(Type::array(element)), Effects::ALL),
            "intrinsic",
        );
    }
}

fn shared_dag() -> Type {
    let mut ty = Type::Mf;
    for _ in 0..30 {
        ty = Type::result(ty.clone(), ty.clone());
    }
    ty
}

#[test]
fn intrinsic_embedded_types_are_bounded_before_callback_comparison_or_formatting() {
    for operation in [
        Intrinsic::Map {
            array: Type::array(shared_dag()),
            callback: Type::function(vec![shared_dag()], Type::Mf, Effects::NONE),
        },
        Intrinsic::Filter {
            array: Type::array(shared_dag()),
            callback: Type::function(vec![shared_dag()], Type::Verdict, Effects::NONE),
        },
        Intrinsic::ForEach {
            array: Type::array(shared_dag()),
            callback: Type::function(vec![shared_dag()], Type::Unit, Effects::NONE),
        },
        Intrinsic::Map {
            array: Type::array(Type::Mf),
            callback: Type::function(vec![Type::Mf], shared_dag(), Effects::NONE),
        },
        Intrinsic::ArrayNew {
            ty: Type::array(shared_dag()),
            count: 0,
        },
    ] {
        assert!(operation.signature().is_none());
        let error = verify_target(unreachable(operation, Effects::ALL)).unwrap_err();
        assert_eq!(error.kind, VerifyErrorKind::ResourceLimit);
        assert_eq!(error.function, 1);
        assert_eq!(error.instruction, Some(2));
    }
}
