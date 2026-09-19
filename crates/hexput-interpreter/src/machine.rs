//! The evaluator: one loop over an explicit stack of continuation frames plus a value stack,
//! mirroring the parser's driver. Nothing here recurses on input nesting, and every frame boundary
//! is a point where a future Executor could suspend the evaluation (Story 3.1).
//!
//! The `Machine` owns the execution's heap. Every collection and scope the Script creates lives
//! there, so dropping the `Machine` — after a result or an error — frees all of it, cycles
//! included. Only the detached result outlives it.
//!
//! The `Machine` (heap and value stack included) must stay `Send`, so a future Executor can hold
//! a suspended evaluation across an `.await` (Story 3.1); a test asserts it.

use std::sync::Arc;

use hexput_ast::{
    AccessKind, AccessLink, BinaryOperator, Category, Code, Diagnostic, ExprId, ExpressionKind,
    Identifier, Literal, ObjectEntry, Program, Span, Spanned, Statement, StatementKind,
    UnaryOperator,
};
use indexmap::IndexMap;

use crate::convert::{number_to_string, to_number, to_string};
use crate::heap::{Heap, RtValue, SlotId};
use crate::{NOT_YET_IMPLEMENTED, Value};

/// A pending unit of work. Frames that consume values document the value-stack shape they
/// expect on entry, top last.
enum Frame<'p> {
    Statement(&'p Statement),
    /// Reclaim the block's scope and restore the one that was current before it was entered.
    ExitScope(SlotId),
    /// Evaluate an expression and push its value.
    Eval(ExprId),
    /// `[value]` → `[]`.
    Discard,
    /// `[value]` → `[]`, binding `name` in the current scope.
    Declare(&'p Identifier),
    /// `[value]` → the Script result, detached from the heap. The id is the returned
    /// expression, which a cyclic result's diagnostic points at.
    Return(ExprId),
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
    values: Vec<RtValue>,
    heap: Heap,
    scope: SlotId,
}

impl<'p> Machine<'p> {
    pub(crate) fn new(program: &'p Program) -> Self {
        let mut frames: Vec<Frame<'p>> = program.statements.iter().map(Frame::Statement).collect();
        frames.reverse();
        let mut heap = Heap::default();
        let scope = heap.push_scope(None);
        Self {
            program,
            frames,
            values: Vec::new(),
            heap,
            scope,
        }
    }

    /// Run to completion. Consuming `self` is the memory contract: the heap is dropped here, on
    /// every path, and only the detached result escapes.
    pub(crate) fn run(mut self) -> Result<Value, Diagnostic> {
        self.execute()
    }

    fn execute(&mut self) -> Result<Value, Diagnostic> {
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
            Frame::ExitScope(outer) => {
                // Nothing can reference a scope once its block exits: values hold only
                // collection handles. Story 1.7 changes that — a closure captures its defining
                // scope — so it must mark a scope captured before any closure references it,
                // and this reclaim must skip captured scopes (they then live until the
                // execution ends, like any other heap garbage).
                self.heap.release(self.scope);
                self.scope = outer;
            }
            Frame::Eval(id) => self.expression(id)?,
            Frame::Discard => {
                self.pop();
            }
            Frame::Declare(name) => {
                let value = self.pop();
                self.heap.declare(self.scope, &name.name, value);
            }
            Frame::Return(expression) => {
                let value = self.pop();
                return match self.heap.detach(&value) {
                    Some(result) => Ok(Some(result)),
                    None => Err(Diagnostic::new(
                        Category::Type,
                        Code::CYCLIC_RESULT,
                        format!(
                            "cannot return this {}: it contains a value that refers back to \
                             itself, and a Script result must be a finite tree of values",
                            value.type_name()
                        ),
                        self.program.expression(expression).span,
                    )),
                };
            }
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
                let array = self.heap.new_array(items);
                self.values.push(array);
            }
            Frame::BuildObject(entries) => {
                let values = self.pop_many(entries.len());
                let map: IndexMap<Arc<str>, RtValue> = entries
                    .iter()
                    .zip(values)
                    .map(|(entry, value)| (Arc::from(entry.key.name.as_str()), value))
                    .collect();
                let object = self.heap.new_object(map);
                self.values.push(object);
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
                if self.heap.assign(self.scope, &name.name, value).is_err() {
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
                    self.frames.push(Frame::Return(*id));
                    self.frames.push(Frame::Eval(*id));
                }
                None => return Ok(Some(Value::Null)),
            },
            StatementKind::Block(id) => {
                let block = self.program.block(*id);
                let inner = self.heap.push_scope(Some(self.scope));
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
                Literal::Null => RtValue::Null,
                Literal::Bool(b) => RtValue::Bool(*b),
                Literal::Number(n) => RtValue::Number(*n),
                Literal::String(s) => RtValue::String(Arc::from(s.as_str())),
            }),
            ExpressionKind::Identifier(name) => {
                let Some(value) = self.heap.lookup(self.scope, &name.name) else {
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
        let receiver_is_null = self.values.last().is_none_or(RtValue::is_null);
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
                let value = self.read_property(&receiver, name, link)?;
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
        receiver: &RtValue,
        key: &RtValue,
        link: &AccessLink,
        key_expression: ExprId,
    ) -> Result<RtValue, Diagnostic> {
        let key_span = self.program.expression(key_expression).span;
        match (receiver, key) {
            (RtValue::Array(array), RtValue::Number(n)) => {
                // §7: anything that is not an in-range whole index reads as absent.
                Ok(array_slot(*n)
                    .and_then(|i| self.heap.array_get(*array, i))
                    .unwrap_or(RtValue::Null))
            }
            (RtValue::Object(object), RtValue::String(k)) => {
                Ok(self.heap.object_get(*object, k).unwrap_or(RtValue::Null))
            }
            (RtValue::Array(_) | RtValue::Object(_), _) => Err(wrong_key(receiver, key, key_span)),
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
                    RtValue::Object(object) => {
                        self.heap.object_store(*object, &name.name, value);
                        Ok(())
                    }
                    RtValue::Null => Err(self.null_access(base, prefix, link, "assign", false)),
                    other => Err(no_properties(other, name, link.span)),
                }
            }
            AccessKind::Index { expression, .. } => {
                let key = self.pop();
                let receiver = self.pop();
                let key_span = self.program.expression(*expression).span;
                match (&receiver, &key) {
                    (RtValue::Null, _) => {
                        Err(self.null_access(base, prefix, link, "assign", false))
                    }
                    (RtValue::Array(array), RtValue::Number(n)) => {
                        if array_slot(*n).is_some_and(|i| self.heap.array_store(*array, i, value)) {
                            Ok(())
                        } else {
                            Err(Diagnostic::new(
                                Category::Reference,
                                Code::INDEX_OUT_OF_RANGE,
                                format!(
                                    "cannot assign at index {} of an array of length {}: only an \
                                     existing index or the length (to append) can be written",
                                    number_to_string(*n),
                                    self.heap.array_len(*array)
                                ),
                                key_span,
                            ))
                        }
                    }
                    (RtValue::Object(object), RtValue::String(k)) => {
                        self.heap.object_store(*object, k, value);
                        Ok(())
                    }
                    (RtValue::Array(_) | RtValue::Object(_), _) => {
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
                let left_truthy = self.values.last().is_some_and(|v| self.heap.is_truthy(v));
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
        value: &RtValue,
    ) -> Result<RtValue, Diagnostic> {
        match operator.kind {
            UnaryOperator::Not => Ok(RtValue::Bool(!self.heap.is_truthy(value))),
            UnaryOperator::Negate => match to_number(value) {
                Some(n) => Ok(RtValue::Number(-n)),
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
        l: &RtValue,
        r: &RtValue,
    ) -> Result<RtValue, Diagnostic> {
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
        let number = |value: &RtValue, span: Span| {
            to_number(value).ok_or_else(|| mismatch(span, not_a_number_reason(value).to_owned()))
        };
        let finite = |n: f64| {
            if n.is_finite() {
                Ok(RtValue::Number(n))
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
                if matches!(l, RtValue::String(_)) || matches!(r, RtValue::String(_)) {
                    let text = |value: &RtValue, span: Span| {
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
                    Ok(RtValue::String(Arc::from(joined)))
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
                let ordering = if let (RtValue::String(a), RtValue::String(b)) = (l, r) {
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
                Ok(RtValue::Bool(result))
            }
            Op::Equal => Ok(RtValue::Bool(l.equals(r))),
            Op::NotEqual => Ok(RtValue::Bool(!l.equals(r))),
            // Resolved in `binary_right`; reaching here means both operands were evaluated.
            Op::And | Op::Or => Ok(r.clone()),
        }
    }

    fn pop(&mut self) -> RtValue {
        debug_assert!(!self.values.is_empty(), "value stack underflow");
        self.values.pop().unwrap_or(RtValue::Null)
    }

    fn pop_many(&mut self, count: usize) -> Vec<RtValue> {
        debug_assert!(self.values.len() >= count, "value stack underflow");
        let at = self.values.len().saturating_sub(count);
        self.values.split_off(at)
    }

    fn read_property(
        &self,
        receiver: &RtValue,
        name: &Identifier,
        link: &AccessLink,
    ) -> Result<RtValue, Diagnostic> {
        match receiver {
            RtValue::Object(object) => Ok(self
                .heap
                .object_get(*object, &name.name)
                .unwrap_or(RtValue::Null)),
            other => Err(no_properties(other, name, link.span)),
        }
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

fn no_properties(receiver: &RtValue, name: &Identifier, span: Span) -> Diagnostic {
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

fn wrong_key(receiver: &RtValue, key: &RtValue, span: Span) -> Diagnostic {
    let expected = if matches!(receiver, RtValue::Array(_)) {
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

fn not_indexable(receiver: &RtValue, span: Span) -> Diagnostic {
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

fn not_a_number_reason(value: &RtValue) -> &'static str {
    match value {
        RtValue::String(_) => "the string does not look like a number",
        RtValue::Array(_) => "an array cannot be converted to a number",
        RtValue::Object(_) => "an object cannot be converted to a number",
        _ => "the value cannot be converted to a number",
    }
}

fn article(value: &RtValue) -> &'static str {
    match value {
        RtValue::Null => "null",
        RtValue::Bool(_) => "a bool",
        RtValue::Number(_) => "a number",
        RtValue::String(_) => "a string",
        RtValue::Array(_) => "an array",
        RtValue::Object(_) => "an object",
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

// These tests live here, not in `hexput-tests`, because they must observe the heap itself: that
// a cycle really is in it, that its contents are released when the `Machine` drops, and that
// block scopes are reclaimed. None of that is visible through the public API, which only ever
// sees detached results. Programs are built by hand because this crate depends on `hexput-ast`
// only — no parser.
#[cfg(test)]
mod tests {
    use hexput_ast::{Block, Expression};

    use super::*;

    const fn span() -> Span {
        Span::new(0, 0, 1, 1)
    }

    fn identifier(name: &str) -> Identifier {
        Identifier {
            name: name.to_owned(),
            span: span(),
        }
    }

    fn statement(kind: StatementKind) -> Statement {
        Statement {
            span: span(),
            terminator: None,
            kind,
        }
    }

    struct Build(Program);

    impl Build {
        fn new() -> Self {
            Self(Program {
                span: span(),
                statements: Vec::new(),
                blocks: Vec::new(),
                expressions: Vec::new(),
            })
        }

        fn expression(&mut self, kind: ExpressionKind) -> ExprId {
            self.0.expressions.push(Expression { span: span(), kind });
            ExprId(self.0.expressions.len() - 1)
        }

        fn name(&mut self, name: &str) -> ExprId {
            self.expression(ExpressionKind::Identifier(identifier(name)))
        }

        fn literal(&mut self, literal: Literal) -> ExprId {
            self.expression(ExpressionKind::Literal(literal))
        }

        fn array(&mut self, elements: Vec<ExprId>) -> ExprId {
            self.expression(ExpressionKind::Array {
                open: span(),
                elements,
                close: span(),
            })
        }

        fn index(&mut self, base: ExprId, index: ExprId) -> ExprId {
            self.expression(ExpressionKind::Access {
                base,
                links: vec![AccessLink {
                    span: span(),
                    operator: span(),
                    optional: false,
                    kind: AccessKind::Index {
                        open: span(),
                        expression: index,
                        close: span(),
                    },
                }],
            })
        }

        fn declare(&mut self, name: &str, initializer: ExprId) -> Statement {
            statement(StatementKind::Let {
                keyword: span(),
                name: identifier(name),
                equals: span(),
                initializer,
            })
        }

        fn assign(&mut self, target: ExprId, value: ExprId) -> Statement {
            statement(StatementKind::Assignment {
                target,
                equals: span(),
                value,
            })
        }

        fn block(&mut self, statements: Vec<Statement>) -> Statement {
            self.0.blocks.push(Block {
                span: span(),
                open: span(),
                close: span(),
                statements,
            });
            statement(StatementKind::Block(hexput_ast::BlockId(
                self.0.blocks.len() - 1,
            )))
        }

        /// `let a = ["marker"]; a[1] = a;` — a self-containing array holding a string.
        fn cycle(&mut self) -> Vec<Statement> {
            let marker = self.literal(Literal::String("marker".to_owned()));
            let array = self.array(vec![marker]);
            let declare = self.declare("a", array);
            let receiver = self.name("a");
            let one = self.literal(Literal::Number(1.0));
            let target = self.index(receiver, one);
            let value = self.name("a");
            vec![declare, self.assign(target, value)]
        }
    }

    fn marker(machine: &Machine<'_>) -> Arc<str> {
        let strings = machine.heap.strings();
        let found = strings.iter().find(|s| s.as_ref() == "marker");
        Arc::clone(found.expect("the marker string is in the heap"))
    }

    #[test]
    fn machine_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Machine<'static>>();
    }

    #[test]
    fn a_cycle_is_freed_when_the_execution_returns() {
        let mut build = Build::new();
        let mut statements = build.cycle();
        let one = build.literal(Literal::Number(1.0));
        statements.push(statement(StatementKind::Return {
            keyword: span(),
            value: Some(one),
        }));
        build.0.statements = statements;
        let program = build.0;

        let mut machine = Machine::new(&program);
        let result = machine.execute().expect("evaluates");
        assert_eq!(result.as_number(), Some(1.0));
        assert!(machine.heap.has_self_containing_array());
        let marker = marker(&machine);
        assert!(Arc::strong_count(&marker) > 1);
        drop(machine);
        assert_eq!(Arc::strong_count(&marker), 1, "the heap released the cycle");
    }

    #[test]
    fn a_cycle_is_freed_when_the_execution_fails() {
        let mut build = Build::new();
        let mut statements = build.cycle();
        let nope = build.name("nope");
        statements.push(statement(StatementKind::Expression(nope)));
        build.0.statements = statements;
        let program = build.0;

        let mut machine = Machine::new(&program);
        let error = machine.execute().expect_err("fails");
        assert_eq!(error.code, Code::UNDECLARED_IDENTIFIER);
        assert!(machine.heap.has_self_containing_array());
        let marker = marker(&machine);
        drop(machine);
        assert_eq!(Arc::strong_count(&marker), 1, "the heap released the cycle");
    }

    #[test]
    fn only_the_detached_result_outlives_the_execution() {
        // `let a = ["marker"]; a[1] = a; return [a[0]];`
        let mut build = Build::new();
        let mut statements = build.cycle();
        let receiver = build.name("a");
        let zero = build.literal(Literal::Number(0.0));
        let element = build.index(receiver, zero);
        let returned = build.array(vec![element]);
        statements.push(statement(StatementKind::Return {
            keyword: span(),
            value: Some(returned),
        }));
        build.0.statements = statements;
        let program = build.0;

        let mut machine = Machine::new(&program);
        let result = machine.execute().expect("evaluates");
        let marker = marker(&machine);
        drop(machine);
        assert_eq!(Arc::strong_count(&marker), 2, "held by the result alone");
        drop(result);
        assert_eq!(Arc::strong_count(&marker), 1);
    }

    #[test]
    fn exited_block_scopes_are_reclaimed() {
        let n = 12_000;
        let mut build = Build::new();
        // Siblings: `{ let y = 1; }` n times — one scope slot, reused.
        let mut statements = Vec::new();
        for _ in 0..n {
            let one = build.literal(Literal::Number(1.0));
            let declare = build.declare("y", one);
            statements.push(build.block(vec![declare]));
        }
        // Nested: `{ { … { let y = 1; } … } }` n deep.
        let one = build.literal(Literal::Number(1.0));
        let mut nested = build.declare("y", one);
        for _ in 0..n {
            nested = build.block(vec![nested]);
        }
        statements.push(nested);
        // Siblings again, after the nested run: they reuse the reclaimed slots.
        for _ in 0..n {
            let one = build.literal(Literal::Number(1.0));
            let declare = build.declare("y", one);
            statements.push(build.block(vec![declare]));
        }
        build.0.statements = statements;
        let program = build.0;

        let mut machine = Machine::new(&program);
        machine.execute().expect("evaluates");
        assert_eq!(machine.heap.live(), 1, "only the root scope is live");
        assert_eq!(
            machine.heap.capacity(),
            n + 1,
            "the nested run peaks at n + 1 scopes; siblings add none"
        );
    }
}
