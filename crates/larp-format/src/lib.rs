//! Lossless, syntax-aware source formatting with an independently checked result.
//!
//! AST annotations distinguish generic delimiters and unary operators; a concrete
//! delimiter tree retains parentheses, original token spellings, and comment bytes.

use larp_diagnostics::Diagnostic;
use larp_source::{SourceFile, Span};
use larp_syntax::{
    Block, Class, CommentKind, Expr, ExprKind, Function, LambdaBody, Program, Role, SourceUnit,
    Statement, StatementKind, Token, TokenKind, TypeExpr, TypeExprKind, lex_with_comments, parse,
    parse_unit,
};
use larp_types::Unary;

const WIDTH: usize = 100;
/// Maximum source bytes admitted by the formatter and its CLI reader.
pub const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
// Every successful output must remain a valid input for a second formatting pass.
const MAX_OUTPUT: usize = MAX_SOURCE_BYTES;
const MAX_LEAVES: usize = 250_000;
const MAX_DEPTH: usize = 512;
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

fn diagnostic(source: &SourceFile, id: &str, message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(id, source.span(0..0).unwrap(), message)]
}

/// Format parseable LARP source without requiring semantic validity.
/// No output is returned if parsing, resource limits, or preservation checks fail.
pub fn format(source: &SourceFile) -> Result<String> {
    format_source(source, Syntax::Program)
}

/// Format an opt-in project source unit, including libraries without main.
/// Imports and exported declarations are preserved without resolving any files.
/// The legacy [`format`] entry point retains its single-file grammar.
pub fn format_unit(source: &SourceFile) -> Result<String> {
    format_source(source, Syntax::Unit)
}

#[derive(Clone, Copy)]
enum Syntax {
    Program,
    Unit,
}

fn format_source(source: &SourceFile, syntax: Syntax) -> Result<String> {
    if source.text().len() > MAX_SOURCE_BYTES {
        return Err(diagnostic(
            source,
            "LARP-P003",
            "Formatter source size limit exceeded",
        ));
    }
    let lexed = lex_with_comments(source)?;
    if lexed.tokens.len() + lexed.comments.len() > MAX_LEAVES {
        return Err(diagnostic(
            source,
            "LARP-P003",
            "Formatter token count limit exceeded",
        ));
    }
    let mut annotations = Annotations::new(source, &lexed.tokens);
    match syntax {
        Syntax::Program => annotations.program(&parse(source)?),
        Syntax::Unit => annotations.unit(&parse_unit(source)?),
    }
    let mut leaves = Vec::with_capacity(lexed.tokens.len() + lexed.comments.len());
    let (mut token, mut comment) = (0, 0);
    while token < lexed.tokens.len() || comment < lexed.comments.len() {
        let next_token = lexed.tokens.get(token);
        let next_comment = lexed.comments.get(comment);
        if next_comment
            .is_some_and(|c| next_token.is_none_or(|t| c.span.range().start < t.span.range().start))
        {
            let item = next_comment.unwrap();
            leaves.push(Leaf {
                raw: source.slice(item.span).unwrap(),
                span: item.span,
                kind: Kind::Comment(item.kind),
                flags: Flags::default(),
                inline_before: false,
                newline_after: false,
                blank_before: false,
            });
            comment += 1;
        } else {
            let item = next_token.unwrap();
            if item.kind != TokenKind::Eof {
                leaves.push(Leaf {
                    raw: source.slice(item.span).unwrap(),
                    span: item.span,
                    kind: Kind::Token,
                    flags: annotations.flags[token],
                    inline_before: false,
                    newline_after: false,
                    blank_before: false,
                });
            }
            token += 1;
        }
    }
    for index in 0..leaves.len() {
        let before = if index == 0 {
            0
        } else {
            leaves[index - 1].span.range().end
        };
        let after = leaves
            .get(index + 1)
            .map_or(source.text().len(), |leaf| leaf.span.range().start);
        let gap = &source.text()[before..leaves[index].span.range().start];
        leaves[index].inline_before = index != 0 && !gap.contains('\n');
        leaves[index].blank_before = gap.bytes().filter(|byte| *byte == b'\n').take(2).count() == 2;
        leaves[index].newline_after =
            source.text()[leaves[index].span.range().end..after].contains('\n');
    }
    let mut position = 0;
    let nodes = tree(&leaves, &mut position, None, 0).map_err(|message| {
        diagnostic(
            source,
            if message == "Formatter delimiter depth limit exceeded" {
                "LARP-P003"
            } else {
                "LARP-C001"
            },
            message,
        )
    })?;
    let mut renderer = Renderer::default();
    renderer
        .sequence(&nodes, Layout::Statements, true)
        .map_err(|message| diagnostic(source, "LARP-P003", message))?;
    renderer
        .newline()
        .map_err(|message| diagnostic(source, "LARP-P003", message))?;
    let result = renderer.output;
    let checked = SourceFile::new(
        source.span(0..0).unwrap().source(),
        source.name(),
        result.as_str(),
    );
    let reparsed = match syntax {
        Syntax::Program => parse(&checked).is_ok(),
        Syntax::Unit => parse_unit(&checked).is_ok(),
    };
    if !reparsed || !same_lexemes(source, &checked) {
        return Err(diagnostic(
            source,
            "LARP-C001",
            "Formatter preservation invariant failed",
        ));
    }
    Ok(result)
}

#[derive(Clone, Copy, Default)]
struct Flags {
    angle_open: bool,
    angle_close: bool,
    unary: bool,
}
struct Annotations<'a> {
    source: &'a SourceFile,
    tokens: &'a [Token],
    flags: Vec<Flags>,
}
impl<'a> Annotations<'a> {
    fn new(source: &'a SourceFile, tokens: &'a [Token]) -> Self {
        Self {
            source,
            tokens,
            flags: vec![Flags::default(); tokens.len()],
        }
    }
    fn mark(&mut self, start: usize, end: usize, raw: &str, reverse: bool, set: fn(&mut Flags)) {
        let first = self
            .tokens
            .partition_point(|token| token.span.range().start < start);
        let last = self
            .tokens
            .partition_point(|token| token.span.range().start < end);
        let found = if reverse {
            (first..last)
                .rev()
                .find(|index| self.source.slice(self.tokens[*index].span) == Some(raw))
        } else {
            (first..last).find(|index| self.source.slice(self.tokens[*index].span) == Some(raw))
        };
        if let Some(index) = found {
            set(&mut self.flags[index]);
        }
    }
    fn angles(&mut self, start: usize, end: usize, arguments: &[TypeExpr]) {
        if let (Some(first), Some(last)) = (arguments.first(), arguments.last()) {
            self.mark(start, first.span.range().start, "<", true, |flags| {
                flags.angle_open = true
            });
            self.mark(last.span.range().end, end, ">", false, |flags| {
                flags.angle_close = true
            });
        }
        for argument in arguments {
            self.ty(argument);
        }
    }
    fn ty(&mut self, ty: &TypeExpr) {
        match &ty.kind {
            TypeExprKind::Named { name, arguments } => {
                self.angles(name.span.range().end, ty.span.range().end, arguments)
            }
            TypeExprKind::Function {
                parameters, result, ..
            } => {
                for parameter in parameters {
                    self.ty(parameter);
                }
                self.ty(result);
            }
            TypeExprKind::Integer(_) => {}
        }
    }
    fn function(&mut self, function: &Function) {
        if let Some(receiver) = &function.receiver {
            self.ty(receiver);
        }
        for (ty, _) in &function.parameters {
            self.ty(ty);
        }
        self.ty(&function.result);
        self.block(&function.body);
    }
    fn program(&mut self, program: &Program) {
        for role in &program.roles {
            self.role(role);
        }
        for class in &program.classes {
            self.class(class);
        }
        for function in &program.functions {
            self.function(function);
        }
        self.block(&program.main);
    }
    fn unit(&mut self, unit: &SourceUnit) {
        for role in &unit.roles {
            self.role(&role.item);
        }
        for class in &unit.classes {
            self.class(&class.item);
        }
        for function in &unit.functions {
            self.function(&function.item);
        }
        if let Some(main) = &unit.main {
            self.block(main);
        }
    }
    fn role(&mut self, role: &Role) {
        for method in &role.methods {
            for (ty, _) in &method.parameters {
                self.ty(ty);
            }
            self.ty(&method.result);
        }
    }
    fn class(&mut self, class: &Class) {
        for field in &class.fields {
            self.ty(&field.ty);
        }
        for method in &class.methods {
            self.function(&method.function);
        }
        for constructor in &class.constructors {
            for (ty, _) in &constructor.parameters {
                self.ty(ty);
            }
            self.block(&constructor.body);
        }
    }
    fn block(&mut self, block: &Block) {
        for statement in &block.statements {
            self.statement(statement);
        }
    }
    fn statement(&mut self, statement: &Statement) {
        match &statement.kind {
            StatementKind::Binding { ty, value, .. } => {
                if let Some(ty) = ty {
                    self.ty(ty);
                }
                self.expr(value);
            }
            StatementKind::Lazy { ty, value, .. } => {
                self.ty(ty);
                self.expr(value);
            }
            StatementKind::Assignment { target, value, .. } => {
                self.expr(target);
                self.expr(value);
            }
            StatementKind::Expression(value) | StatementKind::Flex { value, .. } => {
                self.expr(value)
            }
            StatementKind::Block(block)
            | StatementKind::Forever(block)
            | StatementKind::Sybau(block)
            | StatementKind::Scenario { body: block, .. } => self.block(block),
            StatementKind::If {
                condition,
                then_block,
                otherwise,
            } => {
                self.expr(condition);
                self.block(then_block);
                if let Some(otherwise) = otherwise {
                    self.statement(otherwise);
                }
            }
            StatementKind::While { condition, body } => {
                self.expr(condition);
                self.block(body);
            }
            StatementKind::Range {
                ty,
                start,
                end,
                body,
                ..
            } => {
                self.ty(ty);
                self.expr(start);
                self.expr(end);
                self.block(body);
            }
            StatementKind::Return(value) => {
                if let Some(value) = value {
                    self.expr(value);
                }
            }
            StatementKind::Canon { .. } | StatementKind::Break => {}
        }
    }
    fn expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Unary { op, operand } => {
                let raw = match op {
                    Unary::Negate => "-",
                    Unary::Positive => "+",
                    Unary::Not => "!",
                };
                self.mark(
                    expr.span.range().start,
                    operand.span.range().start,
                    raw,
                    true,
                    |flags| flags.unary = true,
                );
                self.expr(operand);
            }
            ExprKind::Binary { left, right, .. } => {
                self.expr(left);
                self.expr(right);
            }
            ExprKind::New { arguments, .. } | ExprKind::Array(arguments) => {
                for value in arguments {
                    self.expr(value);
                }
            }
            ExprKind::Call {
                callee,
                arguments,
                type_arguments,
            } => {
                self.angles(
                    callee.span.range().end,
                    expr.span.range().end,
                    type_arguments,
                );
                self.expr(callee);
                for argument in arguments {
                    self.expr(argument);
                }
            }
            ExprKind::Member { receiver, .. } => self.expr(receiver),
            ExprKind::Index { receiver, index } => {
                self.expr(receiver);
                self.expr(index);
            }
            ExprKind::Lambda { parameters, body } => {
                for parameter in parameters {
                    if let Some(ty) = &parameter.ty {
                        self.ty(ty);
                    }
                }
                match body {
                    LambdaBody::Expression(expr) => self.expr(expr),
                    LambdaBody::Block(block) => self.block(block),
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Token,
    Comment(CommentKind),
}
#[derive(Clone, Copy)]
struct Leaf<'a> {
    raw: &'a str,
    span: Span,
    kind: Kind,
    flags: Flags,
    inline_before: bool,
    newline_after: bool,
    blank_before: bool,
}
impl Leaf<'_> {
    fn comment(self) -> bool {
        matches!(self.kind, Kind::Comment(_))
    }
    fn open(self) -> Option<&'static str> {
        if self.comment() {
            None
        } else if self.flags.angle_open {
            Some(">")
        } else {
            match self.raw {
                "(" => Some(")"),
                "[" => Some("]"),
                "{" => Some("}"),
                _ => None,
            }
        }
    }
    fn close(self) -> bool {
        !self.comment() && (self.flags.angle_close || matches!(self.raw, ")" | "]" | "}"))
    }
}
struct Node<'a> {
    first: Leaf<'a>,
    last: Leaf<'a>,
    children: Option<Vec<Node<'a>>>,
    flat: Option<usize>,
}
fn tree<'a>(
    leaves: &[Leaf<'a>],
    position: &mut usize,
    end: Option<&str>,
    depth: usize,
) -> std::result::Result<Vec<Node<'a>>, &'static str> {
    if depth > MAX_DEPTH {
        return Err("Formatter delimiter depth limit exceeded");
    }
    let mut nodes = Vec::new();
    while let Some(leaf) = leaves.get(*position).copied() {
        if leaf.close() {
            if end == Some(leaf.raw) {
                return Ok(nodes);
            }
            return Err("Formatter encountered an unmatched closing delimiter");
        }
        *position += 1;
        if let Some(close) = leaf.open() {
            let children = tree(leaves, position, Some(close), depth + 1)?;
            let last = leaves
                .get(*position)
                .copied()
                .ok_or("Formatter encountered an unclosed delimiter")?;
            *position += 1;
            let flat = if leaf.raw == "{" {
                None
            } else {
                measure(leaf, &children, last)
            };
            nodes.push(Node {
                first: leaf,
                last,
                children: Some(children),
                flat,
            });
        } else {
            let flat = if matches!(leaf.kind, Kind::Comment(CommentKind::Line))
                || leaf.raw.contains('\n')
                || (leaf.comment() && (!leaf.inline_before || leaf.newline_after))
            {
                None
            } else {
                Some(leaf.raw.chars().take(WIDTH + 1).count())
            };
            nodes.push(Node {
                first: leaf,
                last: leaf,
                children: None,
                flat,
            });
        }
    }
    if end.is_some() {
        Err("Formatter encountered an unclosed delimiter")
    } else {
        Ok(nodes)
    }
}
fn measure(first: Leaf<'_>, nodes: &[Node<'_>], last: Leaf<'_>) -> Option<usize> {
    let mut width = first.raw.len();
    let mut previous = first;
    for node in nodes {
        width = (width + usize::from(space(previous, node.first)) + node.flat?).min(WIDTH + 1);
        previous = node.last;
    }
    Some((width + usize::from(space(previous, last)) + last.raw.len()).min(WIDTH + 1))
}
fn space(previous: Leaf<'_>, next: Leaf<'_>) -> bool {
    if previous.comment() || next.comment() {
        return true;
    }
    let (left, right) = (previous.raw, next.raw);
    if next.flags.angle_close || matches!(right, ")" | "]" | "," | ";" | "." | "::") {
        return false;
    }
    if previous.flags.angle_open || matches!(left, "(" | "[" | "." | "::") {
        return false;
    }
    if left == "," {
        return true;
    }
    if next.flags.angle_open {
        return false;
    }
    if previous.flags.unary {
        return matches!((left, right), ("-", "-") | ("+", "+"));
    }
    if right == "(" {
        return matches!(
            left,
            "if" | "while" | "loop" | "larp" | "return" | "in" | "="
        ) || is_operator(previous);
    }
    if right == "[" {
        return matches!(left, "return" | "in") || is_operator(previous);
    }
    true
}
fn is_operator(leaf: Leaf<'_>) -> bool {
    !leaf.comment()
        && !leaf.flags.angle_close
        && !leaf.flags.angle_open
        && matches!(
            leaf.raw,
            "+" | "-"
                | "*"
                | "/"
                | "%"
                | "!"
                | "="
                | "+="
                | "-="
                | "*="
                | "/="
                | "%="
                | "<"
                | ">"
                | "<="
                | ">="
                | "=="
                | "!="
                | "&&"
                | "||"
                | "mogg"
                | "=>"
                | "->"
                | ".."
        )
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Layout {
    Statements,
    Flat,
    BrokenList,
}
#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum Break {
    #[default]
    None,
    Required,
    AfterBlock,
}
#[derive(Default)]
struct Renderer<'a> {
    output: String,
    indent: usize,
    column: usize,
    previous: Option<Leaf<'a>>,
    pending_break: Break,
}
impl<'a> Renderer<'a> {
    fn append(&mut self, raw: &str) -> std::result::Result<(), &'static str> {
        if raw.len() > MAX_OUTPUT.saturating_sub(self.output.len()) {
            return Err("Formatter output size limit exceeded");
        }
        self.output.push_str(raw);
        self.column = raw
            .rsplit_once('\n')
            .map_or(self.column + raw.chars().count(), |(_, tail)| {
                tail.chars().count()
            });
        Ok(())
    }
    fn newline(&mut self) -> std::result::Result<(), &'static str> {
        if !self.output.ends_with('\n') {
            self.append("\n")?;
        }
        self.column = 0;
        self.pending_break = Break::None;
        Ok(())
    }
    fn indentation(&mut self) -> std::result::Result<(), &'static str> {
        if self.column == 0 {
            self.append(&" ".repeat(self.indent * 4))?;
        }
        Ok(())
    }
    fn before(
        &mut self,
        leaf: Leaf<'a>,
        layout: Layout,
        root: bool,
    ) -> std::result::Result<(), &'static str> {
        if self.pending_break != Break::None {
            let continuation = self.pending_break == Break::AfterBlock
                && !leaf.comment()
                && matches!(leaf.raw, "else" | ";" | "," | ")" | "]" | "." | "(" | "[");
            if !(continuation || leaf.comment() && leaf.inline_before && self.column != 0) {
                self.newline()?;
                if root
                    && self.previous.is_some_and(|previous| previous.raw == "}")
                    && !self.output.ends_with("\n\n")
                {
                    self.append("\n")?;
                }
            } else if continuation {
                self.pending_break = Break::None;
            }
        }
        if leaf.comment() && !leaf.inline_before && self.column != 0 {
            self.newline()?;
        }
        if layout == Layout::Statements
            && leaf.blank_before
            && self.column == 0
            && !self.output.is_empty()
            && !self.output.ends_with("\n\n")
            && self.previous.is_some_and(|previous| previous.raw != "{")
        {
            self.append("\n")?;
        }
        self.indentation()?;
        if self.column != 0
            && !self.output.ends_with([' ', '\n'])
            && self.previous.is_some_and(|previous| space(previous, leaf))
        {
            self.append(" ")?;
        }
        Ok(())
    }
    fn sequence(
        &mut self,
        nodes: &[Node<'a>],
        layout: Layout,
        root: bool,
    ) -> std::result::Result<(), &'static str> {
        for node in nodes {
            self.before(node.first, layout, root)?;
            self.node(node, layout)?;
        }
        Ok(())
    }
    fn node(&mut self, node: &Node<'a>, layout: Layout) -> std::result::Result<(), &'static str> {
        self.append(node.first.raw)?;
        self.previous = Some(node.first);
        if let Some(children) = &node.children {
            let block = node.first.raw == "{";
            let broken = block
                || !children.is_empty()
                    && node.flat.is_none_or(|width| {
                        self.column.saturating_sub(node.first.raw.len()) + width > WIDTH
                    });
            if broken {
                self.indent += 1;
                self.pending_break = Break::Required;
                self.sequence(
                    children,
                    if block {
                        Layout::Statements
                    } else {
                        Layout::BrokenList
                    },
                    false,
                )?;
                self.newline()?;
                self.indent -= 1;
                self.indentation()?;
            } else {
                self.sequence(children, Layout::Flat, false)?;
            }
            self.append(node.last.raw)?;
            self.previous = Some(node.last);
            if block {
                self.pending_break = Break::AfterBlock;
            }
        } else {
            match node.first.kind {
                Kind::Comment(CommentKind::Line) => self.newline()?,
                Kind::Comment(CommentKind::Block)
                    if node.first.newline_after || node.first.raw.contains('\n') =>
                {
                    self.newline()?
                }
                Kind::Token if node.first.raw == ";" && layout == Layout::Statements => {
                    self.pending_break = Break::Required
                }
                Kind::Token if node.first.raw == "," && layout == Layout::BrokenList => {
                    self.pending_break = Break::Required
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn same_lexemes(before: &SourceFile, after: &SourceFile) -> bool {
    fn sequence(source: &SourceFile) -> Option<Vec<(u8, &str)>> {
        let lexed = lex_with_comments(source).ok()?;
        let mut entries: Vec<_> = lexed
            .tokens
            .iter()
            .filter(|token| token.kind != TokenKind::Eof)
            .map(|token| {
                (
                    token.span.range().start,
                    0,
                    source.slice(token.span).unwrap(),
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
                    source.slice(comment.span).unwrap(),
                )
            }))
            .collect();
        entries.sort_unstable_by_key(|entry| entry.0);
        Some(
            entries
                .into_iter()
                .map(|(_, kind, raw)| (kind, raw))
                .collect(),
        )
    }
    sequence(before) == sequence(after)
}

#[cfg(test)]
mod tests {
    use super::*;
    use larp_source::SourceId;
    use std::path::Path;

    fn source(text: &str) -> SourceFile {
        SourceFile::new(SourceId(49), "fixture.larp", text)
    }
    fn checked(text: &str) -> String {
        let input = source(text);
        let output = format(&input).unwrap_or_else(|errors| panic!("{text}\n{errors:?}"));
        assert!(same_lexemes(&input, &source(&output)));
        let twice = format(&source(&output)).unwrap();
        assert_eq!(twice, output, "Formatting must be idempotent for {text}");
        output
    }

    #[test]
    fn golden_blocks_operators_and_final_newline() {
        assert_eq!(
            checked("main_character{mf n=4;if(n mogg 0){yap(n);}else{yap(0);}}"),
            "main_character {\n    mf n = 4;\n    if (n mogg 0) {\n        yap(n);\n    } else {\n        yap(0);\n    }\n}\n"
        );
    }

    #[test]
    fn nested_generics_calls_comparisons_function_types_and_typed_lambdas() {
        let output = checked(
            "sidequest f(result<mfs<baddie<mf>>,text> x)->fn(mf)->mf uses(abort){return(mf v)=>v+1;}main_character{result<fn(mf)->mf,text> f=win((mf x)=>x+1);let g=ascend<unc>(4);if(2<3&&4>1){yap(g);}let x=-(+(4));}",
        );
        assert!(output.contains("result<mfs<baddie<mf>>, text>"));
        assert!(output.contains("ascend<unc>(4)"));
        assert!(output.contains("2 < 3 && 4 > 1"));
        assert!(output.contains("-(+(4))"));
    }

    #[test]
    fn opaque_comments_unicode_crlf_and_literal_spellings_are_preserved() {
        let input = "// lead я\r\nmain_character{/* open */mf n=1_000; // tail\r\n/* block\r\n    🦀 raw */yap(\"\\n🦀\\t\");n=n/ /* separated */2;}// eof\r";
        let output = checked(input);
        assert!(output.contains("// tail\r\n"));
        assert!(output.contains("/* block\r\n    🦀 raw */"));
        assert!(output.contains("1_000"));
        assert!(output.contains("\"\\n🦀\\t\""));
        assert!(output.ends_with("// eof\r\n"));
    }

    #[test]
    fn comments_at_every_generic_operator_and_delimiter_boundary_survive() {
        checked(
            "main_character/*a*/{/*b*/mfs/*c*/</*d*/mf/*e*/>/*f*/a/*g*/=/*h*/[/*i*/1/*j*/,/*k*/2/*l*/]/*m*/;/*n*/a/*o*/./*p*/forEach/*q*/(/*r*/x/*s*/=>/*t*/{/*u*/yap/*v*/(/*w*/x/*x*/)/*y*/;/*z*/}/*end*/)/*final*/;}",
        );
        checked("main_character{let a=ascend //a\n< //b\nmf //c\n> //d\n( //e\n49 //f\n);}");
    }

    #[test]
    fn token_merging_cannot_change_prefix_signs_division_or_parentheses() {
        let output = checked(
            "main_character{let a=- -4;let b=+ +4;let c=8/ /*x*/2;let d=!(cap);let e=((1+2));}",
        );
        assert!(output.contains("- -4"));
        assert!(output.contains("+ +4"));
        assert!(output.contains("8 / /*x*/ 2"));
        assert!(output.contains("((1 + 2))"));
    }

    #[test]
    fn grouped_statements_get_required_line_breaks_after_semicolons() {
        let output = checked("main_character{yap(1);(x=>x)(2);[1,2].forEach(x=>yap(x));}");
        assert!(output.contains("yap(1);\n    (x => x)(2);\n    [1, 2]"));
    }

    #[test]
    fn long_lists_break_before_nested_arrays_and_typed_lambdas() {
        let output = checked(
            "main_character{let a=select(very_long_identifier_one,very_long_identifier_two,very_long_identifier_three,[1,2],(mf value)=>value);}",
        );
        assert!(output.contains("select(\n"));
        assert!(output.contains(",\n        [1, 2],\n        (mf value) => value\n    )"));
    }

    #[test]
    fn lambda_blocks_else_if_and_chained_calls_keep_continuations() {
        let output = checked(
            "main_character{if(fr){yap(1);}else if(cap){yap(2);}else{}let a=[1,2].map(x=>{return x+1;}).filter(x=>x mogg 1);}",
        );
        assert!(output.contains("} else if (cap) {"));
        assert!(output.contains(").filter"));
    }

    #[test]
    fn declaration_and_member_order_are_preserved() {
        let output = checked(
            "main_character{}sidequest z()->mf{return 0;}role R{sidequest x()->mf;}archetype C{init(){}highkey sidequest x()->mf{return 1;}mf value;}",
        );
        assert!(output.starts_with("main_character"));
        assert!(output.find("sidequest z").unwrap() < output.find("role R").unwrap());
        assert!(output.find("init()").unwrap() < output.find("mf value;").unwrap());
    }

    #[test]
    fn semantic_errors_are_formatable_and_syntax_errors_are_rejected() {
        checked("main_character{mf value=\"wrong type\";unknown(value);}");
        for input in [
            "main_character{mf x=;}",
            "main_character{yap(\"bad\\q\");}",
            "main_character{/* missing",
        ] {
            let errors = format(&source(input)).unwrap_err();
            assert!(errors.iter().all(|error| error.code.id != "LARP-C001"));
        }
    }

    #[test]
    fn parseable_examples_and_conformance_sources_are_idempotent() {
        fn walk(path: &Path, count: &mut usize) {
            for entry in std::fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, count);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension == "larp")
                {
                    let text = std::fs::read_to_string(&path).unwrap();
                    if parse(&source(&text)).is_ok() {
                        checked(&text);
                        *count += 1;
                    }
                }
            }
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut count = 0;
        walk(&root.join("examples"), &mut count);
        walk(&root.join("tests/conformance"), &mut count);
        assert!(count >= 80, "The real syntax corpus must be exercised");
    }
    #[test]
    fn flat_comma_lists_space_nested_delimiters() {
        let output = checked("main_character{call(1,[2],(mf x)=>x);}");
        assert!(output.contains("call(1, [2], (mf x) => x)"));
    }
    #[test]
    fn input_and_token_limits_report_resource_errors_before_ast_construction() {
        let oversized = " ".repeat(MAX_SOURCE_BYTES + 1);
        assert_eq!(
            format(&source(&oversized)).unwrap_err()[0].code.id,
            "LARP-P003"
        );
        let tokens = format!("main_character{{{}}}", "yap(1);".repeat(MAX_LEAVES / 5 + 1));
        let errors = format(&source(&tokens)).unwrap_err();
        assert_eq!(errors[0].code.id, "LARP-P003");
        assert!(errors[0].message.contains("token count"));
    }

    #[test]
    fn layout_expansion_cannot_produce_output_that_exceeds_the_input_limit() {
        let text = format!(
            "main_character{{{}{}{}}}",
            "goon{".repeat(32),
            "yap(1);".repeat(34_000),
            "}".repeat(32),
        );
        assert!(text.len() < MAX_SOURCE_BYTES);
        let errors = format(&source(&text)).unwrap_err();
        assert_eq!(errors[0].code.id, "LARP-P003");
        assert!(errors[0].message.contains("output size"));
    }
}
