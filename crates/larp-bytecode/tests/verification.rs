use larp_bytecode::{Function, Instruction, Module, Op, verify};
use larp_source::{SourceFile, SourceId};
use larp_types::{Binary, Effects, Scalar, Type};

fn function(code: Vec<Op>, locals: Vec<Type>, effects: Effects) -> Function {
    let source = SourceFile::new(SourceId(0), "test", "");
    let span = source.span(0..0).unwrap();
    Function {
        scenario: None,
        owner: None,
        constructor: None,
        name: "test".into(),
        parameters: Vec::new(),
        captures: Vec::new(),
        result: Type::Unit,
        effects,
        locals,
        code: code
            .into_iter()
            .map(|op| Instruction { op, span })
            .collect(),
        span,
    }
}

fn rejects(code: Vec<Op>, locals: Vec<Type>, expected: &str) {
    let result = verify(Module {
        roles: vec![],
        extensions: vec![],
        classes: vec![],
        functions: vec![function(code, locals, Effects::ALL)],
        entry: 0,
    });
    let error = result.unwrap_err();
    assert!(error.message.contains(expected), "{error:?}");
}

#[test]
fn malformed_operands_indices_and_fallthrough_are_rejected() {
    rejects(vec![Op::Binary(Binary::Add, Type::Mf)], vec![], "underflow");
    rejects(
        vec![Op::Constant(Scalar::Mf(1)), Op::Return],
        vec![],
        "Expected unit",
    );
    rejects(vec![Op::Constant(Scalar::Mf(1))], vec![], "outside");
    rejects(vec![Op::Jump(99)], vec![], "target");
    rejects(vec![Op::Load(0)], vec![], "index");
    rejects(vec![Op::Call(99)], vec![], "target");
    rejects(
        vec![Op::Binary(Binary::Multiply, Type::Text)],
        vec![],
        "Invalid binary",
    );
    rejects(
        vec![
            Op::Constant(Scalar::Unit),
            Op::Print,
            Op::Constant(Scalar::Unit),
            Op::Return,
        ],
        vec![],
        "Print requires a scalar value",
    );
}

#[test]
fn extra_return_values_and_wrong_stores_are_rejected() {
    rejects(
        vec![
            Op::Constant(Scalar::Mf(1)),
            Op::Constant(Scalar::Unit),
            Op::Return,
        ],
        vec![],
        "extra operands",
    );
    rejects(
        vec![Op::Constant(Scalar::Verdict(true)), Op::Store(0)],
        vec![Type::Mf],
        "Expected mf",
    );
    rejects(vec![Op::Load(0)], vec![Type::Mf], "uninitialized");
}

#[test]
fn branch_merges_intersect_initialization_even_when_the_other_path_arrives_later() {
    rejects(
        vec![
            Op::Constant(Scalar::Verdict(true)),
            Op::JumpIfFalse(5),
            Op::Constant(Scalar::Mf(1)),
            Op::Store(0),
            Op::Jump(6),
            Op::Jump(6),
            Op::Load(0),
            Op::Print,
            Op::Constant(Scalar::Unit),
            Op::Return,
        ],
        vec![Type::Mf],
        "uninitialized",
    );
    rejects(
        vec![
            Op::Constant(Scalar::Verdict(true)),
            Op::JumpIfFalse(4),
            Op::Constant(Scalar::Mf(1)),
            Op::Jump(5),
            Op::Constant(Scalar::Text("bad".into())),
            Op::Print,
            Op::Constant(Scalar::Unit),
            Op::Return,
        ],
        vec![],
        "Incompatible operand stacks",
    );
}

#[test]
fn effects_cannot_be_hidden_in_direct_calls_or_unreachable_instructions() {
    let entry = function(vec![Op::Call(1), Op::Return], vec![], Effects::NONE);
    let callee = function(
        vec![
            Op::Constant(Scalar::Mf(1)),
            Op::Print,
            Op::Constant(Scalar::Unit),
            Op::Return,
        ],
        vec![],
        Effects::OBSERVE,
    );
    assert!(
        verify(Module {
            roles: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![entry, callee],
            entry: 0
        })
        .unwrap_err()
        .message
        .contains("outside")
    );
    let entry = function(
        vec![Op::Constant(Scalar::Unit), Op::Return, Op::Print],
        vec![],
        Effects::NONE,
    );
    assert!(
        verify(Module {
            roles: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![entry],
            entry: 0
        })
        .unwrap_err()
        .message
        .contains("outside")
    );
    let entry = function(
        vec![Op::Constant(Scalar::Unit), Op::Return, Op::Call(99)],
        vec![],
        Effects::ALL,
    );
    assert!(
        verify(Module {
            roles: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![entry],
            entry: 0
        })
        .unwrap_err()
        .message
        .contains("target")
    );
}

#[test]
fn entry_and_parameter_signatures_are_verified() {
    let mut entry = function(
        vec![Op::Constant(Scalar::Unit), Op::Return],
        vec![],
        Effects::NONE,
    );
    entry.parameters = vec![Type::Mf];
    assert!(
        verify(Module {
            roles: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![entry],
            entry: 0
        })
        .unwrap_err()
        .message
        .contains("Entry")
    );
    let entry = function(
        vec![Op::Constant(Scalar::Unit), Op::Return],
        vec![],
        Effects::NONE,
    );
    let mut callee = entry.clone();
    callee.parameters = vec![Type::Mf];
    assert!(
        verify(Module {
            roles: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![entry, callee],
            entry: 0
        })
        .unwrap_err()
        .message
        .contains("Parameter slots")
    );
}

#[test]
fn verification_is_resource_bounded_before_large_dataflow_allocation() {
    let mut code = Vec::new();
    for _ in 0..80 {
        code.push(Op::Constant(Scalar::Mf(1)));
        code.push(Op::Store(0));
    }
    code.extend([Op::Constant(Scalar::Unit), Op::Return]);
    rejects(code, vec![Type::Mf; 65_536], "analysis budget");
}

#[test]
fn combined_function_headers_are_bounded_before_forward_closure_comparisons() {
    let mut shared = Type::Mf;
    for _ in 0..10 {
        shared = Type::result(shared.clone(), shared);
    }
    assert!(shared.within_limits());
    let entry = function(vec![Op::MakeClosure(1), Op::Return], vec![], Effects::NONE);
    for captures in [false, true] {
        let mut target = function(
            vec![Op::Constant(Scalar::Unit), Op::Return],
            vec![],
            Effects::NONE,
        );
        if captures {
            target.captures = vec![shared.clone(); 3];
        } else {
            target.parameters = vec![shared.clone(); 3];
        }
        target.locals = target
            .parameters
            .iter()
            .chain(&target.captures)
            .cloned()
            .collect();
        let error = verify(Module {
            roles: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![entry.clone(), target],
            entry: 0,
        })
        .unwrap_err();
        assert_eq!(error.kind, larp_bytecode::VerifyErrorKind::ResourceLimit);
        assert_eq!(error.function, 1);
        assert!(error.message.contains("complexity"));
    }
}

#[test]
fn analysis_work_is_bounded_across_individually_valid_functions() {
    let code: Vec<_> = (1..2048)
        .map(Op::Jump)
        .chain([Op::Constant(Scalar::Unit), Op::Return])
        .collect();
    let ordinary = function(code, vec![Type::Mf; 1024], Effects::NONE);
    let error = verify(Module {
        roles: vec![],
        extensions: vec![],
        classes: vec![],
        functions: vec![ordinary; 9],
        entry: 0,
    })
    .unwrap_err();
    assert_eq!(error.kind, larp_bytecode::VerifyErrorKind::ResourceLimit);
    assert!(
        error.message.contains("Module verification work"),
        "{error:?}"
    );
    assert!(error.function > 0);
}

#[test]
fn derived_intrinsic_signatures_share_the_verification_type_work_budget() {
    let mut element = Type::Mf;
    for _ in 0..10 {
        element = Type::result(element.clone(), element);
    }
    let array = Type::array(element);
    assert!(array.is_well_formed());
    let entry = function(
        vec![
            Op::Intrinsic(larp_types::Intrinsic::ArrayNew {
                ty: array,
                count: 65_536,
            }),
            Op::Return,
        ],
        vec![],
        Effects::NONE,
    );
    let error = verify(Module {
        roles: vec![],
        extensions: vec![],
        classes: vec![],
        functions: vec![entry],
        entry: 0,
    })
    .unwrap_err();
    assert_eq!(error.kind, larp_bytecode::VerifyErrorKind::ResourceLimit);
    assert!(error.message.contains("type work"), "{error:?}");
}

#[test]
fn host_io_effects_are_verified_even_in_unreachable_instructions() {
    for operation in [
        larp_types::Intrinsic::Arguments,
        larp_types::Intrinsic::ReadLine,
    ] {
        let entry = function(
            vec![
                Op::Constant(Scalar::Unit),
                Op::Return,
                Op::Intrinsic(operation),
            ],
            vec![],
            Effects::NONE,
        );
        let error = verify(Module {
            roles: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![entry],
            entry: 0,
        })
        .unwrap_err();
        assert_eq!(error.kind, larp_bytecode::VerifyErrorKind::Invalid);
        assert!(error.message.contains("io"), "{error:?}");
    }
}
