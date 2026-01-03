//! Connection registry for tracking active client connections
//!
//! This allows the server to send messages back to clients for remote function calls.

use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Unique connection ID
pub type ConnectionId = String;

/// Request to call a remote function on the client
pub struct RemoteCallRequest {
    pub request_id: String,
    pub function_name: String,
    pub args: Vec<JsonValue>,
}

/// Request to call a remote method on the client
pub struct RemoteMethodRequest {
    pub request_id: String,
    pub object_id: String,
    pub method_name: String,
    pub args: Vec<JsonValue>,
}

/// Handle to communicate with a specific client connection
#[derive(Clone)]
pub struct ConnectionHandle {
    pub connection_id: ConnectionId,
    pub context_id: String,
    function_call_tx: mpsc::UnboundedSender<RemoteCallRequest>,
    method_call_tx: mpsc::UnboundedSender<RemoteMethodRequest>,
}

impl ConnectionHandle {
    pub fn new(
        connection_id: ConnectionId,
        context_id: String,
        function_call_tx: mpsc::UnboundedSender<RemoteCallRequest>,
        method_call_tx: mpsc::UnboundedSender<RemoteMethodRequest>,
    ) -> Self {
        Self {
            connection_id,
            context_id,
            function_call_tx,
            method_call_tx,
        }
    }

    /// Call a remote function on the client and wait for result (DEPRECATED - use send_function_call + pending_calls)
    pub async fn call_function(
        &self,
        request_id: String,
        function_name: String,
        args: Vec<JsonValue>,
    ) -> Result<JsonValue, String> {
        // Deprecated - kept for backwards compatibility
        // New code should use send_function_call + pending_calls
        self.send_function_call(request_id, function_name, args)
            .await?;
        Err("call_function is deprecated, use send_function_call instead".to_string())
    }

    /// Send a remote function call request (non-blocking)
    pub async fn send_function_call(
        &self,
        request_id: String,
        function_name: String,
        args: Vec<JsonValue>,
    ) -> Result<(), String> {
        self.function_call_tx
            .send(RemoteCallRequest {
                request_id,
                function_name,
                args,
            })
            .map_err(|_| "Connection closed".to_string())?;
        Ok(())
    }

    /// Call a remote method on the client and wait for result (DEPRECATED)
    pub async fn call_method(
        &self,
        request_id: String,
        object_id: String,
        method_name: String,
        args: Vec<JsonValue>,
    ) -> Result<JsonValue, String> {
        // Deprecated - kept for backwards compatibility
        self.send_method_call(request_id, object_id, method_name, args)
            .await?;
        Err("call_method is deprecated, use send_method_call instead".to_string())
    }

    /// Send a remote method call request (non-blocking)
    pub async fn send_method_call(
        &self,
        request_id: String,
        object_id: String,
        method_name: String,
        args: Vec<JsonValue>,
    ) -> Result<(), String> {
        self.method_call_tx
            .send(RemoteMethodRequest {
                request_id,
                object_id,
                method_name,
                args,
            })
            .map_err(|_| "Connection closed".to_string())?;
        Ok(())
    }
}

/// Registry of active connections, mapping context_id to connection handles
pub struct ConnectionRegistry {
    /// Maps context_id -> ConnectionHandle
    connections: HashMap<String, ConnectionHandle>,
}

impl Default for ConnectionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionRegistry {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    /// Register a connection for a context
    pub fn register_connection(&mut self, context_id: String, handle: ConnectionHandle) {
        self.connections.insert(context_id, handle);
    }

    /// Unregister a connection
    pub fn unregister_connection(&mut self, context_id: &str) -> bool {
        self.connections.remove(context_id).is_some()
    }

    /// Get connection handle for a context
    pub fn get_connection(&self, context_id: &str) -> Option<&ConnectionHandle> {
        self.connections.get(context_id)
    }

    /// Check if context has an active connection
    pub fn has_connection(&self, context_id: &str) -> bool {
        self.connections.contains_key(context_id)
    }

    /// Get all registered context IDs
    pub fn list_contexts(&self) -> Vec<String> {
        self.connections.keys().cloned().collect()
    }
}

pub type SharedConnectionRegistry = Arc<RwLock<ConnectionRegistry>>;

pub fn create_connection_registry() -> SharedConnectionRegistry {
    Arc::new(RwLock::new(ConnectionRegistry::new()))
}
