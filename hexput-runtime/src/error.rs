use hexput_ast_api::ast_structs::SourceLocation;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("WebSocket error: {0}")]
    WebSocketError(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("AST parsing error: {0}")]
    AstParsingError(String),

    #[error("Invalid request format: {0}")]
    InvalidRequestFormat(String),

    #[error("Missing required field in request: {0}")]
    MissingField(String),

    #[error("Execution error: {0}")]
    ExecutionError(String),

    #[error("Execution error at line {}, column {}: {message}", location.start_line, location.start_column)]
    ExecutionErrorWithLocation {
        message: String,
        location: SourceLocation,
    },

    #[error("Function call error: {0}")]
    FunctionCallError(String),

    #[error("Function not found: {0}")]
    FunctionNotFoundError(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Message parsing error: {0}")]
    MessageParsingError(String),

    #[error("Task execution error: {0}")]
    TaskExecutionError(String),

    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("Timeout error: {0}")]
    TimeoutError(String),
}

impl RuntimeError {
    pub fn format_location(location: &SourceLocation) -> String {
        format!(
            "line {}, column {}",
            location.start_line, location.start_column
        )
    }

    pub fn with_location(message: String, location: SourceLocation) -> Self {
        RuntimeError::ExecutionErrorWithLocation { message, location }
    }
}
