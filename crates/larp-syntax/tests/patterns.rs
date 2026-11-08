use larp_source::{SourceFile, SourceId};
use larp_syntax::{
    Expr, ExprKind, LambdaBody, PatternKind, StatementKind, TypeExprKind, parse, parse_unit,
};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId(73), "patterns.larp", text)
}

fn binding_value(statement: &larp_syntax::Statement) -> &Expr {
    let StatementKind::Binding { value, .. } = &statement.kind else {
        panic!("Expected a binding");
    };
    value
}

fn rejects(text: &str, code: &str) {
    let file = source(text);
    let errors = parse(&file).unwrap_err();
    assert!(
        errors.iter().any(|error| error.code.id == code),
        "{errors:#?}"
    );
    for error in errors {
        assert_eq!(error.primary.span.source(), SourceId(73));
        assert!(file.slice(error.primary.span).is_some());
    }
}

#[test]
fn enum_declarations_preserve_variants_payload_types_visibility_and_source_spans() {
    let text = "// 🦀\nhighkey enum Funding { Bootstrapped, Angel(unc), Deal(text, fn(mf) -> result<unc,text> uses(abort)), } lowkey enum Hidden { X }";
    let file = source(text);
    let unit = parse_unit(&file).unwrap();
    assert!(unit.main.is_none());
    assert_eq!(unit.enums.len(), 2);
    assert!(unit.enums[0].public);
    assert!(!unit.enums[1].public);
    let funding = &unit.enums[0].item;
    assert_eq!(funding.name.text, "Funding");
    assert_eq!(file.slice(funding.name.span), Some("Funding"));
    assert_eq!(
        funding
            .variants
            .iter()
            .map(|variant| variant.name.text.as_str())
            .collect::<Vec<_>>(),
        ["Bootstrapped", "Angel", "Deal"]
    );
    assert!(funding.variants[0].payloads.is_empty());
    assert_eq!(file.slice(funding.variants[1].span), Some("Angel(unc)"));
    assert!(matches!(
        funding.variants[2].payloads[1].kind,
        TypeExprKind::Function { .. }
    ));
    assert_eq!(funding.span.source(), SourceId(73));
    // Duplicate declarations and variants are retained for semantic diagnostics.
    parse(&source("enum E { V, V } enum E { V } main_character {}")).unwrap();
}

#[test]
fn legacy_enum_qualification_does_not_reserve_the_import_identifier() {
    let text = "enum import { _ } main_character { mf _ = 3; import value = import /* type */ :: _; let imported = library::Funding::Angel(49); }";
    let file = source(text);
    let program = parse(&file).unwrap();
    assert_eq!(program.enums[0].name.text, "import");
    assert!(
        matches!(&binding_value(&program.main.statements[1]).kind, ExprKind::Name(name) if name.text == "import::_" && file.slice(name.span) == Some("import /* type */ :: _"))
    );
    let ExprKind::Call { callee, .. } = &binding_value(&program.main.statements[2]).kind else {
        panic!("Expected constructor call");
    };
    assert!(matches!(&callee.kind, ExprKind::Name(name) if name.text == "library::Funding::Angel"));
    assert!(
        parse_unit(&file).is_err(),
        "import remains reserved only in source-unit mode"
    );
}

#[test]
fn match_patterns_and_expression_or_block_arms_retain_exact_source_spans() {
    let text = "main_character { let value = match (input()) { no_bitches => 0, some(score) => score, win(_) => { return; }, fumble(error) => error.size(), fr => 1, cap => 2, lib /* path */ :: Funding::Deal(_, money) => money, Funding::Empty => 3, _ => 4, }; }";
    let file = source(text);
    let program = parse(&file).unwrap();
    let expression = binding_value(&program.main.statements[0]);
    let ExprKind::Match { value, arms } = &expression.kind else {
        panic!("Expected match");
    };
    assert!(matches!(value.kind, ExprKind::Call { .. }));
    assert_eq!(arms.len(), 9);
    assert!(matches!(arms[0].pattern.kind, PatternKind::None));
    assert!(matches!(arms[4].pattern.kind, PatternKind::Verdict(true)));
    assert!(matches!(arms[5].pattern.kind, PatternKind::Verdict(false)));
    assert!(matches!(arms[8].pattern.kind, PatternKind::Wildcard));
    assert!(matches!(arms[2].body, LambdaBody::Block(_)));
    let PatternKind::Constructor {
        path,
        bindings: Some(bindings),
    } = &arms[6].pattern.kind
    else {
        panic!("Expected payload constructor pattern");
    };
    assert_eq!(path.text, "lib::Funding::Deal");
    assert_eq!(
        file.slice(path.span),
        Some("lib /* path */ :: Funding::Deal")
    );
    assert_eq!(
        bindings
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        ["_", "money"]
    );
    assert_eq!(file.slice(bindings[1].span), Some("money"));
    assert!(matches!(
        arms[7].pattern.kind,
        PatternKind::Constructor { bindings: None, .. }
    ));
    assert!(file.slice(arms[6].span).unwrap().ends_with("=> money"));
    // An underscore enum name is distinct from an unqualified wildcard pattern.
    let program = parse(&source(
        "enum _ { V } main_character { match (x) { _::V => 0 }; }",
    ))
    .unwrap();
    let StatementKind::Expression(Expr {
        kind: ExprKind::Match { arms, .. },
        ..
    }) = &program.main.statements[0].kind
    else {
        panic!("Expected match statement");
    };
    assert!(matches!(
        arms[0].pattern.kind,
        PatternKind::Constructor { .. }
    ));
}

#[test]
fn propagation_is_left_associative_with_call_member_index_and_unary_precedence() {
    let file = source(
        "main_character { let value = outer()??.field[index]?; let signed = -parseScore(\"1\")? + 2; }",
    );
    let program = parse(&file).unwrap();
    let ExprKind::Propagate { operand } = &binding_value(&program.main.statements[0]).kind else {
        panic!("Expected outer propagation");
    };
    let ExprKind::Index { receiver, .. } = &operand.kind else {
        panic!("Expected indexed successful value");
    };
    let ExprKind::Member { receiver, .. } = &receiver.kind else {
        panic!("Expected member of successful value");
    };
    assert!(matches!(receiver.kind, ExprKind::Propagate { .. }));
    let ExprKind::Propagate { operand } = &receiver.kind else {
        unreachable!()
    };
    assert!(matches!(operand.kind, ExprKind::Propagate { .. }));
    assert_eq!(
        file.slice(binding_value(&program.main.statements[0]).span),
        Some("outer()??.field[index]?")
    );
    let ExprKind::Binary { left, .. } = &binding_value(&program.main.statements[1]).kind else {
        panic!("Expected addition");
    };
    let ExprKind::Unary { operand, .. } = &left.kind else {
        panic!("Expected unary minus");
    };
    assert!(matches!(operand.kind, ExprKind::Propagate { .. }));
}

#[test]
fn match_children_have_independent_expression_budgets_without_hiding_long_outer_chains() {
    let arms = (0..200)
        .map(|index| format!("E::V{index} => {index},"))
        .collect::<String>();
    let large = format!("main_character {{ let x = call(match (value) {{ {arms} }}); }}");
    parse(&source(&large)).unwrap();
    let nested = format!(
        "main_character {{ let x = match (cap) {{ cap => match (value) {{ {arms} }}, fr => 0 }}{}; }}",
        "+1".repeat(1000)
    );
    rejects(&nested, "LARP-P003");
    let long_child = format!(
        "main_character {{ match (value) {{ _ => 0{} }}; }}",
        "+1".repeat(1000)
    );
    rejects(&long_child, "LARP-P003");
    let nested = format!(
        "main_character {{ {}0{}; }}",
        "match (fr) { _ => ".repeat(200),
        " }".repeat(200)
    );
    rejects(&nested, "LARP-P003");
}

#[test]
fn enum_and_match_syntax_limits_reject_excessive_counts_with_valid_spans() {
    let variants = (0..256)
        .map(|index| format!("V{index},"))
        .collect::<String>();
    parse(&source(&format!(
        "enum E {{ {variants} }} main_character {{}}"
    )))
    .unwrap();
    rejects(
        &format!("enum E {{ {variants} Extra }} main_character {{}}"),
        "LARP-P003",
    );
    let payloads = vec!["mf"; 64].join(",");
    parse(&source(&format!(
        "enum E {{ V({payloads}) }} main_character {{}}"
    )))
    .unwrap();
    rejects(
        &format!("enum E {{ V({payloads}, mf) }} main_character {{}}"),
        "LARP-P003",
    );
    let arms = (0..257)
        .map(|index| format!("E::V{index} => 0,"))
        .collect::<String>();
    parse(&source(&format!(
        "main_character {{ match (value) {{ {arms} }}; }}"
    )))
    .unwrap();
    rejects(
        &format!("main_character {{ match (value) {{ {arms} _ => 0 }}; }}"),
        "LARP-P003",
    );
    let binders = vec!["_"; 64].join(",");
    parse(&source(&format!(
        "main_character {{ match (value) {{ E::V({binders}) => 0 }}; }}"
    )))
    .unwrap();
    rejects(
        &format!("main_character {{ match (value) {{ E::V({binders}, _) => 0 }}; }}"),
        "LARP-P003",
    );
}

#[test]
fn malformed_patterns_payload_declarations_and_reserved_words_are_syntax_errors() {
    for text in [
        "enum Empty {} main_character {}",
        "enum E { V() } main_character {}",
        "enum E { V(mf) W } main_character {}",
        "enum qualified::E { V } main_character {}",
        "main_character { mf enum = 1; }",
        "main_character { let match = 1; }",
        "main_character { match value { _ => 0 }; }",
        "main_character { match (value) { some(some(x)) => x }; }",
        "main_character { match (value) { 49 => 0 }; }",
        "main_character { match (value) { _ if (fr) => 0 }; }",
        "main_character { match (value) { _ => 0 _ => 1 }; }",
        "main_character { match (value) { _ => }; }",
        "main_character { ?value; }",
        "main_character { match (value) { _ => 0 } }",
    ] {
        rejects(text, "LARP-P001");
    }
}
