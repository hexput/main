//! Runtime layer: VM, execution contexts, values
//!
//! This module handles:
//! - Value representation
//! - Execution contexts
//! - VM operations
//! - Stack management
//!
//! The runtime is synchronous from the script's perspective,
//! but may yield internally for RPC calls.

pub mod context;
pub mod error;
pub mod rpc_handler;
pub mod value;
pub mod vm;

pub use context::Context;
pub use error::RuntimeError;
pub use rpc_handler::{NoOpRpcHandler, RpcHandler};
pub use value::Value;
pub use vm::execute;

pub type ExecutionResult<T> = Result<T, RuntimeError>;
