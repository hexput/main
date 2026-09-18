//! Wire envelope, request/response, and event correlation, built on hexput-shared::wire.
//! Health/metrics (FR-11) are served through this same Port, exempted from the init-handshake
//! gate rather than given a separate listener.
//!
//! Binds: AD-1.
