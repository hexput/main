---
title: 'Parse functions, callbacks, objects, and arrays'
type: 'feature'
created: '2026-09-19'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: 'd54ee8322f74ff726d32221a45cabc467ab0222c'
context: ['_bmad-output/planning-artifacts/language/LANGUAGE-REFERENCE.md']
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The parser cannot represent functions, calls, returns, or collection literals, preventing scripts from structuring logic and data.

**Approach:** Extend the spanned AST and pure parser with named declarations, anonymous function values, ordered parameters/arguments, return statements, objects, and arrays.

## Boundaries & Constraints

**Always:**
- Follow LANGUAGE-REFERENCE §§2–7: `fn name(params) {body}` in declaration position; `fn(params) {body}` as a value; calls bind with member/index access above unary operators.
- Preserve parameter, argument, element, and object-entry order, decoded string keys, source spans, and named versus anonymous function identity. Object keys are identifiers or quoted strings; reserved bare keys and parameter names are invalid.
- Accept empty objects/arrays and trailing commas in their literals; reject holes, missing values, missing separators, and mismatched delimiters.
- Support callbacks in arguments and function values inside collections, returned expressions, groups, and access/call chains. Preserve optional-chain boundaries and short-circuit structure; reject the explicitly forbidden `a?.()` form.
- Parse bare and valued returns. Whitespace does not terminate statements: preserve Story 1.4's semicolon rules, including named declarations. A leading statement-position brace remains a block; object expressions can be grouped or used in value positions.
- Function bodies establish their own loop context: a surrounding loop cannot authorize `break`/`continue` inside a nested function. A function's own loops remain legal.
- Preserve flat ownership and iterative handling across expressions and function bodies, including parse errors, cloning, comparison, and destruction. Keep existing expression/control-flow behavior and lexical diagnostics.

**Never:** No execution, closure environment capture, arity/type checking, host access, policy enforcement, new dependencies, unsafe code, Plugin syntax, or optional-call syntax.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected behavior |
|---|---|---|
| Functions | `fn add(a, b) {return a + b}; let f = fn(x) {return x}` | Named declaration versus anonymous value; ordered parameters and body IDs |
| Callback | `items.each(fn(item) {return item})` | Function value is the call argument |
| Chains | `factory()(x).items[0]`, `a?.b(x).c`, `(a?.b)(x)` | Calls/access preserve order and grouping boundaries |
| Collections | `let x = {a: [1, {b: 2}], "if": [],}` | Nested ordered entries/elements and decoded keys |
| Return | `fn f() {return;}`, `fn f() {return {x: [1]}}` | Absent/present return expression |
| Context | `while (x) {let f = fn() {break}}` | Out-of-loop diagnostic within function |
| Invalid lists | `[1,,2]`, `{a:}`, `f(,x)`, reserved parameters/keys | Precise syntax error; no partial success |
| Depth | Deep calls, collections, nested functions and active-frame failures | No native stack overflow; flat clone/equality/drop |
| Locations | BOM, CRLF, Unicode keys, multiline strings, EOF trivia | Exact node and error spans |

### Approved decisions (2026-09-19)

- Top-level `return` is valid and represents the Script result, including within top-level control flow.
- Reject repeated decoded object keys, including collisions between bare, quoted, and escaped spellings.
- Allow trailing commas in calls and parameter lists, as in objects and arrays; holes remain invalid.
- `let`, named functions, and parameters share one block namespace. Reject duplicate parameters and same-body redeclarations; allow nested shadowing.

</frozen-after-approval>

## Code Map

- `crates/hexput-ast/src/lib.rs`: extend flat `ExprId`/`BlockId` storage, `StatementKind`, `ExpressionKind`, and access/call representation.
- `crates/hexput-parser/src/lib.rs`: `Pending`, `expression`, `access`, `valid_target`, and `simple_statement` currently assume expressions cannot contain bodies.
- `crates/hexput-parser/src/statements.rs`: `Frame`, `run`, header parsing, scope names, and loop-context management must cooperate with suspended expression parsing.
- `crates/hexput-shared/src/diagnostics.rs`: reuse stable syntax codes; add codes only for newly settled distinct failures.
- `crates/hexput-tests/tests/parser.rs`, `shared.rs`: structural/location/depth coverage and stable codes; update unsupported examples only when replaced by positive coverage.
- `crates/hexput-lexer/src/lib.rs`: required tokens already exist; preserve its contract.

## Tasks & Acceptance

**Execution:**
- [x] `crates/hexput-ast/src/lib.rs` — add documented function/call/collection/return data and span ownership.
- [x] `crates/hexput-parser/src/lib.rs`, `statements.rs` — implement resumable parsing for body-bearing expressions and nested lists, binding rules, function context, and diagnostics; private modules may be added here.
- [x] `crates/hexput-shared/src/diagnostics.rs`, `crates/hexput-tests/tests/shared.rs` — define/test any needed stable codes.
- [x] `crates/hexput-tests/tests/parser.rs` — test matrix, resolved choices, precedence, optional-chain boundaries, scope restoration, and deep success/error cleanup.
- [x] `_bmad-output/planning-artifacts/language/LANGUAGE-REFERENCE.md` — record the four approved language decisions so later stories use the same rules.
- [x] `AGENTS.md`, `_bmad-output/implementation-artifacts/sprint-status.yaml` — record verified progress.

**Acceptance Criteria:**
- Given named and anonymous functions with returns, when parsed, then parameters, function identity, and body blocks are recoverable from the AST.
- Given nested objects/arrays, when parsed, then entry/element order is preserved at every depth.
- Given a callback argument, when parsed, then it is a function value in the call's argument list.
- Given malformed or deeply nested input, when parsed, then diagnostics remain precise and parser/AST operations remain stack-safe.

## Implementation Notes

- Added flat function/body links, ordered collection data, returns, and ordinary call links in uninterrupted access chains.
- Unified statement/body continuations with resumable expression state in `expressions.rs`; anonymous function bodies suspend and resume the original expression without native recursion.
- Function scopes reset loop context and seed their namespace with parameters. Duplicate decoded keys use `syntax.duplicate_object_key`; all other established failures retain existing codes.
- Recorded the four approved decisions in the language reference. Story tracking is `review` pending the build workflow's independent review.
- Verification passed in the required order: formatting, warning-free workspace clippy, 13 architecture-edge assertions, workspace build, and all workspace tests. Parser coverage now has 32 tests, including 12,000-level mixed function/collection and call inputs, flat clone/equality/drop, and active-frame syntax-error cleanup.

## Spec Change Log

## Review Triage Log

Pass 1: blind hunter, edge-case hunter (no findings), verification-gap reviewer.

| ID | Finding | Verdict | Evidence | Route |
|---|---|---|---|---|
| B1 | Duplicate-key message is unbounded and unescaped | medium | `list_key` interpolates the decoded key directly; large or control-bearing keys grow or inject controls into diagnostics. | patch |
| B2 | Expression-start diagnostic omits new syntax | low | The enumerated choices omit accepted fn/array/object starters; update the message and exact-message tests. | patch |
| B3 | Parameter separator diagnostic omits comma | low | `fn f(a b) {}` reaches the closing-paren expect; a comma is another valid repair. | patch |
| B4 | List closer diagnostic does not name delimiter | low | `finish_list` knows the actual closer but uses a generic label; direct message improvement. | patch |
| B5 | Function header temporarily contains BlockId(0) | false | Internal headers are never returned in Program: FunctionValue/NamedFunction continuations replace body with the completed block before insertion. Every missing body returns an error. No invalid public AST state is demonstrated. | reject |
| B6 | Parameter set is rebuilt for the body | low | Names are hashed/copied twice, but only linearly during parsing; avoiding this requires changing header/state transfer. Negligible for normal parameter lists and more than a direct correction. | reject |
| B7 | Call/collection locations lack assertions | medium | Structural tests do not inspect new delimiter, entry, and call-link fields; direct span tests needed. | patch |
| B8 | Suspension with pending operators is untested | medium | Callback list tests preserve list state but never assert unary/binary state across function bodies. | patch |
| B9 | Deep lexical failure is grouped with active-frame cleanup | low | Tokenization rejects @ before parser frames exist; other syntax failures do cover active frames. Move lexical case to clearly named lexical propagation coverage. | patch |
| B10 | Implementation and audit report different test counts | false | Append-only notes describe successive runs: implementation had 32, audit added a test and explicitly reports 33. Final verification is authoritative; proposed spec edit is also excluded by review rules. | reject |
| V1 | Call and collection source-span contracts lack assertions | medium | Reviewer demonstrated wrong stored call close/entry spans would evade existing assertions. Same root cause as B7. | patch (grouped with B7) |

### Triage correction and patch verification

B9 correction — **false**, reject: direct lexer inspection (`TokenKind::At`, `@` mapping) disproves the reviewer's premise. The deep @ input does create parser continuations and fails syntactically; it remains unchanged. Added a separate genuinely lexical `~` propagation case. This supersedes the initial B9 verdict above.

B1–B4, B7/V1, and B8 were patched. During patch inspection, the new EOF message for an object missing a value suggested a closing brace; corrected that diagnostic to require a value. No language behavior changed and nothing was deferred.

## Design Notes

The current expression and statement engines are independently iterative. Calling them recursively through anonymous function bodies would undo that guarantee. Use explicit continuation/suspend-resume state across both engines and retain flat ownership. Preserve earlier regression tests, especially standalone block IDs and loop-context restoration.

Intent gaps: none; all four decisions approved by the user. Irreversibles: none. Footprint: AST/parser, diagnostics, integration tests, tracking. Full dispatch route.

## Verification

Run in order: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`; `python3 scripts/check-crate-graph.py`; `cargo build --workspace --all-targets --locked`; `cargo test --workspace --locked`.

All must pass. Assert structure, ordered links, exact spans, and active-frame failure cleanup; mere parse success does not cover the matrix.

### Matrix audit and verification

All five required commands passed after audit additions: 86 integration tests (33 parser, 43 lexer, 7 diagnostics, 3 shared), none failed or skipped; 13 crate-graph rules.

Matrix coverage: Functions/Return → `functions_preserve_identity_parameters_bodies_and_returns`; Callback/Collections → `ordered_collections_callbacks_and_decoded_keys_are_structural`; Chains → `calls_share_access_chains_and_respect_unary_and_group_boundaries`; Context → `functions_reset_loop_context_and_share_body_namespace`; Invalid lists → `malformed_function_and_collection_lists_have_exact_locations`; Depth → `deep_functions_collections_calls_and_active_error_frames_are_flat`; Locations → `new_syntax_preserves_bom_crlf_unicode_and_multiline_locations`. `multiple_arguments_keep_order_across_calls_collections_and_function_suspension` verifies ordered arguments across a suspended function body and Return→Object→Array→Number links. Every covering test ran and passed.

### Final verification after review

All five required commands passed after patches: 92 integration tests (39 parser, 43 lexer, 7 diagnostics, 3 shared), no failures or skipped tests; 13 crate-graph rules. Review added exact call/collection location assertions, pending-operator suspension tests, bounded escaped duplicate-key diagnostics, and explicit delimiter/value diagnostic checks. Every actionable finding was resolved; no work was deferred.
