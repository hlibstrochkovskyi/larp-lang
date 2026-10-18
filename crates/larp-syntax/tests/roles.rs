use larp_source::{SourceFile, SourceId};
use larp_syntax::TypeExprKind;

fn parse(text: &str) -> Result<larp_syntax::Program, Vec<larp_diagnostics::Diagnostic>> {
    larp_syntax::parse(&SourceFile::new(SourceId(0), "roles.larp", text))
}

#[test]
fn roles_class_implementation_lists_and_signatures_preserve_order_and_spans() {
    let text = r#"
role Scored {
    sidequest score(mf bonus) -> unc uses(read, abort);
    sidequest observe(fn(mf)->unit uses(observe) sink) -> unit uses(read, observe);
}
main_character {}
archetype Player : Scored, Named, Scored {
    init() {}
    highkey sidequest score(mf bonus) -> unc uses(read, abort) { return 0; }
}
role Named { sidequest name() -> text; }
"#;
    let source = SourceFile::new(SourceId(0), "roles.larp", text);
    let program = larp_syntax::parse(&source).unwrap();
    assert_eq!(program.roles.len(), 2);
    let role = &program.roles[0];
    assert_eq!(role.name.text, "Scored");
    assert_eq!(role.methods.len(), 2);
    assert_eq!(role.methods[0].name.text, "score");
    assert_eq!(role.methods[0].parameters[0].1.text, "bonus");
    assert_eq!(
        role.methods[0]
            .effects
            .as_ref()
            .unwrap()
            .iter()
            .map(|effect| effect.text.as_str())
            .collect::<Vec<_>>(),
        ["read", "abort"]
    );
    assert!(
        matches!(&role.methods[1].parameters[0].0.kind, TypeExprKind::Function { parameters, effects: Some(effects), .. } if parameters.len() == 1 && effects[0].text == "observe")
    );
    assert_eq!(
        source.slice(role.methods[0].span),
        Some("sidequest score(mf bonus) -> unc uses(read, abort);")
    );
    assert!(
        source
            .slice(role.span)
            .unwrap()
            .starts_with("role Scored {")
    );
    assert!(source.slice(role.span).unwrap().ends_with("\n}"));
    assert_eq!(
        program.classes[0]
            .roles
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        ["Scored", "Named", "Scored"]
    );
    assert_eq!(
        source.slice(program.classes[0].roles[1].span),
        Some("Named")
    );
    assert!(program.classes[0].methods[0].function.receiver.is_none());
    assert!(program.roles[1].methods[0].effects.is_none());
}

#[test]
fn concrete_extensions_support_scalar_generic_fixed_and_function_receivers() {
    let text = r#"
sidequest ordinary(mf x) -> mf { return x; }
sidequest mf.doubled() -> mf { return self * 2; }
sidequest mfs<unc>.last() -> unc uses(read, abort) { return self[0]; }
sidequest hediJeans<mf,2>.first() -> mf { return self[0]; }
sidequest fn(mf)->unc uses(read).apply(mf x) -> unc uses(read) { return self(x); }
main_character {}
"#;
    let source = SourceFile::new(SourceId(0), "extensions.larp", text);
    let program = larp_syntax::parse(&source).unwrap();
    assert_eq!(program.functions.len(), 5);
    assert!(program.functions[0].receiver.is_none());
    let expected = [
        "mf",
        "mfs<unc>",
        "hediJeans<mf,2>",
        "fn(mf)->unc uses(read)",
    ];
    for (function, expected) in program.functions[1..].iter().zip(expected) {
        assert_eq!(
            source.slice(function.receiver.as_ref().unwrap().span),
            Some(expected)
        );
    }
    assert_eq!(program.functions[1].name.text, "doubled");
    assert!(
        matches!(&program.functions[3].receiver.as_ref().unwrap().kind, TypeExprKind::Named { arguments, .. } if matches!(&arguments[1].kind, TypeExprKind::Integer(value) if value == "2"))
    );
    assert!(
        matches!(&program.functions[4].receiver.as_ref().unwrap().kind, TypeExprKind::Function { result, effects: Some(effects), .. } if matches!(&result.kind, TypeExprKind::Named { name, .. } if name.text == "unc") && effects[0].text == "read")
    );
    assert_eq!(
        program.functions[4].effects.as_ref().unwrap()[0].text,
        "read"
    );
}

#[test]
fn duplicate_roles_signatures_extensions_and_implementation_names_remain_semantic_inputs() {
    let program = parse("role R { sidequest f() -> unit; sidequest f() -> unit; } role R {} archetype C : R,R { init() {} } sidequest mf.f() -> unit {} sidequest mf.f() -> unit {} main_character {}").unwrap();
    assert_eq!(program.roles.len(), 2);
    assert_eq!(program.roles[0].methods.len(), 2);
    assert_eq!(program.classes[0].roles.len(), 2);
    assert_eq!(program.functions.len(), 2);
}

#[test]
fn role_bodies_fields_visibility_inheritance_and_nested_extensions_are_rejected() {
    for text in [
        "role R { mf field; } main_character {}",
        "role R { highkey sidequest f() -> unit; } main_character {}",
        "role R { lowkey sidequest f() -> unit; } main_character {}",
        "role R { sidequest f() -> unit {} } main_character {}",
        "role R { init() {} } main_character {}",
        "role R : Parent {} main_character {}",
        "role R<T> {} main_character {}",
        "role R { sidequest mf.f() -> unit; } main_character {}",
        "role R { sidequest f() -> unit } main_character {}",
        "archetype C : { init() {} } main_character {}",
        "archetype C : R, { init() {} } main_character {}",
        "archetype C : R<R> { init() {} } main_character {}",
        "archetype C { init() {} sidequest mf.f() -> unit {} } main_character {}",
        "sidequest mfs<mf>.f<T>() -> unit {} main_character {}",
        "sidequest mf.() -> unit {} main_character {}",
        "sidequest mf.f() -> unit; main_character {}",
        "sidequest 3.f() -> unit {} main_character {}",
    ] {
        let source = SourceFile::new(SourceId(0), "invalid-role.larp", text);
        let errors = larp_syntax::parse(&source).unwrap_err();
        assert!(
            errors.iter().any(|error| error.code.id == "LARP-P001"),
            "{text}"
        );
        assert!(
            errors
                .iter()
                .all(|error| source.slice(error.primary.span).is_some())
        );
    }
}

#[test]
fn malformed_role_members_recover_and_receiver_type_nesting_is_bounded() {
    let text = "role R { mf field; sidequest broken(mf) -> unit; sidequest okay() -> unit; } main_character {}";
    let errors = parse(text).unwrap_err();
    assert_eq!(errors.len(), 2);
    assert!(errors.iter().all(|error| error.code.id == "LARP-P001"));
    assert!(
        !errors
            .iter()
            .any(|error| error.primary.message.contains("Missing main_character"))
    );
    let nested_receiver = format!(
        "sidequest {}mf{}.f() -> unit {{}} main_character {{}}",
        "mfs<".repeat(500),
        ">".repeat(500)
    );
    let nested_role_type = format!(
        "role R {{ sidequest f({}mf{} value) -> unit; }} main_character {{}}",
        "mfs<".repeat(500),
        ">".repeat(500)
    );
    for text in [nested_receiver, nested_role_type] {
        assert!(
            parse(&text)
                .unwrap_err()
                .iter()
                .any(|error| error.code.id == "LARP-P003")
        );
    }
}
