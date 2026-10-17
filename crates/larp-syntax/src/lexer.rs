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

pub fn lex(source: &SourceFile) -> Result<Vec<Token>, Vec<Diagnostic>> {
    let mut lexer = Lexer {
        source,
        offset: 0,
        tokens: Vec::new(),
        errors: Vec::new(),
    };
    while lexer.offset < source.text().len() {
        lexer.token();
    }
    lexer.push(Kind::Eof, lexer.offset);
    if lexer.errors.is_empty() {
        Ok(lexer.tokens)
    } else {
        Err(lexer.errors)
    }
}

struct Lexer<'a> {
    source: &'a SourceFile,
    offset: usize,
    tokens: Vec<Token>,
    errors: Vec<Diagnostic>,
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
            "->", "=>", "..", "==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=", "/=", "%=",
            "{", "}", "(", ")", "[", "]", ";", ":", ",", ".", "+", "-", "*", "/", "%", "!", "=",
            "<", ">",
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
