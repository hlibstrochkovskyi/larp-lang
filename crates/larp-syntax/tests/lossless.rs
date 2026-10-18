use larp_source::{SourceFile, SourceId};
use larp_syntax::{CommentKind, TokenKind, lex_with_comments};

fn file(text: &str) -> SourceFile {
    SourceFile::new(SourceId(0), "lossless.larp", text)
}

#[test]
fn comment_spans_retain_exact_unicode_delimiters_and_crlf_boundaries() {
    let source = file("// 🦀 leading\r\nmain_character { /* block\n🦀 */ yap(1); } // ending");
    let lexed = lex_with_comments(&source).unwrap();
    assert_eq!(lexed.comments.len(), 3);
    assert_eq!(lexed.comments[0].kind, CommentKind::Line);
    assert_eq!(
        source.slice(lexed.comments[0].span),
        Some("// 🦀 leading\r")
    );
    assert_eq!(lexed.comments[1].kind, CommentKind::Block);
    assert_eq!(
        source.slice(lexed.comments[1].span),
        Some("/* block\n🦀 */")
    );
    assert_eq!(source.slice(lexed.comments[2].span), Some("// ending"));
    let eof = lexed.tokens.last().unwrap();
    assert_eq!(eof.kind, TokenKind::Eof);
    assert_eq!(eof.span.range(), source.text().len()..source.text().len());
}

#[test]
fn comment_markers_inside_strings_remain_string_contents_and_raw_token_spans() {
    let text = r#"main_character { text first = "// not a comment"; text second = "/* not a block */ \" // 🦀"; /* actual */ }"#;
    let source = file(text);
    let lexed = lex_with_comments(&source).unwrap();
    assert_eq!(lexed.comments.len(), 1);
    assert_eq!(source.slice(lexed.comments[0].span), Some("/* actual */"));
    let strings: Vec<_> = lexed
        .tokens
        .iter()
        .filter(|token| matches!(token.kind, TokenKind::Text(_)))
        .collect();
    assert_eq!(strings.len(), 2);
    assert_eq!(strings[0].kind, TokenKind::Text("// not a comment".into()));
    assert_eq!(
        strings[1].kind,
        TokenKind::Text("/* not a block */ \" // 🦀".into())
    );
    assert_eq!(
        source.slice(strings[1].span),
        Some(r#""/* not a block */ \" // 🦀""#)
    );
}

#[test]
fn comments_separate_keyword_pairs_numeric_literals_and_operator_tokens() {
    let text = "main_character { npc/* pair */loop(mf i in 0/* range */..2){} locked_in// pair\nwhile(cap){} mf value=1_000/* adjacent */+2; }";
    let source = file(text);
    let lexed = lex_with_comments(&source).unwrap();
    larp_syntax::parse(&source).unwrap();
    let kinds: Vec<_> = lexed.tokens.iter().map(|token| &token.kind).collect();
    assert!(kinds.windows(2).any(|pair| matches!(pair, [TokenKind::Word(first), TokenKind::Word(second)] if first == "npc" && second == "loop")));
    assert!(
        lexed
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Integer("1000".into())
                && source.slice(token.span) == Some("1_000"))
    );
    let separated = lex_with_comments(&file("1/* separator */2 >/* separator */= 0")).unwrap();
    assert_eq!(
        separated
            .tokens
            .iter()
            .map(|token| token.kind.clone())
            .collect::<Vec<_>>(),
        vec![
            TokenKind::Integer("1".into()),
            TokenKind::Integer("2".into()),
            TokenKind::Punctuation(">"),
            TokenKind::Punctuation("="),
            TokenKind::Integer("0".into()),
            TokenKind::Eof
        ]
    );
}

#[test]
fn tokens_comments_and_whitespace_gaps_reconstruct_the_source_without_loss() {
    let text = "\t// leading\r\nmain_character{/*a*/text s=\"🦀\\t//\"; /*x\n y*/yap(1_049);}// trailing\n  ";
    let source = file(text);
    let lexed = lex_with_comments(&source).unwrap();
    let mut spans: Vec<_> = lexed
        .tokens
        .iter()
        .filter(|token| token.kind != TokenKind::Eof)
        .map(|token| token.span)
        .chain(lexed.comments.iter().map(|comment| comment.span))
        .collect();
    spans.sort_by_key(|span| span.range().start);
    let mut previous = 0;
    let mut reconstructed = String::new();
    for span in spans {
        let range = span.range();
        assert!(range.start >= previous);
        let whitespace = &text[previous..range.start];
        assert!(whitespace.is_ascii() && whitespace.chars().all(|ch| ch.is_ascii_whitespace()));
        reconstructed.push_str(whitespace);
        reconstructed.push_str(source.slice(span).unwrap());
        previous = range.end;
    }
    reconstructed.push_str(&text[previous..]);
    assert_eq!(reconstructed, text);
}

#[test]
fn comment_only_inputs_non_nested_blocks_and_malformed_input_have_valid_ranges() {
    for text in ["", "//", "// 🦀", "/* */", "// line\n/* block */\t"] {
        let source = file(text);
        let lexed = lex_with_comments(&source).unwrap();
        assert_eq!(lexed.tokens.len(), 1);
        assert_eq!(lexed.tokens[0].kind, TokenKind::Eof);
        assert!(
            lexed
                .comments
                .iter()
                .all(|comment| source.slice(comment.span).is_some())
        );
    }
    let source = file("/* outer /* inner */ tail */");
    let lexed = lex_with_comments(&source).unwrap();
    assert_eq!(lexed.comments.len(), 1);
    assert_eq!(
        source.slice(lexed.comments[0].span),
        Some("/* outer /* inner */")
    );
    assert_eq!(lexed.tokens[0].kind, TokenKind::Word("tail".into()));
    for text in [
        "/* unterminated 🦀",
        "\"// unterminated 🦀",
        "\"\\q\" /* comment */",
        "1__0 // comment",
        "1e/* comment */2",
        "🦀 /* comment */",
    ] {
        let source = file(text);
        let errors = lex_with_comments(&source).unwrap_err();
        assert!(errors.iter().all(|error| error.code.id == "LARP-L001"));
        assert!(
            errors
                .iter()
                .all(|error| source.slice(error.primary.span).is_some())
        );
    }
}
