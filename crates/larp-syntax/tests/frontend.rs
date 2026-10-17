use larp_source::{SourceFile, SourceId};

fn parse(text: &str) -> Result<larp_syntax::Program, Vec<larp_diagnostics::Diagnostic>> {
    larp_syntax::parse(&SourceFile::new(SourceId(0), "test.larp", text))
}

#[test]
fn lexical_boundaries_comments_unicode_strings_and_range_punctuation_parse() {
    parse("main_character { // comment\r\n text name = \"🦀\\n\\t\\\"\"; /* block */ npc /* pair */ loop (mf i in 0..10) {} }").unwrap();
    parse("main_character { geeked x = 1.0e-2; mf y = 1_000; }").unwrap();
    for malformed in ["1__0", "1_", "1e+", "12abc", "1_.0"] {
        let errors = parse(&format!("main_character {{ yap({malformed}); }}")).unwrap_err();
        assert_eq!(errors[0].code.id, "LARP-L001", "{malformed}");
    }
}

#[test]
fn unterminated_tokens_and_invalid_characters_have_valid_spans() {
    for text in [
        "main_character { \"open",
        "/* open",
        "main_character { yap(\"\\q\"); }",
        "main_character { 🦀; }",
    ] {
        let source = SourceFile::new(SourceId(0), "test", text);
        let errors = larp_syntax::parse(&source).unwrap_err();
        assert!(errors.iter().any(|error| error.code.id == "LARP-L001"));
        for error in errors {
            assert!(source.slice(error.primary.span).is_some());
        }
    }
}

#[test]
fn malformed_statements_recover_and_collect_multiple_errors() {
    let errors = parse("main_character { mf x = ; mf y = ; }").unwrap_err();
    assert_eq!(errors.len(), 2);
    assert!(errors.iter().all(|error| error.code.id == "LARP-P001"));
    for text in [
        "main_character",
        "",
        "main_character {} main_character {}",
        "main_character { yap(1 < 2 < 3); }",
        "main_character { yap(fr == cap == fr); }",
    ] {
        assert!(parse(text).is_err(), "{text}");
    }
}

#[test]
fn deeply_nested_and_long_chain_inputs_fail_without_overflowing_the_host_stack() {
    let nested = format!(
        "main_character {{ yap({}1{}); }}",
        "(".repeat(500),
        ")".repeat(500)
    );
    let chain = format!("main_character {{ yap(0{}); }}", "+1".repeat(10_000));
    for text in [nested, chain] {
        let errors = parse(&text).unwrap_err();
        assert!(errors.iter().any(|error| error.code.id == "LARP-P003"));
    }
}

#[test]
fn deterministic_malformed_token_streams_never_panic() {
    let alphabet = b"abc01_(){}[];,+-*/%!<>=&|\"\\ \n";
    let mut state = 49_u64;
    for _ in 0..2000 {
        let mut text = String::from("main_character {");
        for _ in 0..64 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push(char::from(
                alphabet[(state >> 32) as usize % alphabet.len()],
            ));
        }
        text.push('}');
        let _ = parse(&text);
    }
}

#[test]
fn fixed_array_lengths_are_integer_type_arguments_with_original_spans() {
    use larp_syntax::{StatementKind, TypeExprKind};

    let text =
        "main_character { hediJeans<mfs<unc>,1_000> cells = []; hediJeans<mf,0> empty = []; }";
    let source = SourceFile::new(SourceId(0), "arrays.larp", text);
    let program = larp_syntax::parse(&source).unwrap();
    let StatementKind::Binding { ty: Some(ty), .. } = &program.main.statements[0].kind else {
        panic!("Expected an explicitly typed binding");
    };
    let TypeExprKind::Named { name, arguments } = &ty.kind else {
        panic!("Expected a named type");
    };
    assert_eq!(name.text, "hediJeans");
    assert_eq!(arguments.len(), 2);
    assert!(matches!(&arguments[0].kind, TypeExprKind::Named { name, .. } if name.text == "mfs"));
    assert!(matches!(&arguments[1].kind, TypeExprKind::Integer(value) if value == "1000"));
    assert_eq!(source.slice(arguments[1].span), Some("1_000"));

    for malformed in ["-1", "+1", "1.0", "1 + 1", "(1)", "", "1,"] {
        let errors = parse(&format!(
            "main_character {{ hediJeans<mf,{malformed}> values = []; }}"
        ))
        .unwrap_err();
        assert!(
            errors.iter().any(|error| error.code.id == "LARP-P001"),
            "{malformed}"
        );
    }
    assert!(parse("sidequest bad() -> 3 {} main_character {}").is_err());
}

#[test]
fn array_literals_and_postfix_indexing_preserve_order_precedence_and_spans() {
    use larp_syntax::{ExprKind, StatementKind};
    use larp_types::Binary;

    let text =
        "main_character { let values = [[1,2],[]]; yap(-values[next()][0].size() + [3][0]); }";
    let source = SourceFile::new(SourceId(0), "arrays.larp", text);
    let program = larp_syntax::parse(&source).unwrap();
    let StatementKind::Binding { value, .. } = &program.main.statements[0].kind else {
        panic!("Expected an inferred binding");
    };
    let ExprKind::Array(elements) = &value.kind else {
        panic!("Expected an array literal");
    };
    assert_eq!(source.slice(value.span), Some("[[1,2],[]]"));
    assert_eq!(elements.len(), 2);
    assert!(matches!(&elements[0].kind, ExprKind::Array(inner) if inner.len() == 2));
    assert!(matches!(&elements[1].kind, ExprKind::Array(inner) if inner.is_empty()));
    let StatementKind::Expression(expression) = &program.main.statements[1].kind else {
        panic!("Expected a call statement");
    };
    let ExprKind::Call { arguments, .. } = &expression.kind else {
        panic!("Expected a call");
    };
    let ExprKind::Binary {
        op: Binary::Add,
        left,
        right,
    } = &arguments[0].kind
    else {
        panic!("Expected indexing and calls to bind tighter than addition");
    };
    assert!(
        matches!(&left.kind, ExprKind::Unary { operand, .. } if matches!(operand.kind, ExprKind::Call { .. }))
    );
    assert!(
        matches!(&right.kind, ExprKind::Index { receiver, index } if matches!(receiver.kind, ExprKind::Array(_)) && matches!(&index.kind, ExprKind::Integer(value) if value == "0"))
    );
    assert_eq!(source.slice(right.span), Some("[3][0]"));
    for malformed in ["[1,]", "[,1]", "[1,,2]", "values[]", "values[0,1]", "[1"] {
        assert!(
            parse(&format!("main_character {{ yap({malformed}); }}")).is_err(),
            "{malformed}"
        );
    }
    assert!(parse("main_character { yap(values[0] < values[1] < values[2]); }").is_err());
}

#[test]
fn assignment_places_allow_named_index_and_member_paths_but_reject_temporary_roots() {
    use larp_syntax::{ExprKind, StatementKind};
    use larp_types::Binary;

    let text = "main_character { values[next()].field[0] += rhs(); (values)[1] = 2; value = 3; }";
    let source = SourceFile::new(SourceId(0), "places.larp", text);
    let program = larp_syntax::parse(&source).unwrap();
    let StatementKind::Assignment { target, op, value } = &program.main.statements[0].kind else {
        panic!("Expected an assignment");
    };
    assert_eq!(*op, Some(Binary::Add));
    assert!(
        matches!(&target.kind, ExprKind::Index { receiver, .. } if matches!(receiver.kind, ExprKind::Member { .. }))
    );
    assert!(matches!(value.kind, ExprKind::Call { .. }));
    assert_eq!(source.slice(target.span), Some("values[next()].field[0]"));
    for invalid in [
        "1",
        "fr",
        "[1][0]",
        "make()[0]",
        "values[0].make()[1]",
        "(left + right)[0]",
    ] {
        let errors = parse(&format!("main_character {{ {invalid} = 2; }}")).unwrap_err();
        assert!(
            errors.iter().any(|error| error.code.id == "LARP-P001"),
            "{invalid}"
        );
    }
}

#[test]
fn hostile_array_and_index_nesting_and_long_lists_are_bounded() {
    let nested_arrays = format!(
        "main_character {{ let x = {}0{}; }}",
        "[".repeat(500),
        "]".repeat(500)
    );
    let nested_indices = format!(
        "main_character {{ let x = {}0{}; }}",
        "a[".repeat(500),
        "]".repeat(500)
    );
    let chained_indices = format!("main_character {{ let x = a{}; }}", "[0]".repeat(10_000));
    let long_array = format!("main_character {{ let x = [{}0]; }}", "0,".repeat(10_000));
    let nested_types = format!(
        "main_character {{ {}mf{} x = []; }}",
        "mfs<".repeat(500),
        ">".repeat(500)
    );
    for text in [
        nested_arrays,
        nested_indices,
        chained_indices,
        long_array,
        nested_types,
    ] {
        let errors = parse(&text).unwrap_err();
        assert!(errors.iter().any(|error| error.code.id == "LARP-P003"));
    }
}
