//! Semantic error types

use thiserror::Error;

pub type SemanticResult<T> = Result<T, SemanticError>;

#[derive(Debug, Error, Clone)]
pub enum SemanticError {
    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),

    #[error("Undefined function: {0}")]
    UndefinedFunction(String),

    #[error("Variable already declared: {0}")]
    AlreadyDeclared(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid remote call: {0}")]
    InvalidRemoteCall(String),
}
