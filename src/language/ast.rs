//! Abstract Syntax Tree representation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Source location for error reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Self {
            start,
            end,
            line,
            column,
        }
    }

    pub fn unknown() -> Self {
        Self {
            start: 0,
            end: 0,
            line: 0,
            column: 0,
        }
    }
}

/// Top-level AST node
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ast {
    pub statements: Vec<Statement>,
}

/// Statement types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Statement {
    /// Variable declaration: vl x = expr;
    VarDecl {
        name: String,
        value: Expression,
        span: Option<Span>,
    },

    /// Callback definition: cb name(params) { body }
    CallbackDecl {
        name: String,
        params: Vec<String>,
        body: Vec<Statement>,
        span: Option<Span>,
    },

    /// Assignment: x = expr;
    Assignment {
        target: AssignTarget,
        value: Expression,
        span: Option<Span>,
    },

    /// Loop: loop x in expr { body }
    Loop {
        var: String,
        iterable: Expression,
        body: Vec<Statement>,
        span: Option<Span>,
    },

    /// Conditional: if expr { body } [else { body }]
    If {
        condition: Expression,
        then_body: Vec<Statement>,
        else_body: Option<Vec<Statement>>,
        span: Option<Span>,
    },

    /// Return: res expr;
    Return {
        value: Expression,
        span: Option<Span>,
    },

    /// Continue: continue;
    Continue { span: Option<Span> },

    /// End: end;
    End { span: Option<Span> },

    /// Expression statement
    Expression(Expression),

    /// Block: { statements }
    Block {
        statements: Vec<Statement>,
        span: Option<Span>,
    },
}

/// Assignment targets
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AssignTarget {
    /// Simple variable: x
    Identifier(String),

    /// Array index: arr[index]
    Index {
        object: Box<Expression>,
        index: Box<Expression>,
    },

    /// Property access: obj.prop
    Property {
        object: Box<Expression>,
        property: String,
    },
}

/// Expression types with optional span for error reporting
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expression {
    /// Number literal
    Number(f64),

    /// String literal
    String(String),

    /// Boolean literal
    Boolean(bool),

    /// Undefined value
    Undefined,

    /// Identifier (variable reference)
    Identifier(String),

    /// Object literal: { key: value, ... }
    Object(HashMap<String, Expression>),

    /// Array literal: [expr, ...]
    Array(Vec<Expression>),

    /// Function call: name(args)
    Call {
        callee: String,
        args: Vec<Expression>,
    },

    /// Binary operation: left op right
    Binary {
        op: BinaryOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },

    /// Unary operation: op expr
    Unary {
        op: UnaryOp,
        operand: Box<Expression>,
    },

    /// Logical operation: left op right
    Logical {
        op: LogicalOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },

    /// Property access: object.property
    Property {
        object: Box<Expression>,
        property: String,
    },

    /// Index access: object[index]
    Index {
        object: Box<Expression>,
        index: Box<Expression>,
    },

    /// Keys of object: keysof expr
    KeysOf(Box<Expression>),

    /// Typeof expression: typeof expr
    TypeOf(Box<Expression>),

    /// Grouped expression: (expr)
    Grouped(Box<Expression>),
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    // Arithmetic
    Add, // +
    Sub, // -
    Mul, // *
    Div, // /
    Mod, // %

    // Comparison
    Eq,    // ==
    NotEq, // !=
    Lt,    // <
    LtEq,  // <=
    Gt,    // >
    GtEq,  // >=
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,  // - (negation)
    Not,  // ! (logical not)
    Plus, // + (unary plus)
}

/// Logical operators (short-circuiting)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogicalOp {
    And, // &&
    Or,  // ||
}

impl std::fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Mod => "%",
            BinaryOp::Eq => "==",
            BinaryOp::NotEq => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::LtEq => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::GtEq => ">=",
        };
        write!(f, "{}", s)
    }
}

impl std::fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            UnaryOp::Neg => "-",
            UnaryOp::Not => "!",
            UnaryOp::Plus => "+",
        };
        write!(f, "{}", s)
    }
}

impl std::fmt::Display for LogicalOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            LogicalOp::And => "&&",
            LogicalOp::Or => "||",
        };
        write!(f, "{}", s)
    }
}

// Helper implementations

impl Statement {
    /// Get the span of this statement if available
    pub fn span(&self) -> Option<Span> {
        match self {
            Statement::VarDecl { span, .. } => *span,
            Statement::CallbackDecl { span, .. } => *span,
            Statement::Assignment { span, .. } => *span,
            Statement::Loop { span, .. } => *span,
            Statement::If { span, .. } => *span,
            Statement::Return { span, .. } => *span,
            Statement::Continue { span } => *span,
            Statement::End { span } => *span,
            Statement::Block { span, .. } => *span,
            Statement::Expression(_) => None,
        }
    }
}

impl Expression {
    /// Check if this expression is a literal value
    pub fn is_literal(&self) -> bool {
        matches!(
            self,
            Expression::Number(_)
                | Expression::String(_)
                | Expression::Boolean(_)
                | Expression::Undefined
        )
    }

    /// Check if this expression is a simple identifier
    pub fn is_identifier(&self) -> bool {
        matches!(self, Expression::Identifier(_))
    }

    /// Try to extract identifier name
    pub fn as_identifier(&self) -> Option<&str> {
        match self {
            Expression::Identifier(name) => Some(name),
            _ => None,
        }
    }

    /// Check if expression is "truthy" at compile time (for constant folding)
    pub fn is_compile_time_truthy(&self) -> Option<bool> {
        match self {
            Expression::Boolean(b) => Some(*b),
            Expression::Number(n) => Some(*n != 0.0),
            Expression::String(s) => Some(!s.is_empty()),
            Expression::Undefined => Some(false),
            _ => None,
        }
    }
}

impl BinaryOp {
    /// Check if this operator is a comparison operator
    pub fn is_comparison(&self) -> bool {
        matches!(
            self,
            BinaryOp::Eq
                | BinaryOp::NotEq
                | BinaryOp::Lt
                | BinaryOp::LtEq
                | BinaryOp::Gt
                | BinaryOp::GtEq
        )
    }

    /// Check if this operator is an arithmetic operator
    pub fn is_arithmetic(&self) -> bool {
        matches!(
            self,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod
        )
    }

    /// Get operator precedence (higher = tighter binding)
    pub fn precedence(&self) -> u8 {
        match self {
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => 6,
            BinaryOp::Add | BinaryOp::Sub => 5,
            BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => 4,
            BinaryOp::Eq | BinaryOp::NotEq => 3,
        }
    }
}

impl LogicalOp {
    /// Get operator precedence
    pub fn precedence(&self) -> u8 {
        match self {
            LogicalOp::And => 2,
            LogicalOp::Or => 1,
        }
    }
}
