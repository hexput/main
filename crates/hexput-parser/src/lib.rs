#![forbid(clippy::undocumented_unsafe_blocks)]

//! Pure source-to-AST parser for scalar expressions, declarations, and assignments.
//! Explicit operator and delimiter stacks avoid native-stack recursion, including on errors.

use hexput_ast::*;
use hexput_lexer::{Token, TokenKind, tokenize};
use std::collections::HashSet;

/// Parse an entire file, preserving grouping, optional access links, and source locations.
///
/// # Errors
/// Returns lexical diagnostics unchanged, or the first syntax diagnostic. Unsupported language
/// constructs are rejected. Names need not be declared; duplicate top-level declarations fail.
pub fn parse(source: &str) -> Result<Program, Diagnostic> {
    let tokens = tokenize(source)?;
    Parser {
        source,
        tokens,
        pos: 0,
        eof: eof_span(source),
        program: Program {
            span: Span::new(0, source.len(), 1, 1),
            statements: Vec::new(),
            expressions: Vec::new(),
        },
    }
    .run()
}

struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    pos: usize,
    eof: Span,
    program: Program,
}

/// Delimiters isolate reductions; an index frame owns the receiver while its key is parsed.
enum Pending {
    Unary(Spanned<UnaryOperator>),
    Binary(Spanned<BinaryOperator>, u8),
    Group(Span),
    Index {
        base: ExprId,
        operator: Span,
        open: Span,
        optional: bool,
    },
}

impl Pending {
    fn precedence(&self) -> Option<u8> {
        match self {
            Self::Unary(_) => Some(7),
            Self::Binary(_, p) => Some(*p),
            _ => None,
        }
    }
}

impl Parser<'_> {
    fn peek(&self) -> Option<&TokenKind> {
        self.tokens.get(self.pos).map(|t| &t.kind)
    }
    fn span(&self) -> Span {
        self.tokens.get(self.pos).map_or(self.eof, |t| t.span)
    }
    fn bump(&mut self) -> Token {
        let token = &mut self.tokens[self.pos];
        // Consumed kinds are never inspected again; retain spans for statement endings.
        let kind = std::mem::replace(&mut token.kind, TokenKind::Null);
        let span = token.span;
        self.pos += 1;
        Token { kind, span }
    }
    fn expected(&self, expected: &str) -> Diagnostic {
        let found = self.tokens.get(self.pos).map_or_else(
            || "end of input".to_owned(),
            |t| {
                // Bound source scalars before escaping, so even a huge string token produces
                // a small, readable diagnostic. Its diagnostic span still covers every byte.
                let mut chars = self.source[t.span.range()].chars();
                let preview: String = chars.by_ref().take(64).collect();
                let truncated = if chars.next().is_some() {
                    "… (truncated)"
                } else {
                    ""
                };
                format!("`{}{truncated}`", preview.escape_debug())
            },
        );
        Diagnostic::new(
            Category::Syntax,
            Code::EXPECTED_SYNTAX,
            format!("expected {expected}, found {found}"),
            self.span(),
        )
    }
    fn expect(&mut self, kind: TokenKind, label: &str) -> Result<Span, Diagnostic> {
        if self.peek() == Some(&kind) {
            Ok(self.bump().span)
        } else {
            Err(self.expected(label))
        }
    }
    fn identifier(&mut self) -> Result<Identifier, Diagnostic> {
        if !matches!(self.peek(), Some(TokenKind::Ident(_))) {
            return Err(self.expected("an identifier"));
        }
        let Token {
            kind: TokenKind::Ident(name),
            span,
        } = self.bump()
        else {
            unreachable!()
        };
        Ok(Identifier { name, span })
    }
    fn add(&mut self, kind: ExpressionKind, span: Span) -> ExprId {
        let id = ExprId(self.program.expressions.len());
        self.program.expressions.push(Expression { kind, span });
        id
    }
    fn expr_span(&self, id: ExprId) -> Span {
        self.program.expression(id).span
    }

    fn run(mut self) -> Result<Program, Diagnostic> {
        let mut names = HashSet::new();
        while self.peek().is_some() {
            let start = self.span();
            let kind = if self.peek() == Some(&TokenKind::Let) {
                let keyword = self.bump().span;
                let name = self.identifier()?;
                if !names.insert(name.name.clone()) {
                    return Err(Diagnostic::new(
                        Category::Syntax,
                        Code::DUPLICATE_DECLARATION,
                        format!(
                            "expected a new binding name; `{}` is already declared in this scope",
                            name.name
                        ),
                        name.span,
                    ));
                }
                let equals = self.expect(TokenKind::Assign, "`=` and an initializer")?;
                let initializer = self.expression()?;
                StatementKind::Let {
                    keyword,
                    name,
                    equals,
                    initializer,
                }
            } else {
                let target = self.expression()?;
                if self.peek() == Some(&TokenKind::Assign) {
                    if !self.valid_target(target) {
                        return Err(Diagnostic::new(
                            Category::Syntax,
                            Code::INVALID_ASSIGNMENT_TARGET,
                            "expected a name or ordinary property/index assignment target before `=`",
                            self.expr_span(target),
                        ));
                    }
                    let equals = self.bump().span;
                    let value = self.expression()?;
                    StatementKind::Assignment {
                        target,
                        equals,
                        value,
                    }
                } else {
                    StatementKind::Expression(target)
                }
            };
            let end = self.tokens[self.pos - 1].span;
            let terminator = if self.peek() == Some(&TokenKind::Semicolon) {
                Some(self.bump().span)
            } else if self.peek().is_none() {
                None
            } else {
                return Err(self.expected("`;` between statements or end of input"));
            };
            self.program.statements.push(Statement {
                span: cover(start, terminator.unwrap_or(end)),
                terminator,
                kind,
            });
        }
        Ok(self.program)
    }

    fn valid_target(&self, id: ExprId) -> bool {
        match &self.program.expression(id).kind {
            ExpressionKind::Identifier(_) => true,
            // Grouping ends a chain; optional reads inside a receiver group or index are reads.
            ExpressionKind::Access { links, .. } => links.iter().all(|link| !link.optional),
            _ => false,
        }
    }

    fn access(&mut self, base: ExprId, link: AccessLink) -> ExprId {
        let span = cover(self.expr_span(base), link.span);
        if let ExpressionKind::Access { links, .. } = &mut self.program.expressions[base.0].kind {
            links.push(link);
            self.program.expressions[base.0].span = span;
            base
        } else {
            self.add(
                ExpressionKind::Access {
                    base,
                    links: vec![link],
                },
                span,
            )
        }
    }

    fn reduce(&mut self, pending: Pending, values: &mut Vec<ExprId>) {
        let right = values.pop().expect("a complete operand precedes reduction");
        let id = match pending {
            Pending::Unary(operator) => self.add(
                ExpressionKind::Unary {
                    operator,
                    operand: right,
                },
                cover(operator.span, self.expr_span(right)),
            ),
            Pending::Binary(operator, _) => {
                let left = values
                    .pop()
                    .expect("binary operators follow a left operand");
                self.add(
                    ExpressionKind::Binary {
                        left,
                        operator,
                        right,
                    },
                    cover(self.expr_span(left), self.expr_span(right)),
                )
            }
            _ => unreachable!("delimiters are closed separately"),
        };
        values.push(id);
    }

    fn expression(&mut self) -> Result<ExprId, Diagnostic> {
        let mut values = Vec::new();
        let mut pending: Vec<Pending> = Vec::new();
        let mut operand = true;
        loop {
            if operand {
                let span = self.span();
                let kind = match self.peek() {
                    Some(TokenKind::Minus | TokenKind::Bang) => {
                        let op = if self.bump().kind == TokenKind::Minus {
                            UnaryOperator::Negate
                        } else {
                            UnaryOperator::Not
                        };
                        pending.push(Pending::Unary(Spanned { kind: op, span }));
                        continue;
                    }
                    Some(TokenKind::LParen) => {
                        self.bump();
                        pending.push(Pending::Group(span));
                        continue;
                    }
                    Some(
                        TokenKind::Number(_)
                        | TokenKind::Str(_)
                        | TokenKind::Ident(_)
                        | TokenKind::True
                        | TokenKind::False
                        | TokenKind::Null,
                    ) => match self.bump().kind {
                        TokenKind::Number(n) => ExpressionKind::Literal(Literal::Number(n)),
                        TokenKind::Str(s) => ExpressionKind::Literal(Literal::String(s)),
                        TokenKind::True => ExpressionKind::Literal(Literal::Bool(true)),
                        TokenKind::False => ExpressionKind::Literal(Literal::Bool(false)),
                        TokenKind::Null => ExpressionKind::Literal(Literal::Null),
                        TokenKind::Ident(name) => {
                            ExpressionKind::Identifier(Identifier { name, span })
                        }
                        _ => unreachable!(),
                    },
                    _ => {
                        return Err(
                            self.expected("an expression (literal, identifier, `(`, `-`, or `!`)")
                        );
                    }
                };
                values.push(self.add(kind, span));
                operand = false;
                continue;
            }
            match self.peek() {
                Some(TokenKind::Dot | TokenKind::QuestionDot | TokenKind::LBracket) => {
                    let token = self.bump();
                    let optional = token.kind == TokenKind::QuestionDot;
                    let base = values.pop().expect("postfix access follows an operand");
                    let open = if token.kind == TokenKind::LBracket {
                        Some(token.span)
                    } else if optional && self.peek() == Some(&TokenKind::LBracket) {
                        Some(self.bump().span)
                    } else {
                        None
                    };
                    if let Some(open) = open {
                        pending.push(Pending::Index {
                            base,
                            operator: token.span,
                            open,
                            optional,
                        });
                        operand = true;
                    } else {
                        let name = self.identifier()?;
                        let link = AccessLink {
                            span: cover(token.span, name.span),
                            operator: token.span,
                            optional,
                            kind: AccessKind::Property(name),
                        };
                        values.push(self.access(base, link));
                    }
                }
                Some(TokenKind::RParen | TokenKind::RBracket) => {
                    while pending.last().is_some_and(|p| p.precedence().is_some()) {
                        self.reduce(pending.pop().unwrap(), &mut values);
                    }
                    let Some(frame) = pending.pop() else {
                        break;
                    };
                    let inner = values.pop().expect("closing delimiters follow an operand");
                    let id = match frame {
                        Pending::Group(open) => {
                            let close = self.expect(TokenKind::RParen, "`)` to close the group")?;
                            self.add(
                                ExpressionKind::Group {
                                    open,
                                    expression: inner,
                                    close,
                                },
                                cover(open, close),
                            )
                        }
                        Pending::Index {
                            base,
                            operator,
                            open,
                            optional,
                        } => {
                            let close =
                                self.expect(TokenKind::RBracket, "`]` to close the index")?;
                            self.access(
                                base,
                                AccessLink {
                                    span: cover(operator, close),
                                    operator,
                                    optional,
                                    kind: AccessKind::Index {
                                        open,
                                        expression: inner,
                                        close,
                                    },
                                },
                            )
                        }
                        _ => unreachable!(),
                    };
                    values.push(id);
                }
                Some(kind) if binary(kind).is_some() => {
                    let (kind, precedence) = binary(self.peek().unwrap()).unwrap();
                    while pending
                        .last()
                        .and_then(Pending::precedence)
                        .is_some_and(|p| p >= precedence)
                    {
                        self.reduce(pending.pop().unwrap(), &mut values);
                    }
                    let span = self.bump().span;
                    pending.push(Pending::Binary(Spanned { kind, span }, precedence));
                    operand = true;
                }
                _ => break,
            }
        }
        while let Some(op) = pending.pop() {
            if op.precedence().is_some() {
                self.reduce(op, &mut values);
            } else {
                return Err(self.expected(match op {
                    Pending::Group(_) => "`)` to close the group",
                    _ => "`]` to close the index",
                }));
            }
        }
        Ok(values.pop().expect("an expression has one completed value"))
    }
}

fn binary(kind: &TokenKind) -> Option<(BinaryOperator, u8)> {
    use BinaryOperator as B;
    use TokenKind as T;
    Some(match kind {
        T::PipePipe => (B::Or, 1),
        T::AmpAmp => (B::And, 2),
        T::EqEq => (B::Equal, 3),
        T::BangEq => (B::NotEqual, 3),
        T::Lt => (B::Less, 4),
        T::LtEq => (B::LessEqual, 4),
        T::Gt => (B::Greater, 4),
        T::GtEq => (B::GreaterEqual, 4),
        T::Plus => (B::Add, 5),
        T::Minus => (B::Subtract, 5),
        T::Star => (B::Multiply, 6),
        T::Slash => (B::Divide, 6),
        T::Percent => (B::Remainder, 6),
        _ => return None,
    })
}

fn cover(start: Span, end: Span) -> Span {
    Span::new(
        start.offset,
        end.end() - start.offset,
        start.line,
        start.column,
    )
}

/// Match the lexer's scalar-column, BOM, and CRLF conventions, including discarded trivia.
fn eof_span(source: &str) -> Span {
    let mut chars = source
        .strip_prefix('\u{feff}')
        .unwrap_or(source)
        .chars()
        .peekable();
    let (mut line, mut column) = (1, 1);
    while let Some(c) = chars.next() {
        if c == '\n' || (c == '\r' && chars.peek() != Some(&'\n')) {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    Span::new(source.len(), 0, line, column)
}
