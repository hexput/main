//! Static check (FR-26). Exposes a single pure entry point taking a parsed AST plus the
//! callable-name set and the active policy, and returning findings — it never executes a
//! script, never reaches hexput-rpc or hexput-enforce, and holds no state between calls.
//! Depends on hexput-ast and hexput-shared only — never hexput-interpreter, hexput-rpc, or
//! hexput-enforce, by construction.
//!
//! Binds: AD-8.
