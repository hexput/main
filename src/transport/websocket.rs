//! WebSocket transport implementation
//!
//! This module provides WebSocket-based RPC transport.
//! The implementation is async internally but presents blocking
//! semantics to the language runtime.

use super::error::TransportError;
use crate::rpc::protocol::Message;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;
use futures::{SinkExt, StreamExt};

pub type WebSocketStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>
>;

/// WebSocket transport for RPC messages
pub struct WebSocketTransport {
    stream: WebSocketStream,
}

impl WebSocketTransport {
    pub async fn connect(url: &str) -> Result<Self, TransportError> {
        let (stream, _) = tokio_tungstenite::connect_async(url)
            .await
            .map_err(|e| TransportError::WebSocket(e.to_string()))?;

        Ok(Self { stream })
    }

    pub async fn send(&mut self, message: Message) -> Result<(), TransportError> {
        let json = serde_json::to_string(&message)?;
        self.stream
            .send(WsMessage::Text(json.into()))
            .await
            .map_err(|e| TransportError::WebSocket(e.to_string()))?;
        Ok(())
    }

    pub async fn receive(&mut self) -> Result<Message, TransportError> {
        match self.stream.next().await {
            Some(Ok(WsMessage::Text(text))) => {
                let message: Message = serde_json::from_str(&text)?;
                Ok(message)
            }
            Some(Ok(WsMessage::Close(_))) => Err(TransportError::ConnectionClosed),
            Some(Err(e)) => Err(TransportError::WebSocket(e.to_string())),
            None => Err(TransportError::ConnectionClosed),
            _ => Err(TransportError::General("Unexpected message type".to_string())),
        }
    }

    pub async fn close(mut self) -> Result<(), TransportError> {
        self.stream
            .close(None)
            .await
            .map_err(|e| TransportError::WebSocket(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // Tests would require a WebSocket server
    // For now, just verify the module compiles
}
