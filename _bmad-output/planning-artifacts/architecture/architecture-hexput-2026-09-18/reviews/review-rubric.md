# Rubric Walker Review — ARCHITECTURE-SPINE.md (Hexput v2)

**Verdict:** Not yet gate-clean — the spine is structurally strong on the runtime core (AD-1…AD-6 correctly locate and enforce the real divergence points for Transport/Session/Executor/GlobalVar/dispatch) but silently skips the operational/environmental envelope the initiative altitude owns, leaves one PRD-flagged divergence risk (FR-13 reconnect-protection mechanism) neither decided nor deferred, and carries at least two Stack versions that don't match currently-published crate versions.

## Critical

### C-1: Deployment & environments / infra strategy / operations dimension is entirely unaddressed
The PRD's MVP scope (§6.1) explicitly commits to "Standalone daemon (systemd/Docker)". This is a dimension the initiative-altitude spine owns per the checklist ("especially the operational/environmental envelope: deployment & environments, infra/provider strategy, operations"). The spine's only brush with this is one Deferred line about TLS cert reload being "operational, not structural." There is no AD, no Deferred entry, and no open question about:
- systemd-unit vs. Docker-image as the packaging/distribution artifact (or both, and how they share System Config discovery)
- environment strategy (dev/staging/prod config profiles, if any)
- process-supervision/restart policy, upgrade/rollback story
This isn't a "low-risk, defer it" omission — it's a whole dimension left silent, which the checklist calls a finding in its own right, not something to wave through as out-of-scope-by-implication.

**Fix:** Add either an AD (if systemd/Docker packaging has structural consequences, e.g. how `config/` locates the System Config file per packaging mode) or an explicit Deferred entry with rationale ("packaging is deployment-time, not code-structural; no AD needed") — the same treatment already given to TLS reload and benchmark harness structure. Right now it's neither.

## High

### H-1: FR-13 reconnect-protection mechanism is a real divergence risk left neither decided nor deferred
PRD FR-13 requires reconnect protection ("secret issued alongside the Client ID, or similar") applied *uniformly across all four Transports*, and the PRD itself flags this as unresolved (OQ-2). The spine's AD-2 lists FR-13 under **Binds**, which reads as "this AD covers FR-13," but AD-2's Rule only addresses Session/Connection ownership — it says nothing about what the reconnect credential is or how it's validated identically across UDS/Named Pipe/TCP+TLS/WebSocket adapters. Unlike the other PRD-flagged ambiguities (priority tie-break, `async`+`priority` precedence — both explicitly carried into Deferred with a citation to OQ-11), this one has no Deferred entry at all. Two adapters built independently could reasonably implement the credential check differently (e.g. constant-time compare only on one path), which is exactly the kind of "two independently-built units diverging" the checklist is designed to catch.

**Fix:** Either add a minimal AD ("the reconnect credential is validated once in `session/` before an adapter's request reaches Core" — likely already true given AD-1's Port funnel, but currently unstated) or add a Deferred line explicitly citing OQ-2, parallel to the OQ-11 entry already present.

### H-2: FR-11 health/metrics surface has no structural placement and an unaddressed interaction with AD-1
FR-11 requires health/metrics reachable "without requiring an authenticated Backend connection" — i.e., a path that does *not* go through the normal init-handshake-gated flow. AD-1's Rule says every Transport is an adapter into one internal `Port`, and Core never branches per-transport. Does the unauthenticated health check ride the same `Port` (with an exemption from the init-gate), or is it a structurally separate path (e.g., its own listener)? The Structural Seed has no `health/`, `metrics/`, or equivalent module, and the Capability→Architecture Map merely says "cross-cutting via `tracing`" (which covers FR-12 logging, not FR-11's health/metrics surface). This is left to two possible independent implementations to diverge on.

**Fix:** One sentence is enough — either fold it into AD-1's Rule ("health/metrics is served through the same `Port`, pre-init-gate") or add a Deferred/open-question entry naming this as unresolved (parallel to OQ-1, which the PRD itself flags as open).

## Medium

### M-1: Stack versions don't match currently-published crate versions (fails "verified-current")
Web search against crates.io/docs.rs as of the stated date (2026-09-18) found:
- `moka` latest published version ≈ 0.12.10 — spine pins **0.12.16**, which is ahead of what's currently published.
- `rustls` latest stable ≈ 0.23.43 (2026-07-29) — spine pins **0.23.45**, also ahead.
`tokio` 1.53.1 was corroborated as plausible. The two mismatches suggest these version numbers weren't checked against the actual registry and may be fabricated/guessed rather than verified-current, which the checklist calls out explicitly. Given this is a real, buildable Rust project, a wrong pin here isn't cosmetic — a `cargo add moka@0.12.16` will simply fail.

**Fix:** Re-verify every Stack row against crates.io at write time; pin to the newest *actually-published* version, not a plausible-looking one.

## Low

### L-1: Rust stack entry breaks the table's own pinning convention
Every other Stack row pins an exact version; `Rust | current stable (2024 edition)` doesn't. Given the project is thesis-driven and benchmarked, a floating "current stable" is a minor but real reproducibility gap (a "current stable" at day 1 vs. day 200 of a solo, timeline-less project (OQ-6) could be two different compilers). Low severity since edition + "stable" is enough to build, but inconsistent with the rest of the table.

## What the spine gets right (not findings, for calibration)
- AD-1…AD-6 each name a real cross-unit divergence risk (transport leakage, session/connection coupling, budget-check duplication, mailbox-vs-async-write races, config-surface conflation, head-of-line blocking) and each Rule is concrete enough to enforce in code review.
- Deferred section correctly distinguishes "no AD needed because it's implementation detail" (wire schema, keyed-storage layout) from "no AD needed because it's out of this spine's scope" (tree-sitter/LSP, master-slave scaling, OS sandboxing) — good discipline, and explicitly cites the PRD/brief sections backing each exclusion.
- FR coverage is otherwise complete: every FR in the `binds` frontmatter maps to at least one AD or the Capability→Architecture Map; FR-14/FR-15/FR-10 are correctly and explicitly out-of-spine rather than silently dropped.
