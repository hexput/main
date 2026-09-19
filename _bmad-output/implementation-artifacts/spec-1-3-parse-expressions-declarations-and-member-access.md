---
title: 'Parse expressions, declarations, and member access'
type: 'feature'
created: '2026-09-19'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: '7bdd7238ec1d4dd867f4ea4c7998dc453f72949e'
context: ['_bmad-output/planning-artifacts/language/LANGUAGE-REFERENCE.md']
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The lexer produces tokens, but AST and parser crates are stubs. Basic scripts have no representation for evaluation or editor diagnostics.

**Approach:** Add a pure source-to-AST parser for scalar expressions, declarations, assignments, and member/index access. Preserve source spans, precedence, and optional-chain structure; return shared diagnostics for malformed input.

## Boundaries & Constraints

**Always:**
- Follow LANGUAGE-REFERENCE §§2–5, §7: scalar literals, identifiers, grouping, unary/binary operators, initialized `let`, assignment and expression statements. Respect all precedence levels and binary left associativity.
- Preserve spans on nodes, identifiers, operators, and access links; byte offsets/lengths and lexer-compatible line/column. Retain grouping and per-link optionality.
- Require `;` between statements, not at EOF. Consume all input. Reject duplicate top-level `let`; allow undeclared names for later runtime/check handling.
- Assignments are statements targeting names or ordinary properties/indices. Optional chains are reads, following the defined grammar; optional reads inside indices remain legal.
- Propagate lexical errors unchanged. Syntax diagnostics identify expected syntax and the offending token, or actual EOF after trivia.

**Never:** No evaluation, type checking, host access, rendering, unsafe code, or external dependencies. Blocks/control flow belong to 1.4; calls/functions/collections to 1.5; Plugin syntax to Epic 6. Reject unsupported constructs. Preserve production dependencies and lexer contracts.

## I/O & Edge-Case Matrix

| Scenario | Input | Expected behavior |
|---|---|---|
| Empty/trivia | empty source or comments | Empty program |
| Precedence | `let x = 1 + 2 * 3; x = (x - 1) / 2` | Declaration then assignment; correct tree |
| Associativity | `a - b - c`, `a / b * c` | Left-associated binary nodes |
| Unary/access | `!-a.b[0]` | Access before negation before not |
| Optional chain | `a?.b.c?.[i + 1]` | Ordered links: optional, ordinary, optional |
| Grouping | `(a?.b).c` | Group boundary retained |
| Writes | `obj.key = 1; arr[i] = a?.b` | Ordinary targets, optional RHS |
| Bad targets | `1 = 2`, `a + b = 3`, `a?.b.c = 1`, `a = b = 1` | Syntax diagnostic |
| Declarations | `let x;`, `let if = 1`, duplicate `let x` | Syntax diagnostic at missing initializer/name or duplicate name |
| Terminators | `let x = 1 let y = 2` versus `let x = 1` | First rejected; second accepted |
| Incomplete syntax | `(1 + 2`, `a[]`, `a.`, `a?.`, `1 +` | Expected-token/expression diagnostic, no panic |
| Positions | BOM, CRLF/lone CR, Unicode/multiline strings, trailing comments | Exact source slices and EOF location |

</frozen-after-approval>

## Code Map

- `crates/hexput-lexer/src/lib.rs`: reuse `tokenize`, `Token`, `TokenKind`; owned payloads, no EOF token. Leave unchanged.
- `crates/hexput-shared/src/diagnostics.rs`: reuse `Span`, `Category::Syntax`, `Code`, `Diagnostic`.
- `crates/hexput-ast/src/lib.rs`: data-only stub; re-export shared diagnostics to avoid a new parser dependency.
- `crates/hexput-parser/src/lib.rs`: preserve forbid lint; expose `parse(&str) -> Result<Program, Diagnostic>`.
- `crates/hexput-tests/`: integration tests supersede the previous story's in-crate testing instructions.

## Tasks & Acceptance

**Execution:**
- [x] `crates/hexput-shared/src/diagnostics.rs` — add stable syntax codes.
- [x] `crates/hexput-ast/src/lib.rs` — define owned, spanned program/statements/expressions, operators, identifiers, groups, and ordered access links.
- [x] `crates/hexput-parser/src/lib.rs` — implement precedence, statements, target validation, duplicate detection, diagnostics; split private helpers if useful.
- [x] `crates/hexput-tests/Cargo.toml`, `Cargo.lock`, `scripts/check-crate-graph.py` — add AST/parser dev-dependencies, refresh lockfile, assert their production edges.
- [x] `crates/hexput-tests/tests/parser.rs`, `crates/hexput-tests/tests/shared.rs` — test matrix, all operators/precedence boundaries, exact spans, stable codes, malformed and long/deep inputs.
- [x] `AGENTS.md`, `_bmad-output/implementation-artifacts/sprint-status.yaml` — reflect verified outcome through the workflow.

**Acceptance Criteria:**
- Given basic scripts, when parsed, then AST structure preserves precedence, associativity, statement order, and source spans.
- Given mixed ordinary/optional access, when parsed, then consumers can recover link order, optionality, and grouping boundaries.
- Given malformed source, when parsed, then a structured diagnostic identifies the expected construct and location without panic or accepting a partial program.
- Given the completed change, when workspace checks run, then tests pass and production crate boundaries remain intact.

## Implementation Notes

- Implemented flat `ExprId`-linked expression storage and an iterative operator/delimiter stack instead of recursive precedence climbing. This preserves precedence while also avoiding recursive AST destruction, cloning, and comparison; no arbitrary parse-depth limit was introduced.
- Access chains keep ordered links; groups retain chain boundaries. Optional reads inside an assignment receiver group or index remain reads; the outer target must be an ordinary access.
- Added three stable syntax codes and exact AST/lexer/parser production dependency checks (13 graph assertions). Lexer behavior is unchanged.
- Matrix audit strengthened assignment tree assertions and exact error spans/messages; initial tests checked success/category too broadly for those rows.

- Review patches move consumed token payloads instead of cloning, bound escaped token previews to 64 Unicode scalars while retaining full diagnostic spans, and document arena forward references and invalid-ID panics. Added structural unary/binary and nested-index coverage plus exact invalid-target/suffix diagnostics.

## Spec Change Log

## Review Triage Log

Pass 1: blind hunter, edge-case hunter, verification-gap reviewer. The environment rejected a third reviewer thread; the completed independent edge-case reviewer was re-engaged for the verification-gap layer. Edge-case layer returned no findings. Each reported finding is recorded separately below before grouping.

| ID | Finding | Verdict | Evidence | Route |
|---|---|---|---|---|
| B1 | Epic context changes empty/missing callable-name semantics | maybe-false | Story 1.10 itself says the CLI list is empty, but separately exempts an omitted list. The new summary distinguishes these without specifying CLI behavior. No checker exists to establish a runtime regression; settle the pre-existing wording ambiguity before 1.10. Potential medium impact on future checker implementation, unverified. | defer |
| B2 | Consumed payloads are cloned and retained | medium | `bump` clones `Token`, including owned strings, while the original vector remains alive through parsing. This adds an avoidable full payload copy for large strings. | patch |
| B3 | Syntax messages copy unbounded token text | medium | `expected` escapes the complete source slice into the message; a long string in identifier position produces a proportionally large diagnostic. The span can retain extent while a bounded preview identifies the token. | patch |
| B4 | Arena forward-reference documentation is unclear | low | `a.b[c]` appends an index to an earlier Access node after allocating `c`. Existing text notes an exception but does not clearly warn later interpreter/checker consumers against assuming topological order. | patch |
| B5 | Out-of-range ExprId panic is undocumented | low | `Program::expression` directly indexes a public vector with a public ID; caller misuse panics. Document the precondition and panic, without adding another accessor. | patch |
| B6 | Unary/binary boundary lacks structural tests | medium | Unary tests contain no binary operators; binary-pair tests contain no unary operands. A changed unary precedence could pass both. | patch |
| B7 | Nested index/receiver links lack structural tests | medium | Complex assignment cases mostly assert success; no assertion traces `a[b[c] + d].e` receivers and operands. Later evaluation relies on these connections. | patch |
| B8 | Invalid assignment target span lacks regression coverage | low | Invalid-target cases assert code but not target extent; other error-span assertions cover different branches. | patch |
| B9 | Four-token sweep overstates coverage and misses longer malformed transitions | low | Its fixed length cannot reach several complete-index/group followed by malformed-operation transitions. Correct the comment and cover the named cases. | patch |
| V1 | Unary precedence against binary operators is not verified | medium | Verification-gap reviewer demonstrated that changing unary precedence from 7 to 0 evades current structural tests. Same root cause as B6; one patch covers both findings. | patch (grouped with B6) |

All patch entries were fixed and reverified. B1 is recorded in `deferred-work.md`; no parser correctness finding remains open.

## Design Notes

Use an explicit operator/delimiter stack with iterative binary/postfix handling. Explicit chains enable later short-circuiting; groups retain boundaries without deciding runtime behavior here. Avoid stack overflow on deep input, including AST destruction; this is parser robustness, not execution budgeting.

Intent gaps: none. Irreversible effects: none. Footprint: AST/parser public APIs, shared codes, tests, tracking. Source input enables accurate EOF locations despite discarded trivia.

## Verification

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `python3 scripts/check-crate-graph.py`
- `cargo build --workspace --all-targets --locked`
- `cargo test --workspace --locked`

All must pass; tests must assert tree shape and exact spans, not merely successful parsing.

### Final verification (2026-09-19)

All five commands above passed after review patches: 65 integration tests (43 lexer, 7 existing diagnostics, 14 parser, 1 syntax-code contract), no failures/ignored tests; 13 dependency rules asserted. Parser cases include all 169 binary-operator pairs, 20,736 four-token sequences, 20,000-level success/error inputs, and a 2.5 MB malformed string token with bounded diagnostics.

Matrix audit: empty/trivia → `scalars_and_empty_programs`; precedence → `statement_order_values_and_precedence`; associativity → `every_binary_operator_pair_obeys_precedence_and_left_associativity`; unary/access, optional chain, grouping → `unary_access_and_groups_preserve_chain_boundaries`; writes → `assignments_and_declaration_rules`; bad targets → that test plus `errors_cover_complete_targets_and_exact_malformed_suffixes`; declarations/terminators/incomplete syntax → `assignments_and_declaration_rules` and `malformed_and_unsupported_input_is_never_partially_accepted`; positions → `exact_positions_follow_lexer_through_unicode_bom_and_trivia`. Every named test ran and passed.
