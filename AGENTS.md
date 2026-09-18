# AGENTS.md

This file provides guidance to AI coding agents (Claude Code and others) working with code in this repository. `CLAUDE.md` in this same directory is a symlink to this file — edit this file, not that one.

**Keep this file current.** As the project moves past planning and into implementation, update the Project Status section (and anything else below that's gone stale) to reflect what's actually true — don't let this drift into a description of a project that no longer exists.

## Project Status

_Last updated: 2026-09-18._

Pre-implementation. There is no `Cargo.toml`, no `src/`, and no build/test commands yet — planning is finished, code hasn't started. This is a from-scratch v2 rewrite; there is no v1 code in this repository to reference or ratify conventions from.

Planning is complete and final:
- [Product brief](_bmad-output/planning-artifacts/briefs/brief-hexput-2026-09-18/brief.md) (+ [addendum](_bmad-output/planning-artifacts/briefs/brief-hexput-2026-09-18/addendum.md)) — problem, solution, differentiation, deployment model, risks.
- [PRD](_bmad-output/planning-artifacts/prds/prd-hexput-2026-09-18/prd.md) (+ [addendum](_bmad-output/planning-artifacts/prds/prd-hexput-2026-09-18/addendum.md)) — FR-1 through FR-25, MVP scope, success metrics. FR-N ids are stable references; the PRD's `.memlog.md` in the same folder is the audit trail of every decision behind them.
- [Architecture spine](_bmad-output/planning-artifacts/architecture/architecture-hexput-2026-09-18/ARCHITECTURE-SPINE.md) — the paradigm, module boundaries, and AD-1 through AD-7 invariants below are distilled from it. Read the full spine before implementing anything it governs; this file only orients.

- [Epic breakdown](_bmad-output/planning-artifacts/epics.md) — 9 epics, 74 stories, every FR-1…FR-26 covered by Given/When/Then acceptance criteria. Also records the resolutions for PRD open questions OQ-1 (health/metrics are RPC messages, no HTTP listener), OQ-2 (reconnect secret mechanism), OQ-3 (the closed set of language feature toggles), and OQ-11 (handler ordering conventions) — treat those as settled, not open.
- [Language reference](_bmad-output/planning-artifacts/language/LANGUAGE-REFERENCE.md) — **normative definition of the Hexput language**, written during sprint planning because no earlier artifact specified it. Epic 1 implements it, Epic 9's grammar and LSP describe it, Epic 6 extends it. Where a story and this document disagree, the document wins. Its `[DECISION]` markers are approved provenance, not open questions.
- [Sprint status](_bmad-output/implementation-artifacts/sprint-status.yaml) — the tracking file; regenerate with `bmad-sprint-planning` whenever the epics change.

Two amendments landed after the PRD and spine were first marked final, both recorded in their `.memlog.md` files: **FR-26** (optional, Backend-configured static check before execution) and **AD-8** with the twelfth module, `check/`.

**Next step:** implement Epic 1 Story 1.1 — `Cargo.toml` plus the `src/` module skeleton from the Spine's Structural Seed — via `bmad-build`. Once code exists, replace this whole section with real status (what's built, what's in flight, what commands actually work) and add build/lint/test commands to a new section below.

## What Hexput Is

A standalone scripting runtime daemon (Rust) — installed and run like Redis, not embedded like a library. Backend applications connect over a socket (Unix Domain Socket, Named Pipe, TCP+TLS, or WebSocket) and either run stateless scripts (one-shot or cached execution) or register stateful **Plugins** (persistent, event-driven, capability-sandboxed code units). No OS-level sandboxing — the language itself is the trust boundary, permanently, by design (see the brief's Risks section for why).

## Architecture (from the spine — treat the spine as authoritative, this is a summary)

**Paradigm:** Hexagonal (Ports & Adapters) at the transport boundary, wrapping an Actor-model runtime core.

```text
transport/   # UDS, Named Pipe, TCP+TLS, WebSocket adapters — one internal Port interface
port/        # wire envelope (MessagePack), request/response + event correlation
session/     # Client ID -> Session, TTL (from System Config), reconnect + credential validation
connection/  # transient per-connection actor, attached to a Session (0..N per Session)
script/      # Direct/Cached Execution: AST cache, parse/interpret
plugin/      # Plugin actor: registration, event routing/ordering metadata
globalvar/   # Global Variable store: concurrent per-Plugin map, independent of the Plugin actor's mailbox
exec/        # the one shared Executor every execution mode funnels through
enforce/     # Capability checks + Resource Budget enforcement — single implementation
rpc/         # host-function registry, capability grants, outbound RPC to Backend
config/      # System Config (file) parsing — separate from in-protocol per-backend Config
```

**Invariants that bind implementation** (full rationale in the spine; these are the ones a change is most likely to violate):

- Every Transport is an adapter into one `Port`; Core never branches on transport type (AD-1). Health/metrics ride the same `Port`, pre-init-gate.
- A Session may have zero or more concurrently attached Connections — never exactly one; no cross-Connection broadcast, each request/response stays with the Connection that issued it (AD-2).
- Direct Execution, Cached Execution, and Plugin Event handlers all call one `Executor` entry point — no path checks capability or consumes budget independently, including for the full duration of an `async = true` handler (AD-3).
- A Plugin's Global Variable store lives independently of its actor's mailbox; `session/` is the only caller of its `teardown()`, invoked before the actor drops, never implied by `Drop` (AD-4).
- `session/` holds the single live copy of per-backend Config; nothing else snapshots it (AD-5).
- Every execution dispatches as an independent async task; the only permitted serialization is a Plugin's own opted-in `priority` ordering among its own handlers for one Event (AD-6).
- System Config discovery (CLI flag > env var > default path) is identical regardless of packaging (AD-7).
- No lock is ever held across an `.await` point.

**Domain vocabulary** (use these terms verbatim in code — see PRD §3 Glossary for full definitions): `Daemon`, `Backend`, `Session`, `Connection`, `Client ID`, `Config` (per-backend, never a file) vs. `System Config` (file-based, daemon-operational), `Registered Function`, `Capability`, `Script`, `Direct Execution`, `Cached Execution`, `AST Cache`, `Resource Budget`, `Plugin`, `Event`, `Global Variable`, `Global Variable Locking` (concurrency strategy) vs. `Global Variable Behavior` (lifetime strategy: `forever`/`ttl`/`separate_each_trigger`/`keyed`).

**Stack** (pinned versions in the spine's Stack table — verify current before bumping): Rust 1.98.1 (2024 edition), Tokio, `tokio-tungstenite`, `rustls`, `serde`/`rmp-serde` (MessagePack wire format), `dashmap`, `moka` (AST cache), `criterion` (benchmarking), `tracing`.

**Deliberately out of scope** — don't reintroduce without checking the brief/PRD first: OS-level sandboxing (permanent, not deferred), transactional/rollback RPC effects, master-slave horizontal scaling (future work, not precluded by the design).

## Repo Layout Notes

- `.claude/skills/` and `.agents/skills/` — every entry under `.claude/skills/` is a symlink into `.agents/skills/`; that's the real location. Don't duplicate a skill in both places.
- `_bmad/` — BMAD methodology tooling (scripts, skill customization). `_bmad-output/planning-artifacts/` — where the brief, PRD, and architecture spine above live, each with its own `.memlog.md` decision log.
