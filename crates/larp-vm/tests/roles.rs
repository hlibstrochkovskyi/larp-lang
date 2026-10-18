use larp_bytecode::{
    Class, Extension, Field, Function, Implementation, Instruction, Method, Module, Op, Role,
    RoleMethod, verify,
};
use larp_source::{SourceFile, SourceId};
use larp_types::{Binary, Effects, Intrinsic, Scalar, Type};
use larp_vm::{Limits, execute_with_stats};

fn class_type(class: usize) -> Type {
    Type::class(class, if class == 0 { "Counter" } else { "Other" })
}

fn reader() -> Type {
    Type::role(0, "Readable")
}
fn writer() -> Type {
    Type::role(1, "Writable")
}
fn mf(value: i32) -> Op {
    Op::Constant(Scalar::Mf(value))
}
fn unit() -> Op {
    Op::Constant(Scalar::Unit)
}
fn into_reader(class: usize) -> Op {
    Op::IntoRole { class, role: 0 }
}
fn read() -> Op {
    Op::CallRole { role: 0, method: 0 }
}

fn function(
    parameters: Vec<Type>,
    captures: Vec<Type>,
    extra: Vec<Type>,
    result: Type,
    effects: Effects,
    code: Vec<Op>,
) -> Function {
    let source = SourceFile::new(SourceId(0), "roles.larp", "role dispatch");
    let span = source.span(0..source.text().len()).unwrap();
    Function {
        scenario: None,
        name: "test".into(),
        owner: None,
        constructor: None,
        locals: parameters
            .iter()
            .chain(&captures)
            .chain(&extra)
            .cloned()
            .collect(),
        parameters,
        captures,
        result,
        effects,
        code: code
            .into_iter()
            .map(|op| Instruction { op, span })
            .collect(),
        span,
    }
}

fn entry(locals: Vec<Type>, code: Vec<Op>) -> Function {
    function(vec![], vec![], locals, Type::Unit, Effects::ALL, code)
}

fn module(main: Function) -> Module {
    let mut module = Module {
        entry: 0,
        roles: vec![
            Role {
                name: "Readable".into(),
                methods: vec![RoleMethod {
                    name: "get".into(),
                    parameters: vec![],
                    result: Type::Mf,
                    effects: Effects::READ,
                }],
            },
            Role {
                name: "Writable".into(),
                methods: vec![RoleMethod {
                    name: "set".into(),
                    parameters: vec![Type::Mf],
                    result: Type::Unit,
                    effects: Effects::WRITE,
                }],
            },
        ],
        extensions: vec![],
        classes: vec![],
        functions: vec![main],
    };
    for class in 0..2 {
        let constructor = module.functions.len();
        let getter = constructor + 1;
        let setter = constructor + 2;
        module.classes.push(Class {
            name: if class == 0 {
                "Counter".into()
            } else {
                "Other".into()
            },
            constructor,
            implementations: vec![
                Implementation {
                    role: 0,
                    methods: vec![getter],
                },
                Implementation {
                    role: 1,
                    methods: vec![setter],
                },
            ],
            fields: vec![Field {
                name: "value".into(),
                ty: Type::Mf,
                public: false,
            }],
            methods: vec![
                Method {
                    name: "get".into(),
                    function: getter,
                    public: true,
                },
                Method {
                    name: "set".into(),
                    function: setter,
                    public: true,
                },
            ],
        });
        let mut constructor = function(
            vec![Type::Mf],
            vec![],
            vec![],
            class_type(class),
            Effects::READ.union(Effects::WRITE).union(Effects::ABORT),
            vec![Op::Load(0), Op::NewObject(class)],
        );
        constructor.owner = Some(class);
        constructor.constructor = Some(class);
        let mut getter = function(
            vec![class_type(class)],
            vec![],
            vec![],
            Type::Mf,
            Effects::READ,
            vec![Op::Load(0), Op::GetField { class, field: 0 }, Op::Return],
        );
        getter.owner = Some(class);
        let mut setter = function(
            vec![class_type(class), Type::Mf],
            vec![],
            vec![],
            Type::Unit,
            Effects::WRITE,
            vec![
                Op::Load(0),
                Op::Load(1),
                Op::SetField { class, field: 0 },
                Op::Return,
            ],
        );
        setter.owner = Some(class);
        module.functions.extend([constructor, getter, setter]);
    }
    module
}

fn run(module: Module, expected: &str) {
    let module = verify(module).unwrap();
    for gc_stress in [false, true] {
        let mut output = Vec::new();
        let stats = execute_with_stats(
            &module,
            &mut output,
            Limits {
                gc_stress,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            expected,
            "gc_stress={gc_stress}"
        );
        assert_eq!(stats.allocations, stats.reclaimed);
    }
}

#[test]
fn dynamic_dispatch_preserves_class_identity_and_mutations_across_role_aliases() {
    run(
        module(entry(
            vec![class_type(0), reader(), writer(), reader(), Type::Unit],
            vec![
                mf(7),
                Op::Construct(0),
                Op::Store(0),
                Op::Load(0),
                into_reader(0),
                Op::Store(1),
                Op::Load(0),
                Op::IntoRole { class: 0, role: 1 },
                Op::Store(2),
                mf(9),
                Op::Construct(1),
                into_reader(1),
                Op::Store(3),
                Op::Load(1),
                read(),
                Op::Print,
                Op::Load(3),
                read(),
                Op::Print,
                Op::Load(2),
                mf(42),
                Op::CallRole { role: 1, method: 0 },
                Op::Store(4),
                Op::Load(1),
                read(),
                Op::Print,
                Op::Load(0),
                into_reader(0),
                Op::Load(1),
                Op::Binary(Binary::Equal, reader()),
                Op::Print,
                Op::Load(1),
                mf(42),
                Op::Construct(0),
                into_reader(0),
                Op::Binary(Binary::Equal, reader()),
                Op::Print,
                unit(),
                Op::Return,
            ],
        )),
        "7\n9\n42\nfr\ncap\n",
    );
}

#[test]
fn roles_nested_in_result_captures_and_array_elements_remain_gc_roots() {
    let result = Type::result(reader(), Type::Text);
    let array = Type::array(reader());
    let callback = Type::function(vec![], Type::Mf, Effects::READ.union(Effects::ABORT));
    let mut module = module(entry(
        vec![callback.clone(), array.clone(), class_type(0)],
        vec![
            mf(49),
            Op::Construct(0),
            into_reader(0),
            Op::Intrinsic(Intrinsic::Ok(result.clone())),
            Op::MakeClosure(7),
            Op::Store(0),
            mf(91),
            Op::Construct(1),
            into_reader(1),
            Op::Intrinsic(Intrinsic::ArrayNew {
                ty: array.clone(),
                count: 1,
            }),
            Op::Store(1),
            mf(-1),
            Op::Construct(0),
            Op::Store(2),
            Op::Load(0),
            Op::CallIndirect(callback),
            Op::Print,
            Op::Load(1),
            mf(0),
            Op::Intrinsic(Intrinsic::Index(array)),
            read(),
            Op::Print,
            unit(),
            Op::Return,
        ],
    ));
    module.functions.push(function(
        vec![],
        vec![result.clone()],
        vec![],
        Type::Mf,
        Effects::READ.union(Effects::ABORT),
        vec![
            Op::Load(0),
            Op::MatchResult {
                ty: result,
                success: 2,
                error: 4,
            },
            read(),
            Op::Return,
            Op::Crash,
        ],
    ));
    run(module, "49\n91\n");
}

#[test]
fn temporary_role_receivers_on_the_operand_stack_survive_allocation_safepoints() {
    run(
        module(entry(
            vec![class_type(1)],
            vec![
                mf(49),
                Op::Construct(0),
                into_reader(0),
                mf(0),
                Op::Construct(1),
                Op::Store(0),
                read(),
                Op::Print,
                unit(),
                Op::Return,
            ],
        )),
        "49\n",
    );
}

#[test]
fn recursive_role_dispatch_obeys_call_frame_limits_with_the_call_site_span() {
    let mut module = module(entry(
        vec![],
        vec![
            mf(1),
            Op::Construct(0),
            into_reader(0),
            read(),
            Op::Print,
            unit(),
            Op::Return,
        ],
    ));
    let span = module.functions[2].span;
    module.functions[2].code = vec![Op::Load(0), into_reader(0), read(), Op::Return]
        .into_iter()
        .map(|op| Instruction { op, span })
        .collect();
    let module = verify(module).unwrap();
    for gc_stress in [false, true] {
        let mut output = Vec::new();
        let error = execute_with_stats(
            &module,
            &mut output,
            Limits {
                call_frames: 4,
                gc_stress,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.code.id, "LARP-R006");
        assert_eq!(error.primary.span, span);
        assert!(error.message.contains("Call-frame"));
        assert!(output.is_empty());
    }
}

#[test]
fn concrete_role_extensions_execute_as_direct_or_first_class_functions() {
    let signature = Type::function(vec![reader()], Type::Mf, Effects::READ);
    let mut module = module(entry(
        vec![reader()],
        vec![
            mf(49),
            Op::Construct(0),
            into_reader(0),
            Op::Store(0),
            Op::Load(0),
            Op::Call(7),
            Op::Print,
            Op::MakeClosure(7),
            Op::Load(0),
            Op::CallIndirect(signature),
            Op::Print,
            unit(),
            Op::Return,
        ],
    ));
    module.extensions.push(Extension {
        receiver: reader(),
        name: "value".into(),
        function: 7,
    });
    module.functions.push(function(
        vec![reader()],
        vec![],
        vec![],
        Type::Mf,
        Effects::READ,
        vec![Op::Load(0), read(), Op::Return],
    ));
    run(module, "49\n49\n");
}
