use larp_diagnostics::Diagnostic;
use larp_source::{SourceFile, Span};

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Word(String),
    Integer(String),
    Float(String),
    Text(String),
    Punctuation(&'static str),
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: Kind,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentKind {
    Line,
    Block,
}

/// A complete comment range, including its delimiters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Comment {
    pub kind: CommentKind,
    pub span: Span,
}

/// Tokens retain their decoded values; comments retain exact source ranges.
/// Whitespace is available in the source gaps between tokens and comments.
/// A successful scan ends with one zero-width EOF token at the source's end.
#[derive(Clone, Debug)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    pub comments: Vec<Comment>,
}

pub fn lex(source: &SourceFile) -> Result<Vec<Token>, Vec<Diagnostic>> {
    scan(source, false).map(|lexed| lexed.tokens)
}

/// Scan without discarding comments. Line-comment spans exclude LF and retain
/// an immediately preceding CR, matching the compiler scanner's token boundaries.
pub fn lex_with_comments(source: &SourceFile) -> Result<Lexed, Vec<Diagnostic>> {
    scan(source, true)
}

fn scan(source: &SourceFile, record_comments: bool) -> Result<Lexed, Vec<Diagnostic>> {
    let mut lexer = Lexer {
        source,
        offset: 0,
        tokens: Vec::new(),
        errors: Vec::new(),
        record_comments,
        comments: Vec::new(),
    };
    while lexer.offset < source.text().len() {
        lexer.token();
    }
    lexer.push(Kind::Eof, lexer.offset);
    if lexer.errors.is_empty() {
        Ok(Lexed {
            tokens: lexer.tokens,
            comments: lexer.comments,
        })
    } else {
        Err(lexer.errors)
    }
}

struct Lexer<'a> {
    source: &'a SourceFile,
    offset: usize,
    tokens: Vec<Token>,
    errors: Vec<Diagnostic>,
    record_comments: bool,
    comments: Vec<Comment>,
}

impl Lexer<'_> {
    fn tail(&self) -> &str {
        &self.source.text()[self.offset..]
    }
    fn advance(&mut self) -> char {
        let ch = self.tail().chars().next().unwrap();
        self.offset += ch.len_utf8();
        ch
    }
    fn push(&mut self, kind: Kind, start: usize) {
        self.tokens.push(Token {
            kind,
            span: self.source.span(start..self.offset).unwrap(),
        });
    }
    fn error(&mut self, start: usize, message: &str) {
        self.errors.push(Diagnostic::error(
            "LARP-L001",
            self.source.span(start..self.offset).unwrap(),
            message,
        ));
    }
    fn comment(&mut self, kind: CommentKind, start: usize) {
        if self.record_comments {
            self.comments.push(Comment {
                kind,
                span: self.source.span(start..self.offset).unwrap(),
            });
        }
    }
    fn token(&mut self) {
        let start = self.offset;
        let ch = self.advance();
        if ch.is_ascii_whitespace() {
            return;
        }
        if ch == '/' && self.tail().starts_with('/') {
            while !self.tail().is_empty() && !self.tail().starts_with('\n') {
                self.advance();
            }
            self.comment(CommentKind::Line, start);
            return;
        }
        if ch == '/' && self.tail().starts_with('*') {
            self.advance();
            while !self.tail().is_empty() && !self.tail().starts_with("*/") {
                self.advance();
            }
            if self.tail().is_empty() {
                self.error(start, "Unterminated block comment");
            } else {
                self.offset += 2;
                self.comment(CommentKind::Block, start);
            }
            return;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            while self
                .tail()
                .starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
            {
                self.advance();
            }
            self.push(
                Kind::Word(self.source.text()[start..self.offset].into()),
                start,
            );
            return;
        }
        if ch.is_ascii_digit() {
            self.number(start);
            return;
        }
        if ch == '"' {
            self.string(start);
            return;
        }
        for punctuation in [
            "::", "->", "=>", "..", "==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=", "/=",
            "%=", "{", "}", "(", ")", "[", "]", ";", ":", ",", ".", "+", "-", "*", "/", "%", "!",
            "=", "<", ">",
        ] {
            if self.source.text()[start..].starts_with(punctuation) {
                self.offset = start + punctuation.len();
                self.push(Kind::Punctuation(punctuation), start);
                return;
            }
        }
        self.error(start, "Invalid source character");
    }
    fn digits(&mut self) {
        while self
            .tail()
            .starts_with(|c: char| c.is_ascii_digit() || c == '_')
        {
            self.advance();
        }
    }
    fn number(&mut self, start: usize) {
        self.digits();
        let mut float = false;
        if self.tail().starts_with('.')
            && self.tail()[1..].starts_with(|c: char| c.is_ascii_digit())
        {
            float = true;
            self.advance();
            self.digits();
        }
        if self.tail().starts_with(['e', 'E']) {
            float = true;
            self.advance();
            if self.tail().starts_with(['+', '-']) {
                self.advance();
            }
            self.digits();
        }
        if self
            .tail()
            .starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        {
            while self
                .tail()
                .starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
            {
                self.advance();
            }
            self.error(start, "Invalid numeric literal");
            return;
        }
        let raw = &self.source.text()[start..self.offset];
        let bytes = raw.as_bytes();
        if bytes.iter().enumerate().any(|(i, b)| {
            *b == b'_'
                && (i == 0
                    || i + 1 == bytes.len()
                    || !bytes[i - 1].is_ascii_digit()
                    || !bytes[i + 1].is_ascii_digit())
        }) {
            self.error(start, "Numeric separators must occur between digits");
            return;
        }
        let value = raw.replace('_', "");
        if float && value.parse::<f64>().is_err() {
            self.error(start, "Malformed floating-point literal");
            return;
        }
        self.push(
            if float {
                Kind::Float(value)
            } else {
                Kind::Integer(value)
            },
            start,
        );
    }
    fn string(&mut self, start: usize) {
        let mut value = String::new();
        let mut valid = true;
        while !self.tail().is_empty() {
            match self.advance() {
                '"' => {
                    if valid {
                        self.push(Kind::Text(value), start);
                    }
                    return;
                }
                '\n' | '\r' => {
                    self.error(start, "Literal newline in string");
                    return;
                }
                '\\' => {
                    if self.tail().is_empty() {
                        break;
                    }
                    let escaped = match self.advance() {
                        '"' => '"',
                        '\\' => '\\',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        _ => {
                            self.error(start, "Unknown string escape");
                            valid = false;
                            continue;
                        }
                    };
                    value.push(escaped);
                }
                c => value.push(c),
            }
        }
        self.error(start, "Unterminated string literal");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larp_source::SourceId;

    fn same_semantics(text: &str) {
        let source = SourceFile::new(SourceId(0), "lexer.larp", text);
        match (lex(&source), lex_with_comments(&source)) {
            (Ok(ordinary), Ok(lossless)) => {
                assert_eq!(ordinary.len(), lossless.tokens.len());
                for (left, right) in ordinary.iter().zip(&lossless.tokens) {
                    assert_eq!(left.kind, right.kind);
                    assert_eq!(left.span, right.span);
                }
            }
            (Err(ordinary), Err(lossless)) => {
                assert_eq!(ordinary.len(), lossless.len());
                for (left, right) in ordinary.iter().zip(&lossless) {
                    assert_eq!(left.code.id, right.code.id);
                    assert_eq!(left.primary.span, right.primary.span);
                    assert_eq!(left.primary.message, right.primary.message);
                }
            }
            _ => panic!("Comment retention changed lexical acceptance for {text:?}"),
        }
    }

    #[test]
    fn compiler_and_lossless_scans_have_identical_tokens_diagnostics_and_source_positions() {
        for text in [
            "main_character { npc/* pair */loop(mf i in 0..2){} }",
            "// comment\r\n/* 🦀 */\"// text\"",
            "1__0 /* comment */",
            "/* unterminated",
            "\"\\q\"",
            "0/*a*/.5",
            "1e/*a*/2",
        ] {
            same_semantics(text);
        }
        let alphabet = [
            "a", "1", "_", "/", "*", "\"", "\\", "\n", "\r", " ", "🦀", "+", "-", ".", "(", ")",
            "{", "}",
        ];
        let mut state = 49_u64;
        for _ in 0..1000 {
            let mut text = String::new();
            for _ in 0..64 {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                text.push_str(alphabet[(state >> 32) as usize % alphabet.len()]);
            }
            same_semantics(&text);
        }
    }

    #[test]
    fn compiler_scan_does_not_allocate_comment_records() {
        let source = SourceFile::new(
            SourceId(0),
            "lexer.larp",
            "// one\n/* two */ main_character {} // three",
        );
        let ordinary = scan(&source, false).unwrap();
        assert!(ordinary.comments.is_empty());
        assert_eq!(ordinary.comments.capacity(), 0);
        assert_eq!(scan(&source, true).unwrap().comments.len(), 3);
    }
}
