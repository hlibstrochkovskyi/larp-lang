//! Source identities, validated byte spans, and human-readable locations.

use std::ops::Range;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SourceId(pub u32);

/// A half-open UTF-8 byte range validated by its owning source file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    source: SourceId,
    start: usize,
    end: usize,
}

impl Span {
    pub fn source(self) -> SourceId {
        self.source
    }

    pub fn range(self) -> Range<usize> {
        self.start..self.end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug)]
pub struct SourceFile {
    id: SourceId,
    name: String,
    text: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    /// The caller assigns a unique ID within a compilation's source map.
    pub fn new(id: SourceId, name: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        let mut line_starts = vec![0];
        line_starts.extend(text.match_indices('\n').map(|(offset, _)| offset + 1));
        Self {
            id,
            name: name.into(),
            text,
            line_starts,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn span(&self, range: Range<usize>) -> Option<Span> {
        self.text.get(range.clone())?;
        Some(Span {
            source: self.id,
            start: range.start,
            end: range.end,
        })
    }

    pub fn slice(&self, span: Span) -> Option<&str> {
        if span.source != self.id {
            return None;
        }
        self.text.get(span.range())
    }

    /// Columns count Unicode scalar values, not bytes or terminal cells.
    pub fn location(&self, offset: usize) -> Option<Location> {
        if !self.text.is_char_boundary(offset) {
            return None;
        }
        let line_index = self.line_starts.partition_point(|start| *start <= offset) - 1;
        let column = self.text[self.line_starts[line_index]..offset]
            .chars()
            .count()
            + 1;
        Some(Location {
            line: line_index + 1,
            column,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_ranges_and_locations_are_not_confused_with_bytes() {
        let file = SourceFile::new(SourceId(7), "unicode.larp", "a🦀я\nnext");
        let span = file.span(1..7).unwrap();
        assert_eq!(file.slice(span), Some("🦀я"));
        assert_eq!(file.location(5), Some(Location { line: 1, column: 3 }));
        assert_eq!(file.location(8), Some(Location { line: 2, column: 1 }));
        assert!(file.span(2..5).is_none());
        assert!(file.location(2).is_none());
    }

    #[test]
    fn eof_empty_sources_and_crlf_are_supported() {
        let empty = SourceFile::new(SourceId(0), "empty", "");
        assert_eq!(empty.location(0), Some(Location { line: 1, column: 1 }));
        assert!(empty.span(0..0).is_some());
        assert!(empty.location(1).is_none());
        let file = SourceFile::new(SourceId(1), "crlf", "x\r\ny\n");
        assert_eq!(file.location(3), Some(Location { line: 2, column: 1 }));
        assert_eq!(file.location(5), Some(Location { line: 3, column: 1 }));
        assert!(file.span(0..6).is_none());
        let reversed = Range { start: 3, end: 1 };
        assert!(file.span(reversed).is_none());
    }

    #[test]
    fn spans_cannot_be_used_with_another_source() {
        let first = SourceFile::new(SourceId(1), "one", "value");
        let second = SourceFile::new(SourceId(2), "two", "other");
        assert!(second.slice(first.span(0..5).unwrap()).is_none());
    }
}
