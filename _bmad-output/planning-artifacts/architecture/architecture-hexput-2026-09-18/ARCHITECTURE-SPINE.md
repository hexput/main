---
name: 'Hexput v2'
type: architecture-spine
purpose: build-substrate
altitude: initiative
paradigm: 'Hexagonal (Ports & Adapters) at the transport boundary, Actor model for the runtime core'
scope: 'Hexput v2 daemon — the whole system covered by the v2 PRD'
status: final
created: '2026-09-18'
updated: '2026-09-18'
binds: [FR-1, FR-2, FR-3, FR-4, FR-5, FR-6, FR-7, FR-8, FR-9, FR-11, FR-12, FR-13, FR-16, FR-17, FR-18, FR-19, FR-20, FR-21, FR-22, FR-23, FR-24, FR-25]
sources: ['_bmad-output/planning-artifacts/prds/prd-hexput-2026-09-18/prd.md', '_bmad-output/planning-artifacts/briefs/brief-hexput-2026-09-18/brief.md']
companions: []
---

# Architecture Spine — Hexput v2

## Design Paradigm

**Hexagonal (Ports & Adapters)** at the transport boundary, wrapping an **Actor model** runtime core.

- `transport/*` — one adapter per Transport (UDS, Named Pipe, TCP+TLS, WebSocket), each implementing a single internal `Port` interface. No transport-specific behavior crosses into the core.
- `session/`, `connection/`, `plugin/` — the actor layer: a `Session` (durable, Client-ID-keyed) owns a transient `Connection` actor and zero or more `Plugin` actors. Ownership here is lifecycle ownership, not a per-operation consistency boundary: `Session` creates and tears down `Plugin`s (AD-4's `teardown` contract), but a `Plugin` — together with its Global Variable store — is its own consistency boundary for event dispatch, since `session/` never mediates a single Event invocation.
- `exec/`, `enforce/` — the shared execution core every actor's work funnels through; this is where Capability and Resource Budget rules apply, once, for every mode.

## Invariants & Rules

```mermaid
graph TD
  subgraph Adapters
    UDS[UDS Adapter]
    NP[Named Pipe Adapter]
    TCP[TCP+TLS Adapter]
    WS[WebSocket Adapter]
  end
  subgraph Core
    Port[RPC Port]
    Session[Session Manager]
    Conn[Connection Actor]
    Plugin[Plugin Actor]
    GVStore[Global Variable Store]
    Executor[Shared Executor]
    Enforce[Capability + Budget Enforcement]
  end
  UDS --> Port
  NP --> Port
  TCP --> Port
  WS --> Port
  Port --> Session
  Session --> Conn
  Session --> Plugin
  Conn --> Executor
  Plugin --> Executor
  Plugin --> GVStore
  Executor --> Enforce
  Executor --> GVStore
```

Dependency direction: Adapters depend on the Core `Port`; the Core never imports or branches on a specific Adapter. Nothing depends back on Adapters.

### AD-1 — Transport-agnostic core [ADOPTED]

- **Binds:** FR-9, FR-11, all Transports
- **Prevents:** transport-specific behavior leaking into session/execution semantics; adapters and core drifting on per-transport protocol behavior; health/metrics (FR-11) becoming a fifth, structurally separate listener
- **Rule:** every Transport is an adapter implementing one internal `Port` interface; the Core module tree never imports or branches on a specific transport type. Health/metrics (FR-11) is served through that same `Port`, exempted from the init-handshake gate (FR-1) rather than given its own listener.

### AD-2 — Session outlives Connection, and may have several at once [ADOPTED]

- **Binds:** FR-1, FR-2, FR-3, FR-13, FR-21, FR-24
- **Prevents:** Plugin/Config/Registered-Function state being coupled to a single live socket, breaking reconnect (FR-2) and Plugin persistence (FR-21); a Session's connection cardinality being read differently by independently-built units
- **Rule:** `Session` is a distinct, owned entity keyed by Client ID, with a TTL sourced from System Config (AD-5). A Session may have zero or more concurrently attached `Connection` actors (FR-2) — never exactly one. Each `Connection` is attached to at most one `Session` at a time and holds no state that outlives it. Requests and their responses correlate to the specific `Connection` that issued them; there is no cross-Connection broadcast of results — Connections are isolated from each other even when they share a Session. The Session's TTL clock (FR-21, FR-24) only starts once the last attached Connection drops; while at least one Connection is attached, the Session is actively alive with no TTL countdown running. Session-owned state (Plugins, Config, Global Variables) is shared and consistent across every attached Connection. The reconnect credential (FR-13) is validated once, in `session/`, before a reconnect request is accepted — every Transport adapter forwards the credential unchanged through the Port; no adapter performs its own validation.

### AD-3 — One shared Executor enforces Capability + Resource Budget for every execution path, for its full duration [ADOPTED]

- **Binds:** FR-6, FR-7, FR-8, FR-19, FR-25
- **Prevents:** Direct Execution, Cached Execution, and Plugin Event handlers implementing capability/budget checks independently and drifting apart; async handler work (AD-6) silently escaping budget accounting once dispatch returns
- **Rule:** all three execution paths call into one `Executor` entry point; no path may reach a Registered Function or consume Resource Budget outside it. For any execution that continues past its dispatching call's return — every `async = true` Plugin handler (AD-6) — `Executor` hands the spawned task a live budget-accounting handle, not a closed one; every budget-relevant operation that task performs afterward, including Global Variable writes counted as side effects (AD-4), charges through that same handle. `globalvar/` never opens a second, independent path into `enforce/`. Global Variable access itself is intrinsic language state, not a Registered Function — exempt from FR-6/FR-7's capability-grant requirement, reached only through the direct `Executor`/`Plugin → GVStore` path (never through `rpc/`). Both of Plugin's dispatch paths (the priority sequencing task and directly-spawned `async` handlers, AD-4) invoke the same single handler-invocation function in `exec/` — capability checks, budget-handle setup, and Global Variable access all happen in that one shared function; the two paths differ only in *when* they're scheduled, never in *what* runs.

### AD-4 — Global Variable store is independent of the Plugin actor's mailbox [ADOPTED]

- **Binds:** FR-20, FR-22, FR-23, FR-25
- **Prevents:** `async` handlers (FR-23) being forced through a serialized mailbox (breaking their bypass-ordering requirement); `priority`-ordered handlers (FR-22) losing sequencing, or accidentally serializing *different* Events on the same Plugin; the store leaking on teardown; `keyed` variables (FR-25) serializing unrelated partitions
- **Rule:** a Plugin's Global Variables live in a shared concurrent per-Plugin store, independent of and reachable without the Plugin actor being alive. Locking is per-`(top-level-key, partition-key)` — for a non-`keyed` variable, partition-key is a constant, collapsing to plain per-top-level-key locking; for a `keyed` variable (FR-25), different Backend-supplied keys never contend for the same lock. A Global Variable's lock is never held across an `.await` point (e.g. a handler's mid-execution RPC call) — every read/write is a short, synchronous critical section against the store; a handler that needs the value across an await re-acquires it. The Backend-configurable "unsafe/lock-free" strategy (FR-20) is implemented entirely in safe Rust — no `unsafe` blocks — by using the store's finer-grained/optimistic access instead of the default per-key critical section; it trades logical race-safety (permits lost updates, stale reads) for latency, and is unrelated to the Security NFR's "no unsafe Rust in the parser/VM path" rule, which is about memory safety, not this concurrency-semantics choice. `globalvar/` exposes `teardown(plugin_id)` as its sole store-lifecycle entry point; `session/`'s TTL-expiry and explicit-unregister paths (AD-2, FR-21) are its only callers, invoked synchronously before the Plugin actor is dropped — teardown is never implied by actor `Drop`. The Plugin actor owns event routing and ordering *metadata* only (which handler runs at which priority) — it does not itself execute the ordered chain: `priority`-ordered dispatch for one Event is a per-(Plugin, Event) sequencing task spawned independently of the actor's mailbox, so an in-progress priority chain for one Event never blocks a different Event arriving concurrently for the same Plugin. Both that sequencing task and directly-spawned `async` handlers read/write the same Global Variable store.

### AD-5 — Per-backend Config and System Config are separate, non-overlapping surfaces [ADOPTED]

- **Binds:** FR-1, FR-3, FR-24
- **Prevents:** daemon operational settings (bind addresses, TLS paths, log level, Session TTL) being conflated with backend-supplied execution policy, or either surface silently absorbing the other's responsibility; a runtime Config update (FR-3) silently not reaching already-registered Plugins
- **Rule:** System Config (file-based) is the only source of the daemon's own operational settings. Per-backend Config never touches disk and never configures the daemon itself. `session/` holds the single mutable, live copy of a Session's per-backend Config; `plugin/`, `script/`, and `exec/` read through that copy on every dispatch — none of them snapshot or cache it at registration time — so an FR-3 runtime update is visible to Plugin Event dispatch exactly as it is to Direct/Cached Execution, with no separate propagation path to keep in sync.

### AD-6 — Non-blocking dispatch is structural, not incidental [ADOPTED]

- **Binds:** FR-16
- **Prevents:** a slow execution on one connection or Plugin stalling unrelated work — the head-of-line blocking FR-16 explicitly forbids
- **Rule:** every execution (Direct, Cached, Plugin handler) dispatches as an independent async task on the shared runtime. The only permitted serialization is a Plugin's own opted-in `priority` ordering (FR-22) among its own handlers for one Event — never across connections, Plugins, or Events (AD-4's per-(Plugin, Event) sequencing task is what makes this hold in practice).

### AD-7 — System Config discovery is one deployment-agnostic precedence, packaging doesn't change it [ADOPTED]

- **Binds:** FR-24
- **Prevents:** a systemd build and a Docker build independently inventing different config-discovery logic (different default paths, different override mechanisms) that silently diverge
- **Rule:** `config/` resolves the System Config file's location via one fixed precedence regardless of packaging — explicit CLI flag, then environment variable, then a fixed default OS path. systemd units and Docker images both just set the environment variable or bind-mount the default path; neither changes `config/`'s resolution logic.

## Consistency Conventions

| Concern | Convention |
| --- | --- |
| Naming (entities, files, interfaces, events) | Code identifiers match PRD §3 Glossary terms verbatim (`Session`, `Plugin`, `GlobalVariable`, `Capability`, ...) — no synonyms |
| Data & formats (ids, dates, error shapes, envelopes) | MessagePack (`rmp-serde`/`serde`) wire envelope for all Transports; Client ID and error shapes defined once in `port/`, reused by every adapter |
| State & cross-cutting (mutation, errors, logging, config, auth) | All state mutation with security/budget consequences flows through `exec`/`enforce` (AD-3); structured logging via `tracing`, every entry tagged with Client ID (PRD FR-12); config split per AD-5 |
| Locking discipline | No lock (Global Variable or otherwise) is ever held across an `.await` point — critical sections stay short and synchronous; re-acquire after suspension instead |

## Stack

| Name | Version |
| --- | --- |
| Rust | 1.98.1 (2024 edition) |
| tokio | 1.53.1 |
| tokio-tungstenite | 0.30.0 |
| rustls | 0.23.45 |
| serde | 1.0.229 |
| rmp-serde | 1.3.1 |
| dashmap | 6.2.1 |
| moka | 0.12.16 |
| criterion | 0.8.2 |
| tracing | 0.1.44 |

## Structural Seed

```text
src/
  transport/         # adapters: uds.rs, named_pipe.rs, tcp_tls.rs, websocket.rs — one Port impl each
  port/               # internal RPC port: wire envelope (MessagePack), request/response + event correlation
  session/            # Client ID -> Session, TTL (from System Config), reconnect + FR-13 protection
  connection/         # transient per-connection actor, attached to a Session
  script/             # Direct/Cached Execution: AST cache (moka), parse/interpret entry points
  plugin/             # Plugin actor: registration, event routing, priority/async dispatch (AD-4)
  globalvar/          # Global Variable store: per-Plugin concurrent map (dashmap), locking + behavior strategies
  exec/                # the one shared Executor all three modes funnel through (AD-3)
  enforce/             # Capability checks + Resource Budget enforcement — single implementation
  rpc/                 # host-function registry, capability grants, outbound RPC dispatch to Backend
  config/              # System Config (file) parsing — separate from in-protocol per-backend Config
```

```mermaid
erDiagram
  SESSION ||--o{ CONNECTION : "attached to (0..N, transient)"
  SESSION ||--o{ PLUGIN : owns
  SESSION ||--o| CONFIG : "has (per-backend, in-protocol)"
  SESSION ||--o{ REGISTERED_FUNCTION : has
  PLUGIN ||--o{ GLOBAL_VARIABLE : declares
  PLUGIN ||--o{ EVENT_HANDLER : registers
```

## Capability → Architecture Map

| Capability / Area | Lives in | Governed by |
| --- | --- | --- |
| Connection & Session lifecycle (FR-1, FR-2, FR-3, FR-13, FR-21, FR-24) | `session/`, `connection/`, `config/` | AD-2, AD-5 |
| Script execution (FR-4, FR-5, FR-16) | `script/`, `exec/` | AD-3, AD-6 |
| Capability-based RPC (FR-6, FR-7) | `rpc/`, `enforce/` | AD-3 |
| Resource Budgeting (FR-8) | `enforce/` | AD-3 |
| Transport layer (FR-9) | `transport/`, `port/` | AD-1 |
| Plugin Registration & Events (FR-17…FR-23, FR-25) | `plugin/`, `globalvar/`, `exec/` | AD-3, AD-4, AD-6 |
| Health/metrics (FR-11) | `port/` | AD-1 |
| Structured logging (FR-12) | cross-cutting via `tracing` | Consistency Conventions |

## Deferred

- Exact wire message schema/field layout inside `port/` — implementation detail once `rmp-serde`/`serde` types exist; not an invariant two independently-built units would diverge on given AD-1.
- TLS certificate reload mechanism (hot-reload vs. restart-required) — operational detail, not structural.
- Priority tie-breaking convention, default handler order absent `priority`, and `async`+`priority` combination precedence — already flagged as [ASSUMPTION]s in PRD OQ-11; low risk, revisit before `plugin/` ordering logic is implemented.
- Master-slave horizontal scaling — explicitly future work (PRD §6.2, brief Scope) — no AD here; `session/` and `plugin/` ownership models above are designed to not preclude it, not to implement it.
- OS-level sandboxing (seccomp/cgroups/namespaces) — permanently out per PRD/brief; not part of this spine's trust model.
- Tree-sitter grammar / LSP (FR-14, FR-15) — a separate deliverable/tool, not part of the daemon's own runtime architecture; no AD here.
- Client SDKs (FR-10) — external, per-language libraries that speak the `port/` wire protocol; not part of the daemon's own architecture, so out of this spine entirely, not merely unmentioned.
- Benchmark harness structure (PRD SM-1, brief addendum) — thesis-deliverable detail, not a system invariant.
- Exact `keyed` Global Variable storage layout (e.g. nested map keyed by Backend-supplied key) — implementation detail within `globalvar/`, owned by the code once it exists.
- Deployment & environments: systemd-unit vs. Docker-image as the packaging artifact (or both), environment/config profiles (dev/staging/prod), process-supervision and restart policy, upgrade/rollback story. System Config *discovery* is structural and covered by AD-7; everything else here is packaging-time and doesn't change `config/`'s logic or any other module's structure, so no further AD is needed — this is a deliberate defer, not a silent gap.
