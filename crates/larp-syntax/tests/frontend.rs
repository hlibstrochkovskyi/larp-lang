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
    let alphabet = b"abc01_(){};,+-*/%!<>=&|\"\\ \n";
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
