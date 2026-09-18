# Epic 1 Context: Hexput language core

<!-- Compiled from planning artifacts. Edit freely. Regenerate with compile-epic-context if planning docs change. -->

## Goal

A script author can write Hexput source and see it evaluated correctly — variables, conditionals, loops, callbacks, objects, and arrays — running it locally through a CLI eval harness with no daemon, socket, or backend involved. This makes the language itself testable and fuzzable on its own, ahead of any I/O, and gives Epics 4, 6, and 9 a parser and AST to build on rather than invent. Epic 1 owns no FR directly except FR-26's check pass itself (its policy surface belongs to Epic 3) — it is enabling substrate consumed by FR-4, FR-5, FR-14, FR-15, and FR-19. There is no starter template and no v1 code to port: this epic creates the entire Cargo workspace from scratch.

## Stories

- Story 1.1: Project scaffold and pinned toolchain
- Story 1.2: Tokenize Hexput source
- Story 1.3: Parse expressions, declarations, and member access
- Story 1.4: Parse conditionals and loops
- Story 1.5: Parse functions, callbacks, objects, and arrays
- Story 1.6: Evaluate expressions and variable scope
- Story 1.7: Execute control flow, functions, and callbacks
- Story 1.8: Report errors with precise source locations
- Story 1.9: Evaluate a script from the command line
- Story 1.10: Check a script without running it

## Requirements & Constraints

- The normative definition of the language is a dedicated language reference document; where any story and that document disagree, the document wins. Its `[DECISION]` markers are approved provenance, not open questions requiring re-litigation.
- The static check pass (FR-26) must: report undeclared identifier reads/assignments, duplicate `let` in one block, `break`/`continue` outside a loop, wrong argument counts against script-local functions, literal-operand type errors (e.g. `"abc" * 2`), and code unreachable after `return`/`break`/`continue`. Unused-variable findings are warnings only and must never cause rejection. It optionally accepts a caller-supplied set of externally-callable names (empty for the CLI, Registered Function names for the daemon) — an empty list means host-call findings are simply not raised, not that every call is flagged.
- The check pass is a single pure function: parsed AST + callable-name set + policy in, findings out. It never executes the script and holds no state between calls. It is invoked once per submission path (Direct Execution, `CodeRegister`, Plugin registration) — never on `CachedExecutionStart`.
- Findings and runtime errors share one rendering shape: category, stable code, message, source span — so CLI, daemon response, and a future language server render them identically.
- Every lexer/parser AST node must carry byte offset, line, and column (or a source span) so errors and findings can point at exact locations.
- CLI eval and check commands must exit non-zero only when an error-severity finding/runtime error exists; check must never execute any part of the script.
- Memory safety (NFR2) is enforced mechanically: `unsafe` blocks in `hexput-lexer`, `hexput-parser`, `hexput-interpreter`, or `hexput-exec` fail CI unless carrying a reviewed `SAFETY:` justification.

## Technical Decisions

**Crate landing for this epic** (Story 1.1 creates all crates in the workspace empty; this epic's stories fill these): AST types → `hexput-ast`; lexer → `hexput-lexer` (1.2); parser → `hexput-parser` (1.3-1.5); evaluator → `hexput-interpreter` (1.6-1.7); shared diagnostics shape → `hexput-shared::diagnostics` (1.8); check pass → `hexput-check` (1.10); CLI → `hexput-cli-core` plus the `hexput` binary in `hexput-bin` (1.9-1.10).

**Dependency edges relevant to this epic** (compiler-enforced, not conventions): `hexput-ast` depends only on `hexput-shared`; `hexput-lexer` depends only on `hexput-shared`; `hexput-parser` depends on `hexput-lexer` + `hexput-ast`; `hexput-interpreter` depends only on `hexput-ast` (no host reach); `hexput-check` depends on `hexput-ast` and `hexput-shared` only — never `hexput-interpreter`, `hexput-rpc`, or `hexput-enforce` (AD-8), so it structurally cannot execute a script or reach the host; `hexput-cli-core` depends on lexer, parser, interpreter, and check. Adding a forbidden edge must fail `cargo build` with an unresolved-import error, not just a lint.

**Workspace shape:** a single Cargo workspace at the repo root with pinned versions in `[workspace.dependencies]` (Rust 1.98.1, edition 2024; tokio 1.53.1, tokio-tungstenite 0.30.0, rustls 0.23.45, serde 1.0.229, rmp-serde 1.3.1, dashmap 6.2.1, moka 0.12.16, criterion 0.8.2, tracing 0.1.44 — only the ones each crate actually needs; Epic 1's language crates need few if any of these). Every crate is a `[lib]`; `hexput-bin` is the only crate producing binaries, via thin `src/bin/*.rs` files (`hexput-daemon.rs`, `hexput.rs`, `hexput-lsp.rs`) that just parse args and call into a library crate — no other logic lives in `hexput-bin`. Every crate's `lib.rs` doc comment must state its responsibility and name the Architecture Decisions binding it. CI runs `rustfmt` and `clippy` clean with warnings denied across every member crate.

**Full crate list Story 1.1 must scaffold** (all epics, since later epics assume the whole graph exists): `hexput-shared`, `hexput-ast`, `hexput-lexer`, `hexput-parser`, `hexput-interpreter`, `hexput-check`, `hexput-cli-core`, `hexput-transport`, `hexput-port`, `hexput-session`, `hexput-connection`, `hexput-script`, `hexput-plugin`, `hexput-globalvar`, `hexput-exec`, `hexput-enforce`, `hexput-rpc`, `hexput-config`, `hexput-daemon`, `hexput-grammar`, `hexput-lsp-core`, `hexput-bin`. `hexput-shared` is itself internally split (not a grab-bag) into modules such as `diagnostics.rs` (Category/Code/Diagnostic/Span — the one error/finding shape), `wire.rs`, `ids.rs`, and `budget.rs`.

**Language design decisions that bind this epic's parser/interpreter work** (full detail in the language reference — do not re-derive, just respect): ASCII-only identifiers; `//` and non-nesting `/* */` comments; `;`-terminated statements with the terminator optional on the final statement in a block/file; reserved words (`let fn if else while for in return break continue true false null plugin`) cannot be identifiers, bare object keys, or parameter names; one numeric type (IEEE-754 double, no integers); strings support standard escapes, no interpolation; functions are values but cannot be stored in a Global Variable (closures + Global Variable lifetime semantics don't compose); division-by-zero/NaN/infinity are runtime errors, not silent non-finite values; truthiness treats exactly five values as falsy including empty collections (`if (items)` means "any items", Python-style not JS-style); every other type mismatch on an operator is a `type` error naming both operand types; equality (`==`/`!=`) is deliberately narrower than JavaScript's; formatting a collection to string is a `type` error — no implicit stringification; `?.` short-circuits the rest of an access chain but has no optional-call form (a missing Registered Function is always a `capability` error); re-declaring a name in the same block is a compile-time error, assignment to an undeclared name is a runtime error (no implicit globals); `for (item in array)` / `for (key in object)` iterate in insertion order, mutating the collection mid-iteration is a runtime error; `break`/`continue` outside a loop are compile-time errors; call with wrong argument count is a runtime error (no padding, no variadics); closures capture by reference; recursion is bounded by a call-depth limit, raising a runtime error rather than a host stack overflow; reading a missing object key or out-of-range array index yields `null`, not an error (writing to a missing key creates it); two specific cases stay `reference` errors (see language reference §7) because they mean the script itself is wrong; there is no `try`/`catch` in v2 — any error (script or host-side, from a Registered Function) terminates execution and is reported to the Backend; there is no built-in standard library at all, not even `len()`/`push()` — everything beyond operators comes from Registered Functions, keeping the trust boundary exactly at the capability edge.

**Naming convention:** code identifiers match the PRD glossary terms verbatim (e.g. `Session`, `Capability`, `Resource Budget`) — no synonyms, though most glossary terms outside Epic 1's own vocabulary (identifiers, tokens, AST nodes, diagnostics) belong to later epics.

## Cross-Story Dependencies

- Story 1.1 (workspace scaffold) blocks every other story in every epic — nothing else can land until the crate graph and pinned dependencies exist.
- Stories 1.2 → 1.3/1.4/1.5 → 1.6/1.7 → 1.8/1.9/1.10 form a strict pipeline: tokens feed the parser, the parser's AST feeds the interpreter and the check pass, and error rendering (1.8) is a prerequisite for both the CLI (1.9) and check (1.10) to report anything meaningfully.
- Story 1.10's check pass depends on the AST from 1.3-1.5 and the diagnostic shape from 1.8, but must remain independent of the interpreter (1.6-1.7) by construction (AD-8) — it is built in this epic but literally cannot execute anything.
- This epic's parser and AST are consumed directly by Epic 4 (Cached Execution), Epic 6 (Plugin source parsing), and Epic 9 (tree-sitter grammar and LSP reuse the same parser rather than reimplementing the grammar). Epic 3 owns the check pass's policy surface (when it runs, per-Config vs. per-execution override) built on top of the pure function Epic 1 delivers.
