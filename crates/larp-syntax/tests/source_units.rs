use larp_source::{SourceFile, SourceId};
use larp_syntax::{ExprKind, StatementKind, TypeExpr, TypeExprKind, parse, parse_unit};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId(49), "project/unit.larp", text)
}

fn named(ty: &TypeExpr) -> (&larp_syntax::Name, &[TypeExpr]) {
    let TypeExprKind::Named { name, arguments } = &ty.kind else {
        panic!("Expected a named type");
    };
    (name, arguments)
}

#[test]
fn imports_decode_paths_and_preserve_original_nonzero_source_spans() {
    let file = source(
        "// 🦀\nimport /* path */ \"math\\tfile.larp\" as math;\nimport \"more.larp\" as more;",
    );
    let unit = parse_unit(&file).unwrap();
    assert!(unit.main.is_none());
    assert_eq!(unit.imports.len(), 2);
    let import = &unit.imports[0];
    assert_eq!(import.path, "math\tfile.larp");
    assert_eq!(file.slice(import.path_span), Some("\"math\\tfile.larp\""));
    assert_eq!(file.slice(import.alias.span), Some("math"));
    assert_eq!(import.alias.text, "math");
    assert_eq!(
        file.slice(import.span),
        Some("import /* path */ \"math\\tfile.larp\" as math;")
    );
    for span in [import.path_span, import.alias.span, import.span] {
        assert_eq!(span.source(), SourceId(49));
    }
    // Syntax does not perform graph-level alias uniqueness checks.
    assert!(parse_unit(&source("import \"a\" as x; import \"b\" as x;")).is_ok());
}

#[test]
fn libraries_may_omit_main_and_top_level_visibility_wraps_only_declarations() {
    let file = source(
        "highkey role R {} lowkey role Hidden {} role Local {}\n\
         highkey archetype C {} lowkey archetype D {} archetype E {}\n\
         highkey sidequest f() -> unit {} lowkey sidequest g() -> unit {}\n\
         sidequest h() -> unit {}",
    );
    let unit = parse_unit(&file).unwrap();
    assert!(unit.main.is_none());
    assert_eq!(
        unit.roles.iter().map(|d| d.public).collect::<Vec<_>>(),
        [true, false, false]
    );
    assert_eq!(
        unit.classes.iter().map(|d| d.public).collect::<Vec<_>>(),
        [true, false, false]
    );
    assert_eq!(
        unit.functions.iter().map(|d| d.public).collect::<Vec<_>>(),
        [true, false, false]
    );
    assert_eq!(unit.roles[0].item.name.text, "R");
    assert_eq!(unit.classes[0].item.name.text, "C");
    assert_eq!(unit.functions[0].item.name.text, "f");
    assert!(parse_unit(&source("")).unwrap().main.is_none());
    assert!(
        parse_unit(&source("main_character {}"))
            .unwrap()
            .main
            .is_some()
    );
}

#[test]
fn imports_must_precede_declarations_and_visibility_cannot_decorate_import_or_main() {
    for text in [
        "sidequest f() -> unit {} import \"a\" as a;",
        "role R {} import \"a\" as a;",
        "archetype C {} import \"a\" as a;",
        "main_character {} import \"a\" as a;",
        "highkey import \"a\" as a;",
        "lowkey import \"a\" as a;",
        "highkey main_character {}",
        "lowkey main_character {}",
        "main_character {} main_character {}",
        "import 49 as a;",
        "import \"a\";",
        "import \"a\" as a::b;",
        "import \"a\" as import;",
        "main_character { mf import = 1; }",
    ] {
        let file = source(text);
        let errors = parse_unit(&file).unwrap_err();
        assert!(errors.iter().any(|e| e.code.id == "LARP-P001"), "{text}");
        for error in errors {
            assert_eq!(error.primary.span.source(), SourceId(49));
            assert!(file.slice(error.primary.span).is_some());
        }
    }
}

#[test]
fn qualified_types_roles_extensions_and_nested_generics_preserve_ast_shapes() {
    let file = source(
        "import \"math.larp\" as math;\n\
         highkey archetype Founder : math /* role */ :: Mogger {\n\
             highkey mfs<baddie<math::Founder>> peers;\n\
             init(math::Founder peer) {}\n\
         }\n\
         highkey sidequest math::Founder.score(math::Founder other) -> math::Score {\n\
             math /* binding */ :: Founder copy = new math::Founder(other);\n\
             return math::score(copy);\n\
         }",
    );
    let unit = parse_unit(&file).unwrap();
    let class = &unit.classes[0].item;
    assert_eq!(class.roles[0].text, "math::Mogger");
    assert_eq!(
        file.slice(class.roles[0].span),
        Some("math /* role */ :: Mogger")
    );
    let (_, arguments) = named(&class.fields[0].ty);
    let (_, arguments) = named(&arguments[0]);
    assert_eq!(named(&arguments[0]).0.text, "math::Founder");
    assert_eq!(
        named(&class.constructors[0].parameters[0].0).0.text,
        "math::Founder"
    );
    let function = &unit.functions[0].item;
    assert_eq!(function.name.text, "score");
    assert_eq!(
        named(function.receiver.as_ref().unwrap()).0.text,
        "math::Founder"
    );
    assert_eq!(named(&function.parameters[0].0).0.text, "math::Founder");
    assert_eq!(named(&function.result).0.text, "math::Score");
    let StatementKind::Binding {
        ty: Some(ty),
        name,
        value,
    } = &function.body.statements[0].kind
    else {
        panic!("Expected a qualified-type local binding");
    };
    assert_eq!(name.text, "copy");
    assert_eq!(named(ty).0.text, "math::Founder");
    assert_eq!(file.slice(ty.span), Some("math /* binding */ :: Founder"));
    let ExprKind::New { class, .. } = &value.kind else {
        panic!("Expected a constructor call");
    };
    assert_eq!(class.text, "math::Founder");
    assert_eq!(file.slice(class.span), Some("math::Founder"));
}

#[test]
fn qualified_values_calls_and_dot_members_remain_distinct() {
    let file = source(
        "main_character {\n\
            let callback = math /* value */ :: double;\n\
            let result = math::double<math::Score>(2).value;\n\
            let comparison = math::value mogg 0;\n\
            let nested = math::nested::function;\n\
            math::double(2);\n\
         }",
    );
    let main = parse_unit(&file).unwrap().main.unwrap();
    let StatementKind::Binding { value, .. } = &main.statements[0].kind else {
        panic!("Expected a binding");
    };
    let ExprKind::Name(name) = &value.kind else {
        panic!("Expected a function reference");
    };
    assert_eq!(name.text, "math::double");
    assert_eq!(file.slice(name.span), Some("math /* value */ :: double"));
    assert_eq!(name.span.source(), SourceId(49));
    let StatementKind::Binding { value, .. } = &main.statements[1].kind else {
        panic!("Expected a binding");
    };
    let ExprKind::Member { receiver, name } = &value.kind else {
        panic!("Expected a member access");
    };
    assert_eq!(name.text, "value");
    let ExprKind::Call {
        callee,
        type_arguments,
        ..
    } = &receiver.kind
    else {
        panic!("Expected a qualified call");
    };
    assert!(matches!(&callee.kind, ExprKind::Name(name) if name.text == "math::double"));
    assert_eq!(named(&type_arguments[0]).0.text, "math::Score");
    let StatementKind::Binding { value, .. } = &main.statements[2].kind else {
        panic!("Expected a binding");
    };
    assert!(matches!(value.kind, ExprKind::Binary { .. }));
    let StatementKind::Binding { value, .. } = &main.statements[3].kind else {
        panic!("Expected a binding");
    };
    assert!(matches!(&value.kind, ExprKind::Name(name) if name.text == "math::nested::function"));
    assert!(matches!(
        main.statements[4].kind,
        StatementKind::Expression(_)
    ));
}

#[test]
fn declaration_binding_parameter_and_member_names_cannot_be_qualified() {
    for text in [
        "archetype math::Founder {}",
        "role math::Mogger {}",
        "sidequest math::double() -> mf { return 1; }",
        "sidequest f(mf math::x) -> mf { return 1; }",
        "main_character { let math::x = 1; }",
        "main_character { mf math::x = 1; }",
        "main_character { let x = value.math::member; }",
        "main_character { let x = math::; }",
        "main_character { let x = ::math; }",
    ] {
        assert!(parse_unit(&source(text)).is_err(), "{text}");
    }
}

#[test]
fn legacy_parse_still_requires_main_allows_import_identifiers_and_rejects_project_syntax() {
    parse(&source("sidequest import(mf import) -> mf { return import; } main_character { let import = 2; yap(import); }")).unwrap();
    for text in [
        "",
        "sidequest f() -> unit {}",
        "import \"math.larp\" as math; main_character {}",
        "highkey sidequest f() -> unit {} main_character {}",
        "main_character { let x = math::double; }",
        "main_character { math::Founder x = new math::Founder(); }",
        "archetype C : math::R {} main_character {}",
        "sidequest math::Founder.foo() -> unit {} main_character {}",
    ] {
        assert!(parse(&source(text)).is_err(), "{text}");
    }
}
