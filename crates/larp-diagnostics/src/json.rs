//! Versioned JSON Lines rendering without changing canonical diagnostic data.

use crate::{Diagnostic, Label, Severity};
use larp_source::SourceFile;
use std::fmt::Write;

fn quoted(text: &str) -> String {
    let mut output = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            c if c <= '\u{1f}' => write!(output, "\\u{:04x}", c as u32).unwrap(),
            c => output.push(c),
        }
    }
    output.push('"');
    output
}

fn optional(text: Option<&str>) -> String {
    text.map_or_else(|| "null".into(), quoted)
}

fn label(item: &Label, sources: &[SourceFile]) -> String {
    let span = item.span;
    let range = span.range();
    let source = sources.iter().find(|source| source.slice(span).is_some());
    let location = |offset| {
        source
            .and_then(|source| source.location(offset))
            .map_or_else(
                || "null".into(),
                |position| {
                    format!(
                        "{{\"line\":{},\"column\":{}}}",
                        position.line, position.column
                    )
                },
            )
    };
    format!(
        "{{\"source_id\":{},\"path\":{},\"start\":{},\"end\":{},\"start_location\":{},\"end_location\":{},\"text\":{},\"message\":{}}}",
        span.source().0,
        optional(source.map(SourceFile::name)),
        range.start,
        range.end,
        location(range.start),
        location(range.end),
        optional(source.and_then(|source| source.slice(span))),
        quoted(&item.message),
    )
}

/// Render one diagnostic record, including a terminating LF.
pub fn render(diagnostic: &Diagnostic, sources: &[SourceFile]) -> String {
    let related = diagnostic
        .related
        .iter()
        .map(|item| label(item, sources))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema_version\":1,\"kind\":\"diagnostic\",\"severity\":{},\"code\":{},\"title\":{},\"message\":{},\"primary\":{},\"related\":[{}],\"expected_type\":{},\"found_type\":{},\"help\":{}}}\n",
        quoted(match diagnostic.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }),
        quoted(diagnostic.code.id),
        quoted(diagnostic.code.title),
        quoted(&diagnostic.message),
        label(&diagnostic.primary, sources),
        related,
        optional(diagnostic.expected_type.as_deref()),
        optional(diagnostic.found_type.as_deref()),
        optional(diagnostic.help.as_deref()),
    )
}

/// Render an event without a source span, such as inspection or CLI failure.
pub fn event(kind: &str, code: Option<&str>, message: &str) -> String {
    format!(
        "{{\"schema_version\":1,\"kind\":{},\"code\":{},\"message\":{}}}\n",
        quoted(kind),
        optional(code),
        quoted(message),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Label;
    use larp_source::SourceId;

    #[test]
    fn strings_escape_controls_but_preserve_unicode_and_record_boundaries() {
        assert_eq!(
            quoted("\"\\\n\r\t\0\u{1f}🦀"),
            "\"\\\"\\\\\\n\\r\\t\\u0000\\u001f🦀\""
        );
        assert_eq!(
            event("inspection", None, "a\nb"),
            "{\"schema_version\":1,\"kind\":\"inspection\",\"code\":null,\"message\":\"a\\nb\"}\n"
        );
    }

    #[test]
    fn labels_resolve_their_own_sources_and_keep_byte_and_scalar_coordinates() {
        let first = SourceFile::new(SourceId(7), "a\".larp", "\t🦀x\r\n");
        let second = SourceFile::new(SourceId(9), "b.larp", "other");
        let mut diagnostic =
            Diagnostic::error("LARP-T001", first.span(1..6).unwrap(), "bad\nvalue");
        diagnostic.related.push(Label {
            span: second.span(0..5).unwrap(),
            message: "declared here".into(),
        });
        diagnostic.expected_type = Some("unc".into());
        diagnostic.found_type = Some("mf".into());
        let output = render(&diagnostic, &[second, first]);
        assert!(output.contains("\"source_id\":7,\"path\":\"a\\\".larp\",\"start\":1,\"end\":6,\"start_location\":{\"line\":1,\"column\":2},\"end_location\":{\"line\":1,\"column\":4},\"text\":\"🦀x\""));
        assert!(output.contains("\"source_id\":9,\"path\":\"b.larp\""));
        assert!(output.contains("\"expected_type\":\"unc\",\"found_type\":\"mf\",\"help\":null"));
        assert_eq!(output.bytes().filter(|byte| *byte == b'\n').count(), 1);
        let missing = render(&diagnostic, &[]);
        assert!(missing.contains("\"path\":null,\"start\":1,\"end\":6,\"start_location\":null,\"end_location\":null,\"text\":null"));
        let multiline = SourceFile::new(SourceId(4), "multiline.larp", "🦀\r\nnext");
        let mut warning = Diagnostic::error(
            "LARP-T001",
            multiline.span(0..multiline.text().len()).unwrap(),
            "example",
        );
        warning.severity = Severity::Warning;
        let output = render(&warning, &[multiline]);
        assert!(output.contains("\"severity\":\"warning\""));
        assert!(output.contains("\"end_location\":{\"line\":2,\"column\":5}"));
    }
}
