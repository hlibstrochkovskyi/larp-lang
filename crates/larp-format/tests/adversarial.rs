use larp_source::{SourceFile, SourceId};
use larp_syntax::{CommentKind, TokenKind, lex_with_comments};
use std::path::{Path, PathBuf};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId(0), "adversarial.larp", text)
}

fn lexemes(source: &SourceFile) -> Vec<(u8, String)> {
    let lexed = lex_with_comments(source).unwrap();
    let mut entries: Vec<_> = lexed
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
        .collect();
    entries.sort_unstable_by_key(|entry| entry.0);
    entries
        .into_iter()
        .map(|(_, kind, raw)| (kind, raw))
        .collect()
}

fn checked_format(text: &str, label: &str) -> String {
    let source = source(text);
    larp_syntax::parse(&source).unwrap_or_else(|errors| {
        panic!("Generated input must parse: {label}\n{text}\n{errors:#?}")
    });
    let formatted = larp_format::format(&source)
        .unwrap_or_else(|errors| panic!("Formatting failed: {label}\n{text}\n{errors:#?}"));
    let checked = SourceFile::new(SourceId(0), "formatted.larp", formatted.as_str());
    larp_syntax::parse(&checked).unwrap_or_else(|errors| {
        panic!("Formatted output must parse: {label}\n{formatted}\n{errors:#?}")
    });
    assert_eq!(
        lexemes(&source),
        lexemes(&checked),
        "Raw lexeme order changed: {label}\n{text}\n{formatted}"
    );
    let twice = larp_format::format(&checked).unwrap_or_else(|errors| {
        panic!("Second formatting failed: {label}\n{formatted}\n{errors:#?}")
    });
    assert_eq!(
        twice, formatted,
        "Formatting is not idempotent: {label}\nInput:\n{text}"
    );
    formatted
}

fn templates() -> [&'static str; 4] {
    [
        r#"role Score{sidequest score(mf bonus)->mf uses(read,abort);}archetype Player:Score{highkey mf value;lowkey text label;init(mf value){self.value=value;self.label="// not a comment 🦀";}highkey sidequest score(mf bonus)->mf uses(read,abort){return self.value+bonus;}}sidequest Player.doubled()->mf uses(read,abort){return self.score(0)*2;}main_character{Player player=new Player(3);yap(player.doubled());}"#,
        r#"sidequest fn(mf)->mf uses(abort).apply(mf value)->mf uses(abort){return self(value);}sidequest make()->fn(mf)->mf uses(abort) uses(){return x=>x+1;}main_character{mfs<result<baddie<hediJeans<mf,2>>,text>> values=[win(some([1,2]))];fn(mf)->mf uses(abort) callback=(mf x)=>{mf value=+(-x);return ((value*-(-1)));};let mapped=[1,2,3].filter(x=>x mogg 1).map(x=>callback(x));yap(ascend<unc>((1)).cope(err=>0));}"#,
        r#"archetype State{highkey mf value;init(){self.value=0;}}main_character{let root=new State();mf outside=3;nonchalant mf deferred=outside*2;sybau{yap(deferred);}larp(root)as Trial{flex root.value=(outside+1);goon{if(root.value>0){canon root.value;touch_grass;}else{root.value+=1;}}palantir(root);}yap(root.value);}"#,
        r#"sidequest f(mf x,mf y)->mf{if(!(x<y)&&fr||cap){return +(+x);}else if(x==y){return -(-y);}else{return (x+y)*(x-y)/(+2)%3;}}main_character{mf total=0;npc loop(mf i in 0..5){locked_in while(total<3){total+=i;if(total==2){touch_grass;}}}baddie<mf> missing=no_bitches;result<mf,text> value=fumble("/* text */");yap(value.cope(err=>{yap(err);return missing.copium(0);}));}"#,
    ]
}

#[test]
fn every_token_boundary_accepts_an_inserted_block_or_line_comment_without_changing_lexemes() {
    for (template, text) in templates().iter().enumerate() {
        let original = source(text);
        let lexed = lex_with_comments(&original).unwrap();
        for (token, item) in lexed.tokens.iter().enumerate() {
            let position = item.span.range().start;
            for (style, comment) in [
                ("block", format!(" /* boundary {token} 🦀 */ ")),
                ("line", format!(" // boundary {token} 🦀\n")),
                ("crlf", format!(" // boundary {token} 🦀\r\n")),
            ] {
                let mut inserted = String::with_capacity(text.len() + comment.len());
                inserted.push_str(&text[..position]);
                inserted.push_str(&comment);
                inserted.push_str(&text[position..]);
                checked_format(
                    &inserted,
                    &format!("template {template}, token {token}, {style}"),
                );
            }
        }
    }
}

#[test]
fn comments_after_operators_before_closure_blocks_and_trailing_statements_are_stable() {
    for text in [
        "main_character { mf x = 1 + // addition\n2; yap(x); // statement\n} // block\n",
        "main_character { let f = (mf x) => // body\n{ return - /* unary */ (x); }; // lambda\nyap(f(1)); }",
        "main_character { let f = (mf x) => /* before */ { return x; } /* after */; let g = () => { return f(1); }; }",
        "main_character { if(fr) { yap(1); } // between\nelse { yap(2); } /* after if */ }",
        "main_character { mfs<baddie<mf>> values = [some(1) /* item */, no_bitches]; }",
        "// leading\r\nmain_character {\r\n    yap(\"🦀 // text\"); // trailing\r\n}\r\n// eof\r\n",
        "main_character { /* first\nsecond\nthird */ yap(1); /* tail\nend */ }",
        "main_character { mf x = - // sign\n -1; verdict y = ! // not\n cap; yap(x); }",
    ] {
        checked_format(text, "operator/closure/trailing comments");
    }
}

#[test]
fn raw_literal_spelling_and_parentheses_survive_generated_expression_combinations() {
    for expression in [
        "((1_049))",
        "+(-(-1))",
        "1.2500e-02",
        "-0.000",
        "!(!(fr))",
        "(((1+2))*(3-(4/2)))",
        "ascend<unc>(((49))).cope(err=>0)",
        "((mf x)=>{return x+1;})(2)",
        "[1,2,3][(+1)]",
        "fr || (cap && !fr)",
        "1 < 2 == 3 < 4",
        r#""🦀\t\"/* not comment */\n""#,
    ] {
        checked_format(&format!("main_character{{yap({expression});}}"), expression);
    }
}

#[test]
fn huge_identifiers_long_lists_nested_groups_and_function_types_obey_soft_width() {
    let identifier = format!("identifier_{}", "long".repeat(1024));
    let huge = checked_format(
        &format!("main_character{{mf {identifier}=49;yap({identifier});}}"),
        "huge identifier",
    );
    assert!(huge.contains(&identifier));
    let elements = (0..100)
        .map(|index| index.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let listed = checked_format(
        &format!("main_character{{mfs<mf> values=[{elements}];}}"),
        "long list",
    );
    assert!(listed.lines().count() > 5);
    let groups = format!("{}1{}", "(".repeat(40), ")".repeat(40));
    checked_format(
        &format!("main_character{{yap({groups});}}"),
        "nested parentheses",
    );
    let nested = format!("{}mf{}", "baddie<".repeat(20), ">".repeat(20));
    checked_format(
        &format!("main_character{{{nested} value=no_bitches;}}"),
        "nested generic type",
    );
    checked_format(
        "sidequest f(fn(mf,unc,text)->fn(mf)->result<baddie<mf>,text> uses(abort) uses(read) callback)->unit uses(){}main_character{}",
        "nested function type",
    );
}

fn collect_larp_files(directory: &Path, paths: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_larp_files(&path, paths);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "larp")
        {
            paths.push(path);
        }
    }
}

#[test]
fn every_existing_conformance_program_formats_without_mutating_repository_sources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance");
    let mut paths = Vec::new();
    collect_larp_files(&root, &mut paths);
    paths.sort();
    assert!(paths.len() >= 77);
    for path in paths {
        let text = std::fs::read_to_string(&path).unwrap();
        checked_format(&text, path.to_str().unwrap());
    }
}
