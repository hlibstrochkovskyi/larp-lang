use larp_bytecode::{Function, Instruction, Module, Op, verify};
use larp_source::{SourceFile, SourceId};
use larp_types::{Binary, Effects, Scalar, Type};

fn function(code: Vec<Op>, locals: Vec<Type>, effects: Effects) -> Function {
    let source = SourceFile::new(SourceId(0), "test", "");
    let span = source.span(0..0).unwrap();
    Function {
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
