//! Named pipe transport for Windows
//!
//! Provides IPC via Windows named pipes for local communication.

use super::{Transport, TransportError, TransportType};
use crate::rpc::Message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{NamedPipeClient, NamedPipeServer, ServerOptions};

/// Named pipe transport
pub struct NamedPipeTransport {
    pipe: NamedPipeStream,
    name: String,
}

enum NamedPipeStream {
    Server(NamedPipeServer),
    Client(NamedPipeClient),
}

impl NamedPipeTransport {
    /// Create a new named pipe transport from a server pipe
    pub fn from_server(pipe: NamedPipeServer, name: String) -> Self {
        Self {
            pipe: NamedPipeStream::Server(pipe),
            name,
        }
    }

    /// Create a new named pipe transport from a client pipe
    pub fn from_client(pipe: NamedPipeClient, name: String) -> Self {
        Self {
            pipe: NamedPipeStream::Client(pipe),
            name,
        }
    }

    /// Connect to a named pipe
    pub async fn connect(name: &str) -> Result<Self, TransportError> {
        let pipe = tokio::net::windows::named_pipe::ClientOptions::new()
            .open(name)
            .map_err(|e| TransportError::ConnectionFailed(e.to_string()))?;
        
        Ok(Self::from_client(pipe, name.to_string()))
    }

    /// Create a named pipe server
    pub fn create_server(name: &str) -> Result<NamedPipeServerBuilder, TransportError> {
        Ok(NamedPipeServerBuilder {
            name: name.to_string(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    async fn read_exact(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        match &mut self.pipe {
            NamedPipeStream::Server(pipe) => pipe.read_exact(buf).await,
            NamedPipeStream::Client(pipe) => pipe.read_exact(buf).await,
        }
    }

    async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        match &mut self.pipe {
            NamedPipeStream::Server(pipe) => pipe.write_all(buf).await,
            NamedPipeStream::Client(pipe) => pipe.write_all(buf).await,
        }
    }

    async fn flush(&mut self) -> std::io::Result<()> {
        match &mut self.pipe {
            NamedPipeStream::Server(pipe) => pipe.flush().await,
            NamedPipeStream::Client(pipe) => pipe.flush().await,
        }
    }
}

impl Transport for NamedPipeTransport {
    async fn send(&mut self, message: Message) -> Result<(), TransportError> {
        let json = serde_json::to_string(&message)
            .map_err(|e| TransportError::SerializationFailed(e.to_string()))?;
        
        // Length-prefixed framing: send 4-byte length, then message
        let len = json.len() as u32;
        self.write_all(&len.to_be_bytes()).await
            .map_err(|e| TransportError::SendFailed(e.to_string()))?;
        
        self.write_all(json.as_bytes()).await
            .map_err(|e| TransportError::SendFailed(e.to_string()))?;
        
        self.flush().await
            .map_err(|e| TransportError::SendFailed(e.to_string()))?;
        
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<Message>, TransportError> {
        // Read 4-byte length prefix
        let mut len_bytes = [0u8; 4];
        match self.read_exact(&mut len_bytes).await {
            Ok(_) => {},
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
        self.read_exact(&mut buffer).await
            .map_err(|e| TransportError::ReceiveFailed(e.to_string()))?;
        
        let message = serde_json::from_slice(&buffer)
            .map_err(|e| TransportError::DeserializationFailed(e.to_string()))?;
        
        Ok(Some(message))
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        // Named pipes close automatically when dropped
        Ok(())
    }

    fn transport_type(&self) -> TransportType {
        TransportType::NamedPipe
    }
}

/// Named pipe server builder
pub struct NamedPipeServerBuilder {
    name: String,
}

impl NamedPipeServerBuilder {
    pub fn build(&self) -> Result<NamedPipeServerListener, TransportError> {
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&self.name)
            .map_err(|e| TransportError::BindFailed(e.to_string()))?;
        
        Ok(NamedPipeServerListener {
            server,
            name: self.name.clone(),
        })
    }
}

/// Named pipe server listener
pub struct NamedPipeServerListener {
    server: NamedPipeServer,
    name: String,
}

impl NamedPipeServerListener {
    pub async fn accept(&mut self) -> Result<NamedPipeTransport, TransportError> {
        self.server.connect().await
            .map_err(|e| TransportError::AcceptFailed(e.to_string()))?;
        
        // Create next instance
        let next_server = ServerOptions::new()
            .create(&self.name)
            .map_err(|e| TransportError::BindFailed(e.to_string()))?;
        
        let current_server = std::mem::replace(&mut self.server, next_server);
        
        Ok(NamedPipeTransport::from_server(current_server, self.name.clone()))
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::{Request, Response};

    #[tokio::test]
    async fn test_named_pipe_communication() {
        let pipe_name = r"\\.\pipe\hexput_test";
        
        // Spawn server
        let server_name = pipe_name.to_string();
        let server_handle = tokio::spawn(async move {
            let builder = NamedPipeTransport::create_server(&server_name).unwrap();
            let mut listener = builder.build().unwrap();
            let mut transport = listener.accept().await.unwrap();
            
            // Receive request
            let msg = transport.recv().await.unwrap().unwrap();
            
            // Send response
            let response = Message::Response(Response::success(
                "test-id".to_string(),
                serde_json::json!("pong")
            ));
            transport.send(response).await.unwrap();
        });

        // Give server time to bind
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Client
        let mut client = NamedPipeTransport::connect(pipe_name).await.unwrap();
        
        let request = Message::Request(Request::new(
            "test-id".to_string(),
            "ping".to_string(),
            vec![]
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
