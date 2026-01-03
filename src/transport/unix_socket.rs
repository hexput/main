//! Unix domain socket transport
//!
//! Provides IPC via Unix domain sockets for local communication.

use super::{Transport, TransportError, TransportType};
use crate::rpc::Message;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

/// Unix domain socket transport
pub struct UnixSocketTransport {
    stream: UnixStream,
    path: PathBuf,
}

impl UnixSocketTransport {
    /// Create a new Unix socket transport from an existing stream
    pub fn new(stream: UnixStream, path: PathBuf) -> Self {
        Self { stream, path }
    }

    /// Connect to a Unix socket at the given path
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self, TransportError> {
        let path = path.as_ref().to_path_buf();
        let stream = UnixStream::connect(&path)
            .await
            .map_err(|e| TransportError::ConnectionFailed(e.to_string()))?;

        Ok(Self { stream, path })
    }

    /// Create a listener for incoming connections
    pub async fn bind(path: impl AsRef<Path>) -> Result<UnixListener, TransportError> {
        let path = path.as_ref();

        // Remove existing socket file if present
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| TransportError::BindFailed(e.to_string()))?;
        }

        let listener =
            UnixListener::bind(path).map_err(|e| TransportError::BindFailed(e.to_string()))?;

        Ok(listener)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Transport for UnixSocketTransport {
    async fn send(&mut self, message: Message) -> Result<(), TransportError> {
        let json = serde_json::to_string(&message)
            .map_err(|e| TransportError::SerializationFailed(e.to_string()))?;

        // Length-prefixed framing: send 4-byte length, then message
        let len = json.len() as u32;
        self.stream
            .write_all(&len.to_be_bytes())
            .await
            .map_err(|e| TransportError::SendFailed(e.to_string()))?;

        self.stream
            .write_all(json.as_bytes())
            .await
            .map_err(|e| TransportError::SendFailed(e.to_string()))?;

        self.stream
            .flush()
            .await
            .map_err(|e| TransportError::SendFailed(e.to_string()))?;

        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<Message>, TransportError> {
        // Read 4-byte length prefix
        let mut len_bytes = [0u8; 4];
        match self.stream.read_exact(&mut len_bytes).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(TransportError::ReceiveFailed(e.to_string())),
        }

        let len = u32::from_be_bytes(len_bytes) as usize;

        // Sanity check on message size (max 10MB)
        if len > 10 * 1024 * 1024 {
            return Err(TransportError::MessageTooLarge(len));
        }

        // Read message
        let mut buffer = vec![0u8; len];
        self.stream
            .read_exact(&mut buffer)
            .await
            .map_err(|e| TransportError::ReceiveFailed(e.to_string()))?;

        let message = serde_json::from_slice(&buffer)
            .map_err(|e| TransportError::DeserializationFailed(e.to_string()))?;

        Ok(Some(message))
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        self.stream
            .shutdown()
            .await
            .map_err(|e| TransportError::CloseFailed(e.to_string()))?;
        Ok(())
    }

    fn transport_type(&self) -> TransportType {
        TransportType::UnixSocket
    }
}

/// Unix socket server
pub struct UnixSocketServer {
    listener: UnixListener,
    path: PathBuf,
}

impl UnixSocketServer {
    pub async fn bind(path: impl AsRef<Path>) -> Result<Self, TransportError> {
        let path = path.as_ref().to_path_buf();
        let listener = UnixSocketTransport::bind(&path).await?;

        Ok(Self { listener, path })
    }

    pub async fn accept(&mut self) -> Result<UnixSocketTransport, TransportError> {
        let (stream, _addr) = self
            .listener
            .accept()
            .await
            .map_err(|e| TransportError::AcceptFailed(e.to_string()))?;

        Ok(UnixSocketTransport::new(stream, self.path.clone()))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for UnixSocketServer {
    fn drop(&mut self) {
        // Clean up socket file
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::{Request, Response};

    #[tokio::test]
    async fn test_unix_socket_communication() {
        let path = "/tmp/hexput_test.sock";

        // Spawn server
        let server_path = path.to_string();
        let server_handle = tokio::spawn(async move {
            let mut server = UnixSocketServer::bind(&server_path).await.unwrap();
            let mut transport = server.accept().await.unwrap();

            // Receive request
            let _msg = transport.recv().await.unwrap().unwrap();

            // Send response
            let response = Message::Response(Response::success(
                "resp-1".to_string(),
                "test-id".to_string(),
                serde_json::json!("pong"),
            ));
            transport.send(response).await.unwrap();
        });

        // Give server time to bind
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Client
        let mut client = UnixSocketTransport::connect(path).await.unwrap();

        let request = Message::Request(Request::new(
            "req-1".to_string(),
            "test-id".to_string(),
            "ping".to_string(),
            vec![],
        ));

        client.send(request).await.unwrap();
        let response = client.recv().await.unwrap().unwrap();

        match response {
            Message::Response(r) => assert_eq!(r.id, "test-id"),
            _ => panic!("Expected response"),
        }

        server_handle.await.unwrap();
    }
}
