//! Transport error types

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Channel closed")]
    ChannelClosed,

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Bind failed: {0}")]
    BindFailed(String),

    #[error("Accept failed: {0}")]
    AcceptFailed(String),

    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("Receive failed: {0}")]
    ReceiveFailed(String),

    #[error("Close failed: {0}")]
    CloseFailed(String),

    #[error("Serialization failed: {0}")]
    SerializationFailed(String),

    #[error("Deserialization failed: {0}")]
    DeserializationFailed(String),

    #[error("Message too large: {0} bytes")]
    MessageTooLarge(usize),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Transport error: {0}")]
    General(String),
}
