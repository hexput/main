//! RPC error types

use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum RpcError {
    #[error("Function not found: {0}")]
    FunctionNotFound(String),

    #[error("Invalid arguments for function {0}")]
    InvalidArguments(String),

    #[error("RPC call timeout")]
    Timeout,

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("RPC error: {0}")]
    General(String),
}
