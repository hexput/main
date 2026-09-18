# Deferred Work

Append-only. Each entry is work identified during a build but deliberately not done in it.

- source_spec: `spec-1-1-project-scaffold-and-pinned-toolchain.md`
  status: RESOLVED 2026-09-18 — the Spine's graph gained `hexput-session --> hexput-globalvar`, the edge was added to `crates/hexput-session/Cargo.toml`, and `scripts/check-crate-graph.py` now asserts it as a REQUIRED_EDGE so it cannot silently disappear again. Resolved in favour of AD-4's normative text (session is the sole caller) over the derived graph, on the reasoning that the graph was mechanically derived later during the crate split and simply dropped the edge. If the intended mechanism was instead a store handle passed down from `hexput-daemon`, this is the decision to revisit.
  summary: The Architecture Spine's crate graph omits `hexput-session -> hexput-globalvar`, contradicting AD-4's rule that `hexput-session` is the only caller of `hexput-globalvar::teardown(plugin_id)`.
  evidence: AD-4's text names session's TTL-expiry and explicit-unregister paths as teardown's only callers, invoked before the Plugin actor drops. But the Spine's graph gives `hexput-session` edges to `hexput-port` and `hexput-shared` only, so it cannot call into globalvar at all; the only crate that can is `hexput-plugin` — exactly the caller AD-4 forbids. Story 1.1 transcribed AD-4 faithfully into `hexput-globalvar`'s doc comment, so the contradiction is now stated in code. Resolve before Epic 7 (Global Variables) by either adding the `session -> globalvar` edge to the Spine, or amending AD-4 to describe the mechanism that actually reaches teardown (e.g. a store handle passed down from `hexput-daemon`).

- source_spec: `spec-1-1-project-scaffold-and-pinned-toolchain.md`
  summary: CI runs on `ubuntu-latest` only, so the Windows-only Named Pipe transport adapter will never be compiled.
  evidence: AD-1 requires a Named Pipe adapter alongside UDS/TCP+TLS/WebSocket, and AD-7 implies per-OS System Config default paths. Neither is exercised by a Linux-only job. `hexput-transport` is an empty stub today so there is nothing to cross-compile yet; add a `strategy.matrix.os` when Epic 2 lands the adapters.

- source_spec: `spec-1-1-project-scaffold-and-pinned-toolchain.md`
  summary: `[workspace.dependencies]` pins bare versions with no feature sets, and omits `tracing-subscriber` which FR-12's structured logging will need.
  evidence: `tokio = "1.53.1"` with default features has no `rt-multi-thread`/`net`/`sync`/`macros`/`time`; `serde` lacks `derive`; `rustls` lacks a crypto provider. Hoisting versions only pays off if features are hoisted too, otherwise members re-add them locally and diverge. Deliberately not decided in Story 1.1: no crate consumes any of the nine yet, so the correct feature set per crate is not yet knowable. Decide when the first consumer lands (Epic 2).

- source_spec: `spec-1-1-project-scaffold-and-pinned-toolchain.md`
  summary: `SystemConfig::resolve()` takes no arguments, so AD-7's "CLI flag > env var > default path" precedence has no channel for the CLI flag.
  evidence: The stub exists only so the `hexput-daemon` binary's call chain type-checks. Implementing AD-7 requires `resolve()` to accept the parsed `--config` flag and to report parse failure, e.g. `resolve(cli_flag: Option<&Path>) -> Result<Self, ConfigError>`. Settle the signature in the story that implements System Config discovery.

- source_spec: `spec-1-1-project-scaffold-and-pinned-toolchain.md`
  summary: AGENTS.md / CLAUDE.md "Project Status" still says the repo is pre-implementation with no `Cargo.toml` and no build commands, which is false as of this commit.
  evidence: That section explicitly instructs: "Once code exists, replace this whole section with real status ... and add build/lint/test commands to a new section below." This story created `Cargo.toml`, 22 crates, a CI workflow, and a crate-graph guard. The now-real commands are `cargo build --workspace --locked`, `cargo test --workspace --locked`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `python3 scripts/check-crate-graph.py`. Deferred only because build routes fixes that edit agent-context files to deferred work; this should be the next thing done.

- source_spec: `spec-1-2-tokenize-hexput-source.md`
  summary: The lexer materializes the entire source as `Vec<(usize, char)>` — roughly 16 bytes per character, about 16x the source size for ASCII.
  evidence: Chosen so arbitrary lookahead is total, but actual lookahead is bounded at 3 (`peek_at(1 + sign_width)` in `lex_number`). A `Peekable<CharIndices>` with a small buffer, or byte indexing since every lookahead target is ASCII, gets the same result. Deferred because rewriting the cursor touches every scanner and the story was already patched substantially; revisit when Epic 3 lands Resource Budget enforcement, which is also what should bound submitted script size (AD-3 puts that in `hexput-enforce`, not here).

- source_spec: `spec-1-2-tokenize-hexput-source.md`
  summary: `Code` wraps `&'static str` and no diagnostics type derives serde, but the module declares these as `hexput-port`'s error-response types over MessagePack.
  evidence: A `&'static str` newtype has no inbound representation, so a round-tripped or Backend-supplied code cannot be deserialized, and `hexput-shared` has no serde dependency at all. Settling this before `hexput-port` exists (it is still a stub) avoids forking a parallel wire type there. Likely shape: `Cow<'static, str>` or an interned form, plus feature-gated `Serialize`/`Deserialize`.

- source_spec: `spec-1-2-tokenize-hexput-source.md`
  summary: A numeric literal that underflows, e.g. `1e-999`, silently becomes `0.0` while one that overflows is rejected.
  evidence: LANGUAGE-REFERENCE §3 rejects infinity ("a rules engine that returns NaN has failed, not computed") but says nothing about underflow, so the lexer rejects one end of the range and not the other. Every mainstream language underflows silently, which is why this was not changed unilaterally — it is a language decision for §3, not a lexer bug. Decide whether §3 should name underflow, then make the lexer match.
