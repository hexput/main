#![forbid(clippy::undocumented_unsafe_blocks)]

//! Tree-walking evaluator (Stories 1.6-1.7). Depends on hexput-ast only — no host reach,
//! no I/O, no execution-path capability or budget concerns of its own.
//! Binds no Architecture Decision directly; hexput-exec is what wraps it with AD-3's
//! Capability + Resource Budget enforcement.
//!
//! Story 1.6 evaluates values, operators and conversions (LANGUAGE-REFERENCE §3–§4), optional
//! access (§4.4), absent-data and reference rules (§7), block scoping, `let`, assignment, and
//! top-level `return`. Evaluation is driven by an explicit continuation stack, so input nesting
//! never grows the host stack.

mod convert;
mod environment;
mod machine;
mod value;

use hexput_ast::{Code, Diagnostic, Program};

pub use value::{Array, Object, Value};

/// TEMPORARY — reported for constructs Story 1.7 implements (control flow, functions, calls).
/// Deliberately crate-private and absent from `hexput-shared`: it is not a stable code and must
/// be deleted, not published, when Story 1.7 lands.
const NOT_YET_IMPLEMENTED: Code = Code::new("temporary.not_yet_implemented");

/// Evaluate a parsed Script and return its result: the value of the first `return` reached, or
/// `null` when execution runs off the end or hits a bare `return`.
///
/// # Errors
/// The first runtime failure, as a [`Diagnostic`] with its §7 category, stable code, and the
/// span of the offending operator, operand, access link, or name. Evaluation stops there.
pub fn evaluate(program: &Program) -> Result<Value, Diagnostic> {
    machine::Machine::new(program).run()
}
