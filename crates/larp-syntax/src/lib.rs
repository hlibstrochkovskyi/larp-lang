//! Source-preserving AST and handwritten language frontend.

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
    pub roles: Vec<Role>,
    pub classes: Vec<Class>,
    pub functions: Vec<Function>,
    pub main: Block,
}

#[derive(Debug)]
pub struct Class {
    pub name: Name,
    pub roles: Vec<Name>,
    pub fields: Vec<Field>,
    pub methods: Vec<Method>,
    pub constructors: Vec<Constructor>,
    pub span: Span,
}

#[derive(Debug)]
pub struct Field {
    pub name: Name,
    pub ty: TypeExpr,
    pub public: bool,
}

#[derive(Debug)]
pub struct Method {
    pub function: Function,
    pub public: bool,
}

#[derive(Debug)]
pub struct Constructor {
    pub parameters: Vec<(TypeExpr, Name)>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug)]
pub struct Function {
    pub receiver: Option<TypeExpr>,
    pub name: Name,
    pub parameters: Vec<(TypeExpr, Name)>,
    pub result: TypeExpr,
    pub effects: Option<Vec<Name>>,
    pub body: Block,
}

#[derive(Debug)]
pub struct Role {
    pub name: Name,
    pub methods: Vec<RoleMethod>,
    pub span: Span,
}

#[derive(Debug)]
pub struct RoleMethod {
    pub name: Name,
    pub parameters: Vec<(TypeExpr, Name)>,
    pub result: TypeExpr,
    pub effects: Option<Vec<Name>>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct TypeExpr {
    pub kind: TypeExprKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum TypeExprKind {
    Integer(String),
    Named {
        name: Name,
        arguments: Vec<TypeExpr>,
    },
    Function {
        parameters: Vec<TypeExpr>,
        result: Box<TypeExpr>,
        effects: Option<Vec<Name>>,
    },
}

#[derive(Debug)]
pub struct LambdaParameter {
    pub ty: Option<TypeExpr>,
    pub name: Name,
}

#[derive(Debug)]
pub enum LambdaBody {
    Expression(Box<Expr>),
    Block(Block),
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
        ty: Option<TypeExpr>,
        name: Name,
        value: Expr,
    },
    Assignment {
        target: Expr,
        op: Option<Binary>,
        value: Expr,
    },
    Expression(Expr),
    Block(Block),
    Scenario {
        root: Name,
        label: Name,
        body: Block,
    },
    Flex {
        root: Name,
        field: Name,
        value: Expr,
    },
    Canon {
        root: Name,
        field: Name,
    },
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
        ty: TypeExpr,
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
    None,
    Name(Name),
    SelfValue,
    New {
        class: Name,
        arguments: Vec<Expr>,
    },
    Array(Vec<Expr>),
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
        type_arguments: Vec<TypeExpr>,
    },
    Member {
        receiver: Box<Expr>,
        name: Name,
    },
    Index {
        receiver: Box<Expr>,
        index: Box<Expr>,
    },
    Lambda {
        parameters: Vec<LambdaParameter>,
        body: LambdaBody,
    },
}
