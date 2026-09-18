//! Global Variable store: a shared concurrent per-Plugin map (dashmap), independent of and
//! reachable without the Plugin actor being alive. Depends on hexput-shared only — never
//! hexput-plugin or hexput-rpc — so the store cannot be reached through RPC and stays live
//! across the actor's mailbox lifecycle.
//! Exposes teardown(plugin_id) as its sole store-lifecycle entry point, called only by
//! hexput-session before the Plugin actor is dropped.
//!
//! Binds: AD-3, AD-4.
