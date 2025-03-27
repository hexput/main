use crate::error::RuntimeError;
use crate::messages::{FunctionCallResponse, FunctionExistsResponse, WebSocketMessage, WebSocketRequest, WebSocketResponse};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex as TokioMutex, oneshot};
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info};
use std::collections::HashMap;
use serde_json::json;

pub struct ServerConfig {
    pub address: String,
}

pub async fn run_server(config: ServerConfig) -> Result<(), RuntimeError> {
    let addr = config.address.parse::<SocketAddr>().map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid server address")
    })?;

    let listener = TcpListener::bind(&addr).await?;
    info!("WebSocket server listening on: {}", addr);

    let active_connections = Arc::new(TokioMutex::new(0));

    while let Ok((stream, peer_addr)) = listener.accept().await {
        info!("New connection from: {}", peer_addr);

        let connections = active_connections.clone();

        {
            let mut count = connections.lock().await;
            *count += 1;
            info!("Active connections: {}", *count);
        }

        tokio::spawn(async move {
            match handle_connection(stream, peer_addr).await {
                Ok(_) => info!("Connection from {} closed gracefully", peer_addr),
                Err(e) => error!("Error handling connection from {}: {}", peer_addr, e),
            }

            let mut count = connections.lock().await;
            *count -= 1;
            info!("Connection closed. Active connections: {}", *count);
        });
    }

    Ok(())
}

enum SenderMessage {
    Text(String),
    Pong(Vec<u8>),
    Close,
}

async fn handle_connection(stream: TcpStream, peer_addr: SocketAddr) -> Result<(), RuntimeError> {
    debug!("Starting WebSocket handshake with: {}", peer_addr);
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    info!("WebSocket connection established with: {}", peer_addr);

    let (ws_sender, mut ws_receiver) = ws_stream.split();
    
    let (sender_tx, mut sender_rx) = mpsc::channel::<SenderMessage>(100);
    
    let function_calls = Arc::new(Mutex::new(HashMap::<String, oneshot::Sender<FunctionCallResponse>>::new()));
    let function_validations = Arc::new(Mutex::new(HashMap::<String, oneshot::Sender<FunctionExistsResponse>>::new()));
    
    let sender_task = tokio::spawn(async move {
        let mut sender = ws_sender;
        
        while let Some(msg) = sender_rx.recv().await {
            match msg {
                SenderMessage::Text(text) => {
                    if let Err(e) = sender.send(Message::Text(text)).await {
                        error!("Error sending message: {}", e);
                        break;
                    }
                },
                SenderMessage::Pong(data) => {
                    if let Err(e) = sender.send(Message::Pong(data)).await {
                        error!("Error sending pong: {}", e);
                        break;
                    }
                },
                SenderMessage::Close => {
                    break;
                }
            }
        }
        
        let _ = sender.close().await;
    });
    
    let welcome_sender = sender_tx.clone();
    
    if let Err(e) = welcome_sender.send(SenderMessage::Text(
        r#"{"type":"connection","status":"connected"}"#.to_string()
    )).await {
        error!("Failed to send welcome message: {}", e);
        return Err(RuntimeError::ConnectionError("Failed to send welcome message".to_string()));
    }

    let mut task_set: tokio::task::JoinSet<()> = tokio::task::JoinSet::new();

    let create_message_sender = |tx: mpsc::Sender<SenderMessage>| {
        move |message: String| -> futures_util::future::BoxFuture<'static, Result<(), RuntimeError>> {
            let sender = tx.clone();
            Box::pin(async move {
                sender.send(SenderMessage::Text(message)).await
                    .map_err(|_| RuntimeError::ConnectionError("Failed to send message".to_string()))
            })
        }
    };

    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                debug!("Received text message from {}: {}", peer_addr, text);
                
                let function_calls_clone = function_calls.clone();
                let function_validations_clone = function_validations.clone();
                let sender_clone = sender_tx.clone();
                let message_sender = create_message_sender(sender_clone.clone());
                
                match serde_json::from_str::<WebSocketMessage>(&text) {
                    Ok(WebSocketMessage::FunctionResponse(response)) => {
                        debug!("Received function response for ID: {}", response.id);
                        
                        if let Err(e) = handle_function_response_directly(response, function_calls_clone).await {
                            error!("Error processing function response: {}", e);
                        }
                    },
                    Ok(WebSocketMessage::FunctionExistsResponse(response)) => {
                        debug!("Received function exists response for ID: {}, function exists: {}", response.id, response.exists);
                        
                        if let Err(e) = handle_function_exists_response(response, function_validations_clone).await {
                            error!("Error processing function exists response: {}", e);
                        }
                    },
                    Ok(WebSocketMessage::Request(request)) => {
                        debug!("Processing request with ID: {}", request.id);
                        let req_id = request.id.clone();
                        
                        task_set.spawn(async move {
                            match process_request(request, function_calls_clone, function_validations_clone, message_sender).await {
                                Ok(_) => debug!("Request {} processed successfully", req_id),
                                Err(e) => {
                                    error!("Error processing request {}: {}", req_id, e);
                                    
                                    let error_response = WebSocketResponse {
                                        id: req_id,
                                        success: false,
                                        result: None,
                                        error: Some(format!("Internal error: {}", e)),
                                    };
                                    
                                    if let Ok(json) = serde_json::to_string(&error_response) {
                                        if let Err(send_err) = sender_clone.send(SenderMessage::Text(json)).await {
                                            error!("Failed to send error response: {}", send_err);
                                        }
                                    }
                                }
                            }
                        });
                    },
                    Ok(WebSocketMessage::Unknown(value)) => {
                        error!("Received unknown message type: {}", value);
                        
                        let error_msg = json!({
                            "error": "Unknown message format",
                            "details": value
                        }).to_string();
                        
                        if let Err(e) = sender_clone.send(SenderMessage::Text(error_msg)).await {
                            error!("Failed to send error message: {}", e);
                        }
                    },
                    Err(e) => {
                        error!("Failed to parse message: {}", e);
                        
                        let error_msg = json!({
                            "error": "Failed to parse message",
                            "details": e.to_string()
                        }).to_string();
                        
                        if let Err(e) = sender_clone.send(SenderMessage::Text(error_msg)).await {
                            error!("Failed to send error message: {}", e);
                        }
                    }
                }
            },
            Ok(Message::Ping(data)) => {
                debug!("Received ping from {}", peer_addr);
                let pong_sender = sender_tx.clone();
                
                if let Err(e) = pong_sender.send(SenderMessage::Pong(data)).await {
                    error!("Error sending pong to {}: {}", peer_addr, e);
                }
            },
            Ok(Message::Close(_)) => {
                info!("Received close message from {}", peer_addr);
                break;
            },
            Err(e) => {
                error!("Error reading message from {}: {}", peer_addr, e);
                break;
            },
            _ => {
                debug!("Received other message type from {}", peer_addr);
            }
        }
    }

    let _ = sender_tx.send(SenderMessage::Close).await;
    
    if let Err(e) = sender_task.await {
        error!("Error awaiting sender task: {}", e);
    }

    debug!("Cleaning up tasks for connection {}", peer_addr);
    while task_set.join_next().await.is_some() { }

    info!("Closing connection with: {}", peer_addr);
    Ok(())
}

async fn process_request(
    request: WebSocketRequest,
    function_calls: Arc<Mutex<HashMap<String, oneshot::Sender<FunctionCallResponse>>>>,
    function_validations: Arc<Mutex<HashMap<String, oneshot::Sender<FunctionExistsResponse>>>>,
    message_sender: impl Fn(String) -> futures_util::future::BoxFuture<'static, Result<(), RuntimeError>> + Send + Clone + 'static,
) -> Result<(), RuntimeError> {
    handle_request(request, function_calls, function_validations, message_sender).await?;
    
    Ok(())
}

async fn handle_function_response_directly(
    response: FunctionCallResponse,
    function_calls: Arc<Mutex<HashMap<String, oneshot::Sender<FunctionCallResponse>>>>,
) -> Result<(), RuntimeError> {
    debug!("Processing function response for call ID: {}", response.id);
    
    let sender = {
        let mut calls = function_calls.lock().unwrap();
        calls.remove(&response.id)
    };
    
    if let Some(sender) = sender {
        if sender.send(response).is_err() {
            error!("Failed to send response through channel - receiver likely dropped");
        } else {
            debug!("Successfully sent function response through channel");
        }
    } else {
        error!("Received function response for unknown call ID: {}", response.id);
    }
    
    Ok(())
}

async fn handle_function_exists_response(
    response: crate::messages::FunctionExistsResponse,
    function_validations: Arc<Mutex<HashMap<String, oneshot::Sender<FunctionExistsResponse>>>>,
) -> Result<(), RuntimeError> {
    debug!("Processing function exists response for ID: {}", response.id);
    
    let sender = {
        let mut validations = function_validations.lock().unwrap();
        validations.remove(&response.id)
    };
    
    if let Some(sender) = sender {
        if sender.send(response).is_err() {
            error!("Failed to send function exists response through channel - receiver likely dropped");
        } else {
            debug!("Successfully sent function exists response through channel");
        }
    } else {
        error!("Received function exists response for unknown ID: {}", response.id);
    }
    
    Ok(())
}

async fn handle_request(
    request: WebSocketRequest,
    function_calls: Arc<Mutex<HashMap<String, oneshot::Sender<FunctionCallResponse>>>>,
    function_validations: Arc<Mutex<HashMap<String, oneshot::Sender<FunctionExistsResponse>>>>,
    message_sender: impl Fn(String) -> futures_util::future::BoxFuture<'static, Result<(), RuntimeError>> + Send + Clone + 'static,
) -> Result<(), RuntimeError> {
    let result = crate::handler::handle_request(request, function_calls, function_validations, message_sender.clone()).await?;
    
    if !result.is_empty() {
        message_sender(result).await?;
    }
    
    Ok(())
}
