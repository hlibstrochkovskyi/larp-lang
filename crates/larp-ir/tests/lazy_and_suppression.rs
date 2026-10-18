use larp_ir::{Block, Function, Instruction, InstructionKind as Inst, Program, Terminator, lower};
use larp_source::{SourceFile, SourceId};
use larp_types::{Effects, Scalar, Type};

#[test]
fn lazy_cells_forcing_and_balanced_silence_survive_lowering_and_verification() {
    let source = SourceFile::new(SourceId(0), "lazy.larp", "nonchalant");
    let span = source.span(0..source.text().len()).unwrap();
    let instruction = |kind| Instruction { kind, span };
    let function =
        |name: &str, captures: Vec<Type>, result, effects, locals, instructions, terminator| {
            Function {
                scenario: None,
                owner: None,
                constructor: None,
                name: name.into(),
                parameters: vec![],
                captures,
                result,
                effects,
                locals,
                blocks: vec![Block {
                    parameter: None,
                    instructions,
                    terminator,
                    span,
                }],
                span,
            }
        };
    let main = function(
        "main",
        vec![],
        Type::Unit,
        Effects::OBSERVE,
        vec![Type::Mf, Type::lazy(Type::Mf, Effects::NONE), Type::Mf],
        vec![
            instruction(Inst::Constant {
                destination: 0,
                value: Scalar::Mf(49),
            }),
            instruction(Inst::Lazy {
                destination: 1,
                function: 1,
                captures: vec![0],
            }),
            instruction(Inst::Force {
                destination: 2,
                value: 1,
            }),
            instruction(Inst::SilenceEnter),
            instruction(Inst::Print(2)),
            instruction(Inst::SilenceExit),
            instruction(Inst::Print(2)),
        ],
        Terminator::Return(None),
    );
    let thunk = function(
        "thunk",
        vec![Type::Mf],
        Type::Mf,
        Effects::NONE,
        vec![Type::Mf],
        vec![],
        Terminator::Return(Some(0)),
    );
    let program = Program {
        roles: vec![],
        extensions: vec![],
        classes: vec![],
        functions: vec![main, thunk],
        entry: 0,
    };
    let verified = larp_bytecode::verify(lower(&program)).unwrap();
    let code = &verified.module().functions[0].code;
    assert!(
        code.iter()
            .any(|instruction| matches!(instruction.op, larp_bytecode::Op::MakeLazy(1)))
    );
    assert!(code.iter().any(|instruction| matches!(&instruction.op, larp_bytecode::Op::Force(ty) if *ty == Type::lazy(Type::Mf, Effects::NONE))));
    assert_eq!(
        code.iter()
            .filter(|instruction| matches!(
                instruction.op,
                larp_bytecode::Op::SilenceEnter | larp_bytecode::Op::SilenceExit
            ))
            .count(),
        2
    );
    assert!(code.iter().all(|instruction| instruction.span == span));
}
