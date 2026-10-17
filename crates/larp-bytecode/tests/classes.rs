use larp_bytecode::{Class, Field, Function, Instruction, Method, Module, Op, verify};
use larp_source::{SourceFile, SourceId};
use larp_types::{Effects, Intrinsic, Scalar, Type};

fn class_type() -> Type {
    Type::class(0, "Counter")
}
fn constructor_effects() -> Effects {
    Effects::READ.union(Effects::WRITE).union(Effects::ABORT)
}
fn instruction(op: Op) -> Instruction {
    let source = SourceFile::new(SourceId(0), "classes.larp", "");
    Instruction {
        op,
        span: source.span(0..0).unwrap(),
    }
}
fn function(parameters: Vec<Type>, result: Type, effects: Effects, code: Vec<Op>) -> Function {
    let span = instruction(Op::Return).span;
    Function {
        owner: None,
        constructor: None,
        name: "test".into(),
        locals: parameters.clone(),
        parameters,
        captures: vec![],
        result,
        effects,
        code: code.into_iter().map(instruction).collect(),
        span,
    }
}
fn entry(code: Vec<Op>) -> Function {
    function(vec![], Type::Unit, Effects::ALL, code)
}
fn unit() -> Op {
    Op::Constant(Scalar::Unit)
}
fn mf(value: i32) -> Op {
    Op::Constant(Scalar::Mf(value))
}
fn module() -> Module {
    let mut constructor = function(
        vec![Type::Mf],
        class_type(),
        constructor_effects(),
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
    Module {
        classes: vec![Class {
            name: "Counter".into(),
            fields: vec![Field {
                name: "value".into(),
                ty: Type::Mf,
                public: false,
            }],
            constructor: 1,
            methods: vec![Method {
                name: "get".into(),
                function: 2,
                public: true,
            }],
        }],
        functions: vec![entry(vec![unit(), Op::Return]), constructor, getter],
        entry: 0,
    }
}
fn reject(module: Module, expected: &str) {
    let error = verify(module).unwrap_err();
    assert!(
        error.message.contains(expected),
        "expected {expected:?}, found {error:?}"
    );
}

#[test]
fn public_calls_construct_fully_initialized_objects_and_private_fields_stay_lexical() {
    let mut module = module();
    module.functions[0] = entry(vec![
        mf(7),
        Op::Construct(0),
        Op::Call(2),
        Op::Print,
        unit(),
        Op::Return,
    ]);
    verify(module.clone()).unwrap();
    for op in [
        Op::GetField { class: 0, field: 0 },
        Op::SetField { class: 0, field: 0 },
    ] {
        let mut forged = module.clone();
        forged.functions[0].code.push(instruction(op));
        reject(forged, "Private field");
    }
    let mut public = module;
    public.classes[0].fields[0].public = true;
    public.functions[0] = entry(vec![
        mf(3),
        Op::Construct(0),
        Op::GetField { class: 0, field: 0 },
        Op::Print,
        unit(),
        Op::Return,
    ]);
    verify(public).unwrap();
}

#[test]
fn private_methods_and_generated_helpers_cannot_be_named_from_outside_the_class() {
    for op in [Op::Call(2), Op::MakeClosure(2)] {
        let mut module = module();
        module.classes[0].methods[0].public = false;
        module.functions[0].code.push(instruction(op));
        reject(module, "Private method");
    }
    let mut module = module();
    module.classes[0].methods.clear();
    module.functions[0]
        .code
        .push(instruction(Op::MakeClosure(2)));
    reject(module, "lexical helper");
}

#[test]
fn exported_private_capabilities_remain_callable_indirectly() {
    let mut module = module();
    let callback = Type::function(vec![], Type::Mf, Effects::READ);
    let mut helper = function(
        vec![],
        Type::Mf,
        Effects::READ,
        vec![Op::Load(0), Op::GetField { class: 0, field: 0 }, Op::Return],
    );
    helper.owner = Some(0);
    helper.captures = vec![class_type()];
    helper.locals = vec![class_type()];
    module.functions.push(helper);
    module.functions[2].result = callback.clone();
    module.functions[2].code = vec![Op::Load(0), Op::MakeClosure(3), Op::Return]
        .into_iter()
        .map(instruction)
        .collect();
    module.functions[0] = entry(vec![
        mf(42),
        Op::Construct(0),
        Op::Call(2),
        Op::CallIndirect(callback),
        Op::Print,
        unit(),
        Op::Return,
    ]);
    verify(module).unwrap();
}

#[test]
fn nominal_ids_and_names_are_checked_inside_every_embedded_type() {
    for forged in [Type::class(1, "Counter"), Type::class(0, "Impostor")] {
        let mut bad_local = module();
        bad_local.functions[0].locals = vec![Type::option(Type::array(forged.clone()))];
        reject(bad_local, "nominal class");
        let mut bad_field = module();
        bad_field.classes[0].fields[0].ty = Type::result(Type::Mf, forged.clone());
        reject(bad_field, "nominal class");
        let mut bad_instruction = module();
        bad_instruction.functions[0]
            .code
            .push(instruction(Op::Intrinsic(Intrinsic::None(Type::option(
                forged,
            )))));
        reject(bad_instruction, "nominal class");
    }
}

#[test]
fn class_member_shapes_owners_and_constructor_contracts_are_validated() {
    let mut duplicate = module();
    let field = duplicate.classes[0].fields[0].clone();
    duplicate.classes[0].fields.push(field);
    reject(duplicate, "Duplicate");
    let mut clash = module();
    clash.classes[0].methods[0].name = "value".into();
    reject(clash, "Duplicate");
    let mut bad_field = module();
    bad_field.classes[0].fields[0].ty = Type::Unit;
    reject(bad_field, "unit type");
    let mut bad_owner = module();
    bad_owner.functions[2].owner = None;
    reject(bad_owner, "method receiver");
    let mut bad_receiver = module();
    bad_receiver.functions[2].parameters = vec![Type::Mf];
    bad_receiver.functions[2].locals = vec![Type::Mf];
    reject(bad_receiver, "method receiver");
    let mut bad_constructor = module();
    bad_constructor.functions[1].effects = Effects::ALL;
    reject(bad_constructor, "constructor signature");
    let mut bad_constructor = module();
    bad_constructor.functions[1].constructor = None;
    reject(bad_constructor, "constructor signature");
    let mut bad_owner = module();
    bad_owner.functions[2].owner = Some(9);
    reject(bad_owner, "owner class");
    let mut bad_entry = module();
    bad_entry.functions[0].owner = Some(0);
    reject(bad_entry, "Entry");
}

#[test]
fn constructor_functions_are_reachable_only_through_construct() {
    for op in [Op::Call(1), Op::MakeClosure(1)] {
        let mut module = module();
        module.functions[0].code.push(instruction(op));
        reject(module, "Construct instruction");
    }
}

#[test]
fn object_completion_is_terminal_and_requires_exactly_all_field_values() {
    for (code, expected) in [
        (vec![Op::NewObject(0)], "underflow"),
        (
            vec![Op::Constant(Scalar::Text("wrong".into())), Op::NewObject(0)],
            "Expected mf",
        ),
        (vec![mf(1), mf(2), Op::NewObject(0)], "extra operands"),
        (vec![Op::Load(0), Op::Return], "terminal object completion"),
        (
            vec![Op::Load(0), Op::NewObject(0), Op::Return],
            "terminal object completion",
        ),
    ] {
        let mut module = module();
        module.functions[1].code = code.into_iter().map(instruction).collect();
        reject(module, expected);
    }
    let mut outside = module();
    outside.functions[0]
        .code
        .push(instruction(Op::NewObject(0)));
    reject(outside, "requires its constructor");
    let mut returning_existing = module();
    returning_existing.functions[1].parameters = vec![class_type()];
    returning_existing.functions[1].locals = vec![class_type()];
    returning_existing.functions[1].code = vec![Op::Load(0), Op::Return]
        .into_iter()
        .map(instruction)
        .collect();
    reject(returning_existing, "terminal object completion");
}

#[test]
fn constructors_cannot_mutate_other_objects_arrays_or_callbacks() {
    let mut field_write = module();
    field_write.functions[1]
        .code
        .push(instruction(Op::SetField { class: 0, field: 0 }));
    reject(field_write, "existing heap");
    let mut push = module();
    push.functions[1]
        .code
        .push(instruction(Op::Intrinsic(Intrinsic::Push(Type::array(
            Type::Mf,
        )))));
    reject(push, "existing heap");
    let callback = Type::function(vec![], Type::Unit, Effects::WRITE);
    let mut indirect = module();
    indirect.functions[1]
        .code
        .push(instruction(Op::CallIndirect(callback)));
    reject(indirect, "existing heap");
    let mut direct = module();
    direct.functions.push(function(
        vec![],
        Type::Unit,
        Effects::WRITE,
        vec![unit(), Op::Return],
    ));
    direct.functions[1].code.push(instruction(Op::Call(3)));
    reject(direct, "existing heap");
    let mut observed = module();
    observed.functions[1].code.push(instruction(Op::Print));
    reject(observed, "outside");
    let mut deferred = module();
    let mut helper = function(vec![], Type::Unit, Effects::WRITE, vec![unit(), Op::Return]);
    helper.owner = Some(0);
    deferred.functions.push(helper);
    deferred.functions[1]
        .code
        .insert(0, instruction(Op::MakeClosure(3)));
    deferred.functions[1]
        .locals
        .push(Type::function(vec![], Type::Unit, Effects::WRITE));
    deferred.functions[1]
        .code
        .insert(1, instruction(Op::Store(1)));
    verify(deferred).unwrap();
}

#[test]
fn constructing_nested_fresh_objects_is_permitted_inside_constructors() {
    let mut module = module();
    let inner = Type::class(1, "Inner");
    module.classes.push(Class {
        name: "Inner".into(),
        fields: vec![],
        constructor: 3,
        methods: vec![],
    });
    let mut constructor = function(
        vec![],
        inner.clone(),
        constructor_effects(),
        vec![Op::NewObject(1)],
    );
    constructor.owner = Some(1);
    constructor.constructor = Some(1);
    module.functions.push(constructor);
    module.functions[1].locals.push(inner);
    module.functions[1]
        .code
        .insert(0, instruction(Op::Construct(1)));
    module.functions[1]
        .code
        .insert(1, instruction(Op::Store(1)));
    verify(module).unwrap();
}

#[test]
fn receiver_reassignment_and_forged_field_layouts_are_rejected() {
    let mut receiver_write = module();
    receiver_write.functions[2]
        .code
        .push(instruction(Op::Store(0)));
    reject(receiver_write, "receiver is read-only");
    for op in [
        Op::GetField { class: 9, field: 0 },
        Op::GetField { class: 0, field: 9 },
    ] {
        let mut invalid = module();
        invalid.functions[0].code.push(instruction(op));
        reject(invalid, "out of bounds");
    }
    let mut mismatch = module();
    mismatch.classes[0].fields[0].public = true;
    mismatch.functions[0] = entry(vec![
        mf(7),
        Op::GetField { class: 0, field: 0 },
        Op::Print,
        unit(),
        Op::Return,
    ]);
    reject(mismatch, "Expected Counter");
    let mut wrong_write = module();
    wrong_write.classes[0].fields[0].public = true;
    wrong_write.functions[0] = entry(vec![
        mf(7),
        Op::Construct(0),
        Op::Constant(Scalar::Text("wrong".into())),
        Op::SetField { class: 0, field: 0 },
        Op::Return,
    ]);
    reject(wrong_write, "Expected mf");
}
