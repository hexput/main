//! Windows named pipe server implementation

use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tokio::sync::{RwLock, mpsc};

use crate::rpc::protocol::Message;
use crate::server::context_manager::ContextManager;
use crate::server::handler;

pub async fn run_server(pipe_name: &str) -> Result<()> {
    loop {
        let server = ServerOptions::new()
            .first_pipe_instance(false)
            .create(pipe_name)?;
        
        tokio::spawn(async move {
            if let Err(e) = handle_connection(server).await {
                eprintln!("Named pipe connection error: {}", e);
            }
        });
    }
}

async fn handle_connection(
    pipe: NamedPipeServer,
) -> Result<()> {
    // Wait for client to connect
    pipe.connect().await?;
    
    let (mut read_half, mut write_half) = tokio::io::split(pipe);
    
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
        let mut buffer = vec![0u8; 10 * 1024 * 1024]; // 10MB buffer
        let mut _current_context_id: Option<String> = None;
        
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
                // Track context_id from messages
                match &message {
                    Message::RegisterFunction(ref reg) => {
                        _current_context_id = Some(reg.context_id.clone());
                    }
                    Message::RegisterMethod(ref reg) => {
                        _current_context_id = Some(reg.context_id.clone());
                    }
                    Message::ExecutionStart(ref exec) => {
                        _current_context_id = Some(exec.context_id.clone());
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
