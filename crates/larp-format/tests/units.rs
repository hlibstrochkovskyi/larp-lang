use larp_source::{SourceFile, SourceId};
use larp_syntax::{CommentKind, TokenKind, lex_with_comments, parse, parse_unit};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId(49), "library.larp", text)
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
        .map(|(_, kind, text)| (kind, text))
        .collect()
}

fn checked(text: &str) -> String {
    let original = source(text);
    let output = larp_format::format_unit(&original)
        .unwrap_or_else(|errors| panic!("Formatting unit failed:\n{text}\n{errors:#?}"));
    let formatted = source(&output);
    parse_unit(&formatted).unwrap();
    assert_eq!(lexemes(&original), lexemes(&formatted));
    assert_eq!(larp_format::format_unit(&formatted).unwrap(), output);
    assert_eq!(original.text(), text, "Input source must remain immutable");
    output
}

#[test]
fn libraries_and_import_only_units_do_not_require_main() {
    assert_eq!(checked(""), "\n");
    assert_eq!(checked("// library\n"), "// library\n");
    assert_eq!(
        checked("import\"math.larp\"as math;"),
        "import \"math.larp\" as math;\n"
    );
    assert_eq!(
        checked("highkey sidequest answer()->mf{return 49;}"),
        "highkey sidequest answer() -> mf {\n    return 49;\n}\n"
    );
    assert!(larp_format::format(&source("sidequest answer()->mf{return 49;}")).is_err());
}

#[test]
fn imports_keep_original_spelling_decoded_values_and_order() {
    let text = "// imports 🦀\r\nimport\"dir\\tname.larp\"as z;/* middle */import \"a.larp\" as a;\nlowkey sidequest x()->mf{return z::answer();}";
    let output = checked(text);
    assert!(output.contains("import \"dir\\tname.larp\" as z;"));
    assert!(output.find("as z;").unwrap() < output.find("as a;").unwrap());
    let before = parse_unit(&source(text)).unwrap();
    let after_source = source(&output);
    let after = parse_unit(&after_source).unwrap();
    for (before, after) in before.imports.iter().zip(&after.imports) {
        assert_eq!(before.path, after.path);
        assert_eq!(before.alias.text, after.alias.text);
        assert_eq!(after.path_span.source(), SourceId(49));
        assert_eq!(
            source(text).slice(before.path_span),
            after_source.slice(after.path_span)
        );
    }
    assert_eq!(after.imports[0].path, "dir\tname.larp");
}

#[test]
fn namespace_references_stay_compact_in_types_calls_new_and_lambdas() {
    let output = checked(
        "import\"models.larp\"as model;import\"math.larp\"as math;highkey sidequest build(mfs<result<baddie<model ::Founder>,text>> values)->fn(model::Founder)->mf uses(read){return(model::Founder value)=>math :: score(value);}main_character{model ::Founder bro=new model :: Founder(49);let apply=(model ::Founder value)=>math ::score(value);let next=ascend<model ::Number>(4);yap(apply(bro));}",
    );
    for expected in [
        "mfs<result<baddie<model::Founder>, text>>",
        "fn(model::Founder) -> mf uses(",
        "(model::Founder value) => math::score(value)",
        "model::Founder bro = new model::Founder(49)",
        "ascend<model::Number>(4)",
    ] {
        assert!(output.contains(expected), "Missing {expected}:\n{output}");
    }
}

#[test]
fn exported_roles_classes_and_extension_receivers_share_ast_annotations() {
    let output = checked(
        "import\"models.larp\"as model;highkey role Scores{sidequest values()->mfs<result<model::Founder,text>> uses(read);}lowkey archetype Hidden{init(){}}highkey archetype Founder:model :: Scores{lowkey mfs<baddie<model::Founder>> friends;init(mfs<baddie<model::Founder>> friends){self.friends=friends;}highkey sidequest values()->mfs<result<model::Founder,text>> uses(read){return [];}}highkey sidequest model :: Founder.score(mf bonus)->mf uses(read,abort){return self.value+bonus;}lowkey sidequest mfs<model ::Founder>.first()->baddie<model::Founder> uses(read){return no_bitches;}",
    );
    assert!(output.contains("highkey role Scores"));
    assert!(output.contains("lowkey archetype Hidden"));
    assert!(output.contains("Founder : model::Scores"));
    assert!(output.contains("highkey sidequest model::Founder.score(mf bonus)"));
    assert!(output.contains("lowkey sidequest mfs<model::Founder>.first()"));
    assert!(output.contains("mfs<result<model::Founder, text>>"));
    let unit = parse_unit(&source(&output)).unwrap();
    assert!(unit.roles[0].public);
    assert!(!unit.classes[0].public);
    assert!(unit.classes[1].public);
    assert!(unit.functions[0].public);
    assert!(!unit.functions[1].public);
}

#[test]
fn namespace_import_and_visibility_comments_remain_separated_and_exact() {
    for text in [
        "import/* keyword */\"math.larp\"/* path */as/* alias */math/* end */;highkey/* export */sidequest score()->mf{return math/* left */::/* right */answer();}",
        "import // path\n\"math.larp\" // as\nas // alias\nmath;\nmain_character{yap(math // before qualifier\n:: // after qualifier\nanswer());}",
        "import\"models.larp\"as model;highkey sidequest model/* receiver */::/* type */Founder/* member */./* method */score()->mfs/* open */</* type */model/* left */::/* right */Founder/* close */>{return [];}",
        "// Unicode 🦀\r\nimport\"math.larp\"as math;main_character{yap(math/* raw\r\n 🦀 */::answer());}// eof\r\n",
    ] {
        checked(text);
    }
    let output = checked("main_character{yap(math/* before */::/* after */answer());}");
    assert!(output.contains("math /* before */ :: /* after */ answer()"));
}

#[test]
fn unit_syntax_is_opt_in_and_legacy_import_bindings_are_preserved() {
    let text = "main_character{mf import=49;yap(import);}";
    let old = larp_format::format(&source(text)).unwrap();
    assert!(old.contains("mf import = 49;"));
    assert!(parse(&source(&old)).is_ok());
    assert!(larp_format::format_unit(&source(text)).is_err());
    for text in [
        "import\"math.larp\"as math;main_character{}",
        "highkey sidequest answer()->mf{return 49;}main_character{}",
        "main_character{yap(math::answer());}",
    ] {
        checked(text);
        assert!(larp_format::format(&source(text)).is_err());
    }
}

#[test]
fn unit_parse_errors_keep_original_source_spans() {
    let text = "import \"math.larp\" as math;\nmain_character { math::; }";
    let input = source(text);
    let errors = larp_format::format_unit(&input).unwrap_err();
    assert!(!errors.is_empty());
    assert!(errors.iter().all(|error| error.code.id != "LARP-C001"));
    for error in errors {
        assert_eq!(error.primary.span.source(), SourceId(49));
        assert!(input.slice(error.primary.span).is_some());
    }
    assert_eq!(input.text(), text);
}
