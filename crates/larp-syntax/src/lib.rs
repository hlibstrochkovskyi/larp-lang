//! Source-preserving AST and handwritten frontend for the executable scalar core.

mod lexer;
mod parser;

use larp_source::Span;
use larp_types::{Binary, Unary};
pub use parser::parse;

#[derive(Clone, Debug)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

#[derive(Debug)]
pub struct Program {
    pub functions: Vec<Function>,
    pub main: Block,
}

#[derive(Debug)]
pub struct Function {
    pub name: Name,
    pub parameters: Vec<(Name, Name)>,
    pub result: Name,
    pub effects: Option<Vec<Name>>,
    pub body: Block,
}

#[derive(Debug)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum StatementKind {
    Binding {
        ty: Option<Name>,
        name: Name,
        value: Expr,
    },
    Assignment {
        name: Name,
        op: Option<Binary>,
        value: Expr,
    },
    Expression(Expr),
    Block(Block),
    If {
        condition: Expr,
        then_block: Block,
        otherwise: Option<Box<Statement>>,
    },
    While {
        condition: Expr,
        body: Block,
    },
    Range {
        ty: Name,
        name: Name,
        start: Expr,
        end: Expr,
        body: Block,
    },
    Forever(Block),
    Return(Option<Expr>),
    Break,
}

#[derive(Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum ExprKind {
    Integer(String),
    Float(String),
    Text(String),
    Verdict(bool),
    Name(Name),
    Unary {
        op: Unary,
        operand: Box<Expr>,
    },
    Binary {
        op: Binary,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
    },
}
