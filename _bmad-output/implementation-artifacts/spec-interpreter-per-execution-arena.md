---
title: 'Interpreter per-execution arena'
type: 'refactor'
created: '2026-09-19'
status: 'done'
baseline_commit: '28c57b4fdd025a4a4d5fae3cd568aa9fe4797ec8'
route: 'dispatch'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/spec-1-6-evaluate-expressions-and-variable-scope.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** `hexput-interpreter` shares collections and scopes through `Arc` reference counting, so any reference cycle (`let a = []; a[0] = a;` today, every closure capturing its own scope in Story 1.7) is never freed; in the long-running Daemon each such execution leaks.

**Approach:** Every execution's collections and scopes live in one heap owned by the `Machine`; runtime values are plain handles into it. When the execution completes — by result or by error — dropping the `Machine` frees the whole heap, cycles included. The Script result is detached into an owned value, independent of any heap, before it is returned. Language behavior does not change.

## Boundaries & Constraints

**Always:** Every existing interpreter test keeps its meaning (assertions may change only in how they read the public result type). No panics; no native recursion proportional to nesting — including detaching and dropping the result. The public result type is `Send + Sync`. `hexput-interpreter` still depends only on `hexput-ast` (+ `indexmap`). Collection identity (`==` on arrays/objects) is handle identity inside the execution.

**Never:** No garbage collection within an execution (garbage is freed at execution end; Story 3.5's memory budget will bound it). No locks around heap data. No language-visible behavior change. No Story 1.7 work.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Cycle freed | `let a = []; a[0] = a; return 1;` | `1`; heap freed at return | N/A |
| Shared result | `let x = [1]; return [x, x];` | detached `[[1], [1]]` | N/A |
| Deep result | 12,000-deep nested array/object returned | detached, then dropped, without stack overflow | N/A |
| Error path | failure after allocating collections | `Diagnostic` returned; heap freed | unchanged diagnostic |
| Scope churn | 12,000 sibling/nested blocks | scope slots reclaimed on block exit; no growth per exited block | N/A |

## Decisions (approved 2026-09-19)

1. **Cyclic result:** returning a value whose reachable graph contains a cycle is a runtime `type` error with the new stable code `type.cyclic_result`, spanned on the `return` expression. Cycles that are built but not returned are fine. A value reachable twice without a cycle (`[x, x]`) detaches as two copies.

</frozen-after-approval>

## Code Map

- `crates/hexput-interpreter/src/value.rs` -- today: public `Value` with `Array`/`Object` as `Arc<Mutex<…>>`, `equals`, `is_truthy`, flat `Drop`. Split into a crate-private runtime value (handles) and the public detached result (`Value`, `Array`, `Object` owned, keeping `as_*`, `len`, `get`, `to_vec`, `entries`, `type_name`; `ptr_eq` goes away). Keep iterative drop for the detached tree.
- `crates/hexput-interpreter/src/environment.rs` -- `Arc`-linked `Scope` with `Mutex<HashMap>`; becomes heap-resident scope records addressed by handle, with parent handle; lookup/assign walk iteratively.
- `crates/hexput-interpreter/src/machine.rs` -- owns the heap; allocation in `BuildArray`/`BuildObject`, scope push on `Block`, reclaim on `ExitScope`, reads/writes through the heap; detaches the result at `Return`/end.
- `crates/hexput-interpreter/src/convert.rs` -- conversions take runtime values; unchanged rules.
- `crates/hexput-interpreter/src/lib.rs` -- `evaluate` signature unchanged; docs describe the memory contract.
- `crates/hexput-tests/tests/interpreter.rs` -- adapt accessors; add matrix rows.
- `_bmad-output/implementation-artifacts/deferred-work.md` -- mark the spec-1-6 Arc-cycle entry RESOLVED (append a status line; do not rewrite it).

## Tasks & Acceptance

**Execution:**
- [x] `crates/hexput-interpreter/src/{value,environment,machine,convert,lib}.rs` -- heap + handles, scope reclaim, detach, cycle handling per the answered question -- the refactor.
- [x] `crates/hexput-shared/src/diagnostics.rs`, `crates/hexput-tests/tests/shared.rs` -- any new stable code the answer requires.
- [x] `crates/hexput-tests/tests/interpreter.rs` -- keep every existing case; add each matrix row, including proof that a cyclic heap is freed. The tests crate sees only the public API, so this proof is a `#[cfg(test)] mod tests` inside `machine.rs` (the documented exception) that observes the freed heap directly, with a comment saying why it lives there.
- [x] `LANGUAGE-REFERENCE.md` -- record the cyclic-result decision as `[DECISION, 2026-09-19]` if it adds a runtime error.
- [x] `deferred-work.md` -- resolve the entry.

**Acceptance Criteria:**
- Given any execution, when `evaluate` returns, then no allocation made by that execution outlives it except the detached result.
- Given the workspace, when the five CI commands run, then all pass.

## Design Notes

Heap = arena of slots (`Vec<Slot>` with `Slot::{Array(Vec<RtValue>), Object(IndexMap<Arc<str>, RtValue>), Scope{bindings, parent}}`); `RtValue` is `Copy`-cheap except strings (`Arc<str>` is fine — strings cannot form cycles). Scopes are reclaimed on block exit via a free list; Story 1.7 must mark a scope captured before a closure may reference it, so reclaim skips captured scopes — leave a comment at the reclaim site. Detach walks the reachable graph with an explicit stack, tracking the slots on the current path (not merely visited ones): `[x, x]` revisits `x` without a cycle and is not an error. Memoize each slot's detached form so shared acyclic structure is detached once (`let a = []; a = [a, a];` repeated 30 times must stay linear, not 2^30 copies): detached collections are immutable and share storage via `Arc`, which is observably identical to separate copies because the public result exposes no identity.

## Verification

**Commands:**
- `cargo fmt --all --check` -- clean
- `cargo clippy --workspace --all-targets --locked -- -D warnings` -- no warnings
- `python3 scripts/check-crate-graph.py` -- all rules pass
- `cargo build --workspace --all-targets --locked` -- succeeds
- `cargo test --workspace --locked` -- all pass, none skipped

## Implementation Notes

- New module `heap.rs`: `Heap` (`Vec<Slot>` + free list), `SlotId` handles, crate-private `RtValue` (`Copy`-cheap except `Arc<str>` strings), collection reads/writes, truthiness, `==`, and `detach`. `environment.rs` holds `ScopeRecord` and the scope operations as `impl Heap`; lookup/assign walk parent handles iteratively.
- `value.rs` is now only the detached result: `Value`, `Array(Arc<Elements>)`, `Object(Arc<Entries>)`, immutable, `Send + Sync`, flat iterative drop kept. `Array::get`/`Object::get` return `Option<&Value>`; `ptr_eq` and `Value::equals` are gone (identity has no meaning outside the execution); `is_truthy` stays.
- `detach` is an explicit-stack DFS with per-slot marks `OnPath`/`Done(Value)`: re-entering an `OnPath` slot is a cycle; a `Done` slot reuses its memoized `Arc`-shared form, so `a = [a, a]` ×40 detaches 41 collections.
- `Frame::Return` carries the returned `ExprId` so `type.cyclic_result` spans the returned expression. `Machine::run` consumes the machine, so the heap is dropped on every path; `execute(&mut self)` exists so the in-crate tests can inspect the heap before dropping it.
- Scope reclaim is in `Frame::ExitScope`, with the Story 1.7 capture comment. Only scopes are ever released; collections live until the execution ends.
- Proof tests in `machine.rs` `#[cfg(test)] mod tests` build ASTs by hand (the crate has no parser dependency) and observe the heap: a string held only by a cyclic array drops to strong count 1 once the `Machine` drops, on both the return and the error path; exited scopes leave one live slot, and sibling blocks add no slots.

## Spec Change Log

## Review Triage Log

Pass 1: blind hunter, edge-case hunter, verification-gap reviewer (no gaps).

| ID | Finding | Verdict | Evidence | Route |
|---|---|---|---|---|
| B1 | Detached DAG has exponential logical size for walkers/serializers | medium | `a = [a, a]` x40 returns 41 collections but 2^40 logical nodes; identical under the pre-arena `Arc` model. Output-size bounding belongs to Story 3.6. | defer |
| B2/E1 | Cyclic-result message false when the cycle is below the root | low | "it contains itself" is wrong for `return [1, a]`; direct rewording. | patch |
| B3 | `SlotId` reuse has no generation (ABA) | low | Safe today: only scopes are released and no value holds a scope handle. The reclaim-site comment is the 1.7 hand-off. | reject |
| B4 | Invariant violations degrade into user-looking errors | low | Unreachable from machine-produced handles; guarding adds branches with no demonstrated state. | reject |
| B5 | `detach` cannot tell a cycle from corruption | low | Same unreachable premise as B4. | reject |
| B6 | No attach path for host/Global Variable values | false | Those arrive in Epics 3 and 7; this change adds no inbound path to break. | reject |
| B7 | No detach rule for function values | false | Functions do not exist yet; `Value` is `#[non_exhaustive]` and Story 1.7 decides. | reject |
| B8 | In-execution garbage growth only noted inside a RESOLVED entry | low | Accepted design (Never: no GC), but worth an open entry pointing at Story 3.5. | defer |
| B9 | Nothing asserts `Machine: Send` | low | The `Send` requirement moved from values to the machine without a check; one-line compile-time assert. | patch |
| B10 | Scope-reclaim test pins exact capacity | low | Brittleness only; no defect. | reject |
| B11 | Frozen span wording vs implementation | false | "the `return` expression" is the returned expression, which is what the code and reference span. | reject |
| B12 | AGENTS.md Project Status stale | false | AGENTS.md already reads "Stories 1.1–1.6 are done" (committed in 28c57b4). | reject |
| B13 | `detach` bookkeeping heavier than needed | low | Performance only; Epic 4 benchmarking owns it. | reject |
| E2 | Public API break (`ptr_eq`, `equals`, `get` returns ref) | low | No consumer outside the tests crate exists. | reject |
| E3 | Cyclic return now errors despite "no behavior change" | false | The error is the approved frozen decision. | reject |

Patches B2/E1 and B9 applied; all five CI commands re-run and pass (interpreter 27 integration + 5 in-crate tests). B1 and B8 appended to deferred-work.md.
