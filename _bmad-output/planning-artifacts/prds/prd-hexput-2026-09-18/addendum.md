# Hexput v2 PRD — Addendum

Implementation-level detail that doesn't belong in the PRD body per PRD discipline (capabilities, not implementation). See also the [product brief addendum](../../briefs/brief-hexput-2026-09-18/addendum.md) for the thesis research framing and full benchmark methodology — not repeated here.

## Security hardening backlog (reference only)

The [brief's addendum](../../briefs/brief-hexput-2026-09-18/addendum.md#additional-security-hardening-ideas-beyond-language-level-discipline) lists hardening ideas that stay compatible with Hexput's permanent no-OS-sandboxing stance: per-Client-ID rate limiting, broader RPC call auditing (every host-function call attributable/replayable, not just denials/violations), fuzzing the parser/VM in CI, and an `unsafe`-audit release gate. None of these are v2 PRD commitments (no FR/NFR here) — they remain a backlog to evaluate before any untrusted multi-tenant production use, per the brief's Next Steps. Revisit this list, not re-derive it, when that evaluation happens.

## Tech stack

- **Runtime:** Rust
- **Async runtime:** Tokio
- **Local IPC:** Unix Domain Socket (Linux/macOS), Named Pipe (Windows)
- **Remote transport:** TCP + TLS, or WebSocket where needed, via `tokio-tungstenite`
- **Wire protocol/serialization:** MessagePack via `rmp-serde` + `serde`
- **Script execution:** custom restricted interpreter (Rhai-like starting point), AST/bytecode precompiled and cached
- **Cache:** `moka` or `DashMap`-backed in-memory cache
- **RPC model:** bidirectional request/response protocol, capability-based function registration
- **Service deployment:** systemd or Docker
- **Benchmarking:** Criterion + `tracing`/metrics ecosystem

## Config handshake (implementation notes)

- Config travels inline in the connection init message on a fresh connection (FR-1); it is *not* re-sent on reconnect (FR-2) — the daemon must persist it keyed by Client ID across disconnects.
- Runtime config edits (FR-3) need a defined message type distinct from init; per-execution overrides need a way to scope override values to a single execution request without mutating stored state. Exact schema is an open question (PRD OQ-4).

## Observability surface (PM-authored, open for revision)

Proposed shape for FR-11/FR-12, pending confirmation (PRD OQ-1):
- Health: a lightweight RPC message type (`HealthCheck`) usable without a connection having completed the full init handshake, rather than a separate HTTP endpoint — keeps operational surface inside the existing transport/protocol instead of adding a fourth listener type.
- Metrics: expose via the `tracing`/`metrics` crate ecosystem already in the stack, scrapable in Prometheus-compatible format; per-budget-dimension counters (CPU/memory/allocation/RPC-count/output-size/side-effect violations tracked separately, matching FR-8's independent enforcement).
- Logging: structured (`tracing`, JSON-capable) with Client ID as a consistent span/field across connection lifecycle, capability-denial, and budget-violation events.

## Concurrency model (implementation notes)

FR-16 / §8 Concurrency: no execution request may block another. Natural fit for the existing stack — each Backend connection runs as its own Tokio task, and each execution request dispatches onto the async runtime rather than a per-connection serial queue, so a slow Cached Execution on connection A doesn't hold up a fast Direct Execution on connection A or B. The Resource Budget's CPU-time dimension (§4.4) is the backstop against a single execution starving the runtime — without it, a pathological script could still degrade concurrency in practice even though nothing is blocking in the strict sense.

## Plugin Global Variable locking (implementation notes)

FR-20: the default per-top-level-key locking is naturally a `DashMap`-like structure (already in the stack for the AST Cache, per Tech stack above) — one shard/lock per top-level key of a Global Variable rather than one lock for the whole variable, so concurrent Event handlers touching different keys don't contend. The Backend-configurable unsafe/lock-free mode is the same structure with locking compiled or switched out, not a separate code path, to avoid two implementations drifting apart. Per-variable, code-level override (when the Backend allows it) implies the strategy is a property attached to each Global Variable at declaration or first-use, not a single global daemon setting.

FR-22/FR-23: `priority`-ordered handlers most naturally run on a single-worker-per-Event-per-Plugin execution path (a small ordered queue), while `async = true` handlers dispatch straight onto the general Tokio task pool alongside everything else (§Concurrency model above) — consistent with why `async` defaults to unsafe/lock-free: there's no shared queue serializing them the way there is for prioritized handlers, so the safe (mutexed) locking strategy is the thing standing between them and a race, not queue ordering.

## Event declaration (implementation notes)

FR-19: declaring an Event's return shape at registration implies some schema representation travels in the Plugin-registration message — worth deciding early whether this reuses whatever shape-description mechanism the language already has (e.g. an object literal as a template) rather than inventing a second schema language just for this.

## SDK sequencing note

When Phase 2 starts (§4.6 FR-10), a shared protocol test suite — rather than five independently hand-maintained clients — keeps the SDKs from drifting out of sync with each other and with the daemon's protocol.

## Developer tooling notes (tree-sitter + LSP)

- Tree-sitter grammar is the more load-bearing of the two near-term — most editor integrations (syntax highlighting, folding, even some LSP features) can build directly on it.
- LSP scope for v2 is intentionally minimal: diagnostics + basic completion. A natural implementation path is a small custom server wrapping the existing parser/AST rather than a general-purpose language-tooling framework, given the language surface is still small.
