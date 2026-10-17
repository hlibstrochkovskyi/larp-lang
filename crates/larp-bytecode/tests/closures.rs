use larp_bytecode::{Function, Instruction, Module, Op, VerifyError, verify};
use larp_source::{SourceFile, SourceId};
use larp_types::{Effects, Intrinsic, Scalar, Type};

fn function(
    parameters: Vec<Type>,
    captures: Vec<Type>,
    result: Type,
    effects: Effects,
    code: Vec<Op>,
) -> Function {
    let source = SourceFile::new(SourceId(0), "closures.larp", "");
    let span = source.span(0..0).unwrap();
    let locals = parameters.iter().chain(&captures).cloned().collect();
    Function {
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

fn entry(code: Vec<Op>) -> Function {
    function(vec![], vec![], Type::Unit, Effects::ALL, code)
}

fn unit() -> Function {
    entry(vec![Op::Constant(Scalar::Unit), Op::Return])
}

fn reject(functions: Vec<Function>, expected: &str) -> VerifyError {
    let error = verify(Module {
        functions,
        entry: 0,
    })
    .unwrap_err();
    assert!(error.message.contains(expected), "{error:?}");
    error
}

fn captured_mf() -> Function {
    function(
        vec![],
        vec![Type::Mf],
        Type::Mf,
        Effects::NONE,
        vec![Op::Load(0), Op::Return],
    )
}

#[test]
fn capture_slots_must_follow_parameters_with_exact_types() {
    let mut target = function(
        vec![Type::Text],
        vec![Type::Mf],
        Type::Mf,
        Effects::NONE,
        vec![Op::Load(1), Op::Return],
    );
    verify(Module {
        functions: vec![unit(), target.clone()],
        entry: 0,
    })
    .unwrap();
    target.locals.swap(0, 1);
    reject(vec![unit(), target], "Parameter slots");
    let mut target = captured_mf();
    target.locals[0] = Type::Unc;
    reject(vec![unit(), target], "Capture slots");
    let mut target = captured_mf();
    target.locals.clear();
    reject(vec![unit(), target], "Capture slots");
}

#[test]
fn entry_cannot_require_captures() {
    let mut main = unit();
    main.captures = vec![Type::Mf];
    main.locals = vec![Type::Mf];
    reject(vec![main], "Entry");
}

#[test]
fn captured_bindings_are_read_only_even_in_unreachable_code() {
    for code in [
        vec![
            Op::Constant(Scalar::Mf(1)),
            Op::Store(0),
            Op::Load(0),
            Op::Return,
        ],
        vec![Op::Load(0), Op::Return, Op::Store(0)],
    ] {
        let mut target = captured_mf();
        target.code = code
            .into_iter()
            .map(|op| Instruction {
                op,
                span: target.span,
            })
            .collect();
        reject(vec![unit(), target], "read-only");
    }
}

#[test]
fn direct_calls_cannot_initialize_capture_slots() {
    reject(
        vec![entry(vec![Op::Call(1), Op::Return]), captured_mf()],
        "Direct calls cannot supply closure captures",
    );
    let mut main = unit();
    main.code.push(Instruction {
        op: Op::Call(1),
        span: main.span,
    });
    reject(
        vec![main, captured_mf()],
        "Direct calls cannot supply closure captures",
    );
}

#[test]
fn closure_creation_checks_target_and_capture_operands() {
    reject(vec![entry(vec![Op::MakeClosure(99)])], "Closure target");
    reject(
        vec![entry(vec![Op::MakeClosure(1)]), captured_mf()],
        "underflow",
    );
    reject(
        vec![
            entry(vec![Op::Constant(Scalar::Unc(7)), Op::MakeClosure(1)]),
            captured_mf(),
        ],
        "Expected mf, found unc",
    );
}

#[test]
fn closure_capture_order_and_indirect_arguments_are_checked_separately() {
    let target = function(
        vec![Type::Verdict],
        vec![Type::Mf, Type::Text],
        Type::Text,
        Effects::NONE,
        vec![Op::Load(2), Op::Return],
    );
    let signature = Type::function(vec![Type::Verdict], Type::Text, Effects::NONE);
    let prefix = vec![
        Op::Constant(Scalar::Mf(1)),
        Op::Constant(Scalar::Text("capture".into())),
        Op::MakeClosure(1),
    ];
    let mut good = prefix.clone();
    good.extend([
        Op::Constant(Scalar::Verdict(true)),
        Op::CallIndirect(signature.clone()),
        Op::Print,
        Op::Constant(Scalar::Unit),
        Op::Return,
    ]);
    verify(Module {
        functions: vec![entry(good), target.clone()],
        entry: 0,
    })
    .unwrap();
    let mut bad = prefix;
    bad.extend([Op::Constant(Scalar::Mf(0)), Op::CallIndirect(signature)]);
    reject(vec![entry(bad), target], "Expected verdict, found mf");
}

#[test]
fn forged_indirect_signature_cannot_erase_observation_effects() {
    let target = function(
        vec![],
        vec![],
        Type::Unit,
        Effects::OBSERVE,
        vec![
            Op::Constant(Scalar::Mf(1)),
            Op::Print,
            Op::Constant(Scalar::Unit),
            Op::Return,
        ],
    );
    let mut main = entry(vec![
        Op::MakeClosure(1),
        Op::CallIndirect(Type::function(vec![], Type::Unit, Effects::NONE)),
        Op::Return,
    ]);
    main.effects = Effects::NONE;
    reject(vec![main, target], "Expected fn");
}

#[test]
fn indirect_calls_must_fit_the_callers_bound_even_when_unreachable() {
    let signature = Type::function(vec![], Type::Unit, Effects::OBSERVE);
    let mut main = unit();
    main.effects = Effects::NONE;
    main.code.push(Instruction {
        op: Op::CallIndirect(signature),
        span: main.span,
    });
    reject(vec![main], "outside");
    reject(
        vec![entry(vec![Op::CallIndirect(Type::Mf)])],
        "requires a function type",
    );
}

#[test]
fn function_effect_bounds_can_widen_without_changing_parameters_or_results() {
    let target = function(
        vec![],
        vec![],
        Type::Mf,
        Effects::NONE,
        vec![Op::Constant(Scalar::Mf(3)), Op::Return],
    );
    let code = vec![
        Op::MakeClosure(1),
        Op::CallIndirect(Type::function(vec![], Type::Mf, Effects::ALL)),
        Op::Print,
        Op::Constant(Scalar::Unit),
        Op::Return,
    ];
    verify(Module {
        functions: vec![entry(code), target.clone()],
        entry: 0,
    })
    .unwrap();
    for signature in [
        Type::function(vec![Type::Mf], Type::Mf, Effects::ALL),
        Type::function(vec![], Type::Unc, Effects::ALL),
    ] {
        let mut code = vec![Op::MakeClosure(1)];
        if matches!(&signature, Type::Function(sig) if !sig.parameters.is_empty()) {
            code.push(Op::Constant(Scalar::Mf(1)));
        }
        code.push(Op::CallIndirect(signature));
        reject(vec![entry(code), target.clone()], "Expected fn");
    }
}

#[test]
fn malformed_intrinsics_are_rejected_even_after_return() {
    let malformed = Type::function(vec![Type::Unit], Type::Mf, Effects::NONE);
    for intrinsic in [
        Intrinsic::None(Type::Mf),
        Intrinsic::Some(Type::result(Type::Mf, Type::Text)),
        Intrinsic::Ok(Type::option(Type::Mf)),
        Intrinsic::Err(Type::Mf),
        Intrinsic::IsSome(Type::Text),
        Intrinsic::IsOk(Type::option(Type::Mf)),
        Intrinsic::Copium(Type::Mf),
        Intrinsic::Convert {
            from: Type::Text,
            to: Type::Mf,
            fallible: true,
        },
        Intrinsic::Convert {
            from: Type::Mf,
            to: Type::Geeked,
            fallible: false,
        },
        Intrinsic::None(Type::option(malformed.clone())),
        Intrinsic::Ok(Type::result(Type::Mf, malformed)),
    ] {
        let expected = if intrinsic.types().iter().any(|ty| !ty.is_well_formed()) {
            "Malformed type"
        } else {
            "intrinsic"
        };
        let mut main = unit();
        main.code.push(Instruction {
            op: Op::Intrinsic(intrinsic),
            span: main.span,
        });
        reject(vec![main], expected);
    }
}

#[test]
fn option_and_result_construction_check_payloads_without_covariance() {
    for intrinsic in [
        Intrinsic::Some(Type::option(Type::Mf)),
        Intrinsic::Ok(Type::result(Type::Mf, Type::Text)),
        Intrinsic::Err(Type::result(Type::Text, Type::Mf)),
    ] {
        reject(
            vec![entry(vec![
                Op::Constant(Scalar::Unc(2)),
                Op::Intrinsic(intrinsic),
            ])],
            "Expected mf, found unc",
        );
    }
    let mut main = entry(vec![
        Op::Intrinsic(Intrinsic::None(Type::option(Type::function(
            vec![],
            Type::Mf,
            Effects::OBSERVE,
        )))),
        Op::Store(0),
    ]);
    main.locals = vec![Type::option(Type::function(
        vec![],
        Type::Mf,
        Effects::NONE,
    ))];
    reject(vec![main], "Expected baddie");
}

#[test]
fn checked_conversion_abort_effect_cannot_be_hidden() {
    let mut main = unit();
    main.effects = Effects::NONE;
    main.code.push(Instruction {
        op: Op::Intrinsic(Intrinsic::Convert {
            from: Type::Unc,
            to: Type::Mf,
            fallible: false,
        }),
        span: main.span,
    });
    reject(vec![main], "outside");
    let mut main = entry(vec![
        Op::Constant(Scalar::Unc(i64::MAX)),
        Op::Intrinsic(Intrinsic::Convert {
            from: Type::Unc,
            to: Type::Mf,
            fallible: true,
        }),
        Op::Intrinsic(Intrinsic::IsOk(Type::result(
            Type::Mf,
            Type::ConversionError,
        ))),
        Op::Store(0),
        Op::Constant(Scalar::Unit),
        Op::Return,
    ]);
    main.effects = Effects::NONE;
    main.locals = vec![Type::Verdict];
    verify(Module {
        functions: vec![main],
        entry: 0,
    })
    .unwrap();
}

fn dispatch(success: usize, error: usize, ty: Type) -> Op {
    Op::MatchResult { ty, success, error }
}

#[test]
fn result_dispatch_validates_types_and_both_targets_even_when_unreachable() {
    for op in [
        dispatch(0, 0, Type::Mf),
        dispatch(99, 0, Type::result(Type::Mf, Type::Text)),
        dispatch(0, 99, Type::result(Type::Mf, Type::Text)),
    ] {
        let mut main = unit();
        main.code.push(Instruction {
            op,
            span: main.span,
        });
        reject(vec![main], "Invalid result dispatch");
    }
}

#[test]
fn result_dispatch_checks_the_container_and_both_payload_paths() {
    let ty = Type::result(Type::Mf, Type::Text);
    reject(
        vec![entry(vec![
            Op::Constant(Scalar::Mf(1)),
            dispatch(2, 2, ty.clone()),
            Op::Print,
        ])],
        "Expected result",
    );
    // Even though this result is known to be successful, its error path must type-check.
    let mut main = entry(vec![
        Op::Constant(Scalar::Mf(1)),
        Op::Intrinsic(Intrinsic::Ok(ty.clone())),
        dispatch(3, 7, ty),
        Op::Store(0),
        Op::Constant(Scalar::Unit),
        Op::Return,
        Op::Return,
        Op::Store(0),
        Op::Constant(Scalar::Unit),
        Op::Return,
    ]);
    main.locals = vec![Type::Mf];
    reject(vec![main], "Expected mf, found text");
}

#[test]
fn result_payload_stacks_must_agree_at_a_shared_target() {
    let ty = Type::result(Type::Mf, Type::Text);
    reject(
        vec![entry(vec![
            Op::Constant(Scalar::Mf(1)),
            Op::Intrinsic(Intrinsic::Ok(ty.clone())),
            dispatch(3, 3, ty),
            Op::Print,
            Op::Constant(Scalar::Unit),
            Op::Return,
        ])],
        "Incompatible operand stacks",
    );
    let ty = Type::result(Type::Mf, Type::Mf);
    verify(Module {
        functions: vec![entry(vec![
            Op::Constant(Scalar::Mf(1)),
            Op::Intrinsic(Intrinsic::Ok(ty.clone())),
            dispatch(3, 3, ty),
            Op::Print,
            Op::Constant(Scalar::Unit),
            Op::Return,
        ])],
        entry: 0,
    })
    .unwrap();
}

#[test]
fn distinct_result_payloads_can_be_consumed_before_a_control_flow_merge() {
    let ty = Type::result(Type::Mf, Type::Text);
    verify(Module {
        functions: vec![entry(vec![
            Op::Constant(Scalar::Text("error".into())),
            Op::Intrinsic(Intrinsic::Err(ty.clone())),
            dispatch(3, 5, ty),
            Op::Print,
            Op::Jump(6),
            Op::Print,
            Op::Constant(Scalar::Unit),
            Op::Return,
        ])],
        entry: 0,
    })
    .unwrap();
}

#[test]
fn malformed_nested_function_types_are_rejected_at_all_boundaries() {
    let malformed = Type::function(vec![Type::Unit], Type::Text, Effects::NONE);
    let nested = Type::function(vec![], Type::option(malformed), Effects::NONE);
    let mut main = unit();
    main.locals = vec![nested.clone()];
    reject(vec![main], "Malformed type");
    let mut target = captured_mf();
    target.result = nested.clone();
    reject(vec![unit(), target], "Malformed type");
    reject(
        vec![entry(vec![Op::CallIndirect(nested.clone())])],
        "Malformed type",
    );
    reject(
        vec![entry(vec![dispatch(0, 0, Type::result(Type::Mf, nested))])],
        "Malformed type",
    );
}

#[test]
fn nested_function_parameters_and_results_cannot_launder_effect_bounds() {
    let pure = Type::function(vec![], Type::Mf, Effects::NONE);
    let observing = Type::function(vec![], Type::Mf, Effects::OBSERVE);
    for (actual, forged) in [
        (
            Type::function(vec![observing.clone()], Type::Unit, Effects::NONE),
            Type::function(vec![pure.clone()], Type::Unit, Effects::NONE),
        ),
        (
            Type::function(vec![], observing, Effects::NONE),
            Type::function(vec![], pure, Effects::NONE),
        ),
    ] {
        let mut main = entry(vec![Op::Load(0), Op::Store(1)]);
        // Parameters are initialized only in a non-entry function.
        main.parameters = vec![actual.clone()];
        main.locals = vec![actual, forged];
        reject(vec![unit(), main], "Expected fn");
    }
}

#[test]
fn excessive_type_depth_is_rejected_before_dataflow() {
    let mut nested = Type::Mf;
    for _ in 0..130 {
        nested = Type::option(nested);
    }
    let mut main = unit();
    main.locals = vec![nested];
    reject(vec![main], "Type complexity limit exceeded");
}

fn shared_result_dag(depth: usize) -> Type {
    let mut nested = Type::Mf;
    for _ in 0..depth {
        nested = Type::result(nested.clone(), nested.clone());
    }
    nested
}

#[test]
fn shared_type_dags_are_bounded_by_expanded_work_before_dataflow() {
    // Only 30 Arc allocations encode more than a billion expanded payload nodes.
    let mut main = unit();
    main.locals = vec![shared_result_dag(30)];
    let error = verify(Module {
        functions: vec![main],
        entry: 0,
    })
    .unwrap_err();
    assert_eq!(error.function, 0);
    assert_eq!(error.instruction, None);
}

#[test]
fn forward_function_types_are_checked_before_closure_or_call_dataflow() {
    let signature = Type::function(vec![], Type::Unit, Effects::NONE);
    for mut main in [
        entry(vec![Op::Call(1), Op::Return]),
        entry(vec![
            Op::MakeClosure(1),
            Op::CallIndirect(signature.clone()),
            Op::Return,
        ]),
        entry(vec![
            Op::MakeClosure(1),
            Op::Store(0),
            Op::Constant(Scalar::Unit),
            Op::Return,
        ]),
    ] {
        main.locals = vec![signature.clone()];
        let mut target = unit();
        target.result = shared_result_dag(30);
        // Signature comparisons and diagnostic formatting in function 0 must never
        // encounter the unchecked forward result type from function 1.
        let error = verify(Module {
            functions: vec![main, target],
            entry: 0,
        })
        .unwrap_err();
        assert_eq!(error.function, 1);
        assert_eq!(error.instruction, None);
    }
}
