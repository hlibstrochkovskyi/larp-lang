use larp_source::{SourceFile, SourceId};
use larp_syntax::{ExprKind, StatementKind, TypeExprKind};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId(0), "objects.larp", text)
}

fn parse(text: &str) -> Result<larp_syntax::Program, Vec<larp_diagnostics::Diagnostic>> {
    larp_syntax::parse(&source(text))
}

#[test]
fn classes_interleave_with_functions_and_main_and_preserve_member_visibility_and_spans() {
    let text = r#"
sidequest helper() -> mf { return 1; }
archetype Counter {
    highkey mf value;
    text label;
    lowkey mfs<mf> history;
    init(mf value, text label) {
        self.value = value;
        self.label = label;
        self.history = [];
    }
    highkey sidequest get() -> mf uses(read) { return self.value; }
    sidequest hidden() -> unit {}
    lowkey sidequest alsoHidden() -> unit {}
}
main_character { Counter counter = new Counter(helper(), "🦀"); }
archetype Empty { init() {} }
"#;
    let source = source(text);
    let program = larp_syntax::parse(&source).unwrap();
    assert_eq!(program.functions.len(), 1);
    assert_eq!(program.classes.len(), 2);
    let class = &program.classes[0];
    assert_eq!(class.name.text, "Counter");
    assert_eq!(
        class
            .fields
            .iter()
            .map(|field| field.public)
            .collect::<Vec<_>>(),
        [true, false, false]
    );
    assert_eq!(
        class
            .methods
            .iter()
            .map(|method| method.public)
            .collect::<Vec<_>>(),
        [true, false, false]
    );
    assert_eq!(class.constructors.len(), 1);
    assert_eq!(class.constructors[0].parameters.len(), 2);
    assert_eq!(
        class.methods[0].function.effects.as_ref().unwrap()[0].text,
        "read"
    );
    assert!(
        matches!(&class.fields[2].ty.kind, TypeExprKind::Named { name, arguments } if name.text == "mfs" && arguments.len() == 1)
    );
    assert!(
        source
            .slice(class.span)
            .unwrap()
            .starts_with("archetype Counter {")
    );
    assert!(source.slice(class.span).unwrap().ends_with("\n}"));
    assert!(
        source
            .slice(class.constructors[0].span)
            .unwrap()
            .starts_with("init(mf value, text label)")
    );
    assert_eq!(source.slice(class.fields[0].name.span), Some("value"));
    let StatementKind::Assignment { target, .. } = &class.constructors[0].body.statements[0].kind
    else {
        panic!("Expected a constructor field assignment");
    };
    assert!(
        matches!(&target.kind, ExprKind::Member { receiver, name } if matches!(receiver.kind, ExprKind::SelfValue) && name.text == "value")
    );
    assert_eq!(source.slice(target.span), Some("self.value"));
}

#[test]
fn duplicate_class_members_and_constructors_remain_available_for_semantic_diagnostics() {
    let program = parse("archetype Duplicate { mf x; highkey mf x; init() {} init(mf x) {} sidequest x() -> mf { return 1; } sidequest x() -> mf { return 2; } } archetype Missing {} main_character {}").unwrap();
    let class = &program.classes[0];
    assert_eq!(class.fields.len(), 2);
    assert_eq!(class.methods.len(), 2);
    assert_eq!(class.constructors.len(), 2);
    assert!(program.classes[1].constructors.is_empty());
}

#[test]
fn new_and_self_are_distinct_expression_nodes_with_ordinary_postfix_precedence() {
    let text = "main_character { let value = new Box(new Box(1), self).items[0].get(); self.items[choose()] += 1; self = new Box(); }";
    let source = source(text);
    let program = larp_syntax::parse(&source).unwrap();
    let StatementKind::Binding { value, .. } = &program.main.statements[0].kind else {
        panic!("Expected a binding");
    };
    let ExprKind::Call { callee, .. } = &value.kind else {
        panic!("Expected a postfix call");
    };
    let ExprKind::Member { receiver, name } = &callee.kind else {
        panic!("Expected a method member");
    };
    assert_eq!(name.text, "get");
    let ExprKind::Index { receiver, .. } = &receiver.kind else {
        panic!("Expected indexing");
    };
    let ExprKind::Member { receiver, .. } = &receiver.kind else {
        panic!("Expected an items member");
    };
    let ExprKind::New { class, arguments } = &receiver.kind else {
        panic!("Expected construction");
    };
    assert_eq!(class.text, "Box");
    assert_eq!(arguments.len(), 2);
    assert!(matches!(arguments[0].kind, ExprKind::New { .. }));
    assert!(matches!(arguments[1].kind, ExprKind::SelfValue));
    assert_eq!(
        source.slice(receiver.span),
        Some("new Box(new Box(1), self)")
    );
    assert!(
        matches!(&program.main.statements[2].kind, StatementKind::Assignment { target, .. } if matches!(target.kind, ExprKind::SelfValue))
    );
    assert!(parse("main_character { new Box().value = 1; }").is_err());
}

#[test]
fn unsupported_roles_and_extensions_remain_explicit_and_malformed_members_are_rejected() {
    for text in [
        "archetype C : R { init() {} } main_character {}",
        "role R {} main_character {}",
        "sidequest mf.extension() -> mf { return self; } main_character {}",
    ] {
        let errors = parse(text).unwrap_err();
        assert_eq!(errors[0].code.id, "LARP-P002", "{text}");
    }
    for member in [
        "highkey init() {}",
        "lowkey init() {}",
        "init() uses(read) {}",
        "init() -> unit {}",
        "mf initialized = 1;",
        "highkey lowkey mf value;",
        "sidequest method() -> mf;",
        "sidequest method(mf self) -> unit {}",
        "init(mf) {}",
        "archetype Nested {}",
    ] {
        let errors = parse(&format!("archetype C {{ {member} }} main_character {{}}")).unwrap_err();
        assert!(
            errors.iter().any(|error| error.code.id == "LARP-P001"),
            "{member}"
        );
    }
    for expression in [
        "new",
        "new Box",
        "new Box(,)",
        "new Box(1,)",
        "new Box<mf>()",
        "new self()",
    ] {
        assert!(
            parse(&format!("main_character {{ {expression}; }}")).is_err(),
            "{expression}"
        );
    }
}

#[test]
fn malformed_members_recover_to_later_members_and_main_without_invalid_spans() {
    let text = "archetype C { mf first = 1; mf ; sidequest f() -> unit { mf x = ; } mf second = 2; init() {} } main_character {}";
    let source = source(text);
    let errors = larp_syntax::parse(&source).unwrap_err();
    assert_eq!(errors.len(), 4);
    assert!(errors.iter().all(|error| error.code.id == "LARP-P001"));
    assert!(
        errors
            .iter()
            .all(|error| source.slice(error.primary.span).is_some())
    );
    assert!(
        !errors
            .iter()
            .any(|error| error.primary.message.contains("Missing main_character"))
    );
    assert!(parse("archetype C {} main_character {} main_character {}").is_err());
    assert!(parse("archetype C {}").is_err());
}

#[test]
fn deep_construction_member_chains_and_constructor_lists_obey_parser_budgets() {
    let nested = format!(
        "main_character {{ let x = {}0{}; }}",
        "new Box(".repeat(500),
        ")".repeat(500)
    );
    let chain = format!(
        "main_character {{ let x = self{}; }}",
        ".field".repeat(10_000)
    );
    let arguments = format!(
        "main_character {{ let x = new Box({}0); }}",
        "0,".repeat(10_000)
    );
    let blocks = format!(
        "archetype C {{ init() {{ {} {} }} }} main_character {{}}",
        "{".repeat(500),
        "}".repeat(500)
    );
    for text in [nested, chain, arguments, blocks] {
        let errors = parse(&text).unwrap_err();
        assert!(errors.iter().any(|error| error.code.id == "LARP-P003"));
    }
}

#[test]
fn deterministic_malformed_member_streams_recover_without_panicking() {
    let tokens = [
        "init",
        "sidequest",
        "archetype",
        "mf",
        "highkey",
        "lowkey",
        "self",
        "new",
        "x",
        "0",
        "(",
        ")",
        "{",
        "}",
        ";",
        ",",
        ".",
        "=",
        "->",
        "uses",
        "[",
        "]",
    ];
    let mut state = 49_u64;
    for _ in 0..1000 {
        let mut text = String::from("archetype C { ");
        for _ in 0..64 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push_str(tokens[(state >> 32) as usize % tokens.len()]);
            text.push(' ');
        }
        text.push_str("} main_character {}");
        let _ = parse(&text);
    }
}
