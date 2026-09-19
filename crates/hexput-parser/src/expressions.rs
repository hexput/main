//! Resumable shunting-yard expression state. Function bodies suspend this state onto the
//! statement driver's task stack; neither engine calls the other recursively.
use super::*;

pub(super) enum Pending {
    Unary(Spanned<UnaryOperator>),
    Binary(Spanned<BinaryOperator>, u8),
    Group(Span),
    Index {
        base: ExprId,
        operator: Span,
        open: Span,
        optional: bool,
    },
    List(List),
}

impl Pending {
    pub(super) fn precedence(&self) -> Option<u8> {
        match self {
            Self::Unary(_) => Some(7),
            Self::Binary(_, p) => Some(*p),
            _ => None,
        }
    }
}

pub(super) struct List {
    open: Span,
    kind: ListKind,
    values: Vec<ExprId>,
    entries: Vec<ObjectEntry>,
    names: HashSet<String>,
    key: Option<(Identifier, Span)>,
}

enum ListKind {
    Array,
    Object,
    Call(ExprId),
}

impl List {
    fn closing_label(&self) -> &'static str {
        match self.kind {
            ListKind::Array => "`]` to close the array",
            ListKind::Object => "`}` to close the object",
            ListKind::Call(_) => "`)` to close the call arguments",
        }
    }
    fn closing(&self) -> TokenKind {
        match self.kind {
            ListKind::Array => TokenKind::RBracket,
            ListKind::Object => TokenKind::RBrace,
            ListKind::Call(_) => TokenKind::RParen,
        }
    }
}

pub(super) struct ExpressionState {
    values: Vec<ExprId>,
    pending: Vec<Pending>,
    operand: bool,
}

pub(super) enum Progress {
    Complete(ExprId),
    Function(Function),
}

impl ExpressionState {
    pub(super) fn new() -> Self {
        Self {
            values: Vec::new(),
            pending: Vec::new(),
            operand: true,
        }
    }
    pub(super) fn resume(&mut self, value: ExprId) {
        self.values.push(value);
        self.operand = false;
    }
}

impl Parser<'_> {
    pub(super) fn function_header(&mut self) -> Result<Function, Diagnostic> {
        let keyword = self.expect(TokenKind::Fn, "`fn`")?;
        self.function_parameters(keyword)
    }

    pub(super) fn function_parameters(&mut self, keyword: Span) -> Result<Function, Diagnostic> {
        let open = self.expect(TokenKind::LParen, "`(` before function parameters")?;
        let mut parameters = Vec::new();
        let mut names = HashSet::new();
        if self.peek() != Some(&TokenKind::RParen) {
            loop {
                let name = self.identifier()?;
                Self::declare(&mut names, &name)?;
                parameters.push(name);
                if self.peek() != Some(&TokenKind::Comma) {
                    break;
                }
                self.bump();
                if self.peek() == Some(&TokenKind::RParen) {
                    break;
                }
            }
        }
        let close = self.expect(TokenKind::RParen, "`,` or `)` after function parameters")?;
        Ok(Function {
            keyword,
            open,
            parameters,
            close,
            body: BlockId(0),
        })
    }

    fn list_key(&mut self, list: &mut List) -> Result<(), Diagnostic> {
        let span = self.span();
        let key = match self.peek() {
            Some(TokenKind::Ident(_) | TokenKind::Str(_)) => {
                let name = match self.bump().kind {
                    TokenKind::Ident(s) | TokenKind::Str(s) => s,
                    _ => unreachable!(),
                };
                Identifier { name, span }
            }
            _ => {
                return Err(self.expected(if self.peek().is_none() {
                    "an identifier or quoted object key, or `}` to close the object"
                } else {
                    "an identifier or quoted object key"
                }));
            }
        };
        if !list.names.insert(key.name.clone()) {
            return Err(Diagnostic::new(
                Category::Syntax,
                Code::DUPLICATE_OBJECT_KEY,
                format!("object key {} is repeated", diagnostic_preview(&key.name)),
                span,
            ));
        }
        let colon = self.expect(TokenKind::Colon, "`:` after the object key")?;
        list.key = Some((key, colon));
        Ok(())
    }

    fn finish_list(&mut self, list: List) -> Result<ExprId, Diagnostic> {
        let close = self.expect(list.closing(), &format!("`,` or {}", list.closing_label()))?;
        let span = cover(list.open, close);
        Ok(match list.kind {
            ListKind::Array => self.add(
                ExpressionKind::Array {
                    open: list.open,
                    elements: list.values,
                    close,
                },
                span,
            ),
            ListKind::Object => self.add(
                ExpressionKind::Object {
                    open: list.open,
                    entries: list.entries,
                    close,
                },
                span,
            ),
            ListKind::Call(base) => self.access(
                base,
                AccessLink {
                    span,
                    operator: list.open,
                    optional: false,
                    kind: AccessKind::Call {
                        open: list.open,
                        arguments: list.values,
                        close,
                    },
                },
            ),
        })
    }

    fn start_list(
        &mut self,
        state: &mut ExpressionState,
        kind: ListKind,
    ) -> Result<(), Diagnostic> {
        let open = self.bump().span;
        let mut list = List {
            open,
            kind,
            values: Vec::new(),
            entries: Vec::new(),
            names: HashSet::new(),
            key: None,
        };
        if self.peek() == Some(&list.closing()) {
            let id = self.finish_list(list)?;
            state.resume(id);
        } else {
            if matches!(list.kind, ListKind::Object) {
                self.list_key(&mut list)?;
            }
            state.pending.push(Pending::List(list));
            state.operand = true;
        }
        Ok(())
    }

    pub(super) fn expression_step(
        &mut self,
        state: &mut ExpressionState,
    ) -> Result<Progress, Diagnostic> {
        loop {
            if state.operand {
                let span = self.span();
                let kind = match self.peek() {
                    Some(TokenKind::Fn) => return Ok(Progress::Function(self.function_header()?)),
                    Some(TokenKind::LBracket) => {
                        self.start_list(state, ListKind::Array)?;
                        continue;
                    }
                    Some(TokenKind::LBrace) => {
                        self.start_list(state, ListKind::Object)?;
                        continue;
                    }
                    Some(TokenKind::Minus | TokenKind::Bang) => {
                        let kind = if self.bump().kind == TokenKind::Minus {
                            UnaryOperator::Negate
                        } else {
                            UnaryOperator::Not
                        };
                        state.pending.push(Pending::Unary(Spanned { kind, span }));
                        continue;
                    }
                    Some(TokenKind::LParen) => {
                        self.bump();
                        state.pending.push(Pending::Group(span));
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
                        if self.peek().is_none()
                            && let Some(Pending::List(list)) = state.pending.last()
                        {
                            if list.key.is_some() {
                                return Err(
                                    self.expected("an expression for the object entry value")
                                );
                            }
                            return Err(self
                                .expected(&format!("an expression or {}", list.closing_label())));
                        }
                        return Err(self.expected("an expression"));
                    }
                };
                let id = self.add(kind, span);
                state.resume(id);
                continue;
            }
            match self.peek() {
                Some(TokenKind::LParen) => {
                    let base = state.values.pop().unwrap();
                    self.start_list(state, ListKind::Call(base))?;
                }
                Some(TokenKind::Dot | TokenKind::QuestionDot | TokenKind::LBracket) => {
                    let token = self.bump();
                    let optional = token.kind == TokenKind::QuestionDot;
                    let base = state.values.pop().unwrap();
                    let open = if token.kind == TokenKind::LBracket {
                        Some(token.span)
                    } else if optional && self.peek() == Some(&TokenKind::LBracket) {
                        Some(self.bump().span)
                    } else {
                        None
                    };
                    if let Some(open) = open {
                        state.pending.push(Pending::Index {
                            base,
                            operator: token.span,
                            open,
                            optional,
                        });
                        state.operand = true;
                    } else {
                        let name = self.identifier()?;
                        let link = AccessLink {
                            span: cover(token.span, name.span),
                            operator: token.span,
                            optional,
                            kind: AccessKind::Property(name),
                        };
                        let id = self.access(base, link);
                        state.values.push(id);
                    }
                }
                Some(
                    TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace | TokenKind::Comma,
                ) => {
                    while state
                        .pending
                        .last()
                        .is_some_and(|p| p.precedence().is_some())
                    {
                        self.reduce(state.pending.pop().unwrap(), &mut state.values);
                    }
                    let Some(frame) = state.pending.pop() else {
                        break;
                    };
                    let inner = state.values.pop().unwrap();
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
                        Pending::List(mut list) => {
                            if let Some((key, colon)) = list.key.take() {
                                let span = cover(key.span, self.expr_span(inner));
                                list.entries.push(ObjectEntry {
                                    key,
                                    colon,
                                    value: inner,
                                    span,
                                });
                            } else {
                                list.values.push(inner);
                            }
                            if self.peek() == Some(&TokenKind::Comma) {
                                self.bump();
                                if self.peek() != Some(&list.closing()) {
                                    if matches!(list.kind, ListKind::Object) {
                                        self.list_key(&mut list)?;
                                    }
                                    state.pending.push(Pending::List(list));
                                    state.operand = true;
                                    continue;
                                }
                            }
                            self.finish_list(list)?
                        }
                        _ => unreachable!(),
                    };
                    state.values.push(id);
                }
                Some(kind) if binary(kind).is_some() => {
                    let (kind, precedence) = binary(self.peek().unwrap()).unwrap();
                    while state
                        .pending
                        .last()
                        .and_then(Pending::precedence)
                        .is_some_and(|p| p >= precedence)
                    {
                        self.reduce(state.pending.pop().unwrap(), &mut state.values);
                    }
                    let span = self.bump().span;
                    state
                        .pending
                        .push(Pending::Binary(Spanned { kind, span }, precedence));
                    state.operand = true;
                }
                _ => break,
            }
        }
        while let Some(op) = state.pending.pop() {
            if op.precedence().is_some() {
                self.reduce(op, &mut state.values);
            } else {
                let expected = match op {
                    Pending::Group(_) => "`)` to close the group".to_owned(),
                    Pending::List(ref list) => format!("`,` or {}", list.closing_label()),
                    _ => "`]` to close the index".to_owned(),
                };
                return Err(self.expected(&expected));
            }
        }
        Ok(Progress::Complete(state.values.pop().unwrap()))
    }
}
