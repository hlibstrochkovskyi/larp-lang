use larp_bytecode::{
    Class, Extension, Field, Function, Implementation, Instruction, Method, Module, Op, Role,
    RoleMethod, verify,
};
use larp_source::{SourceFile, SourceId};
use larp_types::{Effects, Type};

fn class_type() -> Type {
    Type::class(0, "Counter")
}

fn role_type() -> Type {
    Type::role(0, "Readable")
}

fn instruction(op: Op) -> Instruction {
    let source = SourceFile::new(SourceId(0), "roles.larp", "");
    Instruction {
        op,
        span: source.span(0..0).unwrap(),
    }
}

fn function(parameters: Vec<Type>, result: Type, effects: Effects, code: Vec<Op>) -> Function {
    Function {
        scenario: None,
        name: "test".into(),
        owner: None,
        constructor: None,
        locals: parameters.clone(),
        parameters,
        captures: vec![],
        result,
        effects,
        code: code.into_iter().map(instruction).collect(),
        span: instruction(Op::Return).span,
    }
}

fn module() -> Module {
    let main = function(
        vec![],
        Type::Unit,
        Effects::ALL,
        vec![Op::Constant(larp_types::Scalar::Unit), Op::Return],
    );
    let mut constructor = function(
        vec![Type::Mf],
        class_type(),
        Effects::READ.union(Effects::WRITE).union(Effects::ABORT),
        vec![Op::Load(0), Op::NewObject(0)],
    );
    constructor.owner = Some(0);
    constructor.constructor = Some(0);
    let mut getter = function(
        vec![class_type()],
        Type::Mf,
        Effects::READ,
        vec![Op::Load(0), Op::GetField { class: 0, field: 0 }, Op::Return],
    );
    getter.owner = Some(0);
    let extension = function(
        vec![Type::Mf],
        Type::Mf,
        Effects::NONE,
        vec![Op::Load(0), Op::Return],
    );
    Module {
        entry: 0,
        roles: vec![Role {
            name: "Readable".into(),
            methods: vec![RoleMethod {
                name: "get".into(),
                parameters: vec![],
                result: Type::Mf,
                effects: Effects::READ,
            }],
        }],
        extensions: vec![Extension {
            receiver: Type::Mf,
            name: "echo".into(),
            function: 3,
        }],
        classes: vec![Class {
            name: "Counter".into(),
            constructor: 1,
            implementations: vec![Implementation {
                role: 0,
                methods: vec![2],
            }],
            fields: vec![Field {
                name: "value".into(),
                ty: Type::Mf,
                public: false,
            }],
            methods: vec![Method {
                name: "get".into(),
                function: 2,
                public: true,
            }],
        }],
        functions: vec![main, constructor, getter, extension],
    }
}

fn reject(module: Module, expected: &str) {
    let error = verify(module).unwrap_err();
    assert!(
        error.message.contains(expected),
        "expected {expected}, found {error:?}"
    );
}

fn append(module: &mut Module, op: Op) {
    module.functions[0].code.push(instruction(op));
}

#[test]
fn roles_require_complete_public_methods_of_the_declared_name() {
    verify(module()).unwrap();
    for target in [1, 3, usize::MAX] {
        let mut bad = module();
        bad.classes[0].implementations[0].methods[0] = target;
        reject(bad, "public instance method");
    }
    let mut bad = module();
    bad.classes[0].methods[0].public = false;
    reject(bad, "public instance method");
    let mut bad = module();
    bad.roles[0].methods[0].name = "other".into();
    reject(bad, "public instance method");
    let mut bad = module();
    bad.classes[0].implementations[0].methods.clear();
    reject(bad, "incomplete");
    let mut bad = module();
    bad.classes[0].implementations[0].methods.push(2);
    reject(bad, "incomplete");
}

#[test]
fn role_implementation_signatures_are_invariant_and_effects_can_only_narrow() {
    let mut allowed = module();
    allowed.roles[0].methods[0].effects = Effects::ALL;
    verify(allowed).unwrap();
    let mut bad = module();
    bad.roles[0].methods[0].effects = Effects::NONE;
    reject(bad, "signature or effects");
    let mut bad = module();
    bad.roles[0].methods[0].parameters = vec![Type::Mf];
    reject(bad, "signature or effects");
    let mut bad = module();
    bad.roles[0].methods[0].result = Type::Unc;
    reject(bad, "signature or effects");
}

#[test]
fn duplicate_roles_methods_implementations_and_nominal_names_are_rejected() {
    let mut bad = module();
    bad.roles.push(bad.roles[0].clone());
    reject(bad, "Duplicate");
    let mut bad = module();
    bad.roles[0].name = "Counter".into();
    reject(bad, "Duplicate");
    let mut bad = module();
    let duplicate = bad.roles[0].methods[0].clone();
    bad.roles[0].methods.push(duplicate);
    reject(bad, "role method");
    let mut bad = module();
    bad.roles[0].methods[0].name.clear();
    reject(bad, "role method");
    let mut bad = module();
    let duplicate = bad.classes[0].implementations[0].clone();
    bad.classes[0].implementations.push(duplicate);
    reject(bad, "Duplicate");
    let mut bad = module();
    bad.classes[0].implementations[0].role = usize::MAX;
    reject(bad, "implementation index");
}

#[test]
fn nominal_roles_are_validated_inside_signatures_containers_and_intrinsics() {
    for forged in [Type::role(5, "Readable"), Type::role(0, "Impostor")] {
        let mut bad = module();
        bad.roles[0].methods[0].result = Type::option(forged.clone());
        reject(bad, "nominal role");
        let mut bad = module();
        bad.functions[0].locals.push(Type::array(forged.clone()));
        reject(bad, "nominal role");
        let mut bad = module();
        append(
            &mut bad,
            Op::Intrinsic(larp_types::Intrinsic::None(Type::option(forged))),
        );
        reject(bad, "nominal role");
    }
    let mut bad = module();
    bad.roles[0].methods[0].parameters = vec![Type::Unit];
    reject(bad, "Malformed type");
}

#[test]
fn unreachable_role_instructions_still_check_indices_implementations_and_effects() {
    for op in [
        Op::IntoRole {
            class: usize::MAX,
            role: 0,
        },
        Op::CallRole {
            role: usize::MAX,
            method: 0,
        },
        Op::CallRole {
            role: 0,
            method: usize::MAX,
        },
    ] {
        let mut bad = module();
        append(&mut bad, op);
        reject(bad, "out of bounds");
    }
    let mut bad = module();
    bad.classes[0].implementations.clear();
    append(&mut bad, Op::IntoRole { class: 0, role: 0 });
    reject(bad, "does not implement");
    let mut bad = module();
    bad.functions[0].effects = Effects::NONE;
    append(&mut bad, Op::CallRole { role: 0, method: 0 });
    reject(bad, "outside");
}

#[test]
fn role_conversion_and_dispatch_require_exact_receiver_types() {
    let mut bad = module();
    bad.functions[0].code = vec![
        Op::Constant(larp_types::Scalar::Mf(1)),
        Op::IntoRole { class: 0, role: 0 },
    ]
    .into_iter()
    .map(instruction)
    .collect();
    reject(bad, "Expected Counter");
    let mut bad = module();
    bad.functions[0].code = vec![
        Op::Constant(larp_types::Scalar::Mf(1)),
        Op::Construct(0),
        Op::CallRole { role: 0, method: 0 },
    ]
    .into_iter()
    .map(instruction)
    .collect();
    reject(bad, "Expected Readable");
    let mut bad = module();
    bad.roles.push(Role {
        name: "Other".into(),
        methods: bad.roles[0].methods.clone(),
    });
    bad.classes[0].implementations.push(Implementation {
        role: 1,
        methods: vec![2],
    });
    bad.functions[0].code = vec![
        Op::Constant(larp_types::Scalar::Mf(1)),
        Op::Construct(0),
        Op::IntoRole { class: 0, role: 1 },
        Op::CallRole { role: 0, method: 0 },
    ]
    .into_iter()
    .map(instruction)
    .collect();
    reject(bad, "Expected Readable, found Other");
}

#[test]
fn role_calls_cannot_hide_constructor_writes_or_public_effect_bounds() {
    let mut bad = module();
    bad.roles[0].methods[0].effects = Effects::WRITE;
    bad.functions[2].effects = Effects::WRITE;
    bad.functions[2].code = vec![Op::Constant(larp_types::Scalar::Mf(1)), Op::Return]
        .into_iter()
        .map(instruction)
        .collect();
    bad.functions[1]
        .code
        .push(instruction(Op::CallRole { role: 0, method: 0 }));
    reject(bad, "existing heap");
    let mut bad = module();
    bad.roles[0].methods[0].effects = Effects::ALL;
    bad.functions[0].effects = Effects::READ;
    append(&mut bad, Op::CallRole { role: 0, method: 0 });
    reject(bad, "outside");
}

#[test]
fn extensions_validate_receiver_targets_ownership_captures_and_uniqueness() {
    let mut bad = module();
    bad.extensions[0].function = usize::MAX;
    reject(bad, "Extension function index");
    let mut bad = module();
    bad.extensions[0].receiver = Type::Unc;
    reject(bad, "receiver or ownership");
    let mut bad = module();
    bad.extensions[0].function = 2;
    bad.extensions[0].receiver = class_type();
    reject(bad, "receiver or ownership");
    let mut bad = module();
    bad.functions[3].owner = Some(0);
    reject(bad, "receiver or ownership");
    let mut bad = module();
    bad.functions[3].captures = vec![Type::Mf];
    bad.functions[3].locals.push(Type::Mf);
    reject(bad, "receiver or ownership");
    let mut bad = module();
    bad.extensions.push(bad.extensions[0].clone());
    reject(bad, "Duplicate");
    let mut bad = module();
    bad.extensions.push(Extension {
        receiver: Type::Mf,
        name: "different".into(),
        function: 3,
    });
    reject(bad, "Duplicate");
    let mut bad = module();
    bad.extensions[0].name.clear();
    reject(bad, "empty extension");
}

#[test]
fn extensions_cannot_reassign_receivers_or_gain_private_access() {
    let mut bad = module();
    bad.functions[3].code.push(instruction(Op::Store(0)));
    reject(bad, "receiver is read-only");
    let mut bad = module();
    bad.extensions[0].receiver = class_type();
    bad.functions[3].parameters = vec![class_type()];
    bad.functions[3].locals = vec![class_type()];
    bad.functions[3].effects = Effects::READ;
    bad.functions[3].code = vec![Op::Load(0), Op::GetField { class: 0, field: 0 }, Op::Return]
        .into_iter()
        .map(instruction)
        .collect();
    reject(bad, "Private field");
}

#[test]
fn structural_role_signatures_reject_excessive_type_work_before_comparisons() {
    let mut bad = module();
    let mut huge = Type::Mf;
    for _ in 0..30 {
        huge = Type::result(huge.clone(), huge.clone());
    }
    bad.roles[0].methods[0].result = huge;
    reject(bad, "complexity");
    let mut bad = module();
    bad.extensions[0].receiver = Type::option(role_type());
    reject(bad, "receiver or ownership");
}
