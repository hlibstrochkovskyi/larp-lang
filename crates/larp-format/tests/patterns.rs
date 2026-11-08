use larp_source::{SourceFile, SourceId};
use larp_syntax::{CommentKind, TokenKind, lex_with_comments, parse, parse_unit};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId(73), "patterns.larp", text)
}

fn lexemes(source: &SourceFile) -> Vec<(u8, String)> {
    let lexed = lex_with_comments(source).unwrap();
    let mut entries = lexed
        .tokens
        .iter()
        .filter(|token| token.kind != TokenKind::Eof)
        .map(|token| {
            (
                token.span.range().start,
                0,
                source.slice(token.span).unwrap().to_owned(),
            )
        })
        .chain(lexed.comments.iter().map(|comment| {
            (
                comment.span.range().start,
                if comment.kind == CommentKind::Line {
                    1
                } else {
                    2
                },
                source.slice(comment.span).unwrap().to_owned(),
            )
        }))
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.0);
    entries
        .into_iter()
        .map(|(_, kind, text)| (kind, text))
        .collect()
}

fn checked(text: &str, unit: bool) -> String {
    let input = source(text);
    let format = if unit {
        larp_format::format_unit
    } else {
        larp_format::format
    };
    let formatted = format(&input).unwrap_or_else(|errors| panic!("{text}\n{errors:#?}"));
    let output = source(&formatted);
    if unit {
        parse_unit(&output).unwrap();
    } else {
        parse(&output).unwrap();
    }
    assert_eq!(lexemes(&input), lexemes(&output));
    assert_eq!(format(&output).unwrap(), formatted);
    assert_eq!(input.text(), text);
    formatted
}

#[test]
fn enum_variant_braces_break_at_top_level_commas_and_keep_payloads_compact() {
    let output = checked(
        "enum Funding{Bootstrapped,Angel(unc),Deal(text,unc),}main_character{Funding value=Funding::Angel(fortnine);}",
        false,
    );
    assert_eq!(
        output,
        "enum Funding {\n    Bootstrapped,\n    Angel(unc),\n    Deal(text, unc),\n}\n\nmain_character {\n    Funding value = Funding::Angel(fortnine);\n}\n"
    );
}

#[test]
fn match_arm_braces_are_comma_lists_and_block_arm_bodies_remain_statement_blocks() {
    let output = checked(
        "main_character{let x=match(input){win(value)=>value,fumble(error)=>{yap(error);return;},};}",
        false,
    );
    assert_eq!(
        output,
        "main_character {\n    let x = match (input) {\n        win(value) => value,\n        fumble(error) => {\n            yap(error);\n            return;\n        },\n    };\n}\n"
    );
}

#[test]
fn propagation_stays_tight_through_nested_postfix_operations_and_match_closers() {
    let output = checked(
        "main_character{let x=fetch() ? ?.field[index] ?;let y=match(input){win(v)=>v,fumble(_)=>0} ?;}",
        false,
    );
    assert!(output.contains("fetch()??.field[index]?;"), "{output}");
    assert!(output.contains("    }?;"), "{output}");
    assert!(!output.contains("} ?"));
}

#[test]
fn exported_enums_qualified_patterns_nested_types_and_comments_survive_source_unit_formatting() {
    let text = "import \"funding.larp\" as lib;\r\nhighkey enum Result{Empty,/* 🦀 variant */Data(hediJeans<result<mf,text>,2>,fn(mf)->unc uses(abort)),}\r\nhighkey sidequest choose(lib::Funding input)->unc{return match(input){lib/* namespace */::Funding::Bootstrapped=>0,lib::Funding::Angel(amount)=>amount,// claim\r\n_=>49,};}";
    let output = checked(text, true);
    assert!(output.contains("highkey enum Result {\n"), "{output}");
    assert!(output.contains("/* 🦀 variant */"));
    assert!(output.contains("// claim\r"));
    assert!(output.contains("hediJeans<result<mf, text>, 2>"));
    assert!(output.contains("Funding::Angel(amount) => amount,"));
    assert!(
        output.contains("lib /* namespace */ ::Funding::Bootstrapped"),
        "{output}"
    );
}

#[test]
fn nested_matches_lambdas_and_ordinary_blocks_keep_independent_brace_layouts() {
    let output = checked(
        "main_character{let f=(mf x)=>match(x){fr=>match(other){some(v)=>{if(fr){return v;}return 0;},no_bitches=>0},cap=>0};let z=match(a){_=>y=>y};}",
        false,
    );
    assert!(output.contains("fr => match (other) {"), "{output}");
    assert!(output.contains("if (fr) {\n"), "{output}");
    assert!(output.contains("_ => y => y"), "{output}");
}

#[test]
fn large_match_primary_formats_without_combining_all_child_expression_budgets() {
    let arms = (0..200)
        .map(|index| format!("E::V{index}=>{index},"))
        .collect::<String>();
    let output = checked(
        &format!("main_character{{let x=call(match(v){{{arms}}});}}"),
        false,
    );
    assert_eq!(
        output
            .lines()
            .filter(|line| line.trim_start().starts_with("E::V"))
            .count(),
        200
    );
}
