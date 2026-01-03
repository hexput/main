//! Runtime error types

use thiserror::Error;

pub type RuntimeResult<T> = Result<T, RuntimeError>;

#[derive(Debug, Error, Clone)]
pub enum RuntimeError {
    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),

    #[error("Undefined function: {0}")]
    UndefinedFunction(String),

    #[error("Type error: expected {expected}, got {got}")]
    TypeError { expected: String, got: String },

    #[error("Division by zero")]
    DivisionByZero,

    #[error("Index out of bounds: {0}")]
    IndexOutOfBounds(usize),

    #[error("Invalid property access: {0}")]
    InvalidPropertyAccess(String),

    #[error("Execution limit exceeded: {0}")]
    LimitExceeded(String),

    #[error("Remote call failed: {0}")]
    RemoteCallFailed(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Runtime error: {0}")]
    General(String),
}
