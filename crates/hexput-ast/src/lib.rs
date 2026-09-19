//! Owned, source-spanned syntax data; no parsing, evaluation, or host access.
//!
//! Expressions live in a flat arena and refer to children by [`ExprId`]. This keeps dropping,
//! cloning and inspecting deeply nested source safe without recursive ownership. IDs belong to
//! their enclosing [`Program`]; consumers must not mix IDs from different programs.

pub use hexput_shared::diagnostics::{Category, Code, Diagnostic, Span};

/// An index into [`Program::expressions`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExprId(pub usize);

/// A complete file. Its span includes leading and trailing trivia (and an initial BOM).
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub span: Span,
    pub statements: Vec<Statement>,
    /// Most children precede parents, but index expressions may be forward references:
    /// in `a.b[c]`, the chain node exists before `c` and gains its index link afterward.
    /// Traverse by following [`ExprId`] links, never by assuming arena order is tree order.
    pub expressions: Vec<Expression>,
}

impl Program {
    /// Look up an expression belonging to this program.
    ///
    /// # Panics
    /// Panics if `id.0` is outside [`Self::expressions`].
    #[must_use]
    pub fn expression(&self, id: ExprId) -> &Expression {
        &self.expressions[id.0]
    }
}

/// A statement span includes its semicolon when present.
#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub span: Span,
    pub terminator: Option<Span>,
    pub kind: StatementKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatementKind {
    Let {
        keyword: Span,
        name: Identifier,
        equals: Span,
        initializer: ExprId,
    },
    Assignment {
        target: ExprId,
        equals: Span,
        value: ExprId,
    },
    Expression(ExprId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identifier {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expression {
    pub span: Span,
    pub kind: ExpressionKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExpressionKind {
    Literal(Literal),
    Identifier(Identifier),
    Group {
        open: Span,
        expression: ExprId,
        close: Span,
    },
    Unary {
        operator: Spanned<UnaryOperator>,
        operand: ExprId,
    },
    Binary {
        left: ExprId,
        operator: Spanned<BinaryOperator>,
        right: ExprId,
    },
    /// One uninterrupted chain. A group in `base` is an explicit chain boundary.
    Access {
        base: ExprId,
        links: Vec<AccessLink>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spanned<T> {
    pub kind: T,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Negate,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Multiply,
    Divide,
    Remainder,
    Add,
    Subtract,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    NotEqual,
    And,
    Or,
}

/// A property or index operation, in source order within its chain.
#[derive(Debug, Clone, PartialEq)]
pub struct AccessLink {
    /// From `.` / `?.` / `[` through the property or closing bracket.
    pub span: Span,
    /// `.` or `?.` for properties; `[` or `?.` for indices.
    pub operator: Span,
    pub optional: bool,
    pub kind: AccessKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccessKind {
    Property(Identifier),
    Index {
        open: Span,
        expression: ExprId,
        close: Span,
    },
}
