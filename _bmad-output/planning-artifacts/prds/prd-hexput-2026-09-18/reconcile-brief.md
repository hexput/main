# Reconciliation: Brief → PRD (Hexput v2, 2026-09-18)

Method: read brief.md + brief/addendum.md (source), then prd.md + prd/addendum.md (derived), and diffed content/claims/tone. PRD's own memlog.md treats the brief as settled primary input, so gaps below are things that should have carried forward (as an FR, an NFR, a non-goal, or at minimum an Open Question) but didn't, plus one place where the PRD asserts something stronger than the brief committed to.

## 1. Transport: Named Pipe (Windows) silently dropped

- **Brief, Deployment model:** "Local backends connect over Unix Domain Socket (**or Named Pipe on Windows**)."
- **Brief, Scope (in scope for v2):** "Standalone daemon (systemd/Docker), WebSocket + Unix Domain Socket + **Named Pipe** transports."
- **PRD:** §3 Glossary "Transport" lists only UDS / TCP+TLS / WebSocket. FR-9 says "A Backend can connect via Unix Domain Socket, TCP+TLS, or WebSocket." §6.1 MVP Scope: "all three transports (UDS, TCP+TLS, WebSocket)."
- **Gap:** Named Pipe is a concrete, in-scope transport commitment in the brief that never appears in the PRD — not as an FR, not as an explicit non-goal/deferral either. It's just gone, with no traceable decision to drop it. Worth flagging as either an oversight (restore it, likely as an FR-9 consequence or a new FR) or an undocumented scope cut (should appear in §5 Non-Goals or §6.2 Out of Scope with a reason).

## 2. OS-level sandboxing: PRD asserts a stronger, contradicting position

- **Brief, Scope (out of scope/future work):** lists OS-level sandboxing (seccomp/cgroups/namespaces) as "defense-in-depth **beyond** language-level discipline" — grouped with other *future-work* items (master-slave, rollback).
- **Brief, Next steps #3:** "**Decide**, before any untrusted multi-tenant deployment, whether OS-level defense-in-depth ... is required or deferred with an explicit risk acceptance." — i.e., explicitly framed as an open decision, not resolved.
- **Brief addendum, hardening list:** offers seccomp-bpf + cgroups as an option "to evaluate before any deployment serving mutually-untrusted tenants," again framed as a pending evaluation, not a settled no.
- **PRD, §2.2 Non-Users:** "this is a **permanent design stance**, not a gap to be filled later."
- **PRD, §5 Non-Goals:** "Hexput will not provide OS-level sandboxing ... **ever**, as a permanent design stance, not a deferred feature."
- **Gap/contradiction:** The PRD has converted an explicitly-open decision ("decide before untrusted multi-tenant deployment") into a permanent, closed-off commitment. This is a substantive policy escalation beyond what the brief settled, and it forecloses the brief's own Next Steps item #3 without recording that a decision was made (or by whom/why). At minimum this deserves a line in §9 Assumptions Index or §8 Open Questions noting the brief left this open and the PRD is taking a firmer stance — as written, a reviewer comparing the two documents would reasonably read this as a contradiction, not a refinement.

## 3. Security hardening list (addendum) — partial carry-through

The brief addendum's "Additional security hardening ideas" has 7 items. PRD coverage:

| Hardening idea | PRD coverage |
|---|---|
| seccomp/cgroups | Contradicted — see #2 above |
| **Per-client-ID rate limiting** (independent of per-script resource budgets) | **Missing entirely** — no FR, NFR, or Open Question anywhere in the PRD |
| RPC call auditing/logging ("every host function call ... attributable and replayable") | **Partial** — FR-12 only logs capability *denials* and budget *violations* tagged by Client ID; the brief's ask was broader (every call, allowed or not, attributable/replayable for incident review). Successful/allowed RPC calls are not explicitly required to be logged. |
| Panic isolation + supervised restart | Captured — §7 Reliability NFR |
| **Fuzzing the parser/VM as a standing CI job** (cargo-fuzz) | **Missing entirely** — not mentioned in PRD or PRD addendum, despite the brief explicitly flagging it as "cheap relative to its value given there's no sandbox layer to fall back on" |
| `unsafe` audit as release gate | Captured — §7 Security NFR |
| Replay/reuse protection on reconnect | Captured — FR-13 |

Two items (rate limiting, fuzzing CI) are clean drops with no trace, not even as a deferred/out-of-scope line. Given the brief explicitly calls fuzzing "cheap" and "worth it," and rate limiting closes a distinct abuse vector that per-script budgets don't cover, both look like they should at least appear in §6.2 Out of Scope for MVP or §8 Open Questions if deliberately deferred.

## 4. "No committed timeline" / thesis-driven scope-pruning framing — dropped

- **Brief, Risks & open questions:** "**No committed timeline.** This is thesis-driven work without an external deadline, which is fine for depth but means **scope needs active pruning to stay shippable**."
- **PRD:** No mention of timeline, deadline, or the scope-discipline risk anywhere (§8 Open Questions, §9 Assumptions Index checked). The PRD does reference the benchmark being "a thesis deliverable tracked separately" (§6.2) — the word "thesis" survives once — but the actual risk being named in the brief (no deadline pressure → scope creep risk, needs active pruning) isn't carried into the PRD as a risk, NFR, or process note.
- **Gap:** This is a process/governance risk, not a functional requirement, so it's understandable it doesn't map to an FR — but PRDs typically carry forward risks like this in an Open Questions or Assumptions section, and this one is absent.

## 5. "Why not existing solutions" comparative reasoning — lost, not just condensed

- **Brief:** Gives specific counter-arguments per alternative — Rhai lacks "socket-native multi-tenant service model or capability-mediated RPC"; WASM sandboxes have "compilation/instantiation overhead and toolchain complexity" working against the "frequently re-evaluated, short-lived logic" case; OPA has "policy evaluation but not general expressiveness."
- **PRD:** §5 Non-Goals only captures the OPA/Rhai-adjacent conclusion ("not a general-purpose embeddable language replacement for Rhai/Lua"). The WASM-specific reasoning (compile/instantiation overhead vs. the AST-cache hot-path design) is never restated anywhere in the PRD, even though AST caching (FR-5) is presented as a first-class feature without explaining *why* it exists relative to the WASM alternative.
- **Assessment:** Minor/acceptable per PRD's own stated discipline (§0: "does not re-derive" the brief's settled reasoning) — flagged for completeness, not necessarily a defect. Worth a one-line pointer back to the brief's "Why not existing solutions" section if a reader would otherwise wonder why AST caching matters so much.

## 6. Thesis research question — correctly and deliberately not restated (no gap)

The brief addendum is explicit that the thesis framing is deliberately kept out of the engineering-facing brief; the PRD addendum correctly points back to the brief addendum for it rather than repeating it. This is consistent, not a gap.

## 7. Everything else — checked, no material gap found

- Trust-boundary-is-the-runtime risk: captured (PRD §2.2, §4.3, Non-Goals).
- No-rollback risk: captured (FR-8 Out of Scope, Non-Goals).
- Benchmark-methodology-not-finalized risk: captured (PRD §7 Performance NFR explicitly declines to set a pre-benchmark target).
- Two connection modes / client ID not 1:1 with connections / master-slave groundwork: captured (Glossary, FR-2, Non-Users, Non-Goals).
- Primary/secondary user split: captured faithfully in §2.1 JTBD.
- SDK phasing: captured faithfully (FR-10, addendum sequencing note).

## Summary of actionable gaps

1. **Named Pipe (Windows) transport** — committed in brief scope, absent from PRD (FR-9, Glossary, §6.1).
2. **OS-level sandboxing stance** — PRD declares "permanent, ever" where brief left it as an explicit pre-deployment decision to be made (Next Steps #3). Contradiction, not just omission.
3. **Per-client-ID rate limiting** — in brief addendum's hardening list, absent from PRD entirely.
4. **Fuzzing CI job (cargo-fuzz)** — in brief addendum's hardening list, flagged as high-value/cheap, absent from PRD entirely.
5. **RPC audit logging scope** — brief wants *every* host function call attributable/replayable; PRD's FR-12 only covers denials and budget violations, not successful calls.
6. **"No committed timeline" / scope-pruning risk** — named explicitly in brief's Risks section, not carried into PRD's Open Questions or Assumptions.
7. (Minor) WASM-specific "why not" reasoning behind the AST-cache design choice isn't referenced in the PRD, though this is arguably in-bounds per the PRD's own "don't re-derive" discipline.
