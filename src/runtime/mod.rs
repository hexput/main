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

pub mod value;
pub mod context;
pub mod vm;
pub mod error;
pub mod rpc_handler;

pub use value::Value;
pub use context::Context;
pub use vm::execute;
pub use error::RuntimeError;
pub use rpc_handler::{RpcHandler, NoOpRpcHandler};

pub type ExecutionResult<T> = Result<T, RuntimeError>;
