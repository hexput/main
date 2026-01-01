//! RPC protocol definitions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// RPC message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    Request(Request),
    Response(Response),
    ExecutionStart(ExecutionStart),
    ExecutionResult(ExecutionResult),
    CodeRegister(CodeRegister),
    CodeRegisterResponse(CodeRegisterResponse),
    CachedExecutionStart(CachedExecutionStart),
    RegisterFunction(RegisterFunction),
    RegisterMethod(RegisterMethod),
    RegisterResponse(RegisterResponse),
    /// Server-to-client request to execute a remote function
    RemoteFunctionCall(RemoteFunctionCall),
    /// Client-to-server response with function execution result
    RemoteFunctionResult(RemoteFunctionResult),
    /// Server-to-client request to execute a remote method
    RemoteMethodCall(RemoteMethodCall),
    /// Client-to-server response with method execution result
    RemoteMethodResult(RemoteMethodResult),
    /// Client-to-server request to compile source to bytecode
    CompileBytecode(CompileBytecode),
    /// Server-to-client response with compiled bytecode
    CompileBytecodeResponse(CompileBytecodeResponse),
    /// Client-to-server request to execute bytecode
    BytecodeExecutionStart(BytecodeExecutionStart),
}

/// RPC request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub request_id: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
    pub function: String,
    pub args: Vec<serde_json::Value>,
}

/// Message sent by a host to start an execution.
///
/// The `global_variables` object may contain injected values. Any injected object may include a
/// `secret_data` field (host-only metadata), e.g.:
///
/// ```json
/// {
///   "some_object": {
///     "secret_data": { "id": "object_secret_data_identifier" },
///     "public_field": 123
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStart {
    pub request_id: String,
    pub id: String,
    pub context_id: String,
    pub source: String,
    #[serde(default)]
    pub global_variables: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub response_id: String,
    pub id: String,
    pub context_id: String,
    #[serde(flatten)]
    pub result: ResponseResult,
}

/// Message to register code for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeRegister {
    pub request_id: String,
    pub id: String,
    pub context_id: String,
    pub source: String,
}

/// Response after registering code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeRegisterResponse {
    pub response_id: String,
    pub id: String,
    pub code_id: String,
}

/// Message to execute previously cached code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedExecutionStart {
    pub request_id: String,
    pub code_id: String,
    #[serde(default)]
    pub global_variables: serde_json::Value,
}

/// Register a function in a context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterFunction {
    pub request_id: String,
    pub id: String,
    pub context_id: String,
    pub function_name: String,
}

/// Register a method on an object in a context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterMethod {
    pub request_id: String,
    pub id: String,
    pub context_id: String,
    pub object_id: String,
    pub method_name: String,
}

/// Response after registering function/method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterResponse {
    pub response_id: String,
    pub id: String,
    #[serde(flatten)]
    pub result: ResponseResult,
}

/// Server-to-client: request to execute a remote function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteFunctionCall {
    pub request_id: String,
    pub id: String,
    pub context_id: String,
    pub function_name: String,
    pub args: Vec<serde_json::Value>,
}

/// Client-to-server: result of remote function execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteFunctionResult {
    pub response_id: String,
    pub id: String,
    #[serde(flatten)]
    pub result: ResponseResult,
}

/// Server-to-client: request to execute a remote method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteMethodCall {
    pub request_id: String,
    pub id: String,
    pub context_id: String,
    pub object: serde_json::Value,
    pub method_name: String,
    pub args: Vec<serde_json::Value>,
}

/// Client-to-server: result of remote method execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteMethodResult {
    pub response_id: String,
    pub id: String,
    #[serde(flatten)]
    pub result: ResponseResult,
}

/// RPC response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub response_id: String,
    pub id: String,
    #[serde(flatten)]
    pub result: ResponseResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum ResponseResult {
    #[serde(rename = "success")]
    Success { value: serde_json::Value },

    #[serde(rename = "error")]
    Error { message: String },
}

impl Request {
    pub fn new(
        request_id: String,
        id: String,
        function: String,
        args: Vec<serde_json::Value>,
    ) -> Self {
        Self {
            request_id,
            id,
            context_id: None,
            object_id: None,
            function,
            args,
        }
    }

    pub fn in_context(
        request_id: String,
        id: String,
        context_id: String,
        function: String,
        args: Vec<serde_json::Value>,
    ) -> Self {
        Self {
            request_id,
            id,
            context_id: Some(context_id),
            object_id: None,
            function,
            args,
        }
    }

    pub fn method_call(
        request_id: String,
        id: String,
        context_id: String,
        object_id: String,
        method: String,
        args: Vec<serde_json::Value>,
    ) -> Self {
        Self {
            request_id,
            id,
            context_id: Some(context_id),
            object_id: Some(object_id),
            function: method,
            args,
        }
    }
}

impl Response {
    pub fn success(response_id: String, id: String, value: serde_json::Value) -> Self {
        Self {
            response_id,
            id,
            result: ResponseResult::Success { value },
        }
    }

    pub fn error(response_id: String, id: String, message: String) -> Self {
        Self {
            response_id,
            id,
            result: ResponseResult::Error { message },
        }
    }
}

/// Client-to-server: request to compile source code to bytecode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileBytecode {
    pub request_id: String,
    pub id: String,
    pub source: String,
}

/// Server-to-client: compiled bytecode as base64-encoded bincode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileBytecodeResponse {
    pub response_id: String,
    pub id: String,
    /// Base64-encoded bincode serialized AST, or error message prefixed with "error: "
    pub bytecode: String,
}

/// Client-to-server: execute pre-compiled bytecode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BytecodeExecutionStart {
    pub request_id: String,
    pub id: String,
    pub context_id: String,
    /// Base64-encoded bincode serialized AST
    pub bytecode: String,
    #[serde(default)]
    pub global_variables: HashMap<String, serde_json::Value>,
}
