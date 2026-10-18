use larp_source::{SourceFile, SourceId};
use larp_syntax::{ExprKind, StatementKind};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId(0), "scenarios.larp", text)
}

fn parse(text: &str) -> Result<larp_syntax::Program, Vec<larp_diagnostics::Diagnostic>> {
    larp_syntax::parse(&source(text))
}

#[test]
fn scenarios_claims_and_selection_preserve_exact_names_values_and_spans() {
    let text = "main_character { larp /* entry */ (founder) as PeterThiel { flex founder.valuation = amount(1, 2) + 49; canon founder.aura; palantir(founder); } }";
    let source = source(text);
    let program = larp_syntax::parse(&source).unwrap();
    let statement = &program.main.statements[0];
    let StatementKind::Scenario { root, label, body } = &statement.kind else {
        panic!("Expected a scenario statement");
    };
    assert_eq!(root.text, "founder");
    assert_eq!(label.text, "PeterThiel");
    assert_eq!(source.slice(root.span), Some("founder"));
    assert_eq!(source.slice(label.span), Some("PeterThiel"));
    assert!(
        source
            .slice(statement.span)
            .unwrap()
            .starts_with("larp /* entry */ (founder)")
    );
    assert!(
        source
            .slice(statement.span)
            .unwrap()
            .ends_with("palantir(founder); }")
    );
    assert_eq!(body.statements.len(), 3);
    let StatementKind::Flex { root, field, value } = &body.statements[0].kind else {
        panic!("Expected a claim statement");
    };
    assert_eq!(root.text, "founder");
    assert_eq!(field.text, "valuation");
    assert_eq!(source.slice(value.span), Some("amount(1, 2) + 49"));
    assert_eq!(
        source.slice(body.statements[0].span),
        Some("flex founder.valuation = amount(1, 2) + 49;")
    );
    let StatementKind::Canon { root, field } = &body.statements[1].kind else {
        panic!("Expected a selection statement");
    };
    assert_eq!(root.text, "founder");
    assert_eq!(field.text, "aura");
    assert_eq!(source.slice(field.span), Some("aura"));
    assert_eq!(
        source.slice(body.statements[1].span),
        Some("canon founder.aura;")
    );
    let StatementKind::Expression(expression) = &body.statements[2].kind else {
        panic!("Expected an ordinary observer call expression");
    };
    assert!(
        matches!(&expression.kind, ExprKind::Call { callee, arguments, .. } if matches!(&callee.kind, ExprKind::Name(name) if name.text == "palantir") && arguments.len() == 1)
    );
}

#[test]
fn scenario_context_and_exit_restrictions_remain_semantic_checks() {
    let program = parse("main_character { flex root.value = 1; canon root.value; larp (root) as Label { larp (other) as Nested {} return; goon { touch_grass; } } }").unwrap();
    assert!(matches!(
        program.main.statements[0].kind,
        StatementKind::Flex { .. }
    ));
    assert!(matches!(
        program.main.statements[1].kind,
        StatementKind::Canon { .. }
    ));
    let StatementKind::Scenario { body, .. } = &program.main.statements[2].kind else {
        panic!("Expected a scenario statement");
    };
    assert!(matches!(
        body.statements[0].kind,
        StatementKind::Scenario { .. }
    ));
    assert!(matches!(
        body.statements[1].kind,
        StatementKind::Return(None)
    ));
    assert!(matches!(body.statements[2].kind, StatementKind::Forever(_)));
    parse("sidequest f() -> unit { larp (root) as Label {} } main_character { let palantir = 1; let canonical = 2; let flexed = 3; }").unwrap();
}

#[test]
fn scenario_headers_require_a_single_identifier_root_label_and_block() {
    for statement in [
        "larp () as Label {}",
        "larp (self) as Label {}",
        "larp (root.field) as Label {}",
        "larp (root[0]) as Label {}",
        "larp (make()) as Label {}",
        "larp ((root)) as Label {}",
        "larp (root, other) as Label {}",
        "larp (1) as Label {}",
        "larp root as Label {}",
        "larp (root) Label {}",
        "larp (root) as \"Label\" {}",
        "larp (root) as self {}",
        "larp (root) as Label;",
    ] {
        let text = format!("main_character {{ {statement} }}");
        let source = source(&text);
        let errors = larp_syntax::parse(&source).unwrap_err();
        assert!(
            errors.iter().any(|error| error.code.id == "LARP-P001"),
            "{statement}"
        );
        assert!(
            errors
                .iter()
                .all(|error| source.slice(error.primary.span).is_some())
        );
    }
}

#[test]
fn claim_and_selection_targets_are_exact_direct_identifier_fields() {
    for statement in [
        "flex self.value = 1;",
        "flex (root).value = 1;",
        "flex make().value = 1;",
        "flex root.inner.value = 1;",
        "flex root.values[0] = 1;",
        "flex root.value += 1;",
        "flex root = 1;",
        "flex root.value = ;",
        "canon self.value;",
        "canon (root).value;",
        "canon root.inner.value;",
        "canon root.values[0];",
        "canon root;",
        "canon root.value = 1;",
        "canon root.value();",
    ] {
        let errors = parse(&format!("main_character {{ {statement} }}")).unwrap_err();
        assert!(
            errors.iter().any(|error| error.code.id == "LARP-P001"),
            "{statement}"
        );
    }
}

#[test]
fn malformed_scenario_statements_recover_to_later_statements_with_valid_spans() {
    let text = "main_character { larp (root) as Label { flex root.value = ; canon root; canon root.value; yap(1); } mf after = ; }";
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
    let errors =
        parse("main_character { larp (root.field) as Label { canon root.value; } mf after = ; }")
            .unwrap_err();
    assert_eq!(errors.len(), 2);
}

#[test]
fn deeply_nested_scenario_blocks_and_long_claim_expressions_are_bounded() {
    let nested = format!(
        "main_character {{ {} {} }}",
        "larp (root) as Label {".repeat(500),
        "}".repeat(500)
    );
    let long_claim = format!(
        "main_character {{ flex root.value = 0{}; }}",
        "+1".repeat(10_000)
    );
    for text in [nested, long_claim] {
        assert!(
            parse(&text)
                .unwrap_err()
                .iter()
                .any(|error| error.code.id == "LARP-P003")
        );
    }
}

#[test]
fn deterministic_malformed_scenario_streams_do_not_panic() {
    let tokens = [
        "larp", "flex", "canon", "as", "root", "field", "Label", "self", "palantir", "(", ")", "{",
        "}", "[", "]", ".", ";", "=", "+=", "0", "let", "return",
    ];
    let mut state = 49_u64;
    for _ in 0..1000 {
        let mut text = String::from("main_character { larp (root) as Label { ");
        for _ in 0..64 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push_str(tokens[(state >> 32) as usize % tokens.len()]);
            text.push(' ');
        }
        text.push_str("} }");
        let _ = parse(&text);
    }
}
