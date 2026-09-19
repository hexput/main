//! Iterative statement continuations: every pending body owns one lexical scope.
use super::*;

struct Frame {
    open: Option<Span>,
    statements: Vec<Statement>,
    names: HashSet<String>,
    in_loop: bool,
    owner: Option<(Span, StatementKind)>,
    label: &'static str,
}

impl Parser<'_> {
    pub(super) fn run(mut self) -> Result<Program, Diagnostic> {
        let mut frames = vec![Frame {
            open: None,
            statements: Vec::new(),
            names: HashSet::new(),
            in_loop: false,
            owner: None,
            label: "file",
        }];
        loop {
            if self.peek().is_none() || self.peek() == Some(&TokenKind::RBrace) {
                if frames.len() == 1 {
                    if self.peek().is_some() {
                        return Err(self.expected("a statement, not an unmatched `}`"));
                    }
                    self.program.statements = frames.pop().unwrap().statements;
                    return Ok(self.program);
                }
                let frame = frames.pop().unwrap();
                let close = self.expect(
                    TokenKind::RBrace,
                    &format!("`}}` to close the {} body", frame.label),
                )?;
                let open = frame.open.unwrap();
                let body = BlockId(self.program.blocks.len());
                self.program.blocks.push(Block {
                    span: cover(open, close),
                    open,
                    close,
                    statements: frame.statements,
                });
                let (start, mut kind) = frame.owner.unwrap();
                match &mut kind {
                    StatementKind::Block(id)
                    | StatementKind::While { body: id, .. }
                    | StatementKind::For { body: id, .. } => *id = body,
                    StatementKind::If {
                        branches,
                        else_branch,
                    } => {
                        if let Some(branch) = else_branch {
                            branch.body = body;
                        } else {
                            branches.last_mut().unwrap().body = body;
                        }
                        if else_branch.is_none() && self.peek() == Some(&TokenKind::Else) {
                            let keyword = self.bump().span;
                            let label = if self.peek() == Some(&TokenKind::If) {
                                let if_keyword = self.bump().span;
                                let condition = self.condition("else if")?;
                                branches.push(ConditionalBranch {
                                    else_keyword: Some(keyword),
                                    keyword: if_keyword,
                                    condition,
                                    body: BlockId(0),
                                });
                                "else if"
                            } else {
                                *else_branch = Some(ElseBranch {
                                    keyword,
                                    body: BlockId(0),
                                });
                                "else"
                            };
                            let in_loop = frames.last().unwrap().in_loop;
                            self.push_body(&mut frames, start, kind, in_loop, label)?;
                            continue;
                        }
                    }
                    _ => unreachable!("only body-owning statements create frames"),
                }
                let statement = self.finish_statement(start, kind)?;
                frames.last_mut().unwrap().statements.push(statement);
                continue;
            }
            let start = self.span();
            let in_loop = frames.last().unwrap().in_loop;
            let (kind, body_label, body_loop) = match self.peek() {
                Some(TokenKind::If) => {
                    let keyword = self.bump().span;
                    let condition = self.condition("if")?;
                    (
                        StatementKind::If {
                            branches: vec![ConditionalBranch {
                                else_keyword: None,
                                keyword,
                                condition,
                                body: BlockId(0),
                            }],
                            else_branch: None,
                        },
                        Some("if"),
                        in_loop,
                    )
                }
                Some(TokenKind::While) => {
                    let keyword = self.bump().span;
                    let condition = self.condition("while")?;
                    (
                        StatementKind::While {
                            keyword,
                            condition,
                            body: BlockId(0),
                        },
                        Some("while"),
                        true,
                    )
                }
                Some(TokenKind::For) => {
                    let keyword = self.bump().span;
                    let open = self.expect(TokenKind::LParen, "`(` in the for header")?;
                    let binding = self.identifier().map_err(|mut e| {
                        e.message = format!("for binding: {}", e.message);
                        e
                    })?;
                    let in_keyword = self.expect(TokenKind::In, "`in` after the for binding")?;
                    let iterable = self.header_expression("for iterable")?;
                    let close = self.expect(TokenKind::RParen, "`)` to close the for header")?;
                    (
                        StatementKind::For {
                            keyword,
                            open,
                            binding,
                            in_keyword,
                            iterable,
                            close,
                            body: BlockId(0),
                        },
                        Some("for"),
                        true,
                    )
                }
                Some(TokenKind::LBrace) => {
                    (StatementKind::Block(BlockId(0)), Some("block"), in_loop)
                }
                Some(TokenKind::Break | TokenKind::Continue) => {
                    let token = self.bump();
                    let name = if token.kind == TokenKind::Break {
                        "break"
                    } else {
                        "continue"
                    };
                    if !in_loop {
                        return Err(Diagnostic::new(
                            Category::Syntax,
                            Code::LOOP_CONTROL_OUTSIDE_LOOP,
                            format!("`{name}` requires an enclosing loop"),
                            token.span,
                        ));
                    }
                    let kind = if token.kind == TokenKind::Break {
                        StatementKind::Break {
                            keyword: token.span,
                        }
                    } else {
                        StatementKind::Continue {
                            keyword: token.span,
                        }
                    };
                    (kind, None, in_loop)
                }
                Some(TokenKind::Else) => return Err(self.expected("an if branch before `else`")),
                _ => (
                    self.simple_statement(&mut frames.last_mut().unwrap().names)?,
                    None,
                    in_loop,
                ),
            };
            if let Some(label) = body_label {
                self.push_body(&mut frames, start, kind, body_loop, label)?;
            } else {
                let statement = self.finish_statement(start, kind)?;
                frames.last_mut().unwrap().statements.push(statement);
            }
        }
    }

    fn push_body(
        &mut self,
        frames: &mut Vec<Frame>,
        start: Span,
        kind: StatementKind,
        in_loop: bool,
        label: &'static str,
    ) -> Result<(), Diagnostic> {
        let open = self.expect(
            TokenKind::LBrace,
            &format!("`{{` to begin the {label} body"),
        )?;
        let mut names = HashSet::new();
        if let StatementKind::For { binding, .. } = &kind {
            names.insert(binding.name.clone());
        }
        frames.push(Frame {
            open: Some(open),
            statements: Vec::new(),
            names,
            in_loop,
            owner: Some((start, kind)),
            label,
        });
        Ok(())
    }

    fn condition(&mut self, label: &str) -> Result<Condition, Diagnostic> {
        let open = self.expect(TokenKind::LParen, &format!("`(` in the {label} condition"))?;
        let expression = self.header_expression(label)?;
        let close = self.expect(
            TokenKind::RParen,
            &format!("`)` to close the {label} condition"),
        )?;
        Ok(Condition {
            open,
            expression,
            close,
        })
    }

    fn header_expression(&mut self, label: &str) -> Result<ExprId, Diagnostic> {
        self.expression().map_err(|mut e| {
            e.message = format!("{label}: {}", e.message);
            e
        })
    }

    fn finish_statement(
        &mut self,
        start: Span,
        kind: StatementKind,
    ) -> Result<Statement, Diagnostic> {
        let end = self.tokens[self.pos - 1].span;
        let terminator = if self.peek() == Some(&TokenKind::Semicolon) {
            Some(self.bump().span)
        } else if self.peek().is_none() || self.peek() == Some(&TokenKind::RBrace) {
            None
        } else {
            let label = match &kind {
                StatementKind::If { .. } => "if",
                StatementKind::While { .. } => "while",
                StatementKind::For { .. } => "for",
                StatementKind::Break { .. } => "break",
                StatementKind::Continue { .. } => "continue",
                StatementKind::Block(_) => "block",
                _ => "statement",
            };
            return Err(self.expected(&format!(
                "`;` between statements, `}}`, or end of input after {label}"
            )));
        };
        Ok(Statement {
            span: cover(start, terminator.unwrap_or(end)),
            terminator,
            kind,
        })
    }
}
