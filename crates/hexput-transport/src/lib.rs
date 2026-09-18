//! Adapters: uds.rs, named_pipe.rs, tcp_tls.rs, websocket.rs — one Port implementation each.
//! No transport-specific behavior crosses into the core. Only hexput-daemon depends on this
//! crate, which is what makes AD-1 a compile-time property rather than a review convention.
//!
//! Binds: AD-1.
