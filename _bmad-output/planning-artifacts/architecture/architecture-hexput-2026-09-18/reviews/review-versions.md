# Review — Version/Reality-Check Lens

**Target:** `_bmad-output/planning-artifacts/architecture/architecture-hexput-2026-09-18/ARCHITECTURE-SPINE.md`
**Lens:** every committed decision was web-researched or reality-checked, not asserted from training data — current versions, that named tech still exists and fits, greenfield starter defaults.
**Method:** independently re-queried crates.io's JSON API (`https://crates.io/api/v1/crates/<name>`) for all 9 pinned crates in the Stack table, plus a web search for current Rust stable, on 2026-09-18 (same date the spine claims its own verification).

## Verdict: CONCERNS

8 of 9 pinned versions check out exactly against crates.io. One is stale and was pinned to a version that was no longer newest as of the claimed verification date.

## Findings

### 1. tokio-tungstenite pinned to 0.29.0, but 0.30.0 was already the newest stable release — MEDIUM
- Spine claims: `tokio-tungstenite 0.29.0`, web-verified 2026-09-18.
- crates.io `newest_version`/`max_version` for `tokio-tungstenite`: **0.30.0**, published **2026-07-11** — over two months before the spine's claimed verification date.
- 0.29.0 (2026-03-17) is one minor behind. This isn't a version that shipped *after* verification (which would be excusable) — it was already superseded when the table says it was checked. Either the verification didn't actually hit crates.io for this crate, or it hit an inconsistent/cached view.
- Action: confirm whether 0.30.0 is compatible with the pinned `rustls 0.23.45` (tokio-tungstenite 0.30 lines may expect a newer rustls-pki-types) before deciding to bump; if staying on 0.29.0 is deliberate (e.g. avoiding a breaking change), the spine should say so instead of presenting it as simply "verified."

### 2. Confirmed correct — tokio, dashmap, moka, rustls, serde, rmp-serde, criterion, tracing — INFORMATIONAL
Independently re-checked against crates.io; all match the spine's claimed newest stable version exactly as of 2026-09-18:
- tokio 1.53.1 (newest, 2026-07-20) ✓
- dashmap 6.2.1 (newest stable; 7.0.0-rc2 exists but is a pre-release, correctly not chosen) ✓
- rustls 0.23.45 (newest stable; 0.24.0-dev.1 exists but is dev-only, correctly not chosen) ✓
- serde 1.0.229 (newest, 2026-07-18) ✓
- rmp-serde 1.3.1 (newest, 2025-12-23) ✓
- criterion 0.8.2 (newest, 2026-02-04) ✓
- tracing 0.1.44 (newest, 2025-12-18) ✓
- moka 0.12.16 (newest, active maintenance) ✓ — also confirmed it ships a `future` feature (async-lock/event-listener/futures-util) for async cache usage.

### 3. "Rust: current stable (2024 edition)" has no pinned version number to verify — LOW
- The Stack table's other 9 rows carry exact semver pins; the Rust row is a floating "current stable" with no version. As of 2026-09-18, current stable is Rust **1.98.1** (2026-09-03, per rust-lang blog / endoflife.date). That's fine as a floating policy, but it means this row was never actually checked against anything — there's nothing to falsify. If the intent is "whatever's current when you build," say so explicitly; if a concrete MSRV matters (e.g. for CI pinning), the spine should record the number that was current at verification time, the same way every other row does.

### 4. Architectural fit checks — PASS
- **dashmap** for AD-4's Global Variable store (per-top-level-key locking) — fits: DashMap is a sharded concurrent `HashMap` with per-shard locking, matching the "per-top-level-key locking" rule directly.
- **moka** for the AST Cache (`script/`) — fits: moka is a concurrent, TTL/eviction-capable cache (Caffeine-inspired), appropriate for a hot-path AST cache; its sync map is sufficient here since Direct/Cached Execution paths don't obviously need moka's async variant, but the crate itself is well-suited and still maintained (16 releases in the 0.12.x line, actively shipping).
- **rmp-serde/serde** for the wire envelope, **tracing** for structured logging, **criterion** for benchmarking — all standard, well-fitted, actively maintained choices; no red flags.
- No named technology appears deprecated, archived, or replaced by a different de-facto standard.

## Summary
The Stack table's verification claim is mostly trustworthy but not fully accurate: 8/9 pinned crates are genuinely current as claimed, but **tokio-tungstenite 0.29.0 was already one minor version behind (0.30.0, released 2 months earlier) at the claimed verification date** — this should be corrected or explicitly justified as an intentional hold-back. The floating Rust version row was never actually checked against anything concrete. All named technologies fit their stated architectural roles (dashmap/AD-4, moka/AST cache, rmp-serde+serde/wire envelope, tracing/logging).
