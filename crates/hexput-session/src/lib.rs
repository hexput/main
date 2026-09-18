//! Client ID -> Session, TTL (from System Config), reconnect + FR-13 credential protection.
//! Holds the single mutable, live copy of a Session's per-backend Config; does not depend on
//! hexput-config (System Config), keeping the two surfaces separate.
//!
//! Depends on hexput-globalvar because AD-4 makes this crate the sole caller of
//! `teardown(plugin_id)` — invoked synchronously from the TTL-expiry and explicit-unregister
//! paths, before the Plugin actor is dropped, and never implied by `Drop`.
//!
//! Binds: AD-2, AD-4, AD-5.
