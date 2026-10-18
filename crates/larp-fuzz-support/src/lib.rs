//! Shared production-path invariants for coverage-guided fuzzing and stable replay.
//!
//! These oracles never execute arbitrary VM programs: instruction fuel does not
//! bound the memory allocated by a single instruction. Input limits bound test
//! admission, not the resources of every production operation exercised here.

use larp_artifact::{Artifact, ErrorKind};
use larp_bytecode::VerifiedModule;
use larp_diagnostics::Diagnostic;
use larp_source::{SourceFile, SourceId, Span};
use larp_syntax::{CommentKind, Lexed, TokenKind};

/// Admission limit shared by source and binary targets, before source allocation.
pub const MAX_INPUT_BYTES: usize = 64 * 1024;

fn source(bytes: &[u8]) -> Option<SourceFile> {
    if bytes.len() > MAX_INPUT_BYTES {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    Some(SourceFile::new(SourceId(7), "fuzz-input.larp", text))
}

fn span(source: &SourceFile, span: Span) {
    assert!(
        source.slice(span).is_some(),
        "Invalid source span: {span:?}"
    );
    assert_eq!(source.span(span.range()), Some(span));
}

fn diagnostics(source: &SourceFile, diagnostics: &[Diagnostic]) {
    assert!(
        !diagnostics.is_empty(),
        "An error must contain a diagnostic"
    );
    for diagnostic in diagnostics {
        assert_eq!(
            larp_diagnostics::lookup(diagnostic.code.id),
            Some(diagnostic.code),
            "Unregistered diagnostic code"
        );
        assert!(!diagnostic.message.is_empty());
        span(source, diagnostic.primary.span);
        for label in &diagnostic.related {
            span(source, label.span);
        }
    }
}

fn lexed(source: &SourceFile, lexed: &Lexed) {
    let mut previous_end = 0;
    for (index, token) in lexed.tokens.iter().enumerate() {
        span(source, token.span);
        let range = token.span.range();
        assert!(
            range.start >= previous_end,
            "Overlapping/out-of-order tokens"
        );
        if token.kind == TokenKind::Eof {
            assert_eq!(index + 1, lexed.tokens.len(), "EOF must be last");
            assert_eq!(range, source.text().len()..source.text().len());
        } else {
            assert!(!range.is_empty(), "Only EOF may be empty");
        }
        previous_end = range.end;
    }
    assert_eq!(lexed.tokens.last().map(|t| &t.kind), Some(&TokenKind::Eof));
    previous_end = 0;
    for comment in &lexed.comments {
        span(source, comment.span);
        let range = comment.span.range();
        assert!(!range.is_empty());
        assert!(
            range.start >= previous_end,
            "Overlapping/out-of-order comments"
        );
        previous_end = range.end;
    }
    let mut spans: Vec<_> = lexed
        .tokens
        .iter()
        .filter(|token| token.kind != TokenKind::Eof)
        .map(|token| token.span.range())
        .chain(lexed.comments.iter().map(|comment| comment.span.range()))
        .collect();
    spans.sort_unstable_by_key(|range| range.start);
    assert!(
        spans.windows(2).all(|pair| pair[0].end <= pair[1].start),
        "A comment overlaps a token"
    );
}

/// Exercise the lossless scanner and recovering parser, including failed inputs.
pub fn frontend(bytes: &[u8]) {
    let Some(source) = source(bytes) else {
        return;
    };
    match larp_syntax::lex_with_comments(&source) {
        Ok(result) => lexed(&source, &result),
        Err(errors) => diagnostics(&source, &errors),
    }
    if let Err(errors) = larp_syntax::parse(&source) {
        diagnostics(&source, &errors);
    }
}

// Raw spellings and their order are checked independently of the formatter's
// internal comparison. Category keeps comments distinct from program tokens.
fn lexemes<'a>(source: &'a SourceFile, lexed: &Lexed) -> Vec<(u8, &'a str)> {
    let mut items: Vec<_> = lexed
        .tokens
        .iter()
        .filter(|token| token.kind != TokenKind::Eof)
        .map(|token| (token.span.range().start, 0, token.span))
        .chain(lexed.comments.iter().map(|comment| {
            let category = match comment.kind {
                CommentKind::Line => 1,
                CommentKind::Block => 2,
            };
            (comment.span.range().start, category, comment.span)
        }))
        .collect();
    items.sort_unstable_by_key(|item| item.0);
    items
        .into_iter()
        .map(|(_, category, span)| (category, source.slice(span).unwrap()))
        .collect()
}

/// A successful format must parse, preserve every lexeme, and be idempotent.
pub fn formatter(bytes: &[u8]) {
    let Some(source) = source(bytes) else {
        return;
    };
    let formatted = match larp_format::format(&source) {
        Ok(text) => text,
        Err(errors) => {
            diagnostics(&source, &errors);
            assert!(
                errors.iter().all(|error| error.code.id != "LARP-C001"),
                "Formatter invariant failure: {errors:?}"
            );
            return;
        }
    };
    let result = SourceFile::new(SourceId(7), source.name(), formatted.as_str());
    larp_syntax::parse(&result).expect("Successful formatting must produce valid syntax");
    let before = larp_syntax::lex_with_comments(&source)
        .expect("Successfully formatted input must be lexically valid");
    let after = larp_syntax::lex_with_comments(&result)
        .expect("Successful formatting must produce lexically valid output");
    lexed(&source, &before);
    lexed(&result, &after);
    assert_eq!(lexemes(&source, &before), lexemes(&result, &after));
    assert_eq!(
        larp_format::format(&result).expect("Successful output must remain formattable"),
        formatted,
        "Formatting must be idempotent"
    );
}

fn roundtrip(module: &VerifiedModule, sources: &[SourceFile]) -> Vec<u8> {
    let encoded = larp_artifact::encode(module, sources)
        .expect("Accepted artifact must have a canonical encoding");
    let decoded = larp_artifact::decode(&encoded)
        .expect("Canonical encoding must pass loading and verification");
    assert_eq!(module.disassemble(), decoded.module.disassemble());
    assert_eq!(sources.len(), decoded.sources.len());
    for (before, after) in sources.iter().zip(&decoded.sources) {
        assert_eq!(before.name(), after.name());
        assert_eq!(before.text(), after.text());
        assert_eq!(
            before.span(0..0).unwrap().source(),
            after.span(0..0).unwrap().source()
        );
    }
    assert_eq!(
        larp_artifact::encode(&decoded.module, &decoded.sources)
            .expect("Canonical re-encoding must succeed"),
        encoded,
        "Canonical bytes must be deterministic"
    );
    encoded
}

/// Compiler failures are expected; compiler-generated invalid bytecode is a bug.
pub fn compiler(bytes: &[u8]) {
    let Some(source) = source(bytes) else {
        return;
    };
    let compilation = match larp_compiler::compile(&source) {
        Ok(compilation) => compilation,
        Err(errors) => {
            diagnostics(&source, &errors);
            assert!(
                errors.iter().all(|error| error.code.id != "LARP-B001"),
                "Compiler generated invalid bytecode: {errors:?}"
            );
            return;
        }
    };
    let sources = [source];
    match larp_artifact::encode(&compilation.bytecode, &sources) {
        Ok(encoded) => {
            let artifact =
                larp_artifact::decode(&encoded).expect("Compiler artifact must load and verify");
            assert_eq!(
                compilation.bytecode.disassemble(),
                artifact.module.disassemble()
            );
            assert_eq!(roundtrip(&artifact.module, &artifact.sources), encoded);
        }
        Err(error) => assert_eq!(
            error.kind(),
            ErrorKind::ResourceLimit,
            "Compiler output failed encoding for a non-resource reason: {error}"
        ),
    }
}

fn accepted(artifact: Artifact) {
    roundtrip(&artifact.module, &artifact.sources);
}

/// Exercise the bounded binary loader; every accepted candidate is verified.
/// Besides the original bytes, repair only the outer payload length, so mutations
/// that change file size can reach section validation without bypassing checks.
pub fn artifact(bytes: &[u8]) {
    if bytes.len() > MAX_INPUT_BYTES {
        return;
    }
    if let Ok(decoded) = larp_artifact::decode(bytes) {
        accepted(decoded);
    }
    if bytes.len() >= 16 {
        let length = (bytes.len() - 16) as u32;
        if bytes[12..16] != length.to_le_bytes() {
            let mut candidate = bytes.to_vec();
            candidate[12..16].copy_from_slice(&length.to_le_bytes());
            if let Ok(decoded) = larp_artifact::decode(&candidate) {
                accepted(decoded);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEEDS: &[&str] = &[
        include_str!("../../../examples/hello.larp"),
        include_str!("../../../examples/collections.larp"),
        include_str!("../../../examples/roles.larp"),
        include_str!("../../../examples/config-tuning.larp"),
        include_str!("../../../examples/lazy.larp"),
        include_str!("../../../examples/input-sum.larp"),
        include_str!("../../../tests/conformance/fail/io-erasure.larp"),
    ];

    fn mutate(bytes: &[u8], state: &mut u64) -> Vec<u8> {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let mut mutated = bytes.to_vec();
        let index = (*state >> 16) as usize % (bytes.len() + 1);
        match *state % 4 {
            0 if index < bytes.len() => {
                mutated[index] ^= 1 << ((*state >> 32) % 8);
            }
            1 if index < bytes.len() => {
                mutated.remove(index);
            }
            2 => mutated.insert(index, (*state >> 40) as u8),
            _ => mutated.truncate(index),
        }
        mutated
    }

    #[test]
    fn public_source_seeds_and_deterministic_mutations_replay_all_oracles() {
        let mut state = 0x4c41_5250;
        for seed in SEEDS {
            frontend(seed.as_bytes());
            formatter(seed.as_bytes());
            compiler(seed.as_bytes());
            for _ in 0..24 {
                let bytes = mutate(seed.as_bytes(), &mut state);
                frontend(&bytes);
                formatter(&bytes);
                compiler(&bytes);
            }
        }
    }

    #[test]
    fn production_artifacts_and_binary_mutations_replay_loader_oracle() {
        let mut state = 0x4152_5449_4641_4354;
        for seed in &SEEDS[..6] {
            let source = source(seed.as_bytes()).unwrap();
            let compiled = larp_compiler::compile(&source).unwrap();
            let bytes = larp_artifact::encode(&compiled.bytecode, &[source]).unwrap();
            artifact(&bytes);
            for _ in 0..64 {
                artifact(&mutate(&bytes, &mut state));
            }
        }
    }

    #[test]
    fn invalid_utf8_empty_and_oversized_inputs_are_bounded_at_admission() {
        for bytes in [&[][..], &[0xff, 0xfe][..], &vec![b'('; MAX_INPUT_BYTES + 1]] {
            frontend(bytes);
            formatter(bytes);
            compiler(bytes);
            artifact(bytes);
        }
    }

    #[test]
    fn minimized_regressions_replay_the_matching_production_oracle() {
        use std::{fs, path::Path};

        type Oracle = fn(&[u8]);
        let targets: [(&str, &str, Oracle); 4] = [
            ("frontend", "larp", frontend),
            ("formatter", "larp", formatter),
            ("compiler", "larp", compiler),
            ("artifact", "bin", artifact),
        ];
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/regressions");
        for (target, extension, oracle) in targets {
            let directory = root.join(target);
            assert!(
                fs::symlink_metadata(&directory)
                    .expect("Regression target directory must exist")
                    .file_type()
                    .is_dir(),
                "Regression directory must not be a symlink: {}",
                directory.display()
            );
            let mut paths: Vec<_> = fs::read_dir(&directory)
                .expect("Regression target directory must be readable")
                .map(|entry| {
                    entry
                        .expect("Regression directory entry must be readable")
                        .path()
                })
                .collect();
            paths.sort_unstable();
            let mut count = 0;
            for path in paths {
                assert!(
                    fs::symlink_metadata(&path)
                        .expect("Regression metadata must be readable")
                        .file_type()
                        .is_file(),
                    "Regression entry must be a regular file: {}",
                    path.display()
                );
                if path
                    .file_name()
                    .is_some_and(|name| name == ".gitkeep" || name == "README.md")
                {
                    continue;
                }
                assert_eq!(
                    path.extension().and_then(|value| value.to_str()),
                    Some(extension),
                    "Unexpected regression extension: {}",
                    path.display()
                );
                let bytes = fs::read(&path).expect("Regression input must be readable");
                assert!(
                    bytes.len() <= MAX_INPUT_BYTES,
                    "Regression exceeds oracle admission limit: {}",
                    path.display()
                );
                eprintln!("Replaying {}", path.display());
                oracle(&bytes);
                count += 1;
            }
            eprintln!("Replayed {count} saved {target} regressions");
        }
    }
}
