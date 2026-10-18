use larp_source::{SourceFile, SourceId};
use larp_syntax::{ExprKind, StatementKind, TypeExprKind};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId(0), "lazy-and-suppression.larp", text)
}

fn parse(text: &str) -> Result<larp_syntax::Program, Vec<larp_diagnostics::Diagnostic>> {
    larp_syntax::parse(&source(text))
}

#[test]
fn typed_lazy_bindings_preserve_type_name_initializer_and_cloned_source_spans() {
    let text = "main_character { nonchalant mf answer = base + 49; nonchalant baddie<hediJeans<text,1>> nested = some([\"🦀\"]); }";
    let source = source(text);
    let program = larp_syntax::parse(&source).unwrap();
    let statement = &program.main.statements[0];
    let StatementKind::Lazy { ty, name, value } = &statement.kind else {
        panic!("Expected a lazy binding");
    };
    assert!(matches!(&ty.kind, TypeExprKind::Named { name, .. } if name.text == "mf"));
    assert_eq!(name.text, "answer");
    assert_eq!(source.slice(name.span), Some("answer"));
    assert_eq!(source.slice(value.span), Some("base + 49"));
    assert_eq!(
        source.slice(statement.span),
        Some("nonchalant mf answer = base + 49;")
    );
    let cloned = value.clone();
    assert_eq!(cloned.span, value.span);
    assert!(
        matches!(&cloned.kind, ExprKind::Binary { left, .. } if source.slice(left.span) == Some("base"))
    );
    let StatementKind::Lazy { ty, value, .. } = &program.main.statements[1].kind else {
        panic!("Expected a composite lazy binding");
    };
    assert_eq!(source.slice(ty.span), Some("baddie<hediJeans<text,1>>"));
    assert_eq!(source.slice(value.span), Some("some([\"🦀\"])"));
    let cloned_block = program.main.clone();
    assert_eq!(cloned_block.span, program.main.span);
    assert_eq!(cloned_block.statements.len(), 2);
}

#[test]
fn suppression_is_a_block_statement_and_nests_with_lazy_lambdas_and_scenarios() {
    let text = "main_character { sybau { yap(1); sybau { nonchalant mf value = 2; } let callback = () => { sybau { return; } }; larp (root) as Trial { sybau { palantir(root); } } } yap(3); }";
    let source = source(text);
    let program = larp_syntax::parse(&source).unwrap();
    let StatementKind::Sybau(body) = &program.main.statements[0].kind else {
        panic!("Expected a suppression block");
    };
    assert_eq!(body.statements.len(), 4);
    assert!(matches!(body.statements[1].kind, StatementKind::Sybau(_)));
    assert!(matches!(
        body.statements[3].kind,
        StatementKind::Scenario { .. }
    ));
    assert!(
        source
            .slice(program.main.statements[0].span)
            .unwrap()
            .starts_with("sybau {")
    );
    assert!(source.slice(body.span).unwrap().starts_with("{ yap(1);"));
    assert_eq!(
        source.slice(program.main.statements[1].span),
        Some("yap(3);")
    );
    let cloned = program.main.statements[0].clone();
    assert_eq!(cloned.span, program.main.statements[0].span);
}

#[test]
fn lazy_bindings_require_explicit_types_names_initializers_and_terminators() {
    for statement in [
        "nonchalant answer = 1;",
        "nonchalant let answer = 1;",
        "nonchalant mf = 1;",
        "nonchalant mf answer;",
        "nonchalant mf answer = ;",
        "nonchalant mf answer = 1",
        "nonchalant (mf) answer = 1;",
        "nonchalant mf answer += 1;",
    ] {
        let errors = parse(&format!("main_character {{ {statement} }}")).unwrap_err();
        assert!(
            errors.iter().any(|error| error.code.id == "LARP-P001"),
            "{statement}"
        );
    }
    // Snapshot result and effect restrictions belong to semantic analysis.
    parse("main_character { nonchalant mfs<mf> values = []; nonchalant fn()->mf uses() callback = () => 1; nonchalant unit nothing = yap(1); }").unwrap();
}

#[test]
fn suppression_requires_braces_and_cannot_be_used_as_an_expression() {
    for statement in [
        "sybau;",
        "sybau();",
        "sybau yap(1);",
        "sybau (fr) { yap(1); }",
        "sybau { yap(1); };",
        "let value = sybau {};",
        "yap(sybau {});",
    ] {
        assert!(
            parse(&format!("main_character {{ {statement} }}")).is_err(),
            "{statement}"
        );
    }
    parse("main_character { sybau {} sybau { { yap(1); } } }").unwrap();
}

#[test]
fn lazy_and_suppression_words_remain_reserved_without_reserving_identifier_prefixes() {
    for text in [
        "main_character { mf nonchalant = 1; }",
        "main_character { let sybau = 1; }",
        "main_character { nonchalant mf sybau = 1; }",
        "sidequest nonchalant() -> unit {} main_character {}",
        "sidequest f(mf sybau) -> unit {} main_character {}",
        "archetype A { mf sybau; init() {} } main_character {}",
        "main_character { let callback = (mf nonchalant) => 1; }",
    ] {
        let errors = parse(text).unwrap_err();
        assert!(
            errors.iter().any(|error| error.code.id == "LARP-P001"),
            "{text}"
        );
    }
    parse("sidequest sybauValue(mf nonchalantValue) -> mf { return nonchalantValue; } main_character { text words = \"nonchalant sybau\"; let nonchalantValue = 1; sybau { yap(sybauValue(nonchalantValue)); } }").unwrap();
}

#[test]
fn malformed_lazy_and_suppression_statements_recover_with_valid_source_spans() {
    let text = "main_character { sybau { nonchalant mf first = ; mf valid = 1; } sybau yap(2); nonchalant mf last = ; mf after = 3; }";
    let source = source(text);
    let errors = larp_syntax::parse(&source).unwrap_err();
    assert_eq!(errors.len(), 3);
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
}

#[test]
fn nested_suppression_long_initializers_and_malformed_streams_obey_parser_bounds() {
    let nested = format!(
        "main_character {{ {} {} }}",
        "sybau {".repeat(500),
        "}".repeat(500)
    );
    let initializer = format!(
        "main_character {{ nonchalant mf value = 0{}; }}",
        "+1".repeat(10_000)
    );
    let nested_type = format!(
        "main_character {{ nonchalant {}mf{} value = 1; }}",
        "baddie<".repeat(500),
        ">".repeat(500)
    );
    for text in [nested, initializer, nested_type] {
        assert!(
            parse(&text)
                .unwrap_err()
                .iter()
                .any(|error| error.code.id == "LARP-P003")
        );
    }
    let tokens = [
        "nonchalant",
        "sybau",
        "mf",
        "value",
        "let",
        "fn",
        "uses",
        "return",
        "larp",
        "canon",
        "(",
        ")",
        "{",
        "}",
        "[",
        "]",
        ";",
        "=",
        "=>",
        "0",
    ];
    let mut state = 49_u64;
    for _ in 0..1000 {
        let mut text = String::from("main_character { sybau { ");
        for _ in 0..64 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push_str(tokens[(state >> 32) as usize % tokens.len()]);
            text.push(' ');
        }
        text.push_str("} }");
        let _ = parse(&text);
    }
}
