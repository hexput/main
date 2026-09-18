---
title: 'Tokenize Hexput source'
type: 'feature'
created: '2026-09-18'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: 'a11e42436b76f8435a73baec87fa9dce95ff3c82'
context: ['_bmad-output/planning-artifacts/language/LANGUAGE-REFERENCE.md']
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** `hexput-lexer` is an empty stub. Nothing can turn Hexput source text into tokens, so Stories 1.3–1.5 have no input to parse and no way to point at where a mistake was written.

**Approach:** Implement the tokenizer defined by LANGUAGE-REFERENCE §2–§6: a `tokenize(&str) -> Result<Vec<Token>, Diagnostic>` that produces position-carrying tokens for every literal, identifier, reserved word, operator and punctuator, discards comments and whitespace, and returns a `lexical` diagnostic on the first malformed input. Also lands the minimal `Span` + `Diagnostic` + `Category` types in `hexput-shared::diagnostics` that the lexer must return — Story 1.8 owns their *rendering*, not their existence.

## Boundaries & Constraints

**Always:**
- Every token carries a `Span` with a 0-based byte offset (for slicing), a 1-based line, and a 1-based column counted in **Unicode scalar values, not bytes** — identifiers are ASCII but strings and comments are full UTF-8, and a column must mean what a human sees.
- Discarding a comment or whitespace never shifts the recorded position of any surrounding token.
- Maximal munch on every ambiguous prefix: `==`/`=`, `!=`/`!`, `<=`/`<`, `>=`/`>`, `&&`, `||`, `?.`, and `/` vs `//` vs `/*`.
- Reserved words (`let fn if else while for in return break continue true false null plugin`) lex as their own token kinds, never as identifiers.
- `-` is **always** its own token — never part of a numeric literal. `-3` is unary minus applied to `3` (§4 lists `-a` as an operator), which is what makes `a-3` lex correctly. The sole exception is an exponent sign: the `-` in `1e-3` belongs to the literal.
- A `.` joins a numeric literal only when the next character is a digit, so `1.5` is one number while `1.` is `Number(1)` then `Dot`.
- A numeric literal that cannot be represented as a **finite** f64 (e.g. `1e999`) is a `lexical` error, consistent with §3's rule that infinity is never a value.
- On error, return the first `Diagnostic` and stop. Do not attempt recovery or collect multiple errors.
- Never panic on any input, including malformed UTF-8 boundaries, lone surrogates in `\u{...}`, or unterminated constructs at EOF.

**Never:**
- No `Eof` token — the acceptance criterion requires empty source to yield an *empty* token stream. The parser detects end-of-input from the slice ending.
- No parsing, precedence, or AST construction — that is Stories 1.3–1.5. The lexer is flat and context-free.
- No error *rendering* (source line display, carets, terminal formatting) — that is Story 1.8.
- `hexput-lexer` must not gain any dependency beyond `hexput-shared`.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Empty source | `""` | `Ok(vec![])` — empty stream | N/A |
| Only whitespace/comments | `"  // hi\n/* x */"` | `Ok(vec![])` | N/A |
| Position after comment | `"// c\nx"` | `x` reports line 2, column 1 | N/A |
| Keyword vs identifier | `"let letter"` | `Let`, then `Ident("letter")` | N/A |
| Maximal munch | `"a==b"`, `"a=b"` | `==` one token; `=` one token | N/A |
| Unary minus | `"a-3"` | `Ident`, `Minus`, `Number(3.0)` | N/A |
| Exponent sign | `"1e-3"` | one `Number(0.001)` | N/A |
| Dot not in number | `"1."` | `Number(1.0)`, `Dot` | N/A |
| Optional access | `"a?.b"`, `"a?.[0]"` | `?.` one token; `[` separate | N/A |
| String escapes | `"\"a\\n\\u{1F600}\""` | one `Str` with newline + emoji decoded | N/A |
| Both quote styles | `"'x'"` and `"\"x\""` | equivalent `Str("x")` | N/A |
| Non-ASCII in string | `"\"café\""` | `Str("café")`; following token's column counts chars | N/A |
| Unterminated string | `"\"abc"` | — | `lexical` error at the opening quote (EOF only) |
| Multi-line string | `"\"ab\ncd\""` | one `Str("ab\ncd")`; span covers both lines | N/A |
| Token after multi-line string | `"let a = \"x\ny\";\nb"` | `b` reports line 3, not line 1 | N/A |
| Backslash before newline | `"\"a\\\ncd\""` | — | `lexical` error: invalid escape, no line continuation |
| Unknown escape | `"\"a\\q\""` | — | `lexical` error at the escape |
| Bad `\u{...}` | `"\"\\u{110000}\""` | — | `lexical` error at the escape |
| Unterminated block comment | `"/* x"` | — | `lexical` error at the opening `/*` |
| Unknown character | `"a $ b"`, `"a & b"` | — | `lexical` error naming the character and its position |
| Non-ASCII identifier | `"café = 1"` | — | `lexical` error (§2 rejects non-ASCII identifiers) |
| Numeric overflow | `"1e999"` | — | `lexical` error (not a finite f64) |

</frozen-after-approval>

## Code Map

- `crates/hexput-lexer/src/lib.rs` — currently only `#![forbid(clippy::undocumented_unsafe_blocks)]` plus a doc comment. Keep both; the forbid attribute must stay first.
- `crates/hexput-lexer/Cargo.toml` — depends on `hexput-shared` only. Do not add dependencies; `scripts/check-crate-graph.py` and the review will both catch it.
- `crates/hexput-shared/src/diagnostics.rs` — stub doc comment only. This story defines `Span`, `Category`, `Code`, `Diagnostic` here. The Spine names this file as the one error/finding shape for the whole workspace, so do not create a lexer-local error type.
- `crates/hexput-shared/src/lib.rs` — already declares `pub mod diagnostics;`. No change needed.
- `_bmad-output/planning-artifacts/language/LANGUAGE-REFERENCE.md` — normative. §2 lexical structure, §3 literal forms, §4 operator inventory, §4.4 `?.`, §5 statement punctuation, §6 function syntax, §7 error categories. Where this spec and that document disagree, the document wins.
- Do not touch any other crate. `hexput-ast` will reuse `Span` in Story 1.3; define it so that is possible (public, `Copy`, no lexer-specific fields).

## Tasks & Acceptance

**Execution:**
- [x] `crates/hexput-shared/src/diagnostics.rs` — define `Span` (byte offset, line, column), `Category` (at minimum `Lexical`, with the §7 categories stubbed or complete), a stable `Code`, and `Diagnostic { category, code, message, span }` — the shape every later story reuses
- [x] `crates/hexput-lexer/src/lib.rs` — implement `tokenize`, `Token`, and `TokenKind` covering literals, identifiers, the 14 reserved words, and every operator/punctuator in §4–§6
- [x] `crates/hexput-lexer/` — unit-test every row of the I/O & Edge-Case Matrix, plus position correctness across multi-line input and after discarded comments

**Acceptance Criteria:**
- Given source containing identifiers, all literal forms, operators, punctuation and both comment styles, when tokenized, then every token carries a correct byte offset, line and column, and comments/whitespace are absent without shifting neighbouring positions
- Given any malformed input from the matrix, when tokenized, then a `lexical` `Diagnostic` naming the offending position is returned and the function neither panics nor silently skips input
- Given empty source, when tokenized, then the result is an empty token stream and no error
- Given the crate's manifest after this story, when inspected, then `hexput-lexer` still depends on `hexput-shared` alone

## Implementation Notes

- **Multi-line strings (human-renegotiated mid-implementation).** The frozen block originally made a raw newline inside a string a `lexical` error. Erdem asked for `"merhaba\nsosis\nben"` to lex as one string, so the matrix rows were renegotiated and the behaviour changed: a string is now unterminated only at **end of input**, never at end of line. LANGUAGE-REFERENCE §3 was silent on this, so it gained a `[DECISION]` too — leaving the normative document silent while the lexer had an opinion is exactly the doc/code drift this repo just finished cleaning up. Story 1.8's AC already anticipated multi-line spans ("rendered without truncating the location information"), so nothing downstream had to bend.
- There is **no line-continuation escape**: `\` immediately before a newline is an invalid escape, not a join. Recorded in §3 and asserted.

- **`1e` is now an error, not `Number(1)` + `Ident("e")`.** Rejecting an identifier character that abuts a numeric literal (so `0x10`, `1_000`, `1abc` fail at the literal instead of downstream) collided with the original "incomplete exponent leaves an identifier" behaviour. Only one can hold. The strict reading wins: this language has no implicit multiplication, so a number followed immediately by an identifier is never valid syntax — `1e` is a typo, and saying so at the literal beats a baffling parse error later.

- **`Span` carries `offset`, `len`, `line`, `column`.** `len` matters: Story 1.8's AC requires *marking* the span, not just pointing at its start.
- A leading UTF-8 BOM is skipped by moving the cursor only — `source` and every recorded byte offset stay relative to the original text, so Story 1.8 can still slice it. A BOM anywhere else remains an unknown character.
- Line breaks are `\n`, `\r\n` (counted once) and a lone `\r`. U+2028/U+2029 are deliberately **not** line breaks, matching Rust and the LSP position model.
- Diagnostic messages carry no line/column — the `Span` owns position, and Story 1.8 renders it with a caret. Interpolated characters go through `escape_debug` so a raw control byte never rides along into a wire response or a log.
- `@` is tokenized now. It is real language surface (§9 `@Event` / `@Global`), inert for Stories 1.3–1.5, and needed by Epic 6.

- **One process note worth recording:** the verification-gap reviewer ran `git checkout` on `crates/hexput-lexer/src/lib.rs` while probing, wiping the story's uncommitted work, then restored it. I verified the restoration independently before continuing — working tree diffed byte-identical against the staged review diff, 34/34 tests passing. Uncommitted work is fragile while reviewers have write access to the tree.

## Spec Change Log

## Review Triage Log

Pass 1 — layers: blind-hunter, edge-case-hunter, verification-gap. 43 lexer + 7 shared tests pass after patching.

| # | Finding | Verdict | Evidence | Route |
|---|---------|---------|----------|-------|
| 1 | A number abutting identifier characters lexes as two tokens: `0x10` → `Number(0)`+`Ident("x10")`, `1_000` → `Number(1)`+`Ident("_000")` (blind, edge-case) | medium | Confirmed by probe. The language has neither hex literals nor digit separators, so these are typos that surfaced as a confusing parse error elsewhere. | patch |
| 2 | `lex_escape` and `lex_unicode_escape` comments claim the caller converts the error to an unterminated string; it never runs (blind, verification-gap, mine) | medium | Verified: `?` propagates the `Err` directly. `"a\` yields `lex.invalid_escape`, never `lex.unterminated_string`. Behaviour is the better one — comments were wrong, so the comments changed. | patch |
| 3 | `escape_at_end_of_input_does_not_panic` asserts only `Category::Lexical`, trivially true for every lexer error; `stops_at_the_first_error` asserts only a column (blind, verification-gap) | medium | Confirmed: `Diagnostic::lexical` hardcodes the category, so the assertion cannot fail. This is *why* finding 2 went unnoticed. | patch |
| 4 | `hexput-shared` adds ~199 lines of public API with zero tests; the category/code wire strings are a declared Backend contract asserted nowhere (blind, verification-gap) | medium | `cargo test -p hexput-shared` reported 0 tests. Lexer tests compare `Code` constants against themselves, so a rename of the underlying string passes everything. | patch |
| 5 | Only `unknown_character` duplicates line/column into the message, and a test locks the duplication in (blind, mine) | medium | Confirmed at both call sites. `Diagnostic`'s `Display` already prints the span, so Story 1.8 would render position twice. | patch |
| 6 | A leading UTF-8 BOM makes the whole file fail at 1:1 (edge-case, blind) | medium | Confirmed: U+FEFF is not `char::is_whitespace`. Any editor that writes a BOM produced an unusable script. | patch |
| 7 | Raw control characters are interpolated into diagnostic messages (edge-case) | low | Real — messages travel over the wire and into logs. Fixed with `escape_debug`. | patch |
| 8 | Dead error branch in `lex_number`: `parse()` cannot fail on scanner-accepted text (blind) | low | Confirmed unreachable; overflow parses to infinity and hits the `is_finite` branch instead. Replaced with `expect` naming the invariant. | patch |
| 9 | `.5` behaviour undecided and unasserted (blind) | low | True; it lexed as `Dot`+`Number(5)` only incidentally. Now documented and asserted. | patch |
| 10 | `block_comment_does_not_nest` comment describes a different input; `multibyte_escape_content...` contains no escape (blind, verification-gap) | low | Both confirmed by reading. Comment corrected, test renamed, and the missing escape-width test added. | patch |
| 11 | Only `\n` ends a line, so a lone `\r` breaks line counting (edge-case) | low | True. Cheap to fix and line tracking matters more now that strings span lines. U+2028/U+2029 deliberately excluded. | patch |
| 12 | Lexer materializes the whole source as `Vec<(usize, char)>` — ~16 bytes per character (blind, edge-case) | medium | Real amplification, and actual lookahead is bounded at 3. But rewriting the cursor touches every scanner, and no Resource Budget layer exists to bound script size yet. | defer |
| 13 | `Code(&'static str)` cannot be deserialized and no type derives `serde` (blind) | medium | Real: the module declares these as `hexput-port`'s error-response types and the wire format is MessagePack. `hexput-port` is still a stub, so the shape can still change cheaply. | defer |
| 14 | `1e-999` silently underflows to `0.0` while overflow is rejected (edge-case) | low | True and arguably asymmetric with §3's infinity rule. But every mainstream language underflows silently, and LANGUAGE-REFERENCE is silent — a language decision, not a lexer bug to fix unilaterally. | defer |
| 15 | U+2028/U+2029/U+0085 should count as line breaks (edge-case) | low | Rejected: diverges from Rust and the LSP position model Epic 9 must interoperate with, and identifiers are ASCII so these appear only inside strings and comments. | rejected |
| 16 | Lexer should enforce a maximum source size (edge-case) | low | Rejected as the wrong layer: Resource Budget enforcement is `hexput-enforce`, reachable only via `hexput-exec` (AD-3). A size cap here would be a second, uncoordinated budget path. Folded into finding 12. | rejected |

## Design Notes

`Span` is deliberately three numbers, not a range: the byte offset supports slicing the original source, while line/column exist so Story 1.8 can render without re-scanning. Store the span's length (or end offset) too — Story 1.8's AC requires marking the offending span, not just its start.

Token payloads are owned (`String`) rather than borrowed slices. A borrowing lexer would push a lifetime parameter through the AST, the parser and the AST cache for a gain that the cache (Epic 4 parses each script once) makes marginal.

## Verification

**Commands:**
- `cargo test -p hexput-lexer` — expected: every matrix row covered and passing
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — expected: clean
- `cargo fmt --all --check` — expected: no diff
- `python3 scripts/check-crate-graph.py` — expected: still 10 edges asserted, `hexput-lexer` unchanged
