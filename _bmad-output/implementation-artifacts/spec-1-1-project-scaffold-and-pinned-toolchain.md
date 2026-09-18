---
title: 'Project scaffold and pinned toolchain'
type: 'feature'
created: '2026-09-18'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
context: []
baseline_commit: '182cd0c1e556de541bdf765be30c0bfb25c7f77f'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The repository has no `Cargo.toml` and no `src/` — every later Epic 1+ story needs an architecturally correct crate to land in, and several Architecture Decisions (AD-1, AD-3, AD-4, AD-5, AD-8) are meant to be compiler-enforced dependency edges, not review conventions.

**Approach:** Scaffold the full 22-crate Cargo workspace exactly as the Architecture Spine's Structural Seed defines it — every crate a `[lib]` except `hexput-bin`, pinned toolchain and dependency versions, the five AD-enforcing edges wired correctly, and CI (rustfmt/clippy/test/unsafe-lint) wired to enforce it mechanically. No language or daemon logic is implemented yet — every crate is an empty, compiling stub with a `lib.rs` doc comment stating its responsibility and binding ADs.

## Boundaries & Constraints

**Always:**
- Every crate is `[lib]` except `hexput-bin`, which is the only crate with `[[bin]]` targets, via thin `src/bin/hexput-daemon.rs`, `src/bin/hexput.rs`, `src/bin/hexput-lsp.rs` that just parse args and call `hexput_daemon::run()`, `hexput_cli_core::run()`, `hexput_lsp_core::run_server()` respectively (these three functions may be `todo!()` stubs for now).
- `rust-toolchain.toml` pins `1.98.1` with `edition = "2024"`.
- `[workspace.dependencies]` in the root `Cargo.toml` pins exactly once: tokio 1.53.1, tokio-tungstenite 0.30.0, rustls 0.23.45, serde 1.0.229, rmp-serde 1.3.1, dashmap 6.2.1, moka 0.12.16, criterion 0.8.2, tracing 0.1.44. Member crates that use one inherit with `workspace = true`, never re-pinning.
- The five AD-enforcing edges match the Spine exactly: `hexput-enforce` is a dependency of `hexput-exec` and no other crate (AD-3); `hexput-globalvar` depends on neither `hexput-plugin` nor `hexput-rpc` (AD-4); `hexput-check` depends on `hexput-ast` (+`hexput-shared`) but never `hexput-interpreter`, `hexput-rpc`, or `hexput-enforce` (AD-8); `hexput-session` and `hexput-config` do not depend on each other (AD-5); only `hexput-daemon` depends on `hexput-transport` (AD-1).
- Every crate's `lib.rs` doc comment (`//!`) states its responsibility in the Spine's words and names the ADs binding it (only crates an AD actually binds need to name one).
- `hexput-lexer`, `hexput-parser`, `hexput-interpreter`, `hexput-exec` each carry `#![deny(clippy::undocumented_unsafe_blocks)]` in `lib.rs`.
- Root `Cargo.toml` sets `[workspace.package] publish = false` (or equivalent per-crate) since nothing here ships to crates.io.
- CI (`.github/workflows/ci.yml`) runs, across the whole workspace: `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
- `hexput-shared` is internally split into `diagnostics.rs`, `wire.rs`, `ids.rs`, `budget.rs` modules (empty stub contents are fine; the split itself is the requirement).

**Never:**
- No crate other than `hexput-bin` may declare a `[[bin]]` target or a `fn main()`.
- Do not implement any actual lexer, parser, interpreter, transport, or RPC logic — that is later stories' work; this story only needs everything to compile and link.
- Do not add any dependency edge outside the Spine's graph (see Code Map) without it being one of the five AD-enforcing edges called out above.

</frozen-after-approval>

## Code Map

- `Cargo.toml` (new) -- workspace root: `[workspace]` members, `[workspace.dependencies]` pins, `[workspace.package]` shared metadata (edition 2024, publish = false).
- `rust-toolchain.toml` (new) -- pins channel `1.98.1`.
- `.github/workflows/ci.yml` (new) -- build/test/fmt/clippy across workspace; no existing `.github/` directory today.
- `crates/hexput-shared/{Cargo.toml,src/lib.rs,src/diagnostics.rs,src/wire.rs,src/ids.rs,src/budget.rs}` (new) -- no deps besides workspace-pinned crates it actually needs (likely `serde`).
- `crates/hexput-ast/`, `crates/hexput-lexer/` (new) -- each depends on `hexput-shared` only.
- `crates/hexput-parser/` (new) -- depends on `hexput-lexer`, `hexput-ast`.
- `crates/hexput-interpreter/` (new) -- depends on `hexput-ast` only.
- `crates/hexput-check/` (new) -- depends on `hexput-ast`, `hexput-shared` only (AD-8: never interpreter/rpc/enforce).
- `crates/hexput-cli-core/` (new) -- depends on `hexput-lexer`, `hexput-parser`, `hexput-interpreter`, `hexput-check`.
- `crates/hexput-port/` (new) -- depends on `hexput-shared`.
- `crates/hexput-transport/` (new) -- depends on `hexput-port`.
- `crates/hexput-session/` (new) -- depends on `hexput-port`, `hexput-shared` (not `hexput-config`).
- `crates/hexput-connection/` (new) -- depends on `hexput-session`.
- `crates/hexput-globalvar/` (new) -- depends on `hexput-shared` only (not plugin/rpc).
- `crates/hexput-enforce/` (new) -- depends on `hexput-shared` only.
- `crates/hexput-rpc/` (new) -- depends on `hexput-port`.
- `crates/hexput-exec/` (new) -- depends on `hexput-enforce`, `hexput-interpreter`, `hexput-rpc`, `hexput-globalvar`.
- `crates/hexput-script/` (new) -- depends on `hexput-parser`, `hexput-interpreter`, `hexput-check`, `hexput-exec`.
- `crates/hexput-plugin/` (new) -- depends on `hexput-parser`, `hexput-globalvar`, `hexput-exec`, `hexput-check`.
- `crates/hexput-config/` (new) -- depends on `hexput-shared` only (not `hexput-session`).
- `crates/hexput-daemon/` (new) -- depends on `hexput-transport`, `hexput-session`, `hexput-connection`, `hexput-script`, `hexput-plugin`, `hexput-config`, `hexput-exec`; exposes `run(SystemConfig)`.
- `crates/hexput-grammar/` (new) -- no internal deps yet (tree-sitter grammar comes in Epic 9).
- `crates/hexput-lsp-core/` (new) -- depends on `hexput-lexer`, `hexput-parser`, `hexput-check`; exposes `run_server()`.
- `crates/hexput-bin/{Cargo.toml,src/bin/hexput-daemon.rs,src/bin/hexput.rs,src/bin/hexput-lsp.rs}` (new) -- depends on `hexput-daemon`, `hexput-cli-core`, `hexput-lsp-core`.

## Tasks & Acceptance

**Execution:**
- [x] `Cargo.toml`, `rust-toolchain.toml` -- create workspace root + toolchain pin -- establishes the workspace and the exact Rust version/edition the AC requires
- [x] `crates/hexput-shared/` -- create with the four internal modules -- every other crate needs the shared diagnostics/wire/ids/budget types to exist first
- [x] `crates/hexput-ast/`, `crates/hexput-lexer/`, `crates/hexput-parser/`, `crates/hexput-interpreter/`, `crates/hexput-check/`, `crates/hexput-cli-core/` -- create with correct `[dependencies]` -- the language pipeline crate group; `hexput-check` must NOT depend on `hexput-interpreter`
- [x] `crates/hexput-port/`, `crates/hexput-transport/`, `crates/hexput-session/`, `crates/hexput-connection/`, `crates/hexput-config/` -- create with correct `[dependencies]` -- session/transport group; verify `hexput-session` and `hexput-config` have no edge between them
- [x] `crates/hexput-globalvar/`, `crates/hexput-enforce/`, `crates/hexput-rpc/`, `crates/hexput-exec/`, `crates/hexput-script/`, `crates/hexput-plugin/` -- create with correct `[dependencies]` -- execution group; verify `hexput-enforce` has exactly one dependent (`hexput-exec`) and `hexput-globalvar` excludes `hexput-plugin`/`hexput-rpc`
- [x] `crates/hexput-daemon/` -- create, wiring the daemon-side crates into one `run(SystemConfig)` -- the composition root the AC's dependency table lists
- [x] `crates/hexput-grammar/`, `crates/hexput-lsp-core/` -- create tooling crates -- independent dev-tooling group per the Spine
- [x] `crates/hexput-bin/src/bin/{hexput-daemon,hexput,hexput-lsp}.rs` -- create thin binaries -- the only crate allowed `[[bin]]` targets
- [x] every crate's `src/lib.rs` -- add `//!` doc comment naming responsibility + binding ADs -- required by AC and by future contributors navigating the workspace
- [x] `.github/workflows/ci.yml` -- add build/test/fmt/clippy job across the workspace -- makes rustfmt/clippy-clean and the unsafe-lint mechanically enforced, not reviewer memory

**Acceptance Criteria:**
- Given a clone with no `Cargo.toml`, when `cargo build` and `cargo test` run at the workspace root, then both succeed on Rust 1.98.1/edition 2024 and all 22 named crates exist
- Given the workspace `Cargo.toml`, when inspected, then `[workspace.dependencies]` pins each of the nine stack versions exactly once and member crates inherit via `workspace = true`
- Given every crate but `hexput-bin`, when its `Cargo.toml` is inspected, then it declares `[lib]` and no `[[bin]]`
- Given the five AD-enforcing edges, when each crate's `[dependencies]` is inspected, then it matches the Spine exactly, and given a deliberate attempt to add a forbidden edge (e.g. `hexput-check` -> `hexput-interpreter`), `cargo build` fails with an unresolved-import error
- Given CI, when it runs, then `rustfmt --check` and `clippy -D warnings` pass clean across every member crate, and an undocumented `unsafe` block in `hexput-lexer`/`hexput-parser`/`hexput-interpreter`/`hexput-exec` fails the build
- Given every crate, when its `lib.rs` doc comment is inspected, then it states the crate's responsibility and names the ADs binding it

## Implementation Notes

- **Mid-run handoff.** The implementation subagent hit a session rate limit just after writing the three thin binaries. All 22 crates, manifests, `lib.rs` files, and binaries had already landed; the remaining work (`.github/workflows/ci.yml`) plus all verification was completed directly in the parent session. Both halves were judged against the unified diff, not the subagent's report.

- **`resolver = "3"`, not `"2"`.** The scaffold first set `resolver = "2"`. A virtual manifest must state the resolver explicitly, and resolver 3 is both the edition-2024 default and MSRV-aware, which is the right pairing for a workspace that pins its toolchain. Changed with a comment explaining why; full verification re-run clean afterward.

- **`SystemConfig` lives in `hexput-config`, re-exported from `hexput-daemon`.** The Spine's graph gives `hexput-bin` edges to `hexput-daemon`, `hexput-cli-core`, and `hexput-lsp-core` only — not `hexput-config`. Since the thin `hexput-daemon` binary must construct the argument to `run(SystemConfig)`, `hexput-daemon` re-exports the type rather than `hexput-bin` taking a graph edge the Spine does not sanction. The re-export carries a comment saying so.

- **Stub entry points are `todo!()`.** `hexput_cli_core::run()`, `hexput_daemon::run()`, and `hexput_lsp_core::run_server()` are `todo!()` with doc comments naming the story/epic that fills them. `SystemConfig::resolve()` returns the default rather than `todo!()` so the daemon binary's call chain type-checks end to end.

- **No crate depends on any of the nine pinned stack crates yet.** Nothing is implemented, so adding e.g. `serde` to `hexput-shared` would have introduced an unused dependency. `[workspace.dependencies]` pins all nine as the AC requires; workspace inheritance itself is exercised via `edition.workspace = true` / `publish.workspace = true` on every member.

- **Both AD boundaries were actively probed, not just eyeballed** (probes reverted, verified 0 residue):
  - AD-8: adding a `hexput_interpreter::` reference inside `hexput-check` fails with `error[E0433]: cannot find module or crate hexput_interpreter`.
  - NFR2: an undocumented `unsafe` block added to `hexput-lexer` fails clippy with `error: unsafe block missing a safety comment`.

- **CORRECTION (review pass 1): those two probes proved less than first claimed.** The `E0433` above demonstrates the compiler's *import* rule — it only fires because code referenced the crate. A forbidden edge added to a **manifest alone** compiled and linted completely clean, because every crate is currently an empty stub and nothing imports anything. So the AD boundaries were documentation, not enforcement, and the acceptance criterion asserting otherwise was not actually satisfied. Likewise the unsafe lint held in 4 of 22 crates with nothing preventing its deletion. Both are now genuinely enforced — see the review patches below.

### Review patches (pass 1)

- **`scripts/check-crate-graph.py` (new) — the substantive fix.** Asserts the Spine's AD-enforcing edges against the resolved graph from `cargo metadata --locked`: seven forbidden edges (AD-4, AD-5, AD-8) and two sole-dependent rules (AD-1's `transport <- daemon` only, AD-3's `enforce <- exec` only). Wired into CI. Verified to fail on both violation shapes — a forbidden dependency and an extra dependent — and to pass on the clean tree. This is what makes the crate split mean what the Spine says it means.
- **`--locked` on clippy/build/test.** The committed `Cargo.lock` is now enforced rather than silently re-resolved. It immediately earned its place: it caught that adding `rust-version` had made the lock stale, which `cargo generate-lockfile` then fixed.
- **`rust-version = "1.98.1"` in `[workspace.package]`**, inherited per-crate. Without it resolver 3's MSRV-aware resolution is inert, so the comment justifying the resolver change was self-refuting until this landed.
- **Unsafe lint moved to `[workspace.lints.clippy]`** (`undocumented_unsafe_blocks = "deny"`), inherited by all 22 crates via `[lints] workspace = true`; the four NFR2 crates escalate to `#![forbid(...)]`. Verified: undocumented `unsafe` in `hexput-rpc` (outside the original four) now fails, and an `#[allow]` override in `hexput-lexer` fails with `E0453 ... overruled by previous forbid`.
- **`hexput-rpc` doc comment corrected.** It claimed rpc "reaches capability/budget enforcement solely through hexput-exec" — impossible, since `exec -> rpc` is the actual direction. As written it invited a reader to "fix" it by adding an edge that would create a cycle. Now states the real direction explicitly.
- **CI hardening:** `permissions: contents: read`, a `concurrency` group cancelling superseded runs, and removal of the redundant job-level `RUSTFLAGS: -D warnings` (clippy already passes it, and env RUSTFLAGS would silently override any future `.cargo/config.toml`).
- **Dead code removed:** the three binaries collected `std::env::args()` into a discarded binding; empty `[dependencies]` sections trimmed from `hexput-shared`/`hexput-grammar`.

Five findings were deferred rather than fixed (see `deferred-work.md`) — most notably that **the Spine's own graph contradicts AD-4**: it names `hexput-session` as the sole caller of `hexput-globalvar::teardown()` while giving session no edge to globalvar. Nine were rejected with refutations recorded in the triage log.

- **Doc-comment placement.** In `hexput-lexer`, `hexput-parser`, `hexput-interpreter`, and `hexput-exec`, the `#![deny(...)]` inner attribute precedes the `//!` module doc. That ordering is valid Rust and rustfmt-clean; a naive `head -1` check for `//!` will report a false negative on these four.

## Spec Change Log

## Review Triage Log

Pass 1 — layers: blind-hunter, edge-case-hunter, verification-gap.

| # | Finding | Verdict | Evidence | Route |
|---|---------|---------|----------|-------|
| 1 | AD edges are unenforced: a forbidden edge in a manifest alone compiles/lints clean (verification-gap) | high | Pre-verified by the gap layer and re-confirmed here: appending `hexput-interpreter` to `hexput-check/Cargo.toml` with no code reference passed `build` and `clippy -D warnings`. The earlier `E0433` probe proved the compiler's *import* rule, not that the graph is guarded. | patch |
| 2 | No test/CI asserts the dependency graph (blind-hunter) | high | Same root cause as #1 — grouped. | patch |
| 3 | The nine pins are never resolved; CI never passes `--locked` (verification-gap, blind-hunter, edge-case) | medium | `Cargo.lock` contained only the 22 local packages; gap layer set tokio to a nonexistent version and the build still passed. | patch |
| 4 | `rust-version` absent, making the resolver-3 rationale inert (blind-hunter, edge-case) | medium | Confirmed absent from `[workspace.package]`. MSRV-aware resolution is a no-op without it, so the comment I added with the resolver change was self-refuting. | patch |
| 5 | Unsafe lint covers 4/22 crates; deleting the attribute is unobserved (verification-gap, blind-hunter, edge-case) | medium | Confirmed: no `[workspace.lints]`, no `[lints] workspace = true`. An undocumented `unsafe` in `hexput-rpc` passed clippy before the fix. | patch |
| 6 | `deny` is overridable by an inner `#[allow]` (edge-case) | low | True; `forbid` is not. Folded into #5's fix. | patch |
| 7 | `hexput-rpc` doc comment describes a backwards dependency direction (blind-hunter) | medium | Verified: `hexput-rpc` deps = `{hexput-port}`; `hexput-exec` depends on `hexput-rpc`. The comment claimed rpc "reaches enforcement through hexput-exec", which is impossible and invites a cycle-creating "fix". | patch |
| 8 | CI lacks `permissions` and `concurrency` (blind-hunter) | low | True; standard hardening, additive. | patch |
| 9 | `RUSTFLAGS: -D warnings` redundant and overrides future `.cargo/config.toml` (blind-hunter) | low | True — env RUSTFLAGS wins over config, and clippy already passes `-D warnings`. Direct deletion. | patch |
| 10 | Binaries collect argv then discard it (blind-hunter) | low | Confirmed dead code; direct deletion. | patch |
| 11 | Empty `[dependencies]` + trailing blank lines in shared/grammar (blind-hunter, verification-gap) | low | Confirmed cosmetic; direct deletion. | patch |
| 12 | `hexput-globalvar` doc says `hexput-session` calls `teardown()`, but session has no edge to globalvar (blind-hunter) | medium | Verified real: `hexput-session` deps = `{hexput-port, hexput-shared}`. But the **Spine itself** has this hole — AD-4's rule names session as teardown's only caller while its graph omits `session -> globalvar`. Not introduced by this story; the doc comment transcribes AD-4 faithfully. | defer |
| 13 | CI is Linux-only; `named_pipe.rs` will never compile (blind-hunter) | medium | True, but the transport crate is an empty stub today — nothing to cross-compile yet. | defer |
| 14 | Workspace pins carry no feature sets (blind-hunter) | low | True; but no crate consumes them yet, so choosing features now would be guesswork that later stories would have to undo. | defer |
| 15 | `SystemConfig::resolve()` takes no argv, so AD-7's CLI-flag precedence has no channel (edge-case) | low | True of the stub. The real signature belongs to the story that implements AD-7 discovery. | defer |
| 16 | AGENTS.md/CLAUDE.md Project Status is stale ("no `Cargo.toml`, no `src/`") (blind-hunter) | medium | Verified stale and self-contradicting as of this commit. Routed by rule: entries whose fix edits agent-context files defer. | defer |
| 17 | `.gitignore` missing `target/` (edge-case) | false | Refuted: `.gitignore:4` already contains `target/`. | rejected |
| 18 | Bin target `hexput-daemon` collides with package `hexput-daemon` (edge-case, blind-hunter) | false | Refuted: all three binaries build and resolve; `cargo run -p hexput-daemon` failing is *correct* for a lib-only package, not a collision. Cargo namespaces bin targets per package, and the Spine mandates these names. | rejected |
| 19 | CI may run the runner's default toolchain, not the pin (edge-case) | false | Refuted: `rustup show active-toolchain` reports `1.98.1 (overridden by rust-toolchain.toml)` and auto-installs when absent — observed on this machine's first build. | rejected |
| 20 | `todo!()` entry points return `()`, freezing a no-error signature (blind-hunter) | low | Claimed consequence overstated: "touches every binary" is 3 files of ~8 lines, and the story that implements each `run()` defines its real signature then. Fix adds public surface for no present benefit. | rejected |
| 21 | `criterion` pinned with no `benches/` (blind-hunter) | low | Premature — benchmarks are Epic 4's stories (cache win, rhai comparison). | rejected |
| 22 | Empty `[lib]` sections are no-ops (blind-hunter) | false | Refuted: the AC explicitly requires each crate "declares `[lib]` and no `[[bin]]`". Intentional. | rejected |
| 23 | `hexput-bin` has no `lib.rs`, so the AC's per-crate doc audit fails (edge-case) | low | By design — it is the only bin-producing crate and has no lib target. Its responsibility/AD statement lives in the three binary doc comments, satisfying the intent. | rejected |
| 24 | `rust-toolchain.toml` does not contain `edition = "2024"` (edge-case) | false | Refuted: `edition` is not a `rust-toolchain.toml` field; it lives in `[workspace.package]`, where it is set. The spec's wording was loose, and a finding whose fix edits this build's spec is rejected by rule. | rejected |
| 25 | CI skips pushes to branches other than main/v2 (edge-case) | low | The `pull_request` trigger covers the review path; running every feature-branch push burns minutes for no added signal. | rejected |

## Verification

**Commands:** (these are exactly the CI steps, in order)
- `cargo fmt --all --check` -- expected: no diff
- `cargo clippy --workspace --all-targets --locked -- -D warnings` -- expected: clean
- `python3 scripts/check-crate-graph.py` -- expected: `Crate graph OK — 9 Architecture Decision edges asserted.`
- `cargo build --workspace --all-targets --locked` -- expected: all 22 crates compile with no errors
- `cargo test --workspace --locked` -- expected: 45 test targets report ok (stub crates have no tests yet, so this confirms linking, not behavior)

**Boundary probes** (each must fail, then be reverted):
- add `hexput-interpreter` to `crates/hexput-check/Cargo.toml` -- expected: `check-crate-graph.py` fails with the AD-8 violation. Note `cargo build` does **not** fail on a manifest-only edge while the crates are stubs — that is precisely why the graph check exists.
- add `hexput-enforce` to any crate other than `hexput-exec` -- expected: `check-crate-graph.py` fails with the AD-3 sole-dependent violation.
- add an undocumented `unsafe` block to any crate -- expected: clippy fails with "unsafe block missing a safety comment".
- add `#[allow(clippy::undocumented_unsafe_blocks)]` in `hexput-lexer`/`parser`/`interpreter`/`exec` -- expected: `E0453 ... overruled by previous forbid`.
