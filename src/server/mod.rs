//! Server implementation for hexput runtime
//!
//! Handles WebSocket, Unix domain socket, and named pipe connections.
//! Manages multiple execution contexts with their associated functions and methods.

pub mod context_manager;
pub mod connection;
pub mod connection_registry;
pub mod handler;

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::Value as JsonValue;

use self::connection_registry::{ConnectionRegistry, create_connection_registry};

/// Shared map of pending remote function/method calls
pub type PendingCalls = Arc<RwLock<HashMap<String, tokio::sync::oneshot::Sender<Result<JsonValue, String>>>>>;

/// Create a new shared pending calls map
pub fn create_pending_calls() -> PendingCalls {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// WebSocket bind address
    pub ws_addr: String,
    
    /// Unix domain socket path (Linux/macOS)
    #[cfg(unix)]
    pub unix_socket_path: Option<String>,
    
    /// Named pipe name (Windows)
    #[cfg(windows)]
    pub named_pipe: Option<String>,
    
    /// Enable WebSocket server
    pub enable_ws: bool,
    
    /// Enable Unix socket (Linux/macOS)
    #[cfg(unix)]
    pub enable_unix: bool,
    
    /// Enable named pipe (Windows)
    #[cfg(windows)]
    pub enable_pipe: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            ws_addr: "0.0.0.0:9091".to_string(),
            
            #[cfg(unix)]
            unix_socket_path: Some("/tmp/.s.HXP.9091".to_string()),
            
            #[cfg(windows)]
            named_pipe: Some("\\\\.\\pipe\\hexput".to_string()),
            
            enable_ws: true,
            
            #[cfg(unix)]
            enable_unix: true,
            
            #[cfg(windows)]
            enable_pipe: true,
        }
    }
}

/// Main server instance
pub struct Server {
    config: ServerConfig,
    #[allow(dead_code)]
    connection_registry: Arc<RwLock<ConnectionRegistry>>,
    #[allow(dead_code)]
    pending_calls: PendingCalls,
}

impl Server {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            connection_registry: create_connection_registry(),
            pending_calls: create_pending_calls(),
        }
    }
    
    pub async fn run(&self) -> Result<()> {
        let mut tasks = Vec::new();
        
        // Start WebSocket server
        if self.config.enable_ws {
            let addr = self.config.ws_addr.clone();
            
            tasks.push(tokio::spawn(async move {
                connection::websocket::run_server(&addr).await
            }));
            
            println!("✓ WebSocket server listening on ws://{}", self.config.ws_addr);
        }
        
        // Start Unix domain socket server (Linux/macOS)
        #[cfg(unix)]
        if self.config.enable_unix {
            if let Some(ref path) = self.config.unix_socket_path {
                let path_clone = path.clone();
                let path_display = path.clone();
                
                tasks.push(tokio::spawn(async move {
                    connection::unix_socket::run_server(&path_clone).await
                }));
                
                println!("✓ Unix domain socket listening at {}", path_display);
            }
        }
        
        // Start Named pipe server (Windows)
        #[cfg(windows)]
        if self.config.enable_pipe {
            if let Some(ref pipe) = self.config.named_pipe {
                let pipe_clone = pipe.clone();
                let pipe_display = pipe.clone();
                
                tasks.push(tokio::spawn(async move {
                    connection::named_pipe::run_server(&pipe_clone).await
                }));
                
                println!("✓ Named pipe listening at {}", pipe_display);
            }
        }
        
        if tasks.is_empty() {
            anyhow::bail!("No transports enabled");
        }
        
        println!("\nHexput server ready. Press Ctrl+C to stop.");
        
        // Wait for all tasks (they should run forever)
        for task in tasks {
            if let Err(e) = task.await {
                eprintln!("Task error: {}", e);
            }
        }
        
        Ok(())
    }
}
