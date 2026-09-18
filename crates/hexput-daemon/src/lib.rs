//! Wiring root: composes every daemon crate into one runnable `run(SystemConfig)`. The only
//! crate that depends on hexput-transport, which is what makes AD-1's transport-agnostic core
//! a compile-time property.
//!
//! Binds: AD-1, AD-7.

// Re-exported so `hexput-bin`'s thin `hexput-daemon` binary can construct the argument to
// `run()` without adding a direct dependency edge on `hexput-config` (not in the Spine's graph
// for `hexput-bin`).
pub use hexput_config::SystemConfig;

/// Compose every daemon crate (transport, session, connection, script, plugin, config, exec)
/// into one running daemon.
///
/// Stub: the actual wiring lands with Epic 2+ stories; the thin `hexput-daemon` binary in
/// `hexput-bin` already calls this so the entry point exists ahead of the implementation.
pub fn run(_config: hexput_config::SystemConfig) {
    todo!("daemon wiring lands with Epic 2+")
}
