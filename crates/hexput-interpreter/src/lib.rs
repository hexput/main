#![forbid(clippy::undocumented_unsafe_blocks)]

//! Tree-walking evaluator (Stories 1.6-1.7). Depends on hexput-ast only — no host reach,
//! no I/O, no execution-path capability or budget concerns of its own.
//! Binds no Architecture Decision directly; hexput-exec is what wraps it with AD-3's
//! Capability + Resource Budget enforcement.
