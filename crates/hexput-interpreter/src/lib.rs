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
//!
//! # Memory
//!
//! Each call to [`evaluate`] owns one heap that holds every collection and scope the Script
//! creates; runtime values are handles into it. When evaluation ends, by result or by error,
//! the heap is dropped whole, so reference cycles (`let a = []; a[0] = a;`) cannot outlive the
//! execution. Nothing is collected while the Script runs; block scopes are the only thing
//! reclaimed early. The returned [`Value`] is detached: an owned, immutable copy of what the
//! Script returned, independent of any heap and `Send + Sync`.

mod convert;
mod environment;
mod heap;
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
/// The result is detached from the execution: a value the Script reached twice (`[x, x]`)
/// comes back as two equal copies, and no allocation the execution made outlives this call
/// except the result itself.
///
/// # Errors
/// The first runtime failure, as a [`Diagnostic`] with its §7 category, stable code, and the
/// span of the offending operator, operand, access link, or name. Evaluation stops there.
/// Returning a value that is, or contains, a value referring back to itself is a `type` error (`type.cyclic_result`) spanned on
/// the returned expression; cycles the Script builds but does not return are fine.
pub fn evaluate(program: &Program) -> Result<Value, Diagnostic> {
    machine::Machine::new(program).run()
}
