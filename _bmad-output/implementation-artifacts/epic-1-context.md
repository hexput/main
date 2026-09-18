# Epic 1 Context: Hexput language core

<!-- Compiled from planning artifacts. Edit freely. Regenerate with compile-epic-context if planning docs change. -->

## Goal

A script author can write Hexput source and see it evaluated correctly — variables, conditionals, loops, callbacks, objects, and arrays — running it locally through a CLI eval harness with no daemon, socket, or backend involved. This makes the language testable and fuzzable on its own, ahead of any I/O, and gives the caching, Plugin, and editor-tooling epics a parser, AST, and diagnostics shape to build on rather than invent. Epic 1 owns no functional requirement outright except the static check pass itself (its policy surface belongs to the execution-policy epic); everything else here is enabling substrate. Story 1.1 (workspace scaffold) is complete — the 22-crate workspace, CI, and the crate-graph guard script already exist.

## Stories

- Story 1.1: Project scaffold and pinned toolchain — **done**
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

- **The language reference document is normative.** `_bmad-output/planning-artifacts/language/LANGUAGE-REFERENCE.md` defines the language; where a story and that document disagree, the document wins. Its `[DECISION]` markers are approved provenance, not open questions — do not re-litigate them. Read the relevant section before implementing any story from 1.2 onward.
- **Everything carries a span.** Every token, every AST node, and every error records byte offset, line, and column. Discarding comments and whitespace must not shift the recorded positions of surrounding tokens.
- **Nothing panics on bad input.** Lexical, parse, and runtime failures are returned as structured errors; a malformed script must never abort the process, and unbounded recursion must hit a call-depth limit rather than overflow the host stack. This is the reliability property the daemon later depends on for "a bad script never takes down the daemon".
- **One error/finding shape, everywhere.** Category, stable code, human message, and source span — structured fields, not just formatted text — so the CLI, a daemon error response, and a language server render the same failure identically. Terminal rendering additionally shows the offending source line with the span marked, without truncating location info on multi-line spans.
- **The check pass is advisory and pure.** Parsed AST + callable-name set + active policy in, findings out. It never executes the script, holds no state between calls, and never grants, denies, or substitutes for a capability check or budget charge. An empty callable-name set (the CLI case) means host-call findings are simply not raised — not that every call is flagged. Warning-severity findings (e.g. unused local) can never reject a script, in any mode; the result must distinguish "clean" from "warnings only".
- **CLI behavior:** eval prints the result value and exits zero; errors render to stderr with non-zero exit. Check exits non-zero only when an error-severity finding exists, and executes no part of the script. Input variables supplied on the command line bind as the script's starting variables.
- Memory safety is a security property here: `unsafe` in `hexput-lexer`, `hexput-parser`, `hexput-interpreter`, or `hexput-exec` fails CI unless it carries a reviewed `SAFETY:` justification.

## Technical Decisions

**Crate landing.** AST node types → `hexput-ast` (data only, no logic); tokenizer → `hexput-lexer` (1.2); parser → `hexput-parser` (1.3–1.5); tree-walking evaluator → `hexput-interpreter` (1.6–1.7); the shared diagnostic types → `hexput-shared::diagnostics` (1.8); static check → `hexput-check` (1.10); eval/check command logic → `hexput-cli-core`, invoked by the thin `hexput` binary in `hexput-bin` (1.9–1.10). No logic belongs in `hexput-bin` beyond argument parsing and a call into a library crate.

**Dependency edges are compiler-enforced, not conventions.** `hexput-ast` and `hexput-lexer` depend on `hexput-shared` only; `hexput-parser` on `hexput-lexer` + `hexput-ast`; `hexput-interpreter` on `hexput-ast` only (no host reach); `hexput-check` on `hexput-ast` + `hexput-shared` only — never `hexput-interpreter`, `hexput-rpc`, or `hexput-enforce`, so it structurally cannot execute or reach the host; `hexput-cli-core` on lexer/parser/interpreter/check. `scripts/check-crate-graph.py` guards the graph in CI; adding a forbidden edge must be a build failure, not a review comment.

**Language semantics most likely to be implemented wrong** (all normative; full detail in the reference):

- One numeric type (IEEE-754 double). Division by zero, `NaN`, and infinity are runtime `arithmetic` errors, never non-finite values.
- Falsy is exactly five things: `null`, `false`, `0`, `""`, and an **empty array or empty object** (Python-style, not JavaScript). `&&`/`||` return an operand, not a `bool`, and short-circuit.
- Implicit conversion is deliberately narrow: `+` concatenates when either side is a string, otherwise converts to number; other arithmetic converts to number and raises `type` on a non-numeric string; stringifying an array or object is a `type` error (no `"[object Object]"`).
- Equality is narrower than JavaScript's: same-type compares directly (arrays/objects **by identity**, never structurally), number-vs-string converts the string, and every other cross-type comparison is `false` — so `0 == false` and `[] == false` are both `false`. Truthiness and equality are separate questions.
- Absent data is `null`: reading a missing object key or an out-of-range array index yields `null`, and writing a missing key creates it. Two cases stay `reference` errors because they mean the script is wrong: reading an undeclared identifier, and property access on `null` without `?.`.
- `?.` short-circuits the **rest of the chain** (`a?.b.c.d` is `null` when `a` is `null`) and suppresses only `null` — never a `type` error. There is no optional call form.
- Scoping is lexical and block-level; shadowing is allowed, re-declaring in the same block is a compile-time error, and assignment to an undeclared name is a runtime error (no implicit globals). Closures capture by reference. Wrong argument count is an `arity` error — no padding, no variadics. A function reaching its end without `return` yields `null`.
- Statement terminator is `;`, omittable only for the last statement in a block or file. Comments are `//` and non-nesting `/* */`. Identifiers are ASCII-only; string literals are full UTF-8 with `\n \t \r \\ \" \' \u{...}` escapes and no interpolation. Reserved words (including `plugin`) may not be used as identifiers, bare object keys, or parameter names.
- There is **no standard library at all** — not `len()`, not `push()`. Everything beyond the operators comes from host-registered functions, which appear in the grammar as ordinary call expressions. There is no import/module/filesystem/network/process syntax; the absence is structural.
- Deliberately absent from v2: `try`/`catch`, `throw`, classes, `this`, string interpolation, regex, bitwise ops, ternary, switch, generators, in-script `async`/`await`.

**Error categories** are a fixed, closed set: `lexical`, `syntax`, `type`, `reference`, `arity`, `arithmetic`, `depth`, `capability`, `budget`, `policy`. The last three are raised by later epics but belong to the same shape defined here.

## Cross-Story Dependencies

- 1.2 → 1.3 → 1.4/1.5 → 1.6/1.7 → 1.9 are strictly sequential; 1.10 needs the AST from 1.3–1.5 and the CLI from 1.9.
- Story 1.8's diagnostic types in `hexput-shared::diagnostics` are consumed retroactively by 1.2–1.7 — decide their shape early rather than writing ad-hoc error enums per crate and converting later. The same types are reused by the daemon's wire error responses and by the language server, so they must be structured and serializable-friendly.
- Plugin syntax (`plugin { }` block, `@Global`, `@Event` annotations) is a **later epic's** extension of this grammar, not Epic 1's work — but `plugin` is already a reserved word here, and the parser should be shaped so those additions don't require restructuring.
- The check pass's plugin-specific findings and its `off`/`warn`/`error` mode plumbing land in later epics; Epic 1 delivers the pass and its script-level findings only.
- The tree-sitter grammar and language server describe this same language and reuse this parser and check pass — they must not reimplement the grammar.
