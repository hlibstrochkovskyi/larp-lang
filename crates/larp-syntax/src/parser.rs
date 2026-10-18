use crate::lexer::{self, Kind, Token};
use crate::*;
use larp_diagnostics::Diagnostic;
use larp_source::SourceFile;

type Result<T> = std::result::Result<T, Box<Diagnostic>>;
const MAX_DEPTH: usize = 128;

pub fn parse(source: &SourceFile) -> std::result::Result<Program, Vec<Diagnostic>> {
    let tokens = lexer::lex(source)?;
    let mut parser = Parser {
        source,
        tokens,
        pos: 0,
        depth: 0,
        errors: Vec::new(),
    };
    let mut classes = Vec::new();
    let mut roles = Vec::new();
    let mut functions = Vec::new();
    let mut main = None;
    while !parser.eof() {
        let start = parser.pos;
        let result = if parser.word("sidequest") {
            parser
                .function(true)
                .map(|function| functions.push(function))
        } else if parser.word("role") {
            parser.role().map(|role| roles.push(role))
        } else if parser.word("archetype") {
            parser.class().map(|class| classes.push(class))
        } else if parser.eat_word("main_character") {
            if main.is_some() {
                Err(parser.error(
                    "LARP-P001",
                    "A program must contain exactly one main_character block",
                ))
            } else {
                parser.block().map(|block| main = Some(block))
            }
        } else {
            Err(parser.error(
                "LARP-P002",
                "Expected role, archetype, sidequest, or main_character",
            ))
        };
        if let Err(error) = result {
            parser.errors.push(*error);
            if parser.pos == start {
                parser.advance();
            }
            while !parser.eof()
                && !parser.word("sidequest")
                && !parser.word("archetype")
                && !parser.word("role")
                && !parser.word("main_character")
            {
                parser.advance();
            }
        }
    }
    if main.is_none() {
        parser
            .errors
            .push(*parser.error("LARP-P001", "Missing main_character block"));
    }
    if parser.errors.is_empty() {
        Ok(Program {
            roles,
            classes,
            functions,
            main: main.unwrap(),
        })
    } else {
        Err(parser.errors)
    }
}

struct Parser<'a> {
    source: &'a SourceFile,
    tokens: Vec<Token>,
    pos: usize,
    depth: usize,
    errors: Vec<Diagnostic>,
}

impl Parser<'_> {
    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }
    fn previous(&self) -> &Token {
        &self.tokens[self.pos.saturating_sub(1)]
    }
    fn advance(&mut self) {
        if !self.eof() {
            self.pos += 1;
        }
    }
    fn eof(&self) -> bool {
        self.current().kind == Kind::Eof
    }
    fn word(&self, word: &str) -> bool {
        matches!(&self.current().kind, Kind::Word(value) if value == word)
    }
    fn punct(&self, value: &str) -> bool {
        matches!(&self.current().kind, Kind::Punctuation(found) if *found == value)
    }
    fn eat_word(&mut self, word: &str) -> bool {
        if self.word(word) {
            self.advance();
            true
        } else {
            false
        }
    }
    fn eat(&mut self, value: &str) -> bool {
        if self.punct(value) {
            self.advance();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, value: &str) -> Result<()> {
        if self.eat(value) {
            Ok(())
        } else {
            Err(self.error("LARP-P001", format!("Expected '{value}'")))
        }
    }
    fn expect_word(&mut self, value: &str) -> Result<()> {
        if self.eat_word(value) {
            Ok(())
        } else {
            Err(self.error("LARP-P001", format!("Expected '{value}'")))
        }
    }
    fn error(&self, code: &str, message: impl Into<String>) -> Box<Diagnostic> {
        Box::new(Diagnostic::error(code, self.current().span, message))
    }
    fn joined(&self, start: Span) -> Span {
        self.source
            .span(start.range().start..self.previous().span.range().end)
            .unwrap()
    }
    fn guarded<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        if self.depth >= MAX_DEPTH {
            return Err(self.error("LARP-P003", "Syntax nesting exceeds 128 levels"));
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }
    fn name(&mut self) -> Result<Name> {
        let Kind::Word(text) = &self.current().kind else {
            return Err(self.error("LARP-P001", "Expected an identifier"));
        };
        if matches!(
            text.as_str(),
            "sidequest"
                | "main_character"
                | "let"
                | "if"
                | "else"
                | "return"
                | "npc"
                | "loop"
                | "in"
                | "locked_in"
                | "while"
                | "goon"
                | "touch_grass"
                | "archetype"
                | "role"
                | "highkey"
                | "lowkey"
                | "init"
                | "self"
                | "new"
                | "larp"
                | "as"
                | "flex"
                | "canon"
                | "nonchalant"
                | "sybau"
                | "fn"
                | "uses"
                | "fr"
                | "cap"
                | "no_bitches"
                | "mogg"
        ) {
            return Err(self.error(
                "LARP-P001",
                "A reserved syntax word cannot be used as an identifier",
            ));
        }
        let result = Name {
            text: text.clone(),
            span: self.current().span,
        };
        self.advance();
        Ok(result)
    }
    fn ty(&mut self) -> Result<TypeExpr> {
        self.guarded(|parser| {
            let start = parser.current().span;
            let kind = if parser.eat_word("fn") {
                parser.expect("(")?;
                let mut parameters = Vec::new();
                if !parser.punct(")") {
                    loop {
                        parameters.push(parser.ty()?);
                        if !parser.eat(",") {
                            break;
                        }
                    }
                }
                parser.expect(")")?;
                parser.expect("->")?;
                let result = Box::new(parser.ty()?);
                TypeExprKind::Function {
                    parameters,
                    result,
                    effects: parser.effects()?,
                }
            } else {
                let name = parser.name()?;
                let mut arguments = Vec::new();
                if parser.eat("<") {
                    loop {
                        arguments.push(parser.type_argument()?);
                        if !parser.eat(",") {
                            break;
                        }
                    }
                    parser.expect(">")?;
                }
                TypeExprKind::Named { name, arguments }
            };
            Ok(TypeExpr {
                kind,
                span: parser.joined(start),
            })
        })
    }
    fn type_argument(&mut self) -> Result<TypeExpr> {
        if let Kind::Integer(value) = &self.current().kind {
            let argument = TypeExpr {
                kind: TypeExprKind::Integer(value.clone()),
                span: self.current().span,
            };
            self.advance();
            Ok(argument)
        } else {
            self.ty()
        }
    }
    fn effects(&mut self) -> Result<Option<Vec<Name>>> {
        if !self.eat_word("uses") {
            return Ok(None);
        }
        self.expect("(")?;
        let mut effects = Vec::new();
        if !self.punct(")") {
            loop {
                effects.push(self.name()?);
                if !self.eat(",") {
                    break;
                }
            }
        }
        self.expect(")")?;
        Ok(Some(effects))
    }
    fn function(&mut self, allow_receiver: bool) -> Result<Function> {
        self.expect_word("sidequest")?;
        let ordinary = !self.word("fn")
            && self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|token| token.kind == Kind::Punctuation("("));
        let receiver = if ordinary { None } else { Some(self.ty()?) };
        if receiver.is_some() {
            self.expect(".")?;
        }
        if receiver.is_some() && !allow_receiver {
            return Err(self.error("LARP-P001", "Extensions can only be declared at file scope"));
        }
        let name = self.name()?;
        let parameters = self.parameters()?;
        self.expect("->")?;
        let result = self.ty()?;
        let effects = self.effects()?;
        let body = self.block()?;
        Ok(Function {
            receiver,
            name,
            parameters,
            result,
            effects,
            body,
        })
    }
    fn role(&mut self) -> Result<Role> {
        self.guarded(|parser| {
            let start = parser.current().span;
            parser.expect_word("role")?;
            let name = parser.name()?;
            parser.expect("{")?;
            let mut methods = Vec::new();
            while !parser.eof() && !parser.punct("}") {
                let method_start = parser.pos;
                let result = (|| -> Result<RoleMethod> {
                    let span = parser.current().span;
                    parser.expect_word("sidequest")?;
                    let name = parser.name()?;
                    let parameters = parser.parameters()?;
                    parser.expect("->")?;
                    let result = parser.ty()?;
                    let effects = parser.effects()?;
                    parser.expect(";")?;
                    Ok(RoleMethod {
                        name,
                        parameters,
                        result,
                        effects,
                        span: parser.joined(span),
                    })
                })();
                match result {
                    Ok(method) => methods.push(method),
                    Err(error) => {
                        parser.errors.push(*error);
                        if parser.pos == method_start {
                            parser.advance();
                        }
                        parser.synchronize();
                    }
                }
            }
            parser.expect("}")?;
            Ok(Role {
                name,
                methods,
                span: parser.joined(start),
            })
        })
    }
    fn parameters(&mut self) -> Result<Vec<(TypeExpr, Name)>> {
        self.expect("(")?;
        let mut parameters = Vec::new();
        if !self.punct(")") {
            loop {
                parameters.push((self.ty()?, self.name()?));
                if !self.eat(",") {
                    break;
                }
            }
        }
        self.expect(")")?;
        Ok(parameters)
    }
    fn class(&mut self) -> Result<Class> {
        self.guarded(|parser| {
            let start = parser.current().span;
            parser.expect_word("archetype")?;
            let name = parser.name()?;
            let mut roles = Vec::new();
            if parser.eat(":") {
                loop {
                    roles.push(parser.name()?);
                    if !parser.eat(",") {
                        break;
                    }
                }
            }
            parser.expect("{")?;
            let mut fields = Vec::new();
            let mut methods = Vec::new();
            let mut constructors = Vec::new();
            while !parser.punct("}") && !parser.eof() {
                let member_start = parser.pos;
                let parsed = (|| -> Result<()> {
                    if parser.word("init") {
                        let start = parser.current().span;
                        parser.advance();
                        let parameters = parser.parameters()?;
                        let body = parser.block()?;
                        constructors.push(Constructor {
                            parameters,
                            body,
                            span: parser.joined(start),
                        });
                    } else {
                        let public = if parser.eat_word("highkey") {
                            true
                        } else {
                            parser.eat_word("lowkey");
                            false
                        };
                        if parser.word("sidequest") {
                            methods.push(Method {
                                function: parser.function(false)?,
                                public,
                            });
                        } else {
                            let ty = parser.ty()?;
                            let name = parser.name()?;
                            parser.expect(";")?;
                            fields.push(Field { name, ty, public });
                        }
                    }
                    Ok(())
                })();
                if let Err(error) = parsed {
                    parser.errors.push(*error);
                    if parser.pos == member_start
                        || parser.previous().kind != Kind::Punctuation(";")
                    {
                        parser.synchronize();
                    }
                }
            }
            parser.expect("}")?;
            Ok(Class {
                name,
                roles,
                fields,
                methods,
                constructors,
                span: parser.joined(start),
            })
        })
    }
    fn block(&mut self) -> Result<Block> {
        self.guarded(|parser| {
            let start = parser.current().span;
            parser.expect("{")?;
            let mut statements = Vec::new();
            while !parser.punct("}") && !parser.eof() {
                let statement_start = parser.pos;
                match parser.statement() {
                    Ok(statement) => statements.push(statement),
                    Err(error) => {
                        parser.errors.push(*error);
                        if parser.pos == statement_start
                            || parser.previous().kind != Kind::Punctuation(";")
                        {
                            parser.synchronize();
                        }
                    }
                }
            }
            parser.expect("}")?;
            Ok(Block {
                statements,
                span: parser.joined(start),
            })
        })
    }
    fn synchronize(&mut self) {
        let mut depth = 0;
        while !self.eof() {
            if self.punct("}") {
                if depth == 0 {
                    return;
                }
                depth -= 1;
                self.advance();
                if depth == 0 {
                    return;
                }
            } else if self.eat("{") {
                depth += 1;
            } else if self.eat(";") {
                if depth == 0 {
                    return;
                }
            } else {
                self.advance();
            }
        }
    }
    fn statement(&mut self) -> Result<Statement> {
        self.guarded(Self::statement_inner)
    }
    fn statement_inner(&mut self) -> Result<Statement> {
        let start = self.current().span;
        let kind = if self.punct("{") {
            StatementKind::Block(self.block()?)
        } else if self.eat_word("if") {
            self.expect("(")?;
            let condition = self.expr(0)?;
            self.expect(")")?;
            let then_block = self.block()?;
            let otherwise = if self.eat_word("else") {
                if !self.word("if") && !self.punct("{") {
                    return Err(self.error("LARP-P001", "Expected block or if after else"));
                }
                Some(Box::new(self.statement()?))
            } else {
                None
            };
            StatementKind::If {
                condition,
                then_block,
                otherwise,
            }
        } else if self.eat_word("locked_in") {
            self.expect_word("while")?;
            self.expect("(")?;
            let condition = self.expr(0)?;
            self.expect(")")?;
            StatementKind::While {
                condition,
                body: self.block()?,
            }
        } else if self.eat_word("npc") {
            self.expect_word("loop")?;
            self.expect("(")?;
            let ty = self.ty()?;
            let name = self.name()?;
            self.expect_word("in")?;
            let start = self.expr(0)?;
            self.expect("..")?;
            let end = self.expr(0)?;
            self.expect(")")?;
            StatementKind::Range {
                ty,
                name,
                start,
                end,
                body: self.block()?,
            }
        } else if self.eat_word("goon") {
            StatementKind::Forever(self.block()?)
        } else if self.eat_word("return") {
            let value = if self.punct(";") {
                None
            } else {
                Some(self.expr(0)?)
            };
            self.expect(";")?;
            StatementKind::Return(value)
        } else if self.eat_word("touch_grass") {
            self.expect(";")?;
            StatementKind::Break
        } else if self.eat_word("let") {
            let name = self.name()?;
            self.expect("=")?;
            let value = self.expr(0)?;
            self.expect(";")?;
            StatementKind::Binding {
                ty: None,
                name,
                value,
            }
        } else if matches!(&self.current().kind, Kind::Word(name) if matches!(name.as_str(),
            "larp" | "flex" | "canon" | "nonchalant" | "sybau" | "archetype" | "role"))
        {
            return Err(self.error(
                "LARP-P002",
                "This statement is not implemented in the executable core yet",
            ));
        } else if self.word("fn")
            || (matches!(&self.current().kind, Kind::Word(_))
                && matches!(
                    self.tokens.get(self.pos + 1).map(|t| &t.kind),
                    Some(Kind::Word(_))
                ))
            || (matches!(&self.current().kind, Kind::Word(name) if matches!(name.as_str(), "baddie" | "result" | "mfs" | "hediJeans"))
                && self
                    .tokens
                    .get(self.pos + 1)
                    .is_some_and(|t| t.kind == Kind::Punctuation("<")))
        {
            let ty = self.ty()?;
            let name = self.name()?;
            self.expect("=")?;
            let value = self.expr(0)?;
            self.expect(";")?;
            StatementKind::Binding {
                ty: Some(ty),
                name,
                value,
            }
        } else {
            let expression = self.expr(0)?;
            let assignment = match &self.current().kind {
                Kind::Punctuation("=") => Some(None),
                Kind::Punctuation("+=") => Some(Some(Binary::Add)),
                Kind::Punctuation("-=") => Some(Some(Binary::Subtract)),
                Kind::Punctuation("*=") => Some(Some(Binary::Multiply)),
                Kind::Punctuation("/=") => Some(Some(Binary::Divide)),
                Kind::Punctuation("%=") => Some(Some(Binary::Remainder)),
                _ => None,
            };
            if let Some(op) = assignment {
                self.advance();
                if !Self::is_place(&expression) {
                    return Err(Box::new(Diagnostic::error(
                        "LARP-P001",
                        expression.span,
                        "Assignment requires a binding or a member/index path rooted in a binding",
                    )));
                }
                let value = self.expr(0)?;
                self.expect(";")?;
                StatementKind::Assignment {
                    target: expression,
                    op,
                    value,
                }
            } else {
                self.expect(";")?;
                StatementKind::Expression(expression)
            }
        };
        Ok(Statement {
            kind,
            span: self.joined(start),
        })
    }
    fn is_place(mut expression: &Expr) -> bool {
        loop {
            match &expression.kind {
                ExprKind::Name(_) | ExprKind::SelfValue => return true,
                ExprKind::Member { receiver, .. } | ExprKind::Index { receiver, .. } => {
                    expression = receiver;
                }
                _ => return false,
            }
        }
    }
    fn expr(&mut self, minimum: u8) -> Result<Expr> {
        self.guarded(|parser| parser.expr_inner(minimum))
    }
    fn expr_inner(&mut self, minimum: u8) -> Result<Expr> {
        let token_start = self.pos;
        let start = self.current().span;
        if minimum == 0 && self.looks_like_lambda() {
            return self.lambda();
        }
        let token = self.current().clone();
        self.advance();
        let kind = match token.kind {
            Kind::Integer(value) => ExprKind::Integer(value),
            Kind::Float(value) => ExprKind::Float(value),
            Kind::Text(value) => ExprKind::Text(value),
            Kind::Word(ref word) if word == "fr" || word == "cap" => {
                ExprKind::Verdict(word == "fr")
            }
            Kind::Word(ref word) if word == "self" => ExprKind::SelfValue,
            Kind::Word(ref word) if word == "new" => {
                let class = self.name()?;
                self.expect("(")?;
                let mut arguments = Vec::new();
                if !self.punct(")") {
                    loop {
                        arguments.push(self.expr(0)?);
                        if self.pos - token_start > 256 {
                            return Err(self.error(
                                "LARP-P003",
                                "An expression exceeds the current 256-token limit",
                            ));
                        }
                        if !self.eat(",") {
                            break;
                        }
                    }
                }
                self.expect(")")?;
                ExprKind::New { class, arguments }
            }
            Kind::Word(ref word) if word == "no_bitches" => ExprKind::None,
            Kind::Word(_) => {
                self.pos -= 1;
                ExprKind::Name(self.name()?)
            }
            Kind::Punctuation("(") => {
                let expression = self.expr(0)?;
                self.expect(")")?;
                expression.kind
            }
            Kind::Punctuation(op @ ("!" | "-" | "+")) => ExprKind::Unary {
                op: match op {
                    "!" => Unary::Not,
                    "-" => Unary::Negate,
                    _ => Unary::Positive,
                },
                operand: Box::new(self.expr(13)?),
            },
            Kind::Punctuation("[") => {
                let mut elements = Vec::new();
                if !self.punct("]") {
                    loop {
                        elements.push(self.expr(0)?);
                        if self.pos - token_start > 256 {
                            return Err(self.error(
                                "LARP-P003",
                                "An expression exceeds the current 256-token limit",
                            ));
                        }
                        if !self.eat(",") {
                            break;
                        }
                    }
                }
                self.expect("]")?;
                ExprKind::Array(elements)
            }
            _ => {
                return Err(Box::new(Diagnostic::error(
                    "LARP-P001",
                    token.span,
                    "Expected an expression",
                )));
            }
        };
        let mut left = Expr {
            kind,
            span: self.joined(start),
        };
        let mut non_associative = None;
        loop {
            if self.pos - token_start > 256 {
                return Err(self.error(
                    "LARP-P003",
                    "An expression exceeds the current 256-token limit",
                ));
            }
            let type_arguments = if minimum <= 15 {
                self.try_type_arguments()
            } else {
                None
            };
            if (self.punct("(") || type_arguments.is_some()) && minimum <= 15 {
                self.advance();
                let mut arguments = Vec::new();
                if !self.punct(")") {
                    loop {
                        arguments.push(self.expr(0)?);
                        if !self.eat(",") {
                            break;
                        }
                    }
                }
                self.expect(")")?;
                left = Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(left),
                        arguments,
                        type_arguments: type_arguments.unwrap_or_default(),
                    },
                    span: self.joined(start),
                };
                continue;
            }
            if self.eat(".") {
                let name = self.name()?;
                left = Expr {
                    kind: ExprKind::Member {
                        receiver: Box::new(left),
                        name,
                    },
                    span: self.joined(start),
                };
                continue;
            }
            if minimum <= 15 && self.eat("[") {
                let index = self.expr(0)?;
                self.expect("]")?;
                left = Expr {
                    kind: ExprKind::Index {
                        receiver: Box::new(left),
                        index: Box::new(index),
                    },
                    span: self.joined(start),
                };
                continue;
            }
            if self.punct("=>") {
                return Err(self.error("LARP-P002", "Lambda parameters need parentheses"));
            }
            let Some((op, precedence)) = self.operator() else {
                break;
            };
            if precedence < minimum {
                break;
            }
            if matches!(precedence, 5 | 7) && non_associative == Some(precedence) {
                return Err(self.error(
                    "LARP-P001",
                    "Comparisons cannot be chained; use explicit Boolean operators",
                ));
            }
            self.advance();
            let right = self.expr(precedence + 1)?;
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span: self.joined(start),
            };
            non_associative = Some(precedence);
        }
        Ok(left)
    }
    fn try_type_arguments(&mut self) -> Option<Vec<TypeExpr>> {
        if !self.punct("<") {
            return None;
        }
        let saved = self.pos;
        let result = (|| -> Result<Vec<TypeExpr>> {
            self.expect("<")?;
            let mut arguments = vec![self.type_argument()?];
            while self.eat(",") {
                arguments.push(self.type_argument()?);
            }
            self.expect(">")?;
            if !self.punct("(") {
                return Err(self.error("LARP-P001", "Expected call after type arguments"));
            }
            Ok(arguments)
        })();
        match result {
            Ok(arguments) => Some(arguments),
            Err(_) => {
                self.pos = saved;
                None
            }
        }
    }
    fn looks_like_lambda(&self) -> bool {
        if matches!(self.current().kind, Kind::Word(_))
            && self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|t| t.kind == Kind::Punctuation("=>"))
        {
            return true;
        }
        if !self.punct("(") {
            return false;
        }
        let mut depth = 0;
        for (index, token) in self.tokens.iter().enumerate().skip(self.pos).take(257) {
            if token.kind == Kind::Punctuation("(") {
                depth += 1;
            } else if token.kind == Kind::Punctuation(")") {
                depth -= 1;
                if depth == 0 {
                    return self
                        .tokens
                        .get(index + 1)
                        .is_some_and(|t| t.kind == Kind::Punctuation("=>"));
                }
            }
        }
        false
    }
    fn lambda(&mut self) -> Result<Expr> {
        let start = self.current().span;
        let mut parameters = Vec::new();
        if self.eat("(") {
            if !self.punct(")") {
                loop {
                    let untyped = matches!(self.current().kind, Kind::Word(_))
                        && self
                            .tokens
                            .get(self.pos + 1)
                            .is_some_and(|t| matches!(t.kind, Kind::Punctuation("," | ")")));
                    let ty = if untyped { None } else { Some(self.ty()?) };
                    parameters.push(LambdaParameter {
                        ty,
                        name: self.name()?,
                    });
                    if !self.eat(",") {
                        break;
                    }
                }
            }
            self.expect(")")?;
        } else {
            parameters.push(LambdaParameter {
                ty: None,
                name: self.name()?,
            });
        }
        self.expect("=>")?;
        let body = if self.punct("{") {
            LambdaBody::Block(self.block()?)
        } else {
            LambdaBody::Expression(Box::new(self.expr(0)?))
        };
        Ok(Expr {
            kind: ExprKind::Lambda { parameters, body },
            span: self.joined(start),
        })
    }
    fn operator(&self) -> Option<(Binary, u8)> {
        Some(match &self.current().kind {
            Kind::Punctuation("||") => (Binary::Or, 1),
            Kind::Punctuation("&&") => (Binary::And, 3),
            Kind::Punctuation("==") => (Binary::Equal, 5),
            Kind::Punctuation("!=") => (Binary::NotEqual, 5),
            Kind::Punctuation("<") => (Binary::Less, 7),
            Kind::Punctuation("<=") => (Binary::LessEqual, 7),
            Kind::Punctuation(">") | Kind::Word(_) if self.punct(">") || self.word("mogg") => {
                (Binary::Greater, 7)
            }
            Kind::Punctuation(">=") => (Binary::GreaterEqual, 7),
            Kind::Punctuation("+") => (Binary::Add, 9),
            Kind::Punctuation("-") => (Binary::Subtract, 9),
            Kind::Punctuation("*") => (Binary::Multiply, 11),
            Kind::Punctuation("/") => (Binary::Divide, 11),
            Kind::Punctuation("%") => (Binary::Remainder, 11),
            _ => return None,
        })
    }
}
