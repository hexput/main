//! The evaluator: one loop over an explicit stack of continuation frames plus a value stack,
//! mirroring the parser's driver. Nothing here recurses on input nesting, and every frame boundary
//! is a point where a future Executor could suspend the evaluation (Story 3.1).

use std::sync::Arc;

use hexput_ast::{
    AccessKind, AccessLink, BinaryOperator, Category, Code, Diagnostic, ExprId, ExpressionKind,
    Identifier, Literal, ObjectEntry, Program, Span, Spanned, Statement, StatementKind,
    UnaryOperator,
};
use indexmap::IndexMap;

use crate::convert::{number_to_string, to_number, to_string};
use crate::environment::Scope;
use crate::value::{Array, Object};
use crate::{NOT_YET_IMPLEMENTED, Value};

/// A pending unit of work. Frames that consume values document the value-stack shape they
/// expect on entry, top last.
enum Frame<'p> {
    Statement(&'p Statement),
    /// Restore the scope that was current before a block was entered.
    ExitScope(Arc<Scope>),
    /// Evaluate an expression and push its value.
    Eval(ExprId),
    /// `[value]` → `[]`.
    Discard,
    /// `[value]` → `[]`, binding `name` in the current scope.
    Declare(&'p Identifier),
    /// `[value]` → the Script result.
    Return,
    /// `[operand]` → `[result]`.
    Unary {
        operator: Spanned<UnaryOperator>,
        operand: ExprId,
    },
    /// `[left]` → decides short-circuiting, then schedules the right operand.
    BinaryRight {
        left: ExprId,
        operator: Spanned<BinaryOperator>,
        right: ExprId,
    },
    /// `[left, right]` → `[result]`.
    Binary {
        left: ExprId,
        operator: Spanned<BinaryOperator>,
        right: ExprId,
    },
    /// `[e0 … eN-1]` → `[array]`.
    BuildArray(usize),
    /// `[v0 … vN-1]` → `[object]`.
    BuildObject(&'p [ObjectEntry]),
    /// `[receiver]` → applies `links[index]` onward. `base` names the chain's first receiver.
    /// `in_target` marks an assignment target's receiver chain, where `?.` is not allowed.
    Link {
        base: ExprId,
        links: &'p [AccessLink],
        index: usize,
        in_target: bool,
    },
    /// `[receiver, key]` → `[element]`, then continues with `links[index + 1]`.
    IndexRead {
        base: ExprId,
        links: &'p [AccessLink],
        index: usize,
        in_target: bool,
    },
    /// `[value]` → `[]`.
    AssignName(&'p Identifier),
    /// `[receiver, value]` or `[receiver, key, value]` → `[]`. `prefix` is the chain before the
    /// assigned link, used to name a `null` receiver.
    AssignMember {
        base: ExprId,
        prefix: &'p [AccessLink],
        link: &'p AccessLink,
    },
}

pub(crate) struct Machine<'p> {
    program: &'p Program,
    frames: Vec<Frame<'p>>,
    values: Vec<Value>,
    scope: Arc<Scope>,
}

impl<'p> Machine<'p> {
    pub(crate) fn new(program: &'p Program) -> Self {
        let mut frames: Vec<Frame<'p>> = program.statements.iter().map(Frame::Statement).collect();
        frames.reverse();
        Self {
            program,
            frames,
            values: Vec::new(),
            scope: Scope::root(),
        }
    }

    pub(crate) fn run(mut self) -> Result<Value, Diagnostic> {
        while let Some(frame) = self.frames.pop() {
            if let Some(result) = self.step(frame)? {
                return Ok(result);
            }
        }
        Ok(Value::Null)
    }

    /// Execute one frame. `Some` is the Script result from a `return`.
    fn step(&mut self, frame: Frame<'p>) -> Result<Option<Value>, Diagnostic> {
        match frame {
            Frame::Statement(statement) => return self.statement(statement),
            Frame::ExitScope(scope) => self.scope = scope,
            Frame::Eval(id) => self.expression(id)?,
            Frame::Discard => {
                self.pop();
            }
            Frame::Declare(name) => {
                let value = self.pop();
                self.scope.declare(&name.name, value);
            }
            Frame::Return => return Ok(Some(self.pop())),
            Frame::Unary { operator, operand } => {
                let value = self.pop();
                let result = self.unary(operator, operand, &value)?;
                self.values.push(result);
            }
            Frame::BinaryRight {
                left,
                operator,
                right,
            } => self.binary_right(left, operator, right),
            Frame::Binary {
                left,
                operator,
                right,
            } => {
                let right_value = self.pop();
                let left_value = self.pop();
                let result = self.binary(left, operator, right, &left_value, &right_value)?;
                self.values.push(result);
            }
            Frame::BuildArray(count) => {
                let items = self.pop_many(count);
                self.values.push(Value::Array(Array::new(items)));
            }
            Frame::BuildObject(entries) => {
                let values = self.pop_many(entries.len());
                let map: IndexMap<Arc<str>, Value> = entries
                    .iter()
                    .zip(values)
                    .map(|(entry, value)| (Arc::from(entry.key.name.as_str()), value))
                    .collect();
                self.values.push(Value::Object(Object::new(map)));
            }
            Frame::Link {
                base,
                links,
                index,
                in_target,
            } => self.link(base, links, index, in_target)?,
            Frame::IndexRead {
                base,
                links,
                index,
                in_target,
            } => {
                let key = self.pop();
                let receiver = self.pop();
                let AccessKind::Index { expression, .. } = &links[index].kind else {
                    return Err(internal(links[index].span));
                };
                let element = self.read_index(&receiver, &key, &links[index], *expression)?;
                self.values.push(element);
                self.frames.push(Frame::Link {
                    base,
                    links,
                    index: index + 1,
                    in_target,
                });
            }
            Frame::AssignName(name) => {
                let value = self.pop();
                if self.scope.assign(&name.name, value).is_err() {
                    return Err(Diagnostic::new(
                        Category::Reference,
                        Code::UNDECLARED_ASSIGNMENT,
                        format!(
                            "cannot assign to `{}`: it is not declared; declare it first with `let {} = …`",
                            name.name, name.name
                        ),
                        name.span,
                    ));
                }
            }
            Frame::AssignMember { base, prefix, link } => self.assign_member(base, prefix, link)?,
        }
        Ok(None)
    }

    fn statement(&mut self, statement: &'p Statement) -> Result<Option<Value>, Diagnostic> {
        match &statement.kind {
            StatementKind::Let {
                name, initializer, ..
            } => {
                self.frames.push(Frame::Declare(name));
                self.frames.push(Frame::Eval(*initializer));
            }
            StatementKind::Expression(id) => {
                self.frames.push(Frame::Discard);
                self.frames.push(Frame::Eval(*id));
            }
            StatementKind::Return { value, .. } => match value {
                Some(id) => {
                    self.frames.push(Frame::Return);
                    self.frames.push(Frame::Eval(*id));
                }
                None => return Ok(Some(Value::Null)),
            },
            StatementKind::Block(id) => {
                let block = self.program.block(*id);
                let inner = Scope::child(&self.scope);
                let outer = core::mem::replace(&mut self.scope, inner);
                self.frames.push(Frame::ExitScope(outer));
                self.frames
                    .extend(block.statements.iter().rev().map(Frame::Statement));
            }
            StatementKind::Assignment { target, value, .. } => {
                self.assignment(*target, *value)?;
            }
            StatementKind::Function { name, .. } => {
                return Err(not_yet("named function declarations", name.span));
            }
            StatementKind::If { branches, .. } => {
                let span = branches.first().map_or(statement.span, |b| b.keyword);
                return Err(not_yet("`if` statements", span));
            }
            StatementKind::While { keyword, .. } => {
                return Err(not_yet("`while` loops", *keyword));
            }
            StatementKind::For { keyword, .. } => return Err(not_yet("`for` loops", *keyword)),
            StatementKind::Break { keyword } => return Err(not_yet("`break`", *keyword)),
            StatementKind::Continue { keyword } => return Err(not_yet("`continue`", *keyword)),
        }
        Ok(None)
    }

    fn assignment(&mut self, target: ExprId, value: ExprId) -> Result<(), Diagnostic> {
        let target_expression = self.program.expression(target);
        match &target_expression.kind {
            ExpressionKind::Identifier(name) => {
                self.frames.push(Frame::AssignName(name));
                self.frames.push(Frame::Eval(value));
            }
            ExpressionKind::Access { base, links } => {
                let Some((link, prefix)) = links.split_last() else {
                    return Err(invalid_target(target_expression.span));
                };
                if prefix.iter().any(|l| l.optional) || link.optional {
                    return Err(invalid_target(target_expression.span));
                }
                // Order: receiver, then index key, then the assigned value.
                self.frames.push(Frame::AssignMember {
                    base: *base,
                    prefix,
                    link,
                });
                self.frames.push(Frame::Eval(value));
                match &link.kind {
                    AccessKind::Property(_) => {}
                    AccessKind::Index { expression, .. } => {
                        self.frames.push(Frame::Eval(*expression));
                    }
                    AccessKind::Call { .. } => return Err(invalid_target(target_expression.span)),
                }
                self.frames.push(Frame::Link {
                    base: *base,
                    links: prefix,
                    index: 0,
                    in_target: true,
                });
                self.frames.push(Frame::Eval(*base));
            }
            _ => return Err(invalid_target(target_expression.span)),
        }
        Ok(())
    }

    fn expression(&mut self, id: ExprId) -> Result<(), Diagnostic> {
        let expression = self.program.expression(id);
        match &expression.kind {
            ExpressionKind::Literal(literal) => self.values.push(match literal {
                Literal::Null => Value::Null,
                Literal::Bool(b) => Value::Bool(*b),
                Literal::Number(n) => Value::Number(*n),
                Literal::String(s) => Value::String(Arc::from(s.as_str())),
            }),
            ExpressionKind::Identifier(name) => {
                let Some(value) = self.scope.lookup(&name.name) else {
                    return Err(Diagnostic::new(
                        Category::Reference,
                        Code::UNDECLARED_IDENTIFIER,
                        format!("`{}` is not declared", name.name),
                        name.span,
                    ));
                };
                self.values.push(value);
            }
            ExpressionKind::Group { expression, .. } => self.frames.push(Frame::Eval(*expression)),
            ExpressionKind::Unary { operator, operand } => {
                self.frames.push(Frame::Unary {
                    operator: *operator,
                    operand: *operand,
                });
                self.frames.push(Frame::Eval(*operand));
            }
            ExpressionKind::Binary {
                left,
                operator,
                right,
            } => {
                self.frames.push(Frame::BinaryRight {
                    left: *left,
                    operator: *operator,
                    right: *right,
                });
                self.frames.push(Frame::Eval(*left));
            }
            ExpressionKind::Array { elements, .. } => {
                self.frames.push(Frame::BuildArray(elements.len()));
                self.frames
                    .extend(elements.iter().rev().map(|e| Frame::Eval(*e)));
            }
            ExpressionKind::Object { entries, .. } => {
                self.frames.push(Frame::BuildObject(entries));
                self.frames
                    .extend(entries.iter().rev().map(|e| Frame::Eval(e.value)));
            }
            ExpressionKind::Access { base, links } => {
                self.frames.push(Frame::Link {
                    base: *base,
                    links,
                    index: 0,
                    in_target: false,
                });
                self.frames.push(Frame::Eval(*base));
            }
            ExpressionKind::Function(function) => {
                return Err(not_yet("function expressions", function.keyword));
            }
        }
        Ok(())
    }

    /// Apply `links[index]` to the receiver on top of the value stack.
    fn link(
        &mut self,
        base: ExprId,
        links: &'p [AccessLink],
        index: usize,
        in_target: bool,
    ) -> Result<(), Diagnostic> {
        let Some(link) = links.get(index) else {
            return Ok(()); // chain complete; its value is on the stack
        };
        let receiver_is_null = self.values.last().is_none_or(Value::is_null);
        if receiver_is_null && link.optional {
            // §4.4: `?.` on null short-circuits the whole remaining chain; `null` stays as the
            // chain's value and no later link (or index expression) is evaluated.
            return Ok(());
        }
        if let AccessKind::Call { .. } = link.kind {
            return Err(not_yet("function calls", link.span));
        }
        if receiver_is_null {
            // `?.` is not allowed in an assignment target, so only suggest it for plain reads.
            return Err(self.null_access(base, &links[..index], link, "read", !in_target));
        }
        match &link.kind {
            AccessKind::Property(name) => {
                let receiver = self.pop();
                let value = read_property(&receiver, name, link)?;
                self.values.push(value);
                self.frames.push(Frame::Link {
                    base,
                    links,
                    index: index + 1,
                    in_target,
                });
            }
            AccessKind::Index { expression, .. } => {
                self.frames.push(Frame::IndexRead {
                    base,
                    links,
                    index,
                    in_target,
                });
                self.frames.push(Frame::Eval(*expression));
            }
            AccessKind::Call { .. } => {}
        }
        Ok(())
    }

    fn read_index(
        &self,
        receiver: &Value,
        key: &Value,
        link: &AccessLink,
        key_expression: ExprId,
    ) -> Result<Value, Diagnostic> {
        let key_span = self.program.expression(key_expression).span;
        match (receiver, key) {
            (Value::Array(array), Value::Number(n)) => {
                // §7: anything that is not an in-range whole index reads as absent.
                Ok(array_slot(*n)
                    .and_then(|i| array.get(i))
                    .unwrap_or(Value::Null))
            }
            (Value::Object(object), Value::String(k)) => Ok(object.get(k).unwrap_or(Value::Null)),
            (Value::Array(_) | Value::Object(_), _) => Err(wrong_key(receiver, key, key_span)),
            _ => Err(not_indexable(receiver, link.span)),
        }
    }

    fn assign_member(
        &mut self,
        base: ExprId,
        prefix: &'p [AccessLink],
        link: &'p AccessLink,
    ) -> Result<(), Diagnostic> {
        let value = self.pop();
        match &link.kind {
            AccessKind::Property(name) => {
                let receiver = self.pop();
                match &receiver {
                    Value::Object(object) => {
                        object.store(&name.name, value);
                        Ok(())
                    }
                    Value::Null => Err(self.null_access(base, prefix, link, "assign", false)),
                    other => Err(no_properties(other, name, link.span)),
                }
            }
            AccessKind::Index { expression, .. } => {
                let key = self.pop();
                let receiver = self.pop();
                let key_span = self.program.expression(*expression).span;
                match (&receiver, &key) {
                    (Value::Null, _) => Err(self.null_access(base, prefix, link, "assign", false)),
                    (Value::Array(array), Value::Number(n)) => {
                        if array_slot(*n).is_some_and(|i| array.store(i, value)) {
                            Ok(())
                        } else {
                            Err(Diagnostic::new(
                                Category::Reference,
                                Code::INDEX_OUT_OF_RANGE,
                                format!(
                                    "cannot assign at index {} of an array of length {}: only an \
                                     existing index or the length (to append) can be written",
                                    number_to_string(*n),
                                    array.len()
                                ),
                                key_span,
                            ))
                        }
                    }
                    (Value::Object(object), Value::String(k)) => {
                        object.store(k, value);
                        Ok(())
                    }
                    (Value::Array(_) | Value::Object(_), _) => {
                        Err(wrong_key(&receiver, &key, key_span))
                    }
                    _ => Err(not_indexable(&receiver, link.span)),
                }
            }
            AccessKind::Call { .. } => Err(internal(link.span)),
        }
    }

    /// A `reference` error for applying `link` to `null`, naming what produced the null: the
    /// last of `before` (the chain's earlier links), or the chain's base.
    fn null_access(
        &self,
        base: ExprId,
        before: &[AccessLink],
        link: &AccessLink,
        verb: &str,
        suggest_optional: bool,
    ) -> Diagnostic {
        let subject = match before.last() {
            None => match &self.program.expression(base).kind {
                ExpressionKind::Identifier(name) => format!("`{}`", name.name),
                ExpressionKind::Literal(Literal::Null) => "`null`".to_owned(),
                _ => "the value".to_owned(),
            },
            Some(previous) => match &previous.kind {
                AccessKind::Property(name) => format!("`{}`", name.name),
                AccessKind::Index { .. } => "the indexed element".to_owned(),
                AccessKind::Call { .. } => "the call result".to_owned(),
            },
        };
        let what = match &link.kind {
            AccessKind::Property(name) => format!("property `{}`", name.name),
            _ => "an index".to_owned(),
        };
        let hint = if suggest_optional {
            "; use `?.` to read it as null instead"
        } else {
            ""
        };
        Diagnostic::new(
            Category::Reference,
            Code::NULL_ACCESS,
            format!("cannot {verb} {what}: {subject} is null{hint}"),
            link.span,
        )
    }

    fn binary_right(&mut self, left: ExprId, operator: Spanned<BinaryOperator>, right: ExprId) {
        match operator.kind {
            // §4.1: `||` keeps a truthy left, `&&` keeps a falsy left; otherwise the result is
            // the right operand. The decided case never evaluates the right side.
            BinaryOperator::Or | BinaryOperator::And => {
                let left_truthy = self.values.last().is_some_and(Value::is_truthy);
                if left_truthy == (operator.kind == BinaryOperator::Or) {
                    return;
                }
                self.pop();
                self.frames.push(Frame::Eval(right));
            }
            _ => {
                self.frames.push(Frame::Binary {
                    left,
                    operator,
                    right,
                });
                self.frames.push(Frame::Eval(right));
            }
        }
    }

    fn unary(
        &self,
        operator: Spanned<UnaryOperator>,
        operand: ExprId,
        value: &Value,
    ) -> Result<Value, Diagnostic> {
        match operator.kind {
            UnaryOperator::Not => Ok(Value::Bool(!value.is_truthy())),
            UnaryOperator::Negate => match to_number(value) {
                Some(n) => Ok(Value::Number(-n)),
                None => Err(Diagnostic::new(
                    Category::Type,
                    Code::OPERAND_MISMATCH,
                    format!(
                        "cannot apply unary `-` to {}: {}",
                        article(value),
                        not_a_number_reason(value)
                    ),
                    self.program.expression(operand).span,
                )),
            },
        }
    }

    fn binary(
        &self,
        left: ExprId,
        operator: Spanned<BinaryOperator>,
        right: ExprId,
        l: &Value,
        r: &Value,
    ) -> Result<Value, Diagnostic> {
        use BinaryOperator as Op;
        let left_span = self.program.expression(left).span;
        let right_span = self.program.expression(right).span;
        let symbol = binary_symbol(operator.kind);
        let mismatch = |span: Span, reason: String| {
            Diagnostic::new(
                Category::Type,
                Code::OPERAND_MISMATCH,
                format!(
                    "cannot apply `{symbol}` to {} and {}: {reason}",
                    l.type_name(),
                    r.type_name()
                ),
                span,
            )
        };
        let number = |value: &Value, span: Span| {
            to_number(value).ok_or_else(|| mismatch(span, not_a_number_reason(value).to_owned()))
        };
        let finite = |n: f64| {
            if n.is_finite() {
                Ok(Value::Number(n))
            } else {
                Err(Diagnostic::new(
                    Category::Arithmetic,
                    Code::NON_FINITE,
                    format!("the result of `{symbol}` is not a finite number"),
                    operator.span,
                ))
            }
        };
        match operator.kind {
            Op::Add => {
                if matches!(l, Value::String(_)) || matches!(r, Value::String(_)) {
                    let text = |value: &Value, span: Span| {
                        to_string(value).ok_or_else(|| {
                            mismatch(
                                span,
                                format!("{} cannot be converted to a string", article(value)),
                            )
                        })
                    };
                    let a = text(l, left_span)?;
                    let b = text(r, right_span)?;
                    let mut joined = String::with_capacity(a.len() + b.len());
                    joined.push_str(&a);
                    joined.push_str(&b);
                    Ok(Value::String(Arc::from(joined)))
                } else {
                    finite(number(l, left_span)? + number(r, right_span)?)
                }
            }
            Op::Subtract => finite(number(l, left_span)? - number(r, right_span)?),
            Op::Multiply => finite(number(l, left_span)? * number(r, right_span)?),
            Op::Divide | Op::Remainder => {
                let a = number(l, left_span)?;
                let b = number(r, right_span)?;
                if b == 0.0 {
                    return Err(Diagnostic::new(
                        Category::Arithmetic,
                        Code::DIVISION_BY_ZERO,
                        format!("`{symbol}` by zero"),
                        right_span,
                    ));
                }
                finite(if operator.kind == Op::Divide {
                    a / b
                } else {
                    a % b
                })
            }
            Op::Less | Op::LessEqual | Op::Greater | Op::GreaterEqual => {
                let ordering = if let (Value::String(a), Value::String(b)) = (l, r) {
                    // UTF-8 byte order is Unicode code point order.
                    a.as_ref().partial_cmp(b.as_ref())
                } else {
                    number(l, left_span)?.partial_cmp(&number(r, right_span)?)
                };
                let result = ordering.is_some_and(|o| match operator.kind {
                    Op::Less => o.is_lt(),
                    Op::LessEqual => o.is_le(),
                    Op::Greater => o.is_gt(),
                    _ => o.is_ge(),
                });
                Ok(Value::Bool(result))
            }
            Op::Equal => Ok(Value::Bool(l.equals(r))),
            Op::NotEqual => Ok(Value::Bool(!l.equals(r))),
            // Resolved in `binary_right`; reaching here means both operands were evaluated.
            Op::And | Op::Or => Ok(r.clone()),
        }
    }

    fn pop(&mut self) -> Value {
        debug_assert!(!self.values.is_empty(), "value stack underflow");
        self.values.pop().unwrap_or(Value::Null)
    }

    fn pop_many(&mut self, count: usize) -> Vec<Value> {
        debug_assert!(self.values.len() >= count, "value stack underflow");
        let at = self.values.len().saturating_sub(count);
        self.values.split_off(at)
    }
}

/// A whole, non-negative number as an array position.
fn array_slot(n: f64) -> Option<usize> {
    if n >= 0.0 && n.fract() == 0.0 && n < 9_007_199_254_740_992.0 {
        // In range and whole, so the conversion is exact.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Some(n as usize)
    } else {
        None
    }
}

fn read_property(
    receiver: &Value,
    name: &Identifier,
    link: &AccessLink,
) -> Result<Value, Diagnostic> {
    match receiver {
        Value::Object(object) => Ok(object.get(&name.name).unwrap_or(Value::Null)),
        other => Err(no_properties(other, name, link.span)),
    }
}

fn no_properties(receiver: &Value, name: &Identifier, span: Span) -> Diagnostic {
    Diagnostic::new(
        Category::Type,
        Code::INVALID_PROPERTY_ACCESS,
        format!(
            "cannot access property `{}` on {}: only objects have properties",
            name.name,
            article(receiver)
        ),
        span,
    )
}

fn wrong_key(receiver: &Value, key: &Value, span: Span) -> Diagnostic {
    let expected = if matches!(receiver, Value::Array(_)) {
        "number"
    } else {
        "string"
    };
    Diagnostic::new(
        Category::Type,
        Code::INVALID_INDEX,
        format!(
            "cannot index {} with {}: {} indices must be {}s",
            article(receiver),
            article(key),
            receiver.type_name(),
            expected
        ),
        span,
    )
}

fn not_indexable(receiver: &Value, span: Span) -> Diagnostic {
    Diagnostic::new(
        Category::Type,
        Code::INVALID_INDEX,
        format!(
            "cannot index {}: only arrays and objects can be indexed",
            article(receiver)
        ),
        span,
    )
}

fn not_a_number_reason(value: &Value) -> &'static str {
    match value {
        Value::String(_) => "the string does not look like a number",
        Value::Array(_) => "an array cannot be converted to a number",
        Value::Object(_) => "an object cannot be converted to a number",
        _ => "the value cannot be converted to a number",
    }
}

fn article(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a bool",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

const fn binary_symbol(operator: BinaryOperator) -> &'static str {
    match operator {
        BinaryOperator::Multiply => "*",
        BinaryOperator::Divide => "/",
        BinaryOperator::Remainder => "%",
        BinaryOperator::Add => "+",
        BinaryOperator::Subtract => "-",
        BinaryOperator::Less => "<",
        BinaryOperator::LessEqual => "<=",
        BinaryOperator::Greater => ">",
        BinaryOperator::GreaterEqual => ">=",
        BinaryOperator::Equal => "==",
        BinaryOperator::NotEqual => "!=",
        BinaryOperator::And => "&&",
        BinaryOperator::Or => "||",
    }
}

fn not_yet(construct: &str, span: Span) -> Diagnostic {
    Diagnostic::new(
        Category::Policy,
        NOT_YET_IMPLEMENTED,
        format!("{construct}: not evaluated yet (Story 1.7)"),
        span,
    )
}

/// The parser never produces these shapes; a hand-built AST gets a diagnostic, not a panic.
fn invalid_target(span: Span) -> Diagnostic {
    Diagnostic::new(
        Category::Syntax,
        Code::INVALID_ASSIGNMENT_TARGET,
        "expected a name or ordinary property/index assignment target",
        span,
    )
}

fn internal(span: Span) -> Diagnostic {
    Diagnostic::new(
        Category::Syntax,
        Code::EXPECTED_SYNTAX,
        "malformed syntax tree",
        span,
    )
}
