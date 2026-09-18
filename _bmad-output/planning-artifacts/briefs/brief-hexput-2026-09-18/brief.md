---
title: Hexput v2 Product Brief
status: final
created: 2026-09-18
updated: 2026-09-18
note: OS-level-sandboxing framing corrected from "decide later" to "permanently out" on 2026-09-18, to stay consistent with the PRD; async/non-blocking requirement added same day (see prd-hexput-2026-09-18/.memlog.md).
---

# Hexput v2 Product Brief

## Who this is for

This brief orients anyone joining Hexput v2 as a contributor, PR reviewer, or AI coding agent — it is engineering-facing, not a pitch deck. v1 is not a reference: it was written early in the author's experience, has no real users, and v2 is a from-scratch rewrite that owes it nothing.

## Problem

Systems that want to let users (not just the people with server access) supply their own logic — a plugin ecosystem, or a business rule a non-engineer needs to express (for example, "how does a student pass or fail this course") — have no good place to put that logic. The options today are all compromises:

- **Embed a general scripting language** (Lua, Rhai, JS via QuickJS/Boa) directly in the host process. Fast, but the host must re-embed and re-harden it per application, per language, and isolation between mutually-untrusted scripts is the host's problem.
- **WASM sandboxes** (Wasmtime, Extism). Strong memory isolation, but compilation/instantiation overhead and toolchain complexity work against the "frequently re-evaluated, short-lived logic" case (a rule checked on every request).
- **Policy engines** (OPA/Rego). Good for declarative authorization-style rules, not expressive enough for general algorithmic logic.

None of these are centralized, language-agnostic, or built around the specific shape of the problem: *short, frequently-invoked, user-authored logic that must call back into the host through an explicit, auditable boundary.*

## Solution

Hexput v2 is a **standalone scripting runtime service** — positioned like Redis, not like an embedded library. It runs as its own process (systemd service or Docker container); applications talk to it over a socket rather than linking it in.

**Core mechanics:**

- **AST caching.** User scripts are parsed/compiled once (`CodeRegister`) and re-executed many times with fresh variables (`CachedExecutionStart`), so steady-state execution is a cache lookup plus interpretation — not a parse.
- **Capability-mediated bidirectional RPC.** Scripts cannot touch the host, filesystem, or network directly. The host application explicitly registers every host-side function a script may call; register and handling are separate steps, so the host can grant access via multiple mechanisms (for example, `context.allow()` at registration time, or a `return true` guard evaluated per call).
- **Multi-dimensional resource budgeting.** Safety is not one loop-count limit — it is a budget spanning CPU time, memory, allocation count, RPC call count, output size, and side-effect count, enforced by the language runtime itself rather than an OS/WASM sandbox layer.
- **Language-agnostic access.** Because the boundary is a socket protocol, any backend language (Rust, Python, Node.js, etc.) can register functions and drive executions; v2 ships thin client SDKs (not the runtime) for common languages.
- **Two connection modes.** A client can open a fresh connection, or reconnect using a client ID issued on first connect. IDs are not 1:1 with connections — multiple backend instances may share one ID. This explicitly lays the groundwork for a future master-slave mode, where the standard Hexput daemon can run in "slave" mode, executing work routed to it by a master — the mechanism for horizontal scaling beyond a single node.
- **Fully asynchronous, non-blocking.** Message handling and execution requests are processed asynchronously throughout; a slow or long-running execution on one connection does not block message processing or other execution requests, whether they arrive on the same connection or a different one.
- **Two registration modes.** Alongside stateless Direct/Cached Execution (where each run starts clean, no state carried between calls), a Backend can register a **Plugin**: source with a `plugin { name = ..., ... }` metadata block, persistent top-level global variables shared across calls, and handler functions bound to Backend-defined events via `@Event(<name>)`. This is what makes "give your users a plugin ecosystem" (§Problem) concrete — a Plugin is a stateful, long-lived unit the daemon keeps alive for the life of the Backend's connection, not a one-shot script run. Event names and return shapes are declared upfront at registration; handler ordering across multiple handlers on one event is controllable via `priority` (backend-gated) or bypassed entirely with `async = true`. See the addendum for the full example.

## Why not existing solutions

Hexput's bet is that the *combination* is the differentiator, not any single piece: Rhai has embedding but no socket-native multi-tenant service model or capability-mediated RPC. WASM sandboxes have strong isolation but higher latency and no AST-cache-centric hot path. OPA has policy evaluation but not general expressiveness. None combine language-independence, a centralized service model, and RPC-based host access in one system. Rhai is treated as the closest performance comparator and the primary benchmark target (see addendum for the full evaluation plan).

## Deployment model

- **Local backends** connect over Unix Domain Socket (or Named Pipe on Windows) for lowest latency.
- **Remote backends** connect over TCP/TLS or WebSocket.
- A single Hexput daemon can serve multiple independent, mutually-untrusted backend applications concurrently. Isolation between them relies on the language runtime's own discipline (resource budgets, no ambient host access), not OS-level sandboxing (seccomp/cgroups/namespaces) — that's a permanent design choice, not a gap to close later (see Risks).

## Target users

- **Primary:** backend engineers embedding user-programmable logic into their own product (plugin ecosystems, configurable business rules) who install and run Hexput as infrastructure, the way they'd run Redis or Postgres.
- **Secondary (indirect):** the non-engineer authors who end up writing the actual Hexput scripts inside whatever product the primary user builds (for example, a university admin encoding pass/fail rules).

## Success criteria

Success is **scalable stability**, not feature breadth: execution stays reliable and fast under load as concurrent contexts and script complexity grow, with AST caching and resource budgeting as the load-bearing techniques. A public benchmark against Rhai (and other comparators — see addendum) is planned to substantiate performance claims rather than assert them.

## Scope

**In scope for v2:**
- Standalone daemon (systemd/Docker), WebSocket + Unix Domain Socket + Named Pipe transports
- Fully asynchronous, non-blocking message handling and execution
- AST parse/cache + cached execution protocol (stateless mode)
- Plugin registration + backend-defined events (stateful mode)
- Capability-based bidirectional RPC (register/handling separation, `context.allow()` / per-call `return true` patterns)
- Multi-dimensional resource budgeting (CPU, memory, allocations, RPC calls, output size, side effects)
- Client ID-based reconnection
- MessagePack as the wire protocol (replacing v1's JSON)

**Explicitly out of scope / future work:**
- Master-slave horizontal scaling (slave daemons executing work routed by a master)
- Per-function rollback registration (compensating actions for a failed/over-budget script) — no rollback exists in the v2 baseline; RPC side effects are not transactional

**Explicitly and permanently out of scope (not future work):**
- OS-level sandboxing (seccomp, cgroups, namespaces) as defense-in-depth beyond language-level discipline. This isn't deferred — it's a standing design decision: the project's whole premise is that the language provides sandboxing *by construction*, and an OS-level layer works against the millions-of-requests/second target this needs to hit.

## Risks & open questions

- **Trust boundary is the language runtime, not the OS — permanently, by design.** A memory-safety or logic bug in Hexput's own VM is a bug shared by every tenant on that daemon; there is no process/OS isolation backstop, and there won't be one, because OS-level sandboxing conflicts with the millions-of-requests/second performance target. This is an accepted tradeoff, not a gap to close later (see addendum for hardening ideas that stay compatible with that stance — auditing, rate limiting, fuzzing — worth evaluating even though OS-level isolation itself is off the table).
- **No rollback for host-side effects.** If a script is killed mid-execution after already triggering RPC calls, those effects stand. Errors originating in the host's own registered functions are explicitly not Hexput's problem to solve.
- **Benchmark methodology is not yet finalized** — comparators and metrics are proposed, not validated (see addendum).
- **No committed timeline.** This is thesis-driven work without an external deadline, which is fine for depth but means scope needs active pruning to stay shippable.

## Next steps

1. Turn "Scope" above into an architecture doc / PRD for the v2 rewrite (transport layer, RPC registry, budgeting engine, wire protocol).
2. Stand up the benchmark harness (Rhai, Wasmtime/Extism, OPA comparators) early — it will pressure-test the AST-cache/budgeting design decisions, not just validate them after the fact.
3. Evaluate the addendum's OS-level-*compatible* hardening ideas (rate limiting, RPC audit logging, fuzzing, `unsafe` audit gate) before any untrusted multi-tenant deployment — OS-level sandboxing itself is not on the table (see Risks), so hardening has to come from within that constraint.
