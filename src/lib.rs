//! Hexput Language Runtime
//!
//! A deterministic, synchronous, sandboxed RPC language implemented in Rust.
//!
//! # Architecture
//!
//! The runtime is organized into strict layers:
//! - `language`: Lexical and syntactic analysis (lexer, parser, AST)
//! - `semantic`: Name resolution, symbol tables, capability checking
//! - `runtime`: VM, execution contexts, value representation
//! - `rpc`: Protocol definitions, message dispatching, registry
//! - `transport`: WebSocket and message handling
//! - `sandbox`: Resource limits, guards, timeouts
//! - `util`: Shared utilities

pub mod language;
pub mod semantic;
pub mod runtime;
pub mod rpc;
pub mod transport;
pub mod sandbox;
pub mod util;
pub mod server;

pub use language::parse;
pub use runtime::{Context, execute};
pub use sandbox::Limits;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
