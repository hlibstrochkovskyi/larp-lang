use larp_bytecode::{Class, Field, Function, Instruction, Method, Module, Op, verify};
use larp_source::{SourceFile, SourceId};
use larp_types::{Binary, Effects, Intrinsic, Scalar, Type};
use larp_vm::{Limits, execute_with_stats};

fn counter() -> Type {
    Type::class(0, "Counter")
}
fn ctor_effects() -> Effects {
    Effects::READ.union(Effects::WRITE).union(Effects::ABORT)
}
fn function(
    parameters: Vec<Type>,
    captures: Vec<Type>,
    extra: Vec<Type>,
    result: Type,
    effects: Effects,
    code: Vec<Op>,
) -> Function {
    let source = SourceFile::new(SourceId(0), "objects.larp", "");
    let span = source.span(0..0).unwrap();
    let locals = parameters
        .iter()
        .chain(&captures)
        .chain(&extra)
        .cloned()
        .collect();
    Function {
        owner: None,
        constructor: None,
        name: "test".into(),
        locals,
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
fn mf(value: i32) -> Op {
    Op::Constant(Scalar::Mf(value))
}
fn unit() -> Op {
    Op::Constant(Scalar::Unit)
}
fn intrinsic(operation: Intrinsic) -> Op {
    Op::Intrinsic(operation)
}
fn module(main: Function) -> Module {
    let mut constructor = function(
        vec![Type::Mf],
        vec![],
        vec![],
        counter(),
        ctor_effects(),
        vec![Op::Load(0), Op::NewObject(0)],
    );
    constructor.owner = Some(0);
    constructor.constructor = Some(0);
    let mut setter = function(
        vec![counter(), Type::Mf],
        vec![],
        vec![],
        Type::Unit,
        Effects::WRITE,
        vec![
            Op::Load(0),
            Op::Load(1),
            Op::SetField { class: 0, field: 0 },
            Op::Return,
        ],
    );
    setter.owner = Some(0);
    let callback = Type::function(vec![], Type::Mf, Effects::READ);
    let mut export = function(
        vec![counter()],
        vec![],
        vec![],
        callback,
        Effects::NONE,
        vec![Op::Load(0), Op::MakeClosure(4), Op::Return],
    );
    export.owner = Some(0);
    let mut helper = function(
        vec![],
        vec![counter()],
        vec![],
        Type::Mf,
        Effects::READ,
        vec![Op::Load(0), Op::GetField { class: 0, field: 0 }, Op::Return],
    );
    helper.owner = Some(0);
    Module {
        roles: vec![],
        extensions: vec![],
        classes: vec![Class {
            implementations: vec![],
            name: "Counter".into(),
            constructor: 1,
            fields: vec![Field {
                name: "value".into(),
                ty: Type::Mf,
                public: false,
            }],
            methods: vec![
                Method {
                    name: "set".into(),
                    function: 2,
                    public: true,
                },
                Method {
                    name: "export".into(),
                    function: 3,
                    public: true,
                },
            ],
        }],
        functions: vec![main, constructor, setter, export, helper],
        entry: 0,
    }
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
        assert_eq!(String::from_utf8(output).unwrap(), expected);
        assert_eq!(stats.allocations, stats.reclaimed);
    }
}

#[test]
fn aliases_share_fields_and_exported_private_closures_keep_object_identity() {
    let callback = Type::function(vec![], Type::Mf, Effects::READ);
    run(
        module(entry(
            vec![counter(), counter(), Type::Unit, callback.clone()],
            vec![
                mf(7),
                Op::Construct(0),
                Op::Store(0),
                Op::Load(0),
                Op::Store(1),
                Op::Load(1),
                mf(8),
                Op::Call(2),
                Op::Store(2),
                Op::Load(0),
                Op::Call(3),
                Op::Store(3),
                Op::Load(0),
                Op::Load(1),
                Op::Binary(Binary::Equal, counter()),
                Op::Print,
                Op::Load(0),
                mf(8),
                Op::Construct(0),
                Op::Binary(Binary::Equal, counter()),
                Op::Print,
                Op::Load(3),
                Op::CallIndirect(callback.clone()),
                Op::Print,
                Op::Load(1),
                mf(9),
                Op::Call(2),
                Op::Store(2),
                Op::Load(3),
                Op::CallIndirect(callback),
                Op::Print,
                unit(),
                Op::Return,
            ],
        )),
        "fr\ncap\n8\n9\n",
    );
}

#[test]
fn object_fields_and_constructor_temporaries_are_roots_during_nested_allocations() {
    let holder = Type::class(1, "Holder");
    let callback = Type::function(vec![], Type::Mf, Effects::READ);
    let array = Type::array(counter());
    let mut module = module(entry(
        vec![holder.clone()],
        vec![
            mf(11),
            Op::Construct(0),
            mf(22),
            Op::Construct(0),
            Op::Call(3),
            mf(33),
            Op::Construct(0),
            intrinsic(Intrinsic::ArrayNew {
                ty: array.clone(),
                count: 1,
            }),
            Op::Construct(1),
            Op::Store(0),
            Op::Load(0),
            Op::GetField { class: 1, field: 0 },
            Op::Call(3),
            Op::CallIndirect(callback.clone()),
            Op::Print,
            Op::Load(0),
            Op::GetField { class: 1, field: 1 },
            Op::CallIndirect(callback.clone()),
            Op::Print,
            Op::Load(0),
            Op::GetField { class: 1, field: 2 },
            mf(0),
            intrinsic(Intrinsic::Index(array.clone())),
            Op::Call(3),
            Op::CallIndirect(callback.clone()),
            Op::Print,
            unit(),
            Op::Return,
        ],
    ));
    module.classes.push(Class {
        implementations: vec![],
        name: "Holder".into(),
        constructor: 5,
        fields: vec![
            Field {
                name: "child".into(),
                ty: counter(),
                public: true,
            },
            Field {
                name: "callback".into(),
                ty: callback.clone(),
                public: true,
            },
            Field {
                name: "array".into(),
                ty: array.clone(),
                public: true,
            },
        ],
        methods: vec![],
    });
    let mut constructor = function(
        vec![counter(), callback, array],
        vec![],
        vec![counter()],
        holder,
        ctor_effects(),
        vec![
            mf(99),
            Op::Construct(0),
            Op::Store(3),
            Op::Load(0),
            Op::Load(1),
            Op::Load(2),
            Op::NewObject(1),
        ],
    );
    constructor.owner = Some(1);
    constructor.constructor = Some(1);
    module.functions.push(constructor);
    run(module, "11\n22\n33\n");
}

#[test]
fn aborting_constructor_returns_no_object_and_emits_no_following_observation() {
    let mut module = module(entry(
        vec![counter()],
        vec![
            mf(7),
            Op::Construct(0),
            Op::Store(0),
            Op::Constant(Scalar::Text("after".into())),
            Op::Print,
            unit(),
            Op::Return,
        ],
    ));
    let span = module.functions[1].span;
    module.functions[1].code = vec![
        Op::Constant(Scalar::Text("construction failed".into())),
        Op::Crash,
    ]
    .into_iter()
    .map(|op| Instruction { op, span })
    .collect();
    let module = verify(module).unwrap();
    let mut output = Vec::new();
    let error = execute_with_stats(
        &module,
        &mut output,
        Limits {
            gc_stress: true,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.code.id, "LARP-R003");
    assert_eq!(error.message, "construction failed");
    assert!(output.is_empty());
}
