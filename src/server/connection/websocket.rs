//! WebSocket server implementation

use anyhow::Result;
use futures::{StreamExt, SinkExt};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, mpsc};
use tokio_tungstenite::{accept_async, tungstenite::Message as WsMessage};
use serde_json::Value as JsonValue;

use crate::rpc::protocol::Message;
use crate::server::context_manager::ContextManager;
use crate::server::handler;

// Local types for remote calls (no longer using shared connection registry)
#[allow(dead_code)]
struct RemoteCallRequest {
    request_id: String,
    function_name: String,
    args: Vec<JsonValue>,
}

#[allow(dead_code)]
struct RemoteMethodRequest {
    request_id: String,
    object_id: String,
    method_name: String,
    args: Vec<JsonValue>,
}

pub async fn run_server(
    addr: &str,
) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    
    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream).await {
                        eprintln!("WebSocket connection error from {}: {}", peer_addr, e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Failed to accept WebSocket connection: {}", e);
            }
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
) -> Result<()> {
    let ws_stream = accept_async(stream).await?;
    let (mut write, mut read) = ws_stream.split();
    
    // Create local ContextManager for this connection
    let manager = Arc::new(RwLock::new(ContextManager::new()));
    
    // Create local pending_calls for this connection
    let pending_calls = crate::server::create_pending_calls();
    
    // Create channels for sending outgoing messages
    // One channel for RPC protocol Messages (used by RPC handler)
    let (rpc_tx, mut rpc_rx) = mpsc::unbounded_channel::<Message>();
    // One channel for WebSocket messages (final output)
    let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel::<WsMessage>();
    
    // Spawn task to convert RPC Messages to WebSocket messages
    let outgoing_tx_clone = outgoing_tx.clone();
    tokio::spawn(async move {
        while let Some(rpc_msg) = rpc_rx.recv().await {
            if let Ok(json) = serde_json::to_string(&rpc_msg) {
                let _ = outgoing_tx_clone.send(WsMessage::Text(json.into()));
            }
        }
    });
    
    // Spawn task to send WebSocket messages
    let outgoing_writer_task = tokio::spawn(async move {
        while let Some(msg) = outgoing_rx.recv().await {
            if write.send(msg).await.is_err() {
                break;
            }
        }
    });
    
    // Handle incoming messages
    while let Some(msg) = read.next().await {
        println!("DEBUG WS: Received raw WebSocket message");
        match msg {
            Ok(WsMessage::Text(text)) => {
                println!("DEBUG WS: Received Text message, length: {}", text.len());
                if let Ok(message) = serde_json::from_str::<Message>(&text) {
                    // Track context_id from registration messages (for future use)
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
                    }
                    
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
                            if let Ok(response_text) = serde_json::to_string(&response) {
                                let _ = outgoing_tx_clone.send(WsMessage::Text(response_text.into()));
                            }
                        }
                    });
                }
            }
            Ok(WsMessage::Binary(data)) => {
                if let Ok(message) = serde_json::from_slice::<Message>(&data) {
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
                            if let Ok(response_data) = serde_json::to_vec(&response) {
                                let _ = outgoing_tx_clone.send(WsMessage::Binary(response_data.into()));
                            }
                        }
                    });
                }
            }
            Ok(WsMessage::Close(_)) => {
                break;
            }
            Ok(WsMessage::Ping(data)) => {
                if outgoing_tx.send(WsMessage::Pong(data)).is_err() {
                    break;
                }
            }
            Ok(WsMessage::Pong(_)) => {
                // Ignore pong
            }
            Ok(WsMessage::Frame(_)) => {
                // Ignore raw frames
            }
            Err(e) => {
                eprintln!("WebSocket error: {}", e);
                break;
            }
        }
    }
    
    outgoing_writer_task.abort();
    
    Ok(())
}
