# Hexput v2 — Addendum

Supporting depth that doesn't belong in the core brief: research framing, benchmark methodology, and extra security hardening options.

## Research framing (thesis context)

Hexput v2 is the subject of the author's Master's thesis. The core brief deliberately keeps this out of the main flow (its audience is engineers, PR reviewers, and AI coding agents onboarding, not a thesis committee), but the research question shapes several design choices and is worth recording verbatim:

> Can a capability-based, resource-budgeted scripting sidecar provide a more predictable and safe execution model with lower latency than WebAssembly sandboxes and traditional policy engines, for frequently-evaluated user-defined business logic?

**Framed gap in prior work:** untrusted-code isolation, deterministic execution, resource metering, policy evaluation, and sandboxed plugin systems are usually studied and built separately. This work's contribution is to unify them under one language-independent sidecar architecture with a capability-mediated bidirectional RPC model, and to study the resulting security-expressiveness-latency tradeoff.

Practical implication: design decisions that read as "why not just use X" in the core brief (for example, why not WASM, why not OPA) are also the thesis's comparison axes — they need to be backed by the benchmark in the next section, not just asserted.

## Plugin Registration (full example and design detail)

The core brief's "Two registration modes" bullet points here for the full picture. Example Plugin source, as sketched by the author:

```
plugin {
  name = "plugin_name",
  // backend defined plugin details here
}

let a = 1;
let b = { some_prop_obj: { some_prop: "1" } }
// b is a DashMap-like structure: any function can reference/mutate it,
// and its first-depth keys are locked independently (mutating one key
// doesn't block access to sibling keys). Backends can also configure
// unsafe (lock-free) updates instead. Backends choose the default
// locking strategy for global variables, and may additionally allow
// Plugin code to override that default per-variable.

@Event(BackendRegisteredInit)
fn init_function_here(params) { a = 2; }

@Event(BackendRegisteredEventName, priority = 1)
fn example_event_function(params) {
  // logic goes here — ordered relative to other handlers on this
  // event by priority, if the backend allows ordering control
}

@Event(BackendRegisteredEventName, async = true)
fn example_async_handler(params) {
  // runs concurrently, doesn't wait its turn; defaults to unsafe
  // (lock-free) global-variable mutation unless the backend says otherwise
}
```

Key properties:
- `BackendRegisteredInit` is a reserved event, fired once by the daemon right after registration, before the Plugin accepts any other event.
- All other event names — and the object shape each one returns — are declared by the Backend up front, at registration time, the same way Registered Functions are. A Backend can only fire an event it declared; nothing ad hoc.
- Handler ordering on a shared event name is controllable via `priority` (gated by whether the Backend allows it); `async = true` skips ordering entirely and runs concurrently, defaulting to unsafe/lock-free global-variable mutation unless the Backend configures otherwise.
- Plugin state (its global variables) persists for the life of the Backend's Client ID — it survives a reconnect and is torn down on disconnect-without-reconnect or explicit unregister. An in-flight event invocation on disconnect is left to finish locally, and is only cancelled if it then tries to call back to the Backend (an RPC) and finds it gone.
- Each event handler invocation is still bound by the same Resource Budget and Capability-based RPC rules as any other execution — Plugin mode changes state lifetime and event-driven invocation, not the trust model.

Full mechanics (including a couple of remaining low-risk assumptions on tie-breaking and combination behavior) are in the PRD's §4.9 — this section stays a pointer, not a duplicate.

## Benchmark plan

No fixed timeline exists, so this is a target shape, not a committed methodology yet.

**Comparators:**
- Rhai (closest performance comparator; embedded scripting baseline)
- A WASM sandbox — Wasmtime or Extism (isolation-focused comparator)
- A policy engine — OPA/Rego (declarative-rules comparator, expressiveness ceiling reference)

**Metrics:**
- p50 / p99 execution latency, at steady state (cache warm)
- Throughput (executions/sec) under concurrent load
- Cold-start cost: first-parse time vs. AST-cache-hit time, to quantify the caching win
- Memory footprint per cached script / per concurrent context

**Open methodology questions:**
- What workload(s) are representative? (The university pass/fail-rule example is a good anchor — small, frequently evaluated, host-callback-heavy.)
- Same-machine (UDS) vs. cross-machine (TCP/TLS) numbers need to be reported separately — the transport choice materially changes the latency picture and conflating them would misrepresent both.
- Whether resource-budgeting overhead itself is measured as a separate line item (cost of safety) vs. folded into the headline numbers.

## Additional security hardening ideas (beyond language-level discipline)

Raised because the trust boundary today is "the Hexput VM behaves correctly," with no OS backstop. Not committed scope — options to evaluate before any deployment serving mutually-untrusted tenants:

- **Process-level defense in depth:** seccomp-bpf syscall filtering and cgroup CPU/memory limits per daemon (or per worker, if execution moves to a worker-pool model), so a VM escape still lands in a constrained sandbox rather than the bare host.
- **Per-client-ID rate limiting**, independent of per-script resource budgets — caps abuse patterns that stay under any single execution's budget but hostile in aggregate (for example, many cheap executions).
- **RPC call auditing/logging** at the capability boundary — every host function call a script makes should be attributable and replayable for incident review, since it's the one place scripts touch the outside world.
- **Panic isolation + supervised restart:** if the VM can panic on malformed/adversarial input, wrap execution so a panic degrades to "this execution failed" rather than taking down the daemon (and by extension every other tenant's in-flight work).
- **Fuzzing the parser/VM** as a standing CI job (for example, cargo-fuzz) — the language surface is the attack surface; this is cheap relative to its value given there's no sandbox layer to fall back on.
- **`unsafe` audit as a release gate:** since Rust's memory safety is doing real security work here (not just ergonomics), track and review every `unsafe` block in the VM/runtime path specifically, not just as normal code review.
- **Replay/reuse protection on client-ID reconnection:** since IDs aren't 1:1 with connections, worth explicitly deciding whether a stale/leaked client ID can be replayed by an unrelated party, and what auth (if any) gates reconnection. *Resolved since this list was drafted: the PRD commits to this (FR-13, reconnect protection) — kept here for context, not as an open idea.*
