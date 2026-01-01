//! Transport layer: Communication mechanisms
//!
//! This module handles:
//! - WebSocket connections
//! - Unix domain sockets
//! - Named pipes (Windows)
//! - Message framing and routing
//!
//! This layer is a mechanism, not part of the language.

pub mod websocket;
pub mod message;
pub mod error;

#[cfg(unix)]
pub mod unix_socket;

#[cfg(windows)]
pub mod named_pipe;

pub use message::MessageHandler;
pub use error::TransportError;

use crate::rpc::Message;
use tokio::sync::mpsc;

/// Transport types available
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportType {
    WebSocket,
    #[cfg(unix)]
    UnixSocket,
    #[cfg(windows)]
    NamedPipe,
}

/// Unified transport interface
pub trait Transport: Send + Sync {
    /// Send a message
    fn send(&mut self, message: Message) -> impl std::future::Future<Output = Result<(), TransportError>> + Send;
    
    /// Receive a message
    fn recv(&mut self) -> impl std::future::Future<Output = Result<Option<Message>, TransportError>> + Send;
    
    /// Close the transport
    fn close(&mut self) -> impl std::future::Future<Output = Result<(), TransportError>> + Send;
    
    /// Get transport type
    fn transport_type(&self) -> TransportType;
}

/// Transport channel for message passing
pub struct TransportChannel {
    tx: mpsc::Sender<Message>,
    rx: mpsc::Receiver<Message>,
}

impl TransportChannel {
    pub fn new(buffer_size: usize) -> Self {
        let (tx, rx) = mpsc::channel(buffer_size);
        Self { tx, rx }
    }

    pub async fn send(&self, message: Message) -> Result<(), TransportError> {
        self.tx.send(message).await
            .map_err(|_| TransportError::ChannelClosed)
    }

    pub async fn recv(&mut self) -> Option<Message> {
        self.rx.recv().await
    }

    pub fn sender(&self) -> mpsc::Sender<Message> {
        self.tx.clone()
    }
}

