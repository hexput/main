---
lens: adversarial
target: ARCHITECTURE-SPINE.md (Hexput v2)
created: '2026-09-18'
---

# Adversarial Review — Hexput v2 Architecture Spine

**Method:** for each pair of ADs (and each Deferred item), construct two concrete implementation units, one level below the spine, that each satisfy the letter of every AD they're bound by, and check whether their outputs are still compatible at the seam. Every pair that diverges is a hole an AD (new or tightened) must close.

**Verdict: NOT CLEAR — 7 divergences found, 2 of them (D1, D3) severe enough to block confident parallel implementation of `session/`, `plugin/`, `globalvar/`, and `exec/`.**

---

## D1 — Who tears down a Plugin's Global Variable store on Session TTL expiry (AD-2 × AD-4)

This is the exact scenario named in the brief, and it is real.

- **AD-2** makes `session/` the owner of TTL and the entity that "owns... zero or more Plugin actors." It never states that `session/` is responsible for triggering `globalvar/` teardown specifically — only that the Session owns Plugins.
- **AD-4** makes `globalvar/` a "shared concurrent per-Plugin store... independent of the Plugin actor's mailbox" — independent is the operative word, and the rule never says who owns the store's lifecycle, only who reads/writes it.
- FR-21 is explicit that Global Variable state must be torn down on TTL expiry or explicit unregister, but it's a Plugin-level requirement, not an AD, so it doesn't resolve the ownership question at the module-boundary level the spine actually binds.

**Engineer A** (`session/`) reasonably assumes: TTL expiry drops the `Plugin` actor handle; since AD-4 calls the GV store "per-Plugin," dropping the actor's owning struct will `Drop`-cascade the store away. `session/` never calls into `globalvar/` directly — that would violate AD-1-style layering (session shouldn't reach past plugin/ into globalvar/ implementation details).

**Engineer B** (`globalvar/`), reading "independent of the Plugin actor's mailbox" literally, builds the store as a free-standing registry (e.g. a `DashMap<PluginId, Store>` module-level singleton or service, so it can be written by *both* the sequential path and detached `async` tasks that may outlive the actor's own poll loop — a real requirement, since AD-4 says async handlers "read/write the same store" independent of mailbox liveness). This store is *not* reachable through the `Plugin` struct's `Drop` at all; it requires an explicit `globalvar::teardown(plugin_id)` call.

**Result:** Build A + Build B together leak every Plugin's Global Variable store forever — `session/` never calls the one API that frees it, and `globalvar/`'s independence (mandated by AD-4, needed for async-handler correctness) is precisely what makes implicit `Drop`-cascade teardown impossible to rely on.

**Fix:** tighten AD-4 (or add an AD) that names the teardown contract explicitly: `globalvar/` exposes `teardown(plugin_id)` as its only store-lifecycle entry point, and AD-2 (or FR-21's home) must state that `session/`'s TTL-expiry and explicit-unregister paths are the *only* callers, invoked synchronously before the Plugin actor is dropped — not implied by actor drop.

---

## D2 — Does a Registered async handler's budget window close at spawn or at completion (AD-3 × AD-4 × AD-6)

- **AD-3**: "all three execution paths call into one `Executor` entry point; no path may... consume Resource Budget outside it." This describes the *entry*, not the *duration* of accounting.
- **AD-6**: "every execution... dispatches as an independent async task on the shared runtime" — i.e. `Executor` fires the task and does not block the caller.
- **AD-4**: async handlers are "directly-spawned" and bypass the Plugin's serialized mailbox, reading/writing `globalvar/` on their own schedule, potentially well after the triggering Event dispatch has returned.

**Engineer C** (`exec/`) implements AD-6's non-blocking dispatch literally: `Executor::dispatch()` spawns the task, opens a budget accounting scope for the spawn call, and closes/reports it when the *spawn* returns (consistent with "dispatches as an independent async task" reading as fire-and-forget). CPU time, RPC count, and side-effect count for anything the async handler does *after* that point are attributed to... nothing, because the accounting scope already closed.

**Engineer D** (`globalvar/` + `plugin/`), reading AD-3's "no path may consume Resource Budget outside [the Executor]" as an invariant that must hold for the *entire* lifetime of the async handler (otherwise its Global Variable writes and any RPC calls it makes are literally unbudgeted, violating FR-8 and the security-critical SM-C1 "bypass rate must stay exactly zero"), builds the async task to carry a live budget handle/token captured from `Executor` at spawn time, and has every `globalvar/` write call back into that token to charge side-effect count.

**Result:** Build C silently exempts all async-handler work from budget enforcement (a live SM-C1 violation) unless Build D's callback convention exists — but AD-3/AD-6 as written give no instruction that `exec/` must hand out a live, callable budget token rather than a closed one. Two teams following the letter produce a system where either budgets are bypassed for async handlers, or `globalvar/` ends up importing `enforce/` machinery directly (which AD-3 arguably forbids, since `globalvar/` is not "the Executor").

**Fix:** AD-3 needs an explicit clause: for any execution that continues past the dispatching call's return (i.e. any `async = true` handler), `Executor` must hand the spawned task a live budget-accounting handle, and all budget-relevant operations performed by that task (including `globalvar/` writes counted as side-effects) charge through that handle — never a second, independent path into `enforce/`.

---

## D3 — Session-to-Connection cardinality: the ER diagram contradicts FR-2 (AD-2 × Structural Seed)

- The spine's own `erDiagram` states: `SESSION ||--o| CONNECTION : "attached to (0 or 1, transient)"` — cardinality 0-or-1.
- AD-2's prose only constrains the other direction: "`Connection` is a transient actor attached to at most one `Session` at a time" — it never states a Session may only hold one Connection.
- FR-2 (bound by AD-2) is explicit and testable: "A Client ID is not limited to a single concurrent connection — multiple Backend processes may reconnect under the same ID concurrently."

**Engineer A** (`session/`), building strictly from the diagram (the artifact literally in the spine, labeled 0-or-1) implements `Session { connection: Option<ConnectionHandle> }`. A second concurrent reconnect under the same Client ID evicts or rejects the first.

**Engineer B** (`connection/`), building strictly from FR-2 (an AD-2-bound requirement) implements `Session { connections: Vec<ConnectionHandle> }` and fans out — e.g. broadcasting Plugin Events results to all attached connections, or picking one nondeterministically for response routing.

**Result:** these are not just different internal choices — they produce different, mutually-incompatible protocol behavior (does a second reconnect kick the first off, or do both stay live?), and both engineers can point to a part of the spine that "obeys AD-2 to the letter."

**Fix:** the ER diagram's cardinality is simply wrong given FR-2 and must be corrected to `SESSION ||--o{ CONNECTION`, and AD-2's rule text should gain an explicit sentence covering the Session→Connection direction (fan-out and result-routing behavior when N>1), since that's exactly the kind of state FR-2 promises exists.

---

## D4 — Priority ordering enforcement point: inside the Plugin actor's mailbox, or outside it (AD-4 × AD-6)

- **AD-4**: "The Plugin actor owns event routing/ordering/priority only."
- **AD-6**: "every execution... dispatches as an independent async task... the only permitted serialization is a Plugin's own opted-in `priority` ordering... among its own handlers for one Event — never across connections, Plugins, or Events."

**Engineer A**, reading AD-4's "owns... ordering/priority," implements priority ordering as synchronous, sequential processing inside the Plugin actor's own message loop: for a given Event, the actor calls `Executor` for priority-1's handler and `.await`s the result before advancing to priority-2, etc. This is the natural, direct reading of "the actor owns ordering."

**Engineer B**, reading AD-6's "never across... Events" as an invariant that must hold in practice, realizes that Build A's design breaks it: because the actor has one mailbox, an in-progress priority-ordered handler chain for Event X (which can run for the full CPU-budget window) blocks the same actor from beginning to process a *different* Event Y that arrives concurrently for the same Plugin — an accidental cross-Event serialization AD-6 explicitly prohibits. Engineer B instead builds ordering as a detached mini-scheduler (e.g. a per-(Plugin, Event) await-chain spawned off the actor, with the actor's mailbox loop only doing routing/dispatch bookkeeping, never blocking on `Executor` results).

**Result:** Build A is a straightforward, spec-compliant-sounding reading of AD-4 that silently violates AD-6's cross-Event non-serialization guarantee the moment two different Events fire concurrently for one Plugin. Build B is required for correctness but isn't what AD-4's wording ("the actor owns... ordering") naturally suggests to an implementer.

**Fix:** AD-4 (or a new AD) should state explicitly that "owns ordering" means the actor computes/holds ordering *metadata* only; actual sequencing of Executor calls for priority handlers must happen off the actor's single mailbox path (e.g. via a per-Event await-chain spawned independently), so that concurrent Events for the same Plugin are never serialized against each other even while one Event's priority chain is in flight.

---

## D5 — Which module holds the live, current-truth per-Backend Config once a Plugin exists (AD-5 × FR-3 × AD-2/AD-4)

AD-5 fixes *where* Config lives (in-protocol, never disk) but not *who is the single live copy's owner* once both `session/` and `plugin/` have reason to read it on every dispatch.

**Engineer A** (`plugin/`) snapshots the Backend's Config (budget defaults, feature toggles, ordering permissions) into the `Plugin` struct at registration time, for fast local access on every Event dispatch — a reasonable read of AD-5 ("per-backend Config never touches disk... never configures the daemon"), which says nothing about staleness.

**Engineer B** (`session/`) implements FR-3 ("A runtime config update... is visible to subsequent executions... without a reconnect") by mutating a single `Session`-owned Config store in place, assuming every consumer (including Plugin dispatch) reads through that live pointer rather than caching.

**Result:** with Build A, a Backend's runtime Config update (FR-3) never reaches already-registered Plugins' Event dispatch — priority-ordering permission or budget-default changes silently don't apply to Plugins, only to Direct/Cached Execution, even though FR-3 draws no such distinction. Both builds independently satisfy AD-5's "separate, non-overlapping surfaces" rule; neither AD-2 nor AD-4 states which module is the single source of truth for *live* per-Backend Config reads at dispatch time.

**Fix:** add a rule (to AD-5 or a new AD) naming `session/`'s Config store as the single mutable source of truth, with an explicit statement that `plugin/` and `script/`/`exec/` must read through it (no per-registration snapshotting) so FR-3's runtime-update guarantee applies uniformly to every execution mode.

---

## D6 — `keyed` Global Variable lock granularity: per-top-level-key as written, or per-(key, partition) as the feature implies (AD-4 × Deferred)

- AD-4's rule text is literal: "per-top-level-key locking... per FR-20/FR-25's behavior strategies."
- FR-25's `keyed` behavior partitions a Global Variable's *value* by a Backend-supplied key, independent of locking strategy (FR-25: "independent of its locking strategy (FR-20)").
- Deferred explicitly flags: "Exact `keyed` Global Variable storage layout... implementation detail within `globalvar/`, owned by the code once it exists" — i.e. the spine knowingly leaves this open.

**Engineer A** implements locking exactly as AD-4 states: the lock scope is the top-level key, full stop. A `keyed` variable's different partitions (different Backend-supplied keys) all contend for the *same* top-level-key lock, because that's the literal, only locking unit AD-4 names.

**Engineer B** implements locking per-`(top_level_key, partition_key)`, reasoning that FR-20's whole purpose ("mutating one key does not block concurrent access to sibling keys") is defeated for `keyed` variables under Build A's reading, and that FR-25 calling the two strategies "independent" implies partition-level isolation should compose with lock granularity too.

**Result:** Build A is defensible as literal AD-4 compliance but produces a `keyed` variable that serializes unrelated Backend-supplied-key writes against each other — a real, measurable non-blocking-concurrency regression versus a plain (non-keyed) Global Variable, and arguably a violation of PRD §8's "one Event handler mutating one key must not stall a concurrent handler working on a different key" NFR once "key" is read as "partition key." Build B fixes it but silently redefines what "per-top-level-key locking" scopes over for this one behavior strategy, diverging from Build A's on-disk/in-memory layout entirely (different lock objects, different concurrent-map nesting).

**Fix:** the Deferred item should be promoted to a tightened AD-4 clause before `globalvar/` implementation starts: lock granularity for a `keyed` variable is per-`(top_level_key, partition_key)`, not per-top-level-key alone; "top-level-key locking" in AD-4 should be reworded to make partition-awareness explicit rather than left as an owned-by-the-code detail.

---

## D7 — Does `exec/` (script-level GV opcodes) or `rpc/` (host-function capability gate) own the integration point for in-script Global Variable access (AD-1/AD-3 × AD-4 × FR-7)

- The spine's own invariants diagram draws `Executor --> GVStore` directly, alongside `Plugin --> GVStore`.
- AD-4's rule text only describes Plugin-actor access ("both its sequential path and directly-spawned `async` handlers read/write the same store") — it says nothing about `exec/` touching `GVStore` directly, despite the diagram edge.
- FR-7 (No ambient host access) requires: "A script cannot access... except through a Registered Function explicitly granted to it." It's silent on whether reading/writing a Plugin's *own* declared Global Variables counts as "host access" subject to capability gating, or is intrinsic/ambient language state (like a local variable) exempt from FR-6/FR-7.

**Engineer C** (`exec/`), following the diagram's `Executor --> GVStore` edge, implements Global Variable read/write as built-in interpreter opcodes with a direct dependency on `globalvar/`'s API — no capability check, no `rpc/` involvement, since GV access isn't a "Registered Function" call at all in this build.

**Engineer D** (`rpc/` + `globalvar/`), taking FR-7's "except through a Registered Function" literally as covering *all* host-adjacent state (Global Variables persist server-side across invocations, unlike a local variable, so they read as "host access" under FR-7's spirit), builds GV access as an implicit Registered Function the daemon auto-registers per Plugin, routed through the same capability/handler machinery as Backend-defined RPCs — meaning a Backend could in principle gate or intercept its own Plugin's GV access the way it gates file/network access.

**Result:** these are fully incompatible integration surfaces — Build C makes `exec/` import `globalvar/` directly (contradicting AD-3's "no path may reach a Registered Function... outside [the Executor's single entry point]" only if GV access *is* deemed a Registered-Function-shaped thing, which is exactly the ambiguity), while Build D requires `globalvar/` to be reachable only via `rpc/`'s registry, an entirely different module dependency graph than the Structural Seed implies (`plugin/` and `exec/` both point at `globalvar/` directly in the seed, not via `rpc/`).

**Fix:** add a clause (AD-4 or AD-3) explicitly stating Global Variable access is intrinsic language state, not a Registered Function, exempt from FR-6/FR-7's capability-grant requirement, and reached only via the direct `Executor`/`Plugin --> GVStore` path shown in the diagram — closing off the `rpc/`-mediated reading as a valid alternative.

---

## Summary table

| # | AD pair (+ FR) | Divergent units | Consequence |
| --- | --- | --- | --- |
| D1 | AD-2 × AD-4 (FR-21) | `session/` assumes Drop-cascade teardown vs. `globalvar/` built as an independent registry needing explicit `teardown()` | GV store leaks forever on TTL expiry |
| D2 | AD-3 × AD-4 × AD-6 (FR-8, SM-C1) | Budget scope closes at async spawn vs. must stay live for the task's full duration | Async-handler work unbudgeted — direct SM-C1 violation |
| D3 | AD-2 × Structural Seed ER diagram (FR-2) | Session:Connection modeled as 0..1 (per diagram) vs. 0..N (per FR-2) | Second concurrent reconnect evicted vs. fanned out — incompatible protocol behavior |
| D4 | AD-4 × AD-6 | Priority ordering serialized on the actor's single mailbox vs. off-mailbox await-chain | Mailbox design silently violates AD-6's "never across Events" guarantee |
| D5 | AD-5 × FR-3 | `plugin/` snapshots Config at registration vs. `session/` as live mutable source | Runtime Config updates (FR-3) silently don't reach already-registered Plugins |
| D6 | AD-4 × Deferred (FR-20/FR-25) | Lock scoped to top-level-key only vs. to (top-level-key, partition-key) | `keyed` variables serialize unrelated partitions — regresses the non-blocking guarantee |
| D7 | AD-1/AD-3 × AD-4 × FR-7 | GV access as intrinsic interpreter opcode vs. as an implicit Registered Function through `rpc/` | Two incompatible module dependency graphs; unclear whether GV access is capability-gated |

**Recommendation:** D1 and D3 should block sprint planning for `session/`/`connection/`/`plugin/`/`globalvar/` until resolved — they're outright contradictions (diagram vs. FR, and an ownership gap with no teardown caller at all), not just underspecification. D2 is a security-relevant gap (ties to SM-C1) and should be resolved before `exec/`'s Executor entry point is coded. D4–D7 are all implementable divergences where each side has a legitimate reading of the current AD text; they should become AD amendments before `plugin/`/`globalvar/`/`exec/` work starts in parallel.
