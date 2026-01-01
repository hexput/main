//! Message handler for processing RPC requests and executing scripts

use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};

/// Type alias for pending RPC call responses
type PendingCalls = Arc<RwLock<HashMap<String, oneshot::Sender<Result<JsonValue, String>>>>>;

use super::context_manager::ContextManager;
use crate::rpc::dispatcher::Dispatcher;
use crate::rpc::protocol::{
    BytecodeExecutionStart, CachedExecutionStart, CodeRegister, CodeRegisterResponse,
    CompileBytecode, CompileBytecodeResponse, ExecutionResult, ExecutionStart, Message,
    RegisterFunction, RegisterMethod, RegisterResponse, RemoteFunctionCall, RemoteMethodCall,
    Request, Response, ResponseResult,
};
use crate::runtime::{rpc_handler::RpcHandler, Context as RuntimeContext, Value};
use crate::sandbox::Limits;
use crate::semantic::capabilities::{Capability, CapabilitySet};

/// RPC handler that sends messages to client and waits for responses
pub struct ConnectionRpcHandler {
    message_tx: mpsc::UnboundedSender<Message>,
    pending_calls: PendingCalls,
    request_counter: Arc<std::sync::atomic::AtomicU64>,
}

impl ConnectionRpcHandler {
    pub fn new(message_tx: mpsc::UnboundedSender<Message>, pending_calls: PendingCalls) -> Self {
        Self {
            message_tx,
            pending_calls,
            request_counter: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    fn generate_request_id(&self) -> String {
        let id = self
            .request_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("rpc_{}", id)
    }
}

impl RpcHandler for ConnectionRpcHandler {
    fn call_remote(
        &self,
        function: &str,
        args: Vec<Value>,
    ) -> crate::runtime::error::RuntimeResult<Value> {
        let request_id = self.generate_request_id();
        let id = self.generate_request_id();

        // Convert runtime Values to JSON
        let json_args: Vec<JsonValue> = args.iter().map(runtime_value_to_json).collect();

        // Create the remote function call message
        let message = Message::RemoteFunctionCall(RemoteFunctionCall {
            request_id: request_id.clone(),
            id,
            context_id: "default".to_string(), // Context ID from current execution
            function_name: function.to_string(),
            args: json_args,
        });

        // Create a oneshot channel for the response
        let (tx, rx) = oneshot::channel();

        // Register the pending call
        let pending_calls = self.pending_calls.clone();
        let runtime_handle = tokio::runtime::Handle::current();
        runtime_handle.block_on(async {
            let mut pending = pending_calls.write().await;
            pending.insert(request_id.clone(), tx);
        });

        // Send the message to the client
        if let Err(e) = self.message_tx.send(message) {
            return Err(crate::runtime::error::RuntimeError::General(format!(
                "Failed to send remote call: {}",
                e
            )));
        }

        // Wait for the response
        let result = runtime_handle
            .block_on(async { tokio::time::timeout(std::time::Duration::from_secs(30), rx).await });

        match result {
            Ok(Ok(Ok(json_value))) => json_to_runtime_value(&json_value).map_err(|e| {
                crate::runtime::error::RuntimeError::General(format!(
                    "Response conversion error: {}",
                    e
                ))
            }),
            Ok(Ok(Err(err_msg))) => Err(crate::runtime::error::RuntimeError::General(err_msg)),
            Ok(Err(_)) => Err(crate::runtime::error::RuntimeError::General(
                "Remote call response channel closed".to_string(),
            )),
            Err(_) => Err(crate::runtime::error::RuntimeError::General(
                "Remote call timeout".to_string(),
            )),
        }
    }

    fn call_method(
        &self,
        object: &Value,
        method: &str,
        args: Vec<Value>,
    ) -> crate::runtime::error::RuntimeResult<Value> {
        let request_id = self.generate_request_id();
        let id = self.generate_request_id();

        // Convert runtime Values to JSON
        let json_args: Vec<JsonValue> = args.iter().map(runtime_value_to_json).collect();
        let json_object = runtime_value_to_json(object);

        // Create the remote method call message
        let message = Message::RemoteMethodCall(RemoteMethodCall {
            request_id: request_id.clone(),
            id,
            context_id: "default".to_string(), // Context ID from current execution
            object: json_object,
            method_name: method.to_string(),
            args: json_args,
        });

        // Create a oneshot channel for the response
        let (tx, rx) = oneshot::channel();

        // Register the pending call
        let pending_calls = self.pending_calls.clone();
        let runtime_handle = tokio::runtime::Handle::current();
        runtime_handle.block_on(async {
            let mut pending = pending_calls.write().await;
            pending.insert(request_id.clone(), tx);
        });

        // Send the message to the client
        if let Err(e) = self.message_tx.send(message) {
            return Err(crate::runtime::error::RuntimeError::General(format!(
                "Failed to send remote method call: {}",
                e
            )));
        }

        // Wait for the response
        let result = runtime_handle
            .block_on(async { tokio::time::timeout(std::time::Duration::from_secs(30), rx).await });

        match result {
            Ok(Ok(Ok(json_value))) => json_to_runtime_value(&json_value).map_err(|e| {
                crate::runtime::error::RuntimeError::General(format!(
                    "Response conversion error: {}",
                    e
                ))
            }),
            Ok(Ok(Err(err_msg))) => Err(crate::runtime::error::RuntimeError::General(err_msg)),
            Ok(Err(_)) => Err(crate::runtime::error::RuntimeError::General(
                "Remote method call response channel closed".to_string(),
            )),
            Err(_) => Err(crate::runtime::error::RuntimeError::General(
                "Remote method call timeout".to_string(),
            )),
        }
    }
}

/// Handle incoming message and return response
pub async fn handle_message(
    message: Message,
    manager: &Arc<RwLock<ContextManager>>,
    pending_calls: &crate::server::PendingCalls,
    message_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    println!("DEBUG: Handling incoming message: {:?}", message);
    match message {
        Message::Request(req) => Some(Message::Response(handle_request(req, manager).await)),

        Message::ExecutionStart(exec_start) => Some(Message::ExecutionResult(
            handle_execution_start(exec_start, manager, pending_calls, message_tx).await,
        )),

        Message::CodeRegister(code_reg) => Some(Message::CodeRegisterResponse(
            handle_code_register(code_reg, manager).await,
        )),

        Message::CachedExecutionStart(cached_exec) => Some(Message::ExecutionResult(
            handle_cached_execution_start(cached_exec, manager, pending_calls, message_tx).await,
        )),

        Message::RegisterFunction(reg_fn) => Some(Message::RegisterResponse(
            handle_register_function(reg_fn, manager).await,
        )),

        Message::RegisterMethod(reg_method) => Some(Message::RegisterResponse(
            handle_register_method(reg_method, manager).await,
        )),

        Message::CompileBytecode(compile_req) => Some(Message::CompileBytecodeResponse(
            handle_compile_bytecode(compile_req).await,
        )),

        Message::BytecodeExecutionStart(bytecode_exec) => Some(Message::ExecutionResult(
            handle_bytecode_execution_start(bytecode_exec, manager, pending_calls, message_tx)
                .await,
        )),

        Message::Response(_) => {
            // Client shouldn't send Response, but we'll ignore it
            None
        }

        Message::ExecutionResult(_) => {
            // Client shouldn't send ExecutionResult, ignore it
            None
        }

        Message::CodeRegisterResponse(_) => {
            // Client shouldn't send CodeRegisterResponse, ignore it
            None
        }

        Message::CompileBytecodeResponse(_) => {
            // Client shouldn't send CompileBytecodeResponse, ignore it
            None
        }

        Message::RegisterResponse(_) => {
            // Client shouldn't send RegisterResponse, ignore it
            None
        }

        Message::RemoteFunctionCall(_) => {
            // Client shouldn't send RemoteFunctionCall, ignore it
            None
        }

        Message::RemoteFunctionResult(result) => {
            println!(
                "DEBUG: Handling RemoteFunctionResult for response_id {}",
                result.response_id
            );
            println!("DEBUG: Result content: {:?}", result.result);
            // Handle remote function result by completing pending call
            let mut pending = pending_calls.write().await;
            if let Some(tx) = pending.remove(&result.response_id) {
                let res = match &result.result {
                    ResponseResult::Success { value } => Ok(value.clone()),
                    ResponseResult::Error { message } => Err(message.clone()),
                };
                let _ = tx.send(res);
            }
            None
        }

        Message::RemoteMethodCall(_) => {
            // Client shouldn't send RemoteMethodCall, ignore it
            None
        }

        Message::RemoteMethodResult(result) => {
            // Handle remote method result by completing pending call
            let mut pending = pending_calls.write().await;
            if let Some(tx) = pending.remove(&result.response_id) {
                let res = match &result.result {
                    ResponseResult::Success { value } => Ok(value.clone()),
                    ResponseResult::Error { message } => Err(message.clone()),
                };
                let _ = tx.send(res);
            }
            None
        }
    }
}

/// Handle RPC request
async fn handle_request(req: Request, manager: &Arc<RwLock<ContextManager>>) -> Response {
    let request_id = req.id.clone();

    // Get context registry
    let context_id = req.context_id.as_deref().unwrap_or("default");

    let registry = {
        let mut mgr = manager.write().await;
        mgr.get_or_create_context(context_id).await
    };

    // Create dispatcher
    let registry_read = registry.read().await;
    let dispatcher = Dispatcher::with_registry((*registry_read).clone());
    drop(registry_read);

    // Dispatch the request and extract response
    match dispatcher.dispatch(Message::Request(req)) {
        Ok(Message::Response(mut response)) => {
            response.response_id = request_id;
            response
        }
        Ok(_) => Response {
            response_id: request_id.clone(),
            id: "error".to_string(),
            result: ResponseResult::Error {
                message: "Unexpected message type".to_string(),
            },
        },
        Err(e) => Response {
            response_id: request_id.clone(),
            id: "error".to_string(),
            result: ResponseResult::Error {
                message: format!("Dispatch error: {}", e),
            },
        },
    }
}

/// Handle execution start - parse, inject globals, and execute script
async fn handle_execution_start(
    exec_start: ExecutionStart,
    manager: &Arc<RwLock<ContextManager>>,
    pending_calls: &crate::server::PendingCalls,
    message_tx: &mpsc::UnboundedSender<Message>,
) -> ExecutionResult {
    let request_id = exec_start.request_id.clone();
    let id = exec_start.id.clone();
    let context_id = exec_start.context_id.clone();

    // Parse the script
    let ast = match crate::language::parse(&exec_start.source) {
        Ok(ast) => ast,
        Err(e) => {
            return ExecutionResult {
                response_id: request_id,
                id,
                context_id,
                result: ResponseResult::Error {
                    message: format!("Parse error: {}", e),
                },
            };
        }
    };

    // Get or create context registry
    let registry = {
        let mut mgr = manager.write().await;
        mgr.get_or_create_context(&context_id).await
    };

    // Create capability set from registry's allowed functions/methods
    let capabilities = {
        let reg = registry.read().await;
        let allowed_functions = reg.get_allowed_functions(&context_id);
        let mut cap_set = CapabilitySet::new();
        for func_name in allowed_functions {
            cap_set.grant(Capability::CallRemote(func_name));
        }
        cap_set
    };

    // Create RPC handler
    let rpc_handler = Arc::new(ConnectionRpcHandler::new(
        message_tx.clone(),
        pending_calls.clone(),
    ));

    // Create execution context with RPC handler
    let mut runtime_ctx =
        RuntimeContext::with_rpc_handler(Limits::default(), capabilities, rpc_handler);

    // Inject global variables
    if let Err(e) = inject_globals(&mut runtime_ctx, exec_start.global_variables) {
        return ExecutionResult {
            response_id: request_id,
            id,
            context_id,
            result: ResponseResult::Error {
                message: format!("Global injection error: {}", e),
            },
        };
    }

    // Execute the script in a blocking task to allow WebSocket to continue reading
    println!("DEBUG: About to spawn blocking execution task");
    let exec_result = tokio::task::spawn_blocking(move || {
        println!("DEBUG: Inside blocking task, executing script");
        crate::runtime::execute(&ast, &mut runtime_ctx)
    })
    .await;
    println!("DEBUG: Blocking task completed");

    match exec_result {
        Ok(Ok(value)) => {
            // Convert runtime Value to JSON
            let json_value = runtime_value_to_json(&value);
            ExecutionResult {
                response_id: request_id,
                id,
                context_id,
                result: ResponseResult::Success { value: json_value },
            }
        }
        Ok(Err(e)) => ExecutionResult {
            response_id: request_id,
            id,
            context_id,
            result: ResponseResult::Error {
                message: format!("Runtime error: {}", e),
            },
        },
        Err(e) => ExecutionResult {
            response_id: request_id,
            id,
            context_id,
            result: ResponseResult::Error {
                message: format!("Execution task error: {}", e),
            },
        },
    }
}

/// Handle code registration - parse and cache code
async fn handle_code_register(
    code_reg: CodeRegister,
    manager: &Arc<RwLock<ContextManager>>,
) -> CodeRegisterResponse {
    let request_id = code_reg.request_id.clone();
    let id = code_reg.id.clone();

    // Parse the script
    let ast = match crate::language::parse(&code_reg.source) {
        Ok(ast) => ast,
        Err(e) => {
            // Return error as code_id (could use a better approach)
            return CodeRegisterResponse {
                response_id: request_id,
                id,
                code_id: format!("error: {}", e),
            };
        }
    };

    // Register the parsed code and get generated code_id
    let code_id = {
        let mut mgr = manager.write().await;
        mgr.register_code(ast, code_reg.source)
    };

    CodeRegisterResponse {
        response_id: request_id,
        id,
        code_id,
    }
}

/// Handle cached execution - execute previously registered code
async fn handle_cached_execution_start(
    cached_exec: CachedExecutionStart,
    manager: &Arc<RwLock<ContextManager>>,
    pending_calls: &crate::server::PendingCalls,
    message_tx: &mpsc::UnboundedSender<Message>,
) -> ExecutionResult {
    let request_id = cached_exec.request_id.clone();
    let code_id = cached_exec.code_id.clone();

    // Retrieve cached code
    let cached_code = {
        let mgr = manager.read().await;
        match mgr.get_cached_code(&code_id) {
            Some(code) => code.clone(),
            None => {
                return ExecutionResult {
                    response_id: request_id,
                    id: code_id.clone(),
                    context_id: "unknown".to_string(),
                    result: ResponseResult::Error {
                        message: format!("Code not found: {}", code_id),
                    },
                };
            }
        }
    };

    // For cached execution, we need a context_id from somewhere
    // Since it's not in the message, we'll use "default" or extract from global_variables
    let context_id = if let JsonValue::Object(ref map) = cached_exec.global_variables {
        if let Some(JsonValue::String(ctx_id)) = map.get("__context_id") {
            ctx_id.clone()
        } else {
            "default".to_string()
        }
    } else {
        "default".to_string()
    };

    // Get or create context registry
    let registry = {
        let mut mgr = manager.write().await;
        mgr.get_or_create_context(&context_id).await
    };

    // Create capability set from registry's allowed functions/methods
    let capabilities = {
        let reg = registry.read().await;
        let allowed_functions = reg.get_allowed_functions(&context_id);
        let mut cap_set = CapabilitySet::new();
        for func_name in allowed_functions {
            cap_set.grant(Capability::CallRemote(func_name));
        }
        cap_set
    };

    // Create RPC handler
    let rpc_handler = Arc::new(ConnectionRpcHandler::new(
        message_tx.clone(),
        pending_calls.clone(),
    ));

    // Create execution context with RPC handler
    let mut runtime_ctx =
        RuntimeContext::with_rpc_handler(Limits::default(), capabilities, rpc_handler);

    // Inject global variables
    if let Err(e) = inject_globals(&mut runtime_ctx, cached_exec.global_variables) {
        return ExecutionResult {
            response_id: request_id,
            id: code_id,
            context_id,
            result: ResponseResult::Error {
                message: format!("Global injection error: {}", e),
            },
        };
    }

    // Execute the cached AST in a blocking task to allow WebSocket to continue reading
    let cached_ast = cached_code.ast.clone();
    let exec_result =
        tokio::task::spawn_blocking(move || crate::runtime::execute(&cached_ast, &mut runtime_ctx))
            .await;

    match exec_result {
        Ok(Ok(value)) => {
            let json_value = runtime_value_to_json(&value);
            ExecutionResult {
                response_id: request_id,
                id: code_id,
                context_id,
                result: ResponseResult::Success { value: json_value },
            }
        }
        Ok(Err(e)) => ExecutionResult {
            response_id: request_id,
            id: code_id,
            context_id,
            result: ResponseResult::Error {
                message: format!("Runtime error: {}", e),
            },
        },
        Err(e) => ExecutionResult {
            response_id: request_id,
            id: code_id,
            context_id,
            result: ResponseResult::Error {
                message: format!("Execution task error: {}", e),
            },
        },
    }
}

/// Inject global variables into runtime context
fn inject_globals(ctx: &mut RuntimeContext, globals: JsonValue) -> Result<(), String> {
    if let JsonValue::Object(map) = globals {
        for (key, json_val) in map {
            let runtime_val = json_to_runtime_value(&json_val)?;
            ctx.define(key, runtime_val);
        }
    }
    Ok(())
}

/// Convert JSON value to runtime Value, detecting MethodObject
fn json_to_runtime_value(json: &JsonValue) -> Result<Value, String> {
    match json {
        JsonValue::Null => Ok(Value::Undefined),
        JsonValue::Bool(b) => Ok(Value::Boolean(*b)),
        JsonValue::Number(n) => {
            if let Some(f) = n.as_f64() {
                Ok(Value::Number(f))
            } else {
                Err("Invalid number".to_string())
            }
        }
        JsonValue::String(s) => Ok(Value::String(s.clone())),
        JsonValue::Array(arr) => {
            let values: Result<Vec<Value>, String> =
                arr.iter().map(json_to_runtime_value).collect();
            Ok(Value::Array(values?))
        }
        JsonValue::Object(map) => {
            // Check if this is a MethodObject (has secret_data.id)
            if let Some(JsonValue::Object(secret_data)) = map.get("secret_data") {
                if let Some(JsonValue::String(object_id)) = secret_data.get("id") {
                    // This is a MethodObject - extract fields (excluding secret_data)
                    let mut fields = HashMap::new();
                    for (k, v) in map {
                        if k != "secret_data" {
                            fields.insert(k.clone(), json_to_runtime_value(v)?);
                        }
                    }

                    return Ok(Value::MethodObject {
                        object_id: object_id.clone(),
                        fields,
                    });
                }
            }

            // Regular object
            let mut obj_map = HashMap::new();
            for (k, v) in map {
                obj_map.insert(k.clone(), json_to_runtime_value(v)?);
            }
            Ok(Value::Object(obj_map))
        }
    }
}

/// Convert runtime Value to JSON
fn runtime_value_to_json(value: &Value) -> JsonValue {
    match value {
        Value::Undefined => JsonValue::Null,
        Value::Boolean(b) => JsonValue::Bool(*b),
        Value::Number(n) => JsonValue::Number(
            serde_json::Number::from_f64(*n).unwrap_or(serde_json::Number::from(0)),
        ),
        Value::String(s) => JsonValue::String(s.clone()),
        Value::Array(arr) => JsonValue::Array(arr.iter().map(runtime_value_to_json).collect()),
        Value::Object(map) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in map {
                obj.insert(k.clone(), runtime_value_to_json(v));
            }
            JsonValue::Object(obj)
        }
        Value::MethodObject {
            object_id: _,
            fields,
        } => {
            // Return fields only, not secret_data (security boundary)
            let mut obj = serde_json::Map::new();
            for (k, v) in fields {
                obj.insert(k.clone(), runtime_value_to_json(v));
            }
            JsonValue::Object(obj)
        }
        Value::Callback { .. } => JsonValue::String("<callback>".to_string()),
    }
}

/// Handle function registration
async fn handle_register_function(
    reg_fn: RegisterFunction,
    manager: &Arc<RwLock<ContextManager>>,
) -> RegisterResponse {
    let request_id = reg_fn.request_id.clone();
    let id = reg_fn.id.clone();

    // Get or create context registry
    let registry = {
        let mut mgr = manager.write().await;
        mgr.get_or_create_context(&reg_fn.context_id).await
    };

    // Mark function as allowed in the registry
    {
        let mut reg = registry.write().await;
        reg.allow_function(&reg_fn.context_id, &reg_fn.function_name);
    }

    RegisterResponse {
        response_id: request_id,
        id,
        result: ResponseResult::Success {
            value: serde_json::json!({
                "function": reg_fn.function_name,
                "context_id": reg_fn.context_id,
                "status": "registered"
            }),
        },
    }
}

/// Handle method registration
async fn handle_register_method(
    reg_method: RegisterMethod,
    manager: &Arc<RwLock<ContextManager>>,
) -> RegisterResponse {
    let request_id = reg_method.request_id.clone();
    let id = reg_method.id.clone();

    // Get or create context registry
    let registry = {
        let mut mgr = manager.write().await;
        mgr.get_or_create_context(&reg_method.context_id).await
    };

    // Mark method as allowed in the registry
    {
        let mut reg = registry.write().await;
        reg.allow_method(
            &reg_method.context_id,
            &reg_method.object_id,
            &reg_method.method_name,
        );
    }

    RegisterResponse {
        response_id: request_id,
        id,
        result: ResponseResult::Success {
            value: serde_json::json!({
                "object_id": reg_method.object_id,
                "method": reg_method.method_name,
                "context_id": reg_method.context_id,
                "status": "registered"
            }),
        },
    }
}

/// Handle bytecode compilation request
async fn handle_compile_bytecode(compile_req: CompileBytecode) -> CompileBytecodeResponse {
    let request_id = compile_req.request_id.clone();
    let id = compile_req.id.clone();

    // Parse the script
    let ast = match crate::language::parse(&compile_req.source) {
        Ok(ast) => ast,
        Err(e) => {
            // Return error as bytecode field (prefixed with "error: ")
            return CompileBytecodeResponse {
                response_id: request_id,
                id,
                bytecode: format!("error: {}", e),
            };
        }
    };

    // Serialize AST to bincode
    let bincode_data = match bincode::serialize(&ast) {
        Ok(data) => data,
        Err(e) => {
            return CompileBytecodeResponse {
                response_id: request_id,
                id,
                bytecode: format!("error: Failed to serialize AST: {}", e),
            };
        }
    };

    // Encode to base64
    let base64_data =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bincode_data);

    CompileBytecodeResponse {
        response_id: request_id,
        id,
        bytecode: base64_data,
    }
}

/// Handle bytecode execution start by deserializing and executing pre-compiled AST
async fn handle_bytecode_execution_start(
    bytecode_exec: BytecodeExecutionStart,
    manager: &Arc<RwLock<ContextManager>>,
    pending_calls: &crate::server::PendingCalls,
    message_tx: &mpsc::UnboundedSender<Message>,
) -> ExecutionResult {
    let request_id = bytecode_exec.request_id.clone();
    let id = bytecode_exec.id.clone();
    let context_id = bytecode_exec.context_id.clone();

    // Decode base64
    let bincode_data = match base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &bytecode_exec.bytecode,
    ) {
        Ok(data) => data,
        Err(e) => {
            return ExecutionResult {
                response_id: request_id,
                id,
                context_id,
                result: ResponseResult::Error {
                    message: format!("Failed to decode base64: {}", e),
                },
            };
        }
    };

    // Deserialize AST from bincode
    let ast: crate::language::ast::Ast = match bincode::deserialize(&bincode_data) {
        Ok(ast) => ast,
        Err(e) => {
            return ExecutionResult {
                response_id: request_id,
                id,
                context_id,
                result: ResponseResult::Error {
                    message: format!("Failed to deserialize AST: {}", e),
                },
            };
        }
    };

    // Get or create context registry
    let registry = {
        let mut mgr = manager.write().await;
        mgr.get_or_create_context(&context_id).await
    };

    // Create capability set from registry's allowed functions/methods
    let capabilities = {
        let reg = registry.read().await;
        let allowed_functions = reg.get_allowed_functions(&context_id);
        let mut cap_set = CapabilitySet::new();
        for func_name in allowed_functions {
            cap_set.grant(Capability::CallRemote(func_name));
        }
        cap_set
    };

    // Create RPC handler
    let rpc_handler = Arc::new(ConnectionRpcHandler::new(
        message_tx.clone(),
        pending_calls.clone(),
    ));

    // Create execution context with RPC handler
    let mut runtime_ctx =
        RuntimeContext::with_rpc_handler(Limits::default(), capabilities, rpc_handler);

    // Inject global variables
    let globals_json = JsonValue::Object(bytecode_exec.global_variables.into_iter().collect());
    if let Err(e) = inject_globals(&mut runtime_ctx, globals_json) {
        return ExecutionResult {
            response_id: request_id,
            id,
            context_id,
            result: ResponseResult::Error {
                message: format!("Global injection error: {}", e),
            },
        };
    }

    // Execute the AST in a blocking task
    let exec_result =
        tokio::task::spawn_blocking(move || crate::runtime::execute(&ast, &mut runtime_ctx)).await;

    match exec_result {
        Ok(Ok(value)) => {
            // Convert runtime Value to JSON
            let json_value = runtime_value_to_json(&value);
            ExecutionResult {
                response_id: request_id,
                id,
                context_id,
                result: ResponseResult::Success { value: json_value },
            }
        }
        Ok(Err(e)) => ExecutionResult {
            response_id: request_id,
            id,
            context_id,
            result: ResponseResult::Error {
                message: format!("{}", e),
            },
        },
        Err(e) => ExecutionResult {
            response_id: request_id,
            id,
            context_id,
            result: ResponseResult::Error {
                message: format!("Execution panic: {}", e),
            },
        },
    }
}
