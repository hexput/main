//! System Config (file) parsing — separate crate from hexput-session's per-backend Config.
//! Resolves the System Config file's location via one fixed, deployment-agnostic precedence:
//! explicit CLI flag, then environment variable, then a fixed default OS path.
//!
//! Binds: AD-5, AD-7.

/// The daemon's own operational settings (bind addresses, TLS paths, log level, Session TTL, ...),
/// resolved via AD-7's fixed precedence (CLI flag > env var > default OS path).
///
/// Stub: fields land with Epic 2/8; `hexput-daemon::run` already takes this type so the AD-7
/// discovery precedence has one place to plug into once it exists.
#[derive(Debug, Clone, Default)]
pub struct SystemConfig;

impl SystemConfig {
    /// Resolve the System Config file's location per AD-7's fixed precedence and parse it.
    ///
    /// Stub: always returns the default. Real CLI-flag/env-var/default-path resolution lands
    /// with the `config/` implementation story.
    pub fn resolve() -> Self {
        Self
    }
}
