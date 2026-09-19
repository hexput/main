---
title: 'Story 1.6: Evaluate expressions and variable scope'
type: 'feature'
created: '2026-09-19'
status: 'done'
baseline_commit: '1cf6f0d6ea28725aa840b672b1749c7371015194'
route: 'dispatch'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/planning-artifacts/language/LANGUAGE-REFERENCE.md'
  - '{project-root}/_bmad-output/implementation-artifacts/epic-1-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Hexput source parses into an AST, but nothing evaluates it: declarations, operators, and member access produce no values, so no script can compute anything.

**Approach:** Implement a stack-safe, continuation-driven evaluator in `hexput-interpreter` covering values, LANGUAGE-REFERENCE §4 operators/conversions/truthiness/equality, §4.4 optional access, §7 absent-data and reference rules, block scoping with shadowing, `let`, name/property/index assignment, and top-level `return` as the Script result. Runtime failures are `hexput_shared::diagnostics::Diagnostic`s with new stable codes.

## Boundaries & Constraints

**Always:** LANGUAGE-REFERENCE.md is normative; its specific §4.1/§7 rules beat its summary tables (any value is a condition; out-of-range index reads `null`). No panics on any AST the parser produces; no native recursion proportional to input nesting. `hexput-interpreter` keeps depending on `hexput-ast` only (diagnostic types via its re-export); it may add the `indexmap` crate, pinned in `[workspace.dependencies]`. Values are `Send` so a future Executor can hold a suspended evaluation across an `.await` (Story 3.1). Every error span points at the offending source (operator, operand, link, or name).

**Never:** Implement control flow, loops, functions, calls, closures, or depth limits (Story 1.7); diagnostic rendering (1.8); CLI (1.9); capability/budget checks or any host reach. No structural `PartialEq` on `Value` that could be mistaken for language equality. No new stable code for "not implemented yet".

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Mixed `+` | `"Total: " + 5`, `"10" + 5`, `true + 1`, `"Order " + null` | `"Total: 5"`, `"105"`, `2`, `"Order null"` | N/A |
| Arithmetic coercion | `"10" - 1`, `true * 3`, `" 2 " * 2` | `9`, `3`, `4` | N/A |
| Bad conversion | `"abc" * 2`, `[] - 1`, `"x" + [1]`, `-{}` | — | `type`, naming both operand types |
| Non-finite | `1 / 0`, `0 % 0`, `1e308 * 10` | — | `arithmetic` (division-by-zero vs non-finite codes) |
| Equality | `0 == false`, `"" == false`, `[] == false`, `"5" == 5`, `"a" == 1`, `null == null`, `a == a` for one array | `false`, `false`, `false`, `true`, `false`, `true`, `true`; two equal-looking literals `[] == []` → `false` | N/A |
| Ordering | `"b" > "a"`, `"10" < 9`, `null < 1`, `"x" < 1` | `true`, `false`, `true`; last → `type` | per §4.2 |
| Logic | `null \|\| "u"`, `0 && x_undeclared`, `!""`, `!"a"` | `"u"`, `0` (right never evaluated), `true`, `false` | N/A |
| Shadowing | `let x=1; { let x=2; y... } return x;` | inner reads 2; outer returns 1 | N/A |
| Undeclared | read `nope`; `nope = 1` | — | `reference`, span on the name |
| Null access | `let o={c:null}; o.c.name` | — | `reference`, naming `c` |
| Absent data | `o.missing`, `[1][5]`, `o.k = 1` | `null`, `null`, key created (appended in order) | N/A |
| Optional | `o.c?.name`, `a?.b.c.d` with `a=null`, `a?.[f]` with `a=null` and `f` undeclared | `null`, `null`, `null` (index never evaluated) | `1?.x` still `type` |
| Index types | `[1]["0"]`, `"ab"[0]`, `5.x`-style property on number/bool/string/array | — | `type` |
| Script result | no `return`; bare `return;`; `return` inside a nested block | `null`; `null`; value, rest skipped | N/A |
| Deep input | 12,000 nested groups/binaries/blocks/access links | evaluates without stack overflow | N/A |

## Decisions (approved 2026-09-19)

1. **Array write outside range:** `a[len] = v` appends; any other out-of-range, negative, or fractional index write is an error (`reference.index_out_of_range`). Reads outside range still yield `null`.
2. **Number index on an object:** `type` error — object indices must be strings.
3. **Signed numeric strings:** to-number accepts one optional leading `+` or `-` before the literal, with surrounding whitespace (`"-3" - 1` → `-4`); anything else non-literal (including `""`, `"NaN"`, `"Infinity"`, `"0x10"`) is a `type` error.
4. **Number → string:** JavaScript-style — exponent form for magnitudes ≥ 1e21 or < 1e-6 (`1e21` → `"1e+21"`, `1e-7` → `"1e-7"`), shortest round-trip digits otherwise, whole values without a decimal point, `-0` → `"0"`.
5. **Spec size:** full spec kept (~2,400 tokens) — one story, the matrix is the reference's rules made testable.

</frozen-after-approval>

## Code Map

- `crates/hexput-ast/src/lib.rs` -- flat `Program` arenas; `Program::expression/block`; `StatementKind`, `ExpressionKind`, `AccessLink{optional, kind}`, `Literal`, operators. Read-only for this story. Re-exports `Category, Code, Diagnostic, Span`.
- `crates/hexput-interpreter/src/lib.rs` -- stub; public API lives here, private modules (value, environment, machine, convert) alongside. Keep the `#![forbid(...)]` line.
- `crates/hexput-interpreter/Cargo.toml` -- add `indexmap` (workspace-pinned, 2.14.x is in the local cargo cache).
- `crates/hexput-shared/src/diagnostics.rs` -- add runtime `Code` constants next to existing ones; update `Category::Reference` doc (out-of-range index is no longer a reference error).
- `crates/hexput-parser/src/{lib,expressions,statements}.rs` -- reference for the continuation-driver pattern (explicit frame stack, no recursion). Do not modify.
- `crates/hexput-tests/Cargo.toml`, `tests/interpreter.rs` (new), `tests/shared.rs` -- dev-dependency on the interpreter; tests parse real source via `hexput_parser::parse` then evaluate.
- `scripts/check-crate-graph.py` -- no rule change needed (interpreter gains no workspace edge).

## Tasks & Acceptance

**Execution:**
- [x] `Cargo.toml` -- pin `indexmap` in `[workspace.dependencies]`; update `Cargo.lock` -- ordered object storage.
- [x] `crates/hexput-shared/src/diagnostics.rs` -- add documented runtime codes (e.g. `type.operand_mismatch`, `type.invalid_index`, `type.invalid_property_access`, `reference.undeclared_identifier`, `reference.undeclared_assignment`, `reference.null_access`, `arithmetic.division_by_zero`, `arithmetic.non_finite`, plus any answer-driven ones) -- stable machine-readable failures.
- [x] `crates/hexput-interpreter/src/*` -- `Value` (null/bool/number/`Arc<str>`/shared array/shared object; identity via pointer), conversions, scope chain, iterative evaluator, `pub fn evaluate(&Program) -> Result<Value, Diagnostic>`; constructs owned by Story 1.7 return a `Diagnostic` built from a crate-private, clearly temporary code (not in `hexput-shared`) -- the story's runtime core.
- [x] `crates/hexput-tests/tests/interpreter.rs` -- cover every I/O matrix row, exact codes/categories/spans, evaluation-order proofs for short-circuiting, and deep-input stack safety.
- [x] `crates/hexput-tests/tests/shared.rs` -- assert new code strings.
- [x] `_bmad-output/planning-artifacts/language/LANGUAGE-REFERENCE.md` -- record answered Open Questions as `[DECISION, 2026-09-19]` and fix the §3 `bool` and §7 index-out-of-range table contradictions.
- [x] `AGENTS.md`, `sprint-status.yaml` -- record verified progress.

**Acceptance Criteria:**
- Given a script whose evaluation fails, when evaluated, then exactly one `Diagnostic` returns with the §7 category, a stable code, and a message naming the offending types or name.
- Given any parsed program, when evaluated, then the interpreter never panics and deep nesting evaluates without host stack growth.
- Given the workspace, when the five CI commands run, then all pass.

## Design Notes

Mirror the parser: an explicit work stack of continuation frames (evaluate expr / apply operator / resume access link / run next statement / pop scope) plus a value stack, driven by one loop. This gives stack safety now and a natural suspension point for Story 3.1's host calls later. Shared collections use `Arc<Mutex<…>>` (short, synchronous critical sections; recover from poisoning rather than unwrap) so `Value: Send`; scopes are an `Arc`-linked chain so Story 1.7 closures can capture by reference. `?.` short-circuit: when the link's receiver is `null`, the chain's remaining links are skipped and the whole chain yields `null`.

## Verification

**Commands:**
- `cargo fmt --all --check` -- clean
- `cargo clippy --workspace --all-targets --locked -- -D warnings` -- no warnings
- `python3 scripts/check-crate-graph.py` -- all rules pass
- `cargo build --workspace --all-targets --locked` -- succeeds
- `cargo test --workspace --locked` -- all tests pass, none skipped

## Implementation Notes

- Modules: `value` (`Value`, opaque `Array`/`Object` over `Arc<Mutex<…>>`, shallow `Debug`, `Value::equals` for §4.2), `convert` (to-number/to-string, signed numeric strings, JS-style number formatting), `environment` (`Arc`-linked `Scope` chain), `machine` (frame stack + value stack, one loop). `Value` is `#[non_exhaustive]` so Story 1.7 can add functions.
- Stack safety covers drop as well as evaluation: collections and scope chains release children through a flat work list (`Arc::into_inner`), so a 12,000-deep nested literal or a failure 12,000 blocks deep drops without recursion.
- Span choices: conversion failures point at the offending operand; division by zero at the divisor; non-finite results at the operator; null access, property-on-non-object, and index-on-non-collection at the access link; a wrong key type at the index expression; out-of-range array writes at the index expression; undeclared names at the name.
- Assignment evaluates receiver, then index key, then value, then stores; a null or wrong-type receiver is reported at the store.
- Story 1.7 constructs return category `policy`, crate-private code `temporary.not_yet_implemented` (`NOT_YET_IMPLEMENTED` in `hexput-interpreter/src/lib.rs`) — delete it in 1.7.
- Known limitation: a collection that contains itself (`a[0] = a`) is an `Arc` cycle and leaks until the process exits; no crash or recursion results. Worth revisiting alongside Story 1.7 closures and Epic 5 memory budgets.

## Spec Change Log

## Review Triage Log

Pass 1: blind hunter, edge-case hunter, verification-gap reviewer.

| ID | Finding | Verdict | Evidence | Route |
|---|---|---|---|---|
| B1 | AGENTS.md says 1.6 done while tracker says review | low | Same convention AGENTS.md already used for 1.1–1.5 (tracker `review`); agent-context file. | reject |
| B2 | `evaluate` takes no starting bindings | low | Story 1.9 owns CLI starting variables; an additive `evaluate_with` extends the API without breaking it. | reject |
| B3 | No outside hook to drive/meter evaluation | false | Budgets/capabilities are excluded by intent (Never); the frame loop is private and can expose stepping later without API churn. No defect in 1.6 behavior. | reject |
| B4 | Temporary code uses `policy` category | low | Crate-private, never on the wire, deleted in 1.7; a category is mandatory and none fits better. | reject |
| B5 | `not_yet` message ungrammatical for singular constructs | low | "`break` are not evaluated yet" confirmed in `not_yet`; direct rewording. | patch |
| B6 | Malformed-AST paths report `syntax` | low | Unreachable from parser output; exists only to avoid panics on hand-built ASTs. | reject |
| B7 | `Category::Type` doc omits index/property errors | low | Doc still said only "a conversion"; direct doc fix. | patch |
| B8 | `%` sign/fraction semantics undecided in reference | low | Truncated remainder is the conventional reading of "remainder" and is test-locked; no user-visible defect. | reject |
| B9 | to-number trims Unicode whitespace | low | Reference says "surrounding whitespace" without restriction; Unicode trimming is the more forgiving reading. | reject |
| B10 | Literals/keys allocate per evaluation | low | Real but a hot-path cost only; Epic 4 benchmarking owns performance. | reject |
| B11 | Name lookup O(scope depth) with a lock per level | low | Real, performance only; slot resolution is a larger redesign. | reject |
| B12 | Self-referencing collections leak; only recorded in the spec | medium | `Arc` cycles have no collector; 1.7 closures make cycles routine. Memory-model fix is beyond a patch and belongs before 1.7 closures. | defer |
| B13 | Missing message/category/object-drop/call-order tests | low | Verification-gap reviewer showed removing `Drop for Entries` fails the deep test, so object drop is covered; the rest are message wording checks. | reject |
| E1 | Assignment-prefix null access suggests `?.`, which targets reject | low | `Frame::Link` always used the "read" verb with the `?.` hint for assignment prefixes; authors hit `o.c.d = 1` with null `c`. | patch |
| E2 | Null-access message for group/index base names no binding | low | The span marks the exact link; Story 1.8 renders the source. | reject |
| V1 | Multi-entry object literal key/value pairing untested | medium | Reviewer reversed the zip in a copy and all 21 tests still passed. | patch |

Patches B5, B7, E1, V1 applied by the implementation agent; all five CI commands re-run and pass (interpreter 21 tests, parser 39, lexer 43, diagnostics 7, shared 4). B12 appended to deferred-work.md.
