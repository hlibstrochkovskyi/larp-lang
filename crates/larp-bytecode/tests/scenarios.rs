use larp_bytecode::{
    Class, Field, Function, Instruction, Method, Module, Op, ScenarioContext, verify,
};
use larp_source::{SourceFile, SourceId};
use larp_types::{Binary, Effects, Intrinsic, Scalar, Type};

fn root() -> Type {
    Type::class(0, "Founder")
}
fn context() -> ScenarioContext {
    ScenarioContext {
        entry: 3,
        class: 0,
        origin: None,
    }
}
fn unit() -> Op {
    Op::Constant(Scalar::Unit)
}
fn mf(value: i32) -> Op {
    Op::Constant(Scalar::Mf(value))
}
fn instruction(op: Op) -> Instruction {
    let source = SourceFile::new(SourceId(0), "scenarios.larp", "");
    Instruction {
        op,
        span: source.span(0..0).unwrap(),
    }
}
fn function(parameters: Vec<Type>, result: Type, effects: Effects, code: Vec<Op>) -> Function {
    Function {
        scenario: None,
        owner: None,
        constructor: None,
        name: "test".into(),
        locals: parameters.clone(),
        parameters,
        captures: vec![],
        result,
        effects,
        code: code.into_iter().map(instruction).collect(),
        span: instruction(Op::Return).span,
    }
}
fn regional(code: Vec<Op>) -> Function {
    let mut function = function(
        vec![root(), Type::Mf],
        Type::Unit,
        Effects::READ
            .union(Effects::WRITE)
            .union(Effects::OBSERVE)
            .union(Effects::ABORT),
        code,
    );
    function.scenario = Some(context());
    function.locals.push(Type::Unit);
    function
}
fn module() -> Module {
    let mut constructor = function(
        vec![Type::Mf, Type::Unc],
        root(),
        Effects::READ.union(Effects::WRITE).union(Effects::ABORT),
        vec![Op::Load(0), Op::Load(1), Op::NewObject(0)],
    );
    constructor.owner = Some(0);
    constructor.constructor = Some(0);
    let mut getter = function(
        vec![root()],
        Type::Mf,
        Effects::READ,
        vec![Op::Load(0), Op::GetField { class: 0, field: 0 }, Op::Return],
    );
    getter.owner = Some(0);
    Module {
        roles: vec![],
        extensions: vec![],
        entry: 0,
        classes: vec![Class {
            implementations: vec![],
            name: "Founder".into(),
            constructor: 1,
            fields: vec![
                Field {
                    name: "aura".into(),
                    ty: Type::Mf,
                    public: true,
                },
                Field {
                    name: "valuation".into(),
                    ty: Type::Unc,
                    public: false,
                },
            ],
            methods: vec![Method {
                name: "get".into(),
                function: 2,
                public: true,
            }],
        }],
        functions: vec![
            function(
                vec![],
                Type::Unit,
                Effects::ALL,
                vec![
                    mf(7),
                    Op::Constant(Scalar::Unc(20)),
                    Op::Construct(0),
                    mf(4),
                    Op::Scenario {
                        entry: 3,
                        label: "PeterThiel".into(),
                    },
                    Op::Return,
                ],
            ),
            constructor,
            getter,
            regional(vec![
                Op::Load(0),
                mf(49),
                Op::Flex { class: 0, field: 0 },
                Op::Store(2),
                Op::Load(0),
                Op::Canon { class: 0, field: 0 },
                Op::Store(2),
                Op::Load(0),
                Op::Palantir {
                    class: 0,
                    fields: vec![0],
                },
                Op::Store(2),
                unit(),
                Op::Return,
            ]),
        ],
    }
}
fn reject(module: Module, expected: &str) {
    let error = verify(module).unwrap_err();
    assert!(
        error.message.contains(expected),
        "expected {expected}, found {error:?}"
    );
}
fn append(module: &mut Module, index: usize, op: Op) {
    module.functions[index].code.push(instruction(op));
}
fn replace(module: &mut Module, index: usize, code: Vec<Op>) {
    module.functions[index].code = code.into_iter().map(instruction).collect();
}

#[test]
fn complete_scenario_entry_and_dedicated_operations_verify() {
    verify(module()).unwrap();
}

#[test]
fn scenario_root_fields_and_frame_types_exclude_external_references() {
    for ty in [
        Type::array(Type::Mf),
        root(),
        Type::option(root()),
        Type::function(vec![], Type::Mf, Effects::NONE),
        Type::result(Type::Mf, Type::array(Type::Mf)),
    ] {
        let mut bad = module();
        bad.classes[0].fields[0].ty = ty.clone();
        reject(bad, "root fields");
        let mut bad = module();
        bad.functions[3].locals.push(Type::option(ty));
        reject(bad, "external reference");
    }
}

#[test]
fn forged_contexts_roots_entry_captures_and_results_are_rejected() {
    let mut bad = module();
    bad.functions[3].scenario.as_mut().unwrap().entry = usize::MAX;
    reject(bad, "entry index");
    let mut bad = module();
    bad.functions[3].scenario.as_mut().unwrap().class = usize::MAX;
    reject(bad, "root class index");
    let mut bad = module();
    bad.functions[3].scenario.as_mut().unwrap().entry = 2;
    reject(bad, "does not match");
    let mut bad = module();
    bad.functions[3].parameters[0] = Type::Mf;
    reject(bad, "entry requires");
    let mut bad = module();
    bad.functions[3].parameters.push(root());
    reject(bad, "entry requires");
    let mut bad = module();
    bad.functions[3].captures.push(Type::Mf);
    reject(bad, "entry requires");
    let mut bad = module();
    bad.functions[3].result = Type::Mf;
    reject(bad, "entry requires");
    let mut bad = module();
    bad.functions[0].scenario = Some(context());
    reject(bad, "Entry");
}

#[test]
fn ordinary_calls_and_closures_cannot_enter_regions_or_reenter_scenario_entries() {
    for (caller, target, expected) in [
        (0, 3, "cannot cross"),
        (3, 2, "cannot cross"),
        (3, 3, "Scenario instruction"),
    ] {
        for op in [Op::Call(target), Op::MakeClosure(target)] {
            let mut bad = module();
            append(&mut bad, caller, op);
            reject(bad, expected);
        }
    }
    let mut bad = module();
    append(
        &mut bad,
        3,
        Op::Scenario {
            entry: 3,
            label: "Nested".into(),
        },
    );
    reject(bad, "Nested");
    let mut bad = module();
    append(
        &mut bad,
        0,
        Op::Scenario {
            entry: 2,
            label: "Invalid".into(),
        },
    );
    reject(bad, "regional entry");
    let mut bad = module();
    append(
        &mut bad,
        0,
        Op::Scenario {
            entry: usize::MAX,
            label: "Invalid".into(),
        },
    );
    reject(bad, "entry index");
}

#[test]
fn regional_contexts_and_origins_cannot_forge_ordinary_member_capabilities() {
    let mut good = module();
    let mut helper = good.functions[2].clone();
    helper.scenario = Some(ScenarioContext {
        origin: Some(2),
        ..context()
    });
    good.functions.push(helper);
    replace(
        &mut good,
        3,
        vec![Op::Load(0), Op::Call(4), Op::Print, unit(), Op::Return],
    );
    verify(good.clone()).unwrap();
    let mut bad = good.clone();
    bad.classes[0].methods[0].public = false;
    reject(bad, "Private method");
    let mut bad = good.clone();
    bad.functions[4].scenario.as_mut().unwrap().origin = Some(usize::MAX);
    reject(bad, "origin function index");
    let mut bad = good.clone();
    bad.functions[4].scenario.as_mut().unwrap().origin = Some(3);
    reject(bad, "preserve its ordinary origin");
    let mut bad = good.clone();
    bad.functions[4].owner = None;
    reject(bad, "preserve its ordinary origin");
    let mut bad = good;
    bad.classes[0].methods[0].function = 4;
    reject(bad, "registered ordinary members");
    let mut bad = module();
    bad.functions[3].owner = Some(0);
    reject(bad, "lexical ownership");
}

#[test]
fn helpers_have_one_root_value_only_arguments_results_and_snapshot_captures() {
    let mut base = module();
    let mut helper = function(
        vec![root(), Type::Mf],
        Type::Mf,
        Effects::NONE,
        vec![Op::Load(1), Op::Return],
    );
    helper.scenario = Some(context());
    base.functions.push(helper);
    verify(base.clone()).unwrap();
    let mut bad = base.clone();
    bad.functions[4].parameters.push(root());
    reject(bad, "at most one root");
    let mut bad = base.clone();
    bad.functions[4].parameters.push(Type::array(Type::Mf));
    reject(bad, "value-only arguments");
    let mut bad = base.clone();
    bad.functions[4].result = root();
    reject(bad, "cannot return references");
    let mut bad = base.clone();
    bad.functions[4].result = Type::function(vec![], Type::Mf, Effects::NONE);
    reject(bad, "cannot return references");
    let mut bad = base;
    bad.functions[4].captures.push(root());
    reject(bad, "capture only snapshot");
}

#[test]
fn regional_value_closures_are_callable_but_views_cannot_be_bound_as_values() {
    let mut good = module();
    let mut callback = function(
        vec![Type::Mf],
        Type::Mf,
        Effects::NONE,
        vec![Op::Load(1), Op::Return],
    );
    callback.scenario = Some(context());
    callback.captures = vec![Type::Mf];
    callback.locals.push(Type::Mf);
    good.functions.push(callback);
    replace(
        &mut good,
        3,
        vec![
            mf(49),
            Op::MakeClosure(4),
            mf(0),
            Op::CallIndirect(Type::function(vec![Type::Mf], Type::Mf, Effects::NONE)),
            Op::Print,
            unit(),
            Op::Return,
        ],
    );
    verify(good).unwrap();
    let mut bad = module();
    let mut helper = function(
        vec![root()],
        Type::Mf,
        Effects::READ,
        vec![Op::Load(0), Op::GetField { class: 0, field: 0 }, Op::Return],
    );
    helper.scenario = Some(context());
    bad.functions.push(helper);
    append(&mut bad, 3, Op::MakeClosure(4));
    reject(bad, "value-only signatures");
    for signature in [
        Type::function(vec![root()], Type::Mf, Effects::READ),
        Type::function(vec![], root(), Effects::NONE),
        Type::function(vec![], Type::Mf, Effects::IO),
    ] {
        let mut bad = module();
        append(&mut bad, 3, Op::CallIndirect(signature));
        reject(bad, "indirect calls require");
    }
}

#[test]
fn dedicated_operations_obey_context_visibility_indices_and_order() {
    for op in [
        Op::Canon { class: 0, field: 0 },
        Op::Flex { class: 0, field: 0 },
        Op::Palantir {
            class: 0,
            fields: vec![0],
        },
    ] {
        let mut bad = module();
        append(&mut bad, 0, op);
        reject(bad, "regional frame");
    }
    for op in [
        Op::Canon { class: 0, field: 1 },
        Op::Flex { class: 0, field: 1 },
        Op::Palantir {
            class: 0,
            fields: vec![0, 1],
        },
    ] {
        let mut bad = module();
        append(&mut bad, 3, op);
        reject(bad, "Private field");
    }
    for fields in [vec![0, 0], vec![1, 0]] {
        let mut bad = module();
        append(&mut bad, 3, Op::Palantir { class: 0, fields });
        reject(bad, "declaration order");
    }
    let mut bad = module();
    append(&mut bad, 3, Op::Canon { class: 1, field: 0 });
    reject(bad, "active root class");
    let mut bad = module();
    append(
        &mut bad,
        3,
        Op::Flex {
            class: 0,
            field: usize::MAX,
        },
    );
    reject(bad, "Field index");
}

#[test]
fn allocation_role_dispatch_view_identity_and_reference_intrinsics_are_rejected() {
    for op in [
        Op::Construct(0),
        Op::NewObject(0),
        Op::IntoRole { class: 0, role: 0 },
        Op::CallRole { role: 0, method: 0 },
    ] {
        let mut bad = module();
        append(&mut bad, 3, op);
        reject(bad, "Allocation and role");
    }
    let mut bad = module();
    append(&mut bad, 3, Op::Binary(Binary::Equal, root()));
    reject(bad, "first-class identity");
    for intrinsic in [
        Intrinsic::ArrayNew {
            ty: Type::array(Type::Mf),
            count: 0,
        },
        Intrinsic::None(Type::option(root())),
    ] {
        let mut bad = module();
        append(&mut bad, 3, Op::Intrinsic(intrinsic));
        reject(bad, "snapshot values");
    }
    let mut bad = module();
    append(&mut bad, 3, Op::GetField { class: 1, field: 0 });
    reject(bad, "active root class");
}

#[test]
fn scenario_effect_bounds_apply_to_entry_and_unreachable_commit_observations() {
    let mut bad = module();
    bad.functions[3].effects = Effects::ALL;
    reject(bad, "use IO");
    let mut bad = module();
    bad.functions[0].effects = Effects::READ.union(Effects::WRITE).union(Effects::ABORT);
    reject(bad, "outside");
    for op in [
        Op::Canon { class: 0, field: 0 },
        Op::Flex { class: 0, field: 0 },
        Op::Palantir {
            class: 0,
            fields: vec![0],
        },
    ] {
        let mut bad = module();
        replace(&mut bad, 3, vec![unit(), Op::Return, op]);
        bad.functions[3].effects = Effects::NONE;
        reject(bad, "outside");
    }
}

#[test]
fn region_entry_root_cannot_be_overwritten_and_helper_context_cannot_change_regions() {
    let mut bad = module();
    append(&mut bad, 3, Op::Store(0));
    reject(bad, "receiver is read-only");
    let mut bad = module();
    let mut other_entry = regional(vec![unit(), Op::Return]);
    other_entry.scenario = Some(ScenarioContext {
        entry: 4,
        class: 0,
        origin: None,
    });
    bad.functions.push(other_entry);
    append(&mut bad, 3, Op::Call(4));
    reject(bad, "cannot cross");
}
