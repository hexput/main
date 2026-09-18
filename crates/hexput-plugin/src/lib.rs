//! Plugin actor: registration, event routing, priority/async dispatch. Owns event routing
//! and ordering metadata only — it does not itself execute the ordered chain; a per-
//! (Plugin, Event) sequencing task, spawned independently of the actor's mailbox, does that.
//! Invokes hexput-check once, at Plugin registration.
//!
//! Binds: AD-3, AD-4, AD-6, AD-8.
