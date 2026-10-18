use larp_bytecode::{
    Class, Extension, Field, Function, Instruction, Module, Op, Role, RoleMethod, VerifyErrorKind,
    verify,
};
use larp_source::{SourceFile, SourceId};
use larp_types::{Effects, Scalar, Type};

fn cell(effects: Effects) -> Type {
    Type::lazy(Type::Mf, effects)
}
fn op(op: Op) -> Instruction {
    Instruction {
        op,
        span: SourceFile::new(SourceId(0), "lazy.larp", "")
            .span(0..0)
            .unwrap(),
    }
}
fn function(result: Type, effects: Effects, code: Vec<Op>) -> Function {
    Function {
        scenario: None,
        owner: None,
        constructor: None,
        name: "test".into(),
        parameters: vec![],
        captures: vec![],
        locals: vec![],
        result,
        effects,
        code: code.into_iter().map(op).collect(),
        span: op(Op::Return).span,
    }
}
fn unit() -> Op {
    Op::Constant(Scalar::Unit)
}
fn mf(value: i32) -> Op {
    Op::Constant(Scalar::Mf(value))
}
fn module() -> Module {
    let mut main = function(
        Type::Unit,
        Effects::ALL,
        vec![
            mf(49),
            Op::MakeLazy(1),
            Op::Store(0),
            Op::Load(0),
            Op::Force(cell(Effects::NONE)),
            Op::Print,
            unit(),
            Op::Return,
        ],
    );
    main.locals = vec![cell(Effects::NONE)];
    let mut thunk = function(Type::Mf, Effects::NONE, vec![Op::Load(0), Op::Return]);
    thunk.captures = vec![Type::Mf];
    thunk.locals = vec![Type::Mf];
    Module {
        classes: vec![],
        roles: vec![],
        extensions: vec![],
        entry: 0,
        functions: vec![main, thunk],
    }
}
fn reject(module: Module, message: &str) {
    let error = verify(module).unwrap_err();
    assert!(
        error.message.contains(message),
        "expected {message}, got {error:?}"
    );
}

#[test]
fn lazy_creation_checks_target_bounds_arity_result_captures_and_effects() {
    verify(module()).unwrap();
    let mut bad = module();
    bad.functions[0].code[1] = op(Op::MakeLazy(usize::MAX));
    reject(bad, "target out of bounds");
    let mut bad = module();
    bad.functions[1].parameters = vec![Type::Mf];
    bad.functions[1].locals.push(Type::Mf);
    reject(bad, "Lazy thunks");
    let mut bad = module();
    bad.functions[1].result = Type::array(Type::Mf);
    reject(bad, "Lazy thunks");
    for capture in [
        Type::array(Type::Mf),
        Type::function(vec![], Type::Mf, Effects::NONE),
    ] {
        let mut bad = module();
        bad.functions[1].captures = vec![capture.clone()];
        bad.functions[1].locals = vec![capture];
        reject(bad, "Lazy thunks");
    }
    for effects in [Effects::READ, Effects::WRITE, Effects::OBSERVE, Effects::IO] {
        let mut bad = module();
        bad.functions[1].effects = effects;
        reject(bad, "Lazy thunks");
    }
}

#[test]
fn forcing_checks_the_exact_payload_and_effect_bound_even_after_return() {
    for ty in [
        Type::Mf,
        Type::lazy(Type::Text, Effects::NONE),
        cell(Effects::ABORT),
    ] {
        let mut bad = module();
        bad.functions[0].code[4] = op(Op::Force(ty));
        assert!(verify(bad).is_err());
    }
    let mut bad = module();
    bad.functions[0].code.push(op(Op::Force(Type::Mf)));
    reject(bad, "Force requires");
    let mut bad = module();
    bad.functions[0].code.push(op(Op::MakeLazy(usize::MAX)));
    reject(bad, "target out of bounds");
    let mut bad = module();
    bad.functions[1].effects = Effects::ABORT;
    bad.functions[0].locals[0] = cell(Effects::ABORT);
    bad.functions[0].code = vec![
        mf(49),
        Op::MakeLazy(1),
        Op::Store(0),
        Op::Load(0),
        Op::Force(cell(Effects::ABORT)),
        Op::Store(1),
        unit(),
        Op::Return,
    ]
    .into_iter()
    .map(op)
    .collect();
    bad.functions[0].locals.push(Type::Mf);
    bad.functions[0].effects = Effects::NONE;
    reject(bad.clone(), "Instruction requires");
    bad.functions[0].effects = Effects::ABORT;
    verify(bad).unwrap();
}

#[test]
fn lazy_cells_cannot_escape_through_function_signatures_captures_or_containers() {
    for location in 0..3 {
        let mut bad = module();
        let function = &mut bad.functions[1];
        match location {
            0 => {
                function.parameters = vec![cell(Effects::NONE)];
                function.locals.insert(0, cell(Effects::NONE));
            }
            1 => {
                function.captures = vec![cell(Effects::NONE)];
                function.locals[0] = cell(Effects::NONE);
            }
            _ => function.result = cell(Effects::NONE),
        }
        reject(bad, "Lazy cells cannot escape");
    }
    for ty in [
        Type::option(cell(Effects::NONE)),
        Type::array(cell(Effects::NONE)),
        Type::fixed_array(cell(Effects::NONE), 0),
        Type::function(vec![], cell(Effects::NONE), Effects::NONE),
    ] {
        let mut bad = module();
        bad.functions[0].locals.push(ty);
        reject(bad, "Malformed type");
    }
}

#[test]
fn lazy_cells_cannot_be_fields_role_arguments_or_extension_receivers() {
    let mut bad = module();
    bad.classes.push(Class {
        implementations: vec![],
        name: "Root".into(),
        fields: vec![Field {
            name: "cell".into(),
            ty: cell(Effects::NONE),
            public: true,
        }],
        methods: vec![],
        constructor: 1,
    });
    reject(bad, "Lazy cells cannot be class fields");
    let mut bad = module();
    bad.roles.push(Role {
        name: "Role".into(),
        methods: vec![RoleMethod {
            name: "force".into(),
            parameters: vec![cell(Effects::NONE)],
            result: Type::Mf,
            effects: Effects::NONE,
        }],
    });
    reject(bad, "Malformed type");
    let mut bad = module();
    bad.extensions.push(Extension {
        receiver: cell(Effects::NONE),
        name: "force".into(),
        function: 1,
    });
    reject(bad, "Lazy cells cannot be extension receivers");
}

#[test]
fn lazy_type_bounds_reject_non_snapshot_payloads_impure_bounds_and_excessive_work() {
    for ty in [
        Type::lazy(Type::Unit, Effects::NONE),
        Type::lazy(Type::array(Type::Mf), Effects::NONE),
        Type::lazy(Type::Mf, Effects::READ),
        Type::lazy(cell(Effects::NONE), Effects::NONE),
    ] {
        let mut bad = module();
        bad.functions[0].locals.push(ty);
        reject(bad, "Malformed type");
    }
    let mut dag = Type::Mf;
    for _ in 0..30 {
        dag = Type::result(dag.clone(), dag);
    }
    let mut bad = module();
    bad.functions[0]
        .code
        .push(op(Op::Force(Type::lazy(dag, Effects::NONE))));
    assert_eq!(
        verify(bad).unwrap_err().kind,
        VerifyErrorKind::ResourceLimit
    );
}

#[test]
fn silence_regions_require_balanced_returns_and_equal_depth_at_control_flow_joins() {
    let mut good = module();
    good.functions[0] = function(
        Type::Unit,
        Effects::NONE,
        vec![Op::SilenceEnter, Op::SilenceExit, unit(), Op::Return],
    );
    verify(good).unwrap();
    let mut bad = module();
    bad.functions[0] = function(
        Type::Unit,
        Effects::NONE,
        vec![Op::SilenceExit, unit(), Op::Return],
    );
    reject(bad, "underflow");
    let mut bad = module();
    bad.functions[0] = function(
        Type::Unit,
        Effects::NONE,
        vec![Op::SilenceEnter, unit(), Op::Return],
    );
    reject(bad, "open silence");
    let mut bad = module();
    bad.functions[0] = function(
        Type::Unit,
        Effects::NONE,
        vec![
            Op::Constant(Scalar::Verdict(true)),
            Op::JumpIfFalse(4),
            Op::SilenceEnter,
            Op::Jump(5),
            Op::Jump(5),
            Op::Jump(5),
        ],
    );
    reject(bad, "silence depth");
    let mut bad = module();
    bad.functions[0] = function(
        Type::Unit,
        Effects::NONE,
        vec![
            Op::SilenceEnter,
            Op::Constant(Scalar::Text("hidden".into())),
            Op::Print,
            Op::SilenceExit,
            unit(),
            Op::Return,
        ],
    );
    reject(bad, "Instruction requires");
}

#[test]
fn silence_nesting_is_resource_bounded_and_constructor_completion_must_restore_it() {
    let mut bad = module();
    bad.functions[0] = function(
        Type::Unit,
        Effects::NONE,
        std::iter::repeat_n(Op::SilenceEnter, 65_537)
            .chain([unit(), Op::Return])
            .collect(),
    );
    assert_eq!(
        verify(bad).unwrap_err().kind,
        VerifyErrorKind::ResourceLimit
    );
    let mut bad = module();
    bad.functions[0] = function(Type::Unit, Effects::ALL, vec![unit(), Op::Return]);
    bad.classes.push(Class {
        implementations: vec![],
        name: "Root".into(),
        fields: vec![],
        methods: vec![],
        constructor: 1,
    });
    let constructor = &mut bad.functions[1];
    *constructor = function(
        Type::class(0, "Root"),
        Effects::READ.union(Effects::WRITE).union(Effects::ABORT),
        vec![Op::SilenceEnter, Op::NewObject(0)],
    );
    constructor.owner = Some(0);
    constructor.constructor = Some(0);
    reject(bad, "open silence");
}
