---
title: 'Parse conditionals and loops'
type: 'feature'
created: '2026-09-19'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: 'b8edfd0feb0e6e71fb8926a0c841cb5bfc2e76ec'
context: ['_bmad-output/planning-artifacts/language/LANGUAGE-REFERENCE.md']
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The parser cannot represent branching rules or iteration for later evaluation and checking.

**Approach:** Add spanned blocks, conditionals, loops, and loop control while preserving expression behavior and diagnostics.

## Boundaries & Constraints

**Always:**
- Follow LANGUAGE-REFERENCE §§2, 4.1, 5, 7. Conditions require parentheses and bodies require braces; conditions and iterable clauses accept expressions without type checking.
- Give each body a block node; retain order, nesting, keyword/delimiter/name spans, and statement terminators.
- Enforce lexical block scopes: reject duplicate `let` within one block; permit shadowing and identical names in sibling blocks. The iteration binding belongs to its loop body scope, may shadow an outer name, and conflicts with a same-body `let`; a nested block may shadow it.
- Reject `break`/`continue` outside loops; preserve loop context through nested bodies.
- Apply §2 termination uniformly: `;` separates complete statements, including control statements; omit it only before a closing block brace or EOF. `else` is part of its `if`, so no semicolon separates them.
- Propagate lexical errors; syntax errors name the construct with stable codes and exact token/EOF spans. Consume all source.
- Preserve safe handling of deeply nested valid and invalid input, including AST clone, comparison, and destruction.

**Never:** No evaluation, truthiness computation, iterable-type validation, host access, new dependencies, unsafe code, or syntax expansion beyond this story. Functions, calls, collections, and `return` remain for Story 1.5; Plugin syntax remains for Epic 6.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected behavior |
|---|---|---|
| Branches | `if (x) {} else if (y) {1} else {2}` | Ordered conditions and distinct spanned blocks |
| Nested flow | `while (x) {if (y) {break} else {continue}}` | Correct nesting and legal loop control |
| Iteration | `for (item in source?.items) {let x = item}` | Binding, iterable expression, and body are distinct |
| Scope | `let x = 0; if (x) {let x = 1} else {let x = 2}` | Shadowing/sibling bindings accepted |
| Duplicates | `while (x) {let y = 1; let y = 2}` | Duplicate declaration at second name |
| Binding scope | `for (x in xs) {let x = 1}` | Duplicate declaration at body name |
| Terminators | `if (x) {}; x` versus `if (x) {} x` | First accepted; second rejected |
| Loop control | `break`, `if (x) {continue}`, `while (x) {}; break` | Out-of-loop syntax diagnostic |
| Bad headers | `if () {}`, `while x {}`, `for (let x in xs) {}` | Construct-aware syntax diagnostic |
| Bad bodies | Missing brace, orphan `else`, missing else body, trailing tokens | No partial success; precise syntax error |
| Depth/positions | Deep mixed blocks/chains; Unicode strings, BOM, CRLF, EOF trivia | No stack overflow; exact source spans |

</frozen-after-approval>

## Code Map

- `crates/hexput-ast/src/lib.rs`: extend `Program`/`StatementKind`; preserve the flat expression arena.
- `crates/hexput-parser/src/lib.rs`: generalize `run` into explicit frames; reuse expression parsing, target validation, token helpers, bounded diagnostics, and span helpers.
- `crates/hexput-lexer/src/lib.rs`: required tokens already exist; no changes needed.
- `crates/hexput-shared/src/diagnostics.rs`: reuse syntax codes; add loop-context code.
- `crates/hexput-tests/tests/parser.rs`: preserve structural/location/depth regressions; revise obsolete unsupported-control cases.

## Tasks & Acceptance

**Execution:**
- [x] `crates/hexput-ast/src/lib.rs` — add flat block storage and control-flow variants with documented ID ownership and spans.
- [x] `crates/hexput-shared/src/diagnostics.rs` — define stable loop-context diagnostic code.
- [x] `crates/hexput-parser/src/lib.rs` — implement iterative block/control parsing, scope tracking, header/body diagnostics, and termination rules; private helper modules may be introduced under this directory.
- [x] `crates/hexput-tests/tests/parser.rs`, `crates/hexput-tests/tests/shared.rs` — cover the matrix, structural links, exact spans/codes, scope restoration, malformed transitions, and deep success/failure inputs.
- [x] `AGENTS.md`, `_bmad-output/implementation-artifacts/sprint-status.yaml` — update story progress to the verified workflow outcome.

**Acceptance Criteria:**
- Given nested/chained conditionals, when parsed, then each branch has its own block and correct parent/child relationships.
- Given while/for statements with local declarations, when parsed, then condition/iteration clauses are distinct from bodies and declaration rules respect block boundaries.
- Given malformed control flow or invalid loop control, when parsed, then a structured diagnostic identifies the construct and source location without panic or partial success.
- Given the completed implementation, when workspace checks run, then prior expression contracts and production dependency boundaries remain valid.

## Implementation Notes

- Added flat block storage and iterative statement frames; scopes and loop context follow body frames. Expression parsing and production dependencies are unchanged.
- Matrix audit added explicit condition-link, iterable/binding, separator, and successful Unicode position assertions.

## Spec Change Log

## Review Triage Log

Pass 1: blind hunter, edge-case hunter (no findings), verification-gap reviewer.

| ID | Finding | Verdict | Evidence | Route |
|---|---|---|---|---|
| B1 | Deep failure leaves only one frame open | medium | Existing truncation removes the outermost closing brace after inner frames complete; active-frame error cleanup has no regression test. | patch |
| B2 | Malformed transitions check only code | low | Six cases cannot detect location/message regressions; direct test assertions close the gap. | patch |
| B3 | Incomplete headers lack exact EOF tests | low | Current EOF test covers a while body only, not for/if/else-if headers. | patch |
| B4 | Standalone block IDs lack structural tests | medium | Successful scope tests never follow standalone Block IDs; a stale placeholder could pass. | patch |
| B5 | Loop context leakage into sibling branches is untested | medium | Only top-level post-while break is checked; sibling branch and post-for continue paths need regression coverage. | patch |
| B6 | Nested header delimiter interaction is untested | medium | Header tests use simple identifiers/literals; group/index interactions can regress independently of expression tests. | patch |
| B7 | Block arena ordering is undocumented | low | New consumers may mistake close-order storage for source traversal; add direct API documentation. | patch |
| V1 | Standalone block references are not verified | medium | Reviewer demonstrated that retaining BlockId(0) would pass existing standalone-block cases. Same root cause as B4. | patch (grouped with B4) |

All findings were patched and reverified; nothing was deferred. B4/V1 share one structural regression test.

## Design Notes

Use flat block storage and explicit frames; each frame owns declaration names and loop context. Avoid recursive ownership/calls for bodies and else-if chains. Preserve the top-level statement and expression APIs where practical.

Intent gaps: none. Irreversible effects: none. Footprint: AST/parser, diagnostics, tests, tracking; full dispatch required.

## Verification

Run in order; every command must pass:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `python3 scripts/check-crate-graph.py`
- `cargo build --workspace --all-targets --locked`
- `cargo test --workspace --locked`

Tests must inspect AST structure and diagnostic locations, not merely successful parsing. Include deep nested bodies and long else-if chains through parse/clone/equality/drop and failure cleanup.

### Verification evidence

All five required commands passed. Workspace integration tests: 73 passed (21 parser, 43 lexer, 7 diagnostics, 2 shared), none failed or skipped. The crate graph asserts 13 rules.

Matrix mapping: branches/nested flow → `conditional_branches_and_loops_preserve_structure_and_spans`; iteration → `iteration_keeps_optional_iterable_and_binding_read_separate`; scope/duplicates/binding scope/loop control → `block_bindings_shadow_and_restore_without_leaking`; terminators → `control_statement_separator_preserves_the_following_statement` and `malformed_flow_reports_construct_and_exact_token`; bad headers/bodies → `malformed_flow_reports_construct_and_exact_token`; depth/positions → `deep_control_flow_is_flat_on_success_and_failure` and `successful_blocks_keep_bom_crlf_and_unicode_positions`. All ran successfully.

### Final verification after review

All five required checks passed after the review patches: 77 integration tests (25 parser, 43 lexer, 7 diagnostics, 2 shared), no failures or skipped tests; 13 dependency rules. Added active-frame failure cleanup, incomplete-header locations, standalone block links, sibling loop-context isolation, and nested header delimiter coverage.
