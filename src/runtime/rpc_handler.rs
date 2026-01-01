//! RPC handler trait for remote function calls
//!
//! This allows the VM to make remote calls while maintaining
//! synchronous semantics from the script's perspective.

use super::value::Value;
use super::error::RuntimeResult;

/// Handler for remote function calls
///
/// The VM calls this when a function is not found locally.
/// The handler should block (from the VM's perspective) until
/// the remote call completes or fails.
pub trait RpcHandler: Send + Sync {
    /// Call a remote function
    ///
    /// # Arguments
    /// * `function` - Function name to call
    /// * `args` - Evaluated argument values
    ///
    /// # Returns
    /// The result value or an error
    fn call_remote(&self, function: &str, args: Vec<Value>) -> RuntimeResult<Value>;
    
    /// Call a method on a remote object
    ///
    /// # Arguments
    /// * `object_id` - The object's identifier (from secret_data.id)
    /// * `method` - Method name to call
    /// * `args` - Evaluated argument values
    ///
    /// # Returns
    /// The result value or an error
    fn call_method(&self, object_id: &str, method: &str, args: Vec<Value>) -> RuntimeResult<Value>;
}

/// No-op RPC handler that always fails
///
/// Used when remote calls are disabled
pub struct NoOpRpcHandler;

impl RpcHandler for NoOpRpcHandler {
    fn call_remote(&self, function: &str, _args: Vec<Value>) -> RuntimeResult<Value> {
        Err(super::error::RuntimeError::UndefinedFunction(function.to_string()))
    }
    
    fn call_method(&self, _object_id: &str, method: &str, _args: Vec<Value>) -> RuntimeResult<Value> {
        Err(super::error::RuntimeError::General(
            format!("Method '{}' not found (remote calls disabled)", method)
        ))
    }
}
