//! Unix domain socket server implementation

use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{RwLock, mpsc};

use crate::rpc::protocol::Message;
use crate::server::context_manager::ContextManager;
use crate::server::handler;

pub async fn run_server(path: &str) -> Result<()> {
    // Remove old socket if it exists
    let _ = std::fs::remove_file(path);
    
    let listener = UnixListener::bind(path)?;
    
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream).await {
                        eprintln!("Unix socket connection error: {}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Failed to accept Unix socket connection: {}", e);
            }
        }
    }
}

async fn handle_connection(
    stream: UnixStream,
) -> Result<()> {
    let (read_half, write_half) = stream.into_split();
    
    // Create local ContextManager for this connection
    let manager = Arc::new(RwLock::new(ContextManager::new()));
    
    // Create local pending_calls for this connection
    let pending_calls = crate::server::create_pending_calls();
    
    // Channels for outgoing messages
    // One channel for RPC protocol Messages (used by RPC handler)
    let (rpc_tx, mut rpc_rx) = mpsc::unbounded_channel::<Message>();
    // One channel for serialized bytes (final output)
    let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    
    // Spawn task to convert RPC Messages to bytes
    let outgoing_tx_clone = outgoing_tx.clone();
    tokio::spawn(async move {
        while let Some(rpc_msg) = rpc_rx.recv().await {
            if let Ok(data) = serde_json::to_vec(&rpc_msg) {
                let _ = outgoing_tx_clone.send(data);
            }
        }
    });
    
    // Spawn writer task
    let writer_task = tokio::spawn(async move {
        let mut write_half = write_half;
        while let Some(data) = outgoing_rx.recv().await {
            let len = data.len() as u32;
            if write_half.write_all(&len.to_be_bytes()).await.is_err() {
                break;
            }
            if write_half.write_all(&data).await.is_err() {
                break;
            }
            if write_half.flush().await.is_err() {
                break;
            }
        }
    });
    
    // Spawn reader task
    let reader_task = tokio::spawn(async move {
        let mut read_half = read_half;
        let mut buffer = vec![0u8; 10 * 1024 * 1024]; // 10MB buffer
        
        loop {
            // Read length prefix (4 bytes)
            let mut len_buf = [0u8; 4];
            match read_half.read_exact(&mut len_buf).await {
                Ok(_) => {}
                Err(_) => break, // Connection closed
            }
            
            let len = u32::from_be_bytes(len_buf) as usize;
            if len > buffer.len() {
                eprintln!("Message too large: {} bytes", len);
                break;
            }
            
            // Read message
            if read_half.read_exact(&mut buffer[..len]).await.is_err() {
                break;
            }
            
            // Parse message
            if let Ok(message) = serde_json::from_slice::<Message>(&buffer[..len]) {
                // Track context_id from messages (for future use)
                match &message {
                    Message::RegisterFunction(ref reg) => {
                        let _ = reg.context_id.clone();
                    }
                    Message::RegisterMethod(ref reg) => {
                        let _ = reg.context_id.clone();
                    }
                    Message::ExecutionStart(ref exec) => {
                        let _ = exec.context_id.clone();
                    }
                    _ => {}
                };
                
                // Spawn a new task to handle this message
                let manager_clone = Arc::clone(&manager);
                let pending_calls_clone = Arc::clone(&pending_calls);
                let rpc_tx_clone = rpc_tx.clone();
                let outgoing_tx_clone = outgoing_tx.clone();
                
                tokio::spawn(async move {
                    if let Some(response) = handler::handle_message(
                        message,
                        &manager_clone,
                        &pending_calls_clone,
                        &rpc_tx_clone,
                    ).await {
                        // Serialize and send response
                        if let Ok(response_data) = serde_json::to_vec(&response) {
                            let _ = outgoing_tx_clone.send(response_data);
                        }
                    }
                });
            }
        }
    });
    
    // Wait for reader to finish
    let _ = reader_task.await;
    
    // Cleanup
    writer_task.abort();
    
    Ok(())
}
