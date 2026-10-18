use larp_bytecode::{Class, Field, Op, ScenarioContext, verify};
use larp_ir::{Block, Function, Instruction, InstructionKind, Program, Terminator, lower};
use larp_source::{SourceFile, SourceId};
use larp_types::{Effects, Scalar, Type};

#[test]
fn scenario_lowering_preserves_region_targets_metadata_and_source_spans() {
    let source = SourceFile::new(SourceId(0), "scenario.larp", "larp (bro) as Test {}");
    let span = source.span(0..source.text().len()).unwrap();
    let root = Type::class(0, "Founder");
    let context = ScenarioContext {
        entry: 2,
        class: 0,
        origin: None,
    };
    let function = |parameters: Vec<Type>,
                    locals: Vec<Type>,
                    result,
                    effects,
                    kinds: Vec<InstructionKind>,
                    terminator| Function {
        scenario: None,
        owner: None,
        constructor: None,
        name: "test".into(),
        parameters,
        captures: vec![],
        result,
        effects,
        locals,
        blocks: vec![Block {
            parameter: None,
            instructions: kinds
                .into_iter()
                .map(|kind| Instruction { kind, span })
                .collect(),
            terminator,
            span,
        }],
        span,
    };
    let main = function(
        vec![],
        vec![Type::Mf, root.clone(), Type::Unit],
        Type::Unit,
        Effects::ALL,
        vec![
            InstructionKind::Constant {
                destination: 0,
                value: Scalar::Mf(7),
            },
            InstructionKind::Construct {
                destination: 1,
                class: 0,
                arguments: vec![0],
            },
            InstructionKind::Scenario {
                destination: 2,
                entry: 2,
                label: "Test".into(),
                arguments: vec![1],
            },
        ],
        Terminator::Return(None),
    );
    let mut constructor = function(
        vec![Type::Mf],
        vec![Type::Mf],
        root.clone(),
        Effects::READ.union(Effects::WRITE).union(Effects::ABORT),
        vec![],
        Terminator::ConstructReturn {
            class: 0,
            fields: vec![0],
        },
    );
    constructor.owner = Some(0);
    constructor.constructor = Some(0);
    let mut scenario = function(
        vec![root.clone()],
        vec![root, Type::Mf, Type::Unit],
        Type::Unit,
        Effects::WRITE.union(Effects::OBSERVE),
        vec![
            InstructionKind::Constant {
                destination: 1,
                value: Scalar::Mf(49),
            },
            InstructionKind::Flex {
                destination: 2,
                receiver: 0,
                class: 0,
                field: 0,
                value: 1,
            },
            InstructionKind::Canon {
                destination: 2,
                receiver: 0,
                class: 0,
                field: 0,
            },
            InstructionKind::Palantir {
                destination: 2,
                receiver: 0,
                class: 0,
                fields: vec![0],
            },
        ],
        Terminator::Return(None),
    );
    scenario.scenario = Some(context);
    let program = Program {
        roles: vec![],
        extensions: vec![],
        entry: 0,
        classes: vec![Class {
            implementations: vec![],
            name: "Founder".into(),
            constructor: 1,
            fields: vec![Field {
                name: "aura".into(),
                ty: Type::Mf,
                public: true,
            }],
            methods: vec![],
        }],
        functions: vec![main, constructor, scenario],
    };
    let copy = program.clone();
    let bytecode = lower(&copy);
    assert_eq!(bytecode.functions[2].scenario, Some(context));
    assert!(bytecode.functions[0].code.iter().any(
        |instruction| matches!(&instruction.op, Op::Scenario { entry: 2, label } if label == "Test")
    ));
    for function in &bytecode.functions {
        assert!(
            function
                .code
                .iter()
                .all(|instruction| instruction.span == span)
        );
    }
    let verified = verify(bytecode).unwrap();
    let disassembly = verified.disassemble();
    for name in ["Scenario", "Canon", "Flex", "Palantir"] {
        assert!(disassembly.contains(name));
    }
}
