use crate::error::RuntimeError;
use crate::messages::{
    CallbackFunction, ExecutionResult, FunctionCallRequest, FunctionCallResponse,
    FunctionExistsRequest, FunctionExistsResponse, WebSocketMessage, WebSocketRequest,
    WebSocketResponse,
};
use crate::builtins;
use hexput_ast_api::ast_structs::{Statement, UnaryOperator};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::oneshot;
use tokio::time::{timeout, Duration};
use tracing::{debug, error, warn};
use uuid::Uuid;

type PendingFunctionCalls = Arc<Mutex<HashMap<String, oneshot::Sender<FunctionCallResponse>>>>;
type PendingFunctionValidations =
    Arc<Mutex<HashMap<String, oneshot::Sender<FunctionExistsResponse>>>>;

const FORBIDDEN_KEY: &str = "secret_data";

struct ExecutionContext {
    variables: HashMap<String, serde_json::Value>,
    callbacks: HashMap<String, CallbackFunction>,
    parent: Option<Box<ExecutionContext>>,
}

impl ExecutionContext {
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
            callbacks: HashMap::new(),
            parent: None,
        }
    }

    fn with_parent(parent: &ExecutionContext) -> Self {
        Self {
            variables: HashMap::new(),
            callbacks: parent.callbacks.clone(),
            parent: Some(Box::new(parent.clone())),
        }
    }

    fn get_variable(&self, name: &str) -> Option<&serde_json::Value> {
        if let Some(value) = self.variables.get(name) {
            return Some(value);
        }

        if let Some(parent) = &self.parent {
            return parent.get_variable(name);
        }

        None
    }

    fn set_variable(&mut self, name: String, value: serde_json::Value) {
        self.variables.insert(name, value);
    }

    fn get_callback(&self, name: &str) -> Option<&CallbackFunction> {
        self.callbacks.get(name)
    }

    fn add_callback(&mut self, callback: CallbackFunction) {
        self.callbacks.insert(callback.name.clone(), callback);
    }

    fn clone(&self) -> Self {
        Self {
            variables: self.variables.clone(),
            callbacks: self.callbacks.clone(),
            parent: self.parent.as_ref().map(|p| Box::new((**p).clone())),
        }
    }
}

pub async fn handle_message(
    message_data: &str,
    function_calls: PendingFunctionCalls,
    function_validations: PendingFunctionValidations,
    send_message: impl Fn(String) -> futures_util::future::BoxFuture<'static, Result<(), RuntimeError>>,
) -> Result<String, RuntimeError> {
    let message: WebSocketMessage = serde_json::from_str(message_data).map_err(|e| {
        RuntimeError::InvalidRequestFormat(format!("Failed to parse message: {}", e))
    })?;

    match message {
        WebSocketMessage::Request(request) => {
            handle_request(request, function_calls, function_validations, send_message).await
        }
        WebSocketMessage::FunctionResponse(response) => {
            handle_function_response_message(response, function_calls).await?;
            Ok("".to_string())
        }
        WebSocketMessage::FunctionExistsResponse(response) => {
            handle_function_exists_response(response, function_validations).await?;
            Ok("".to_string())
        }
        WebSocketMessage::Unknown(value) => Err(RuntimeError::InvalidRequestFormat(format!(
            "Unknown message format: {}",
            value
        ))),
    }
}

pub async fn handle_request(
    request: WebSocketRequest,
    function_calls: PendingFunctionCalls,
    function_validations: PendingFunctionValidations,
    send_message: impl Fn(String) -> futures_util::future::BoxFuture<'static, Result<(), RuntimeError>>,
) -> Result<String, RuntimeError> {
    debug!("Received request with ID: {}", request.id);
    debug!("Action: {}", request.action);

    match request.action.as_str() {
        "parse" => handle_parse_request(request).await,
        "execute" => {
            handle_execute_request(request, function_calls, function_validations, send_message)
                .await
        }
        "function_response" => Err(RuntimeError::InvalidRequestFormat(
            "Function responses should be processed directly, not through the action field"
                .to_string(),
        )),
        _ => {
            let response = WebSocketResponse {
                id: request.id,
                success: false,
                result: None,
                error: Some(format!("Unknown action: {}", request.action)),
            };
            Ok(serde_json::to_string(&response)?)
        }
    }
}

async fn handle_function_response_message(
    response: FunctionCallResponse,
    function_calls: PendingFunctionCalls,
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
        error!(
            "Received function response for unknown call ID: {}",
            response.id
        );
    }

    Ok(())
}

async fn handle_function_exists_response(
    response: FunctionExistsResponse,
    function_validations: PendingFunctionValidations,
) -> Result<(), RuntimeError> {
    debug!(
        "Processing function exists response for ID: {}",
        response.id
    );

    let sender = {
        let mut validations = function_validations.lock().unwrap();
        validations.remove(&response.id)
    };

    if let Some(sender) = sender {
        if sender.send(response).is_err() {
            error!(
                "Failed to send function exists response through channel - receiver likely dropped"
            );
        } else {
            debug!("Successfully sent function exists response through channel");
        }
    } else {
        error!(
            "Received function exists response for unknown ID: {}",
            response.id
        );
    }

    Ok(())
}

pub async fn handle_function_response(
    request_data: &str,
    function_calls: PendingFunctionCalls,
) -> Result<(), RuntimeError> {
    let response: FunctionCallResponse = serde_json::from_str(request_data)
        .map_err(|e| RuntimeError::InvalidRequestFormat(e.to_string()))?;

    handle_function_response_message(response, function_calls).await
}

async fn handle_parse_request(request: WebSocketRequest) -> Result<String, RuntimeError> {
    let code = request.code.clone();
    let options = request.options.clone();
    let id = request.id.clone();

    let start_time = Instant::now();
    
    let process_result = tokio::task::spawn_blocking(move || {
        let feature_flags = options.to_feature_flags();

        match hexput_ast_api::process_code(&code, feature_flags) {
            Ok(program) => {
                let result = if options.minify {
                    hexput_ast_api::to_json_string(&program, options.include_source_mapping)
                } else {
                    hexput_ast_api::to_json_string_pretty(&program, options.include_source_mapping)
                };

                match result {
                    Ok(json_str) => {
                        match serde_json::from_str::<Value>(&json_str) {
                            Ok(value) => Ok::<(bool, Option<Value>, Option<String>), RuntimeError>(
                                (true, Some(value), None),
                            ),
                            Err(e) => Ok::<(bool, Option<Value>, Option<String>), RuntimeError>((
                                false,
                                None,
                                Some(format!("Error deserializing JSON: {}", e)),
                            )),
                        }
                    }
                    Err(e) => Ok::<(bool, Option<Value>, Option<String>), RuntimeError>((
                        false,
                        None,
                        Some(format!("Error serializing AST: {}", e)),
                    )),
                }
            }
            Err(e) => Ok::<(bool, Option<Value>, Option<String>), RuntimeError>((
                false,
                None,
                Some(format!("Error parsing AST: {}", e)),
            )),
        }
    })
    .await
    .map_err(|e| RuntimeError::ExecutionError(format!("Task join error: {}", e)))?;

    let elapsed_time = start_time.elapsed();
    debug!("AST parsing completed in {:.2?}", elapsed_time);

    let (success, result, error) = process_result?;

    let response = WebSocketResponse {
        id,
        success,
        result,
        error,
    };

    Ok(serde_json::to_string(&response)?)
}

async fn handle_execute_request(
    request: WebSocketRequest,
    function_calls: PendingFunctionCalls,
    function_validations: PendingFunctionValidations,
    send_message: impl Fn(String) -> futures_util::future::BoxFuture<'static, Result<(), RuntimeError>>,
) -> Result<String, RuntimeError> {
    let code = request.code.clone();
    let options = request.options.clone();
    let id = request.id.clone();
    let context_variables = request.context.clone();
    let secret_context = request.secret_context.clone();

    let parse_start_time = Instant::now();
    
    let program_result = tokio::task::spawn_blocking(move || {
        let feature_flags = options.to_feature_flags();
        hexput_ast_api::process_code(&code, feature_flags)
    })
    .await
    .map_err(|e| RuntimeError::ExecutionError(format!("Task join error: {}", e)))?;

    let parse_elapsed = parse_start_time.elapsed();
    debug!("AST parsing for execution completed in {:.2?}", parse_elapsed);

    let program = match program_result {
        Ok(p) => p,
        Err(e) => {
            let response = WebSocketResponse {
                id,
                success: false,
                result: None,
                error: Some(format!("Error parsing AST: {}", e)),
            };
            return Ok(serde_json::to_string(&response)?);
        }
    };

    let exec_start_time = Instant::now();
    
    let execution_result =
        execute_program(program, context_variables, secret_context, function_calls, function_validations, send_message).await;
    
    let exec_elapsed = exec_start_time.elapsed();
    debug!("Program execution completed in {:.2?}", exec_elapsed);

    let error_message = match &execution_result.error {
        Some(error_text) => {
            if error_text.contains("line") && error_text.contains("column") {
                Some(error_text.clone())
            } else {
                Some(error_text.clone())
            }
        }
        _ => None,
    };

    let response = WebSocketResponse {
        id,
        success: execution_result.error.is_none(),
        result: Some(execution_result.value),
        error: error_message,
    };

    Ok(serde_json::to_string(&response)?)
}

async fn execute_program(
    program: hexput_ast_api::ast_structs::Program,
    context_variables: serde_json::Map<String, serde_json::Value>,
    secret_context: Option<serde_json::Value>,
    function_calls: PendingFunctionCalls,
    function_validations: PendingFunctionValidations,
    send_message: impl Fn(String) -> futures_util::future::BoxFuture<'static, Result<(), RuntimeError>>,
) -> ExecutionResult {
    let mut context = ExecutionContext::new();
    
    for (name, value) in context_variables {
        context.set_variable(name, value);
    }

    for statement in program.statements {
        match Box::pin(execute_statement(
            statement,
            &mut context,
            secret_context.as_ref(),
            function_calls.clone(),
            function_validations.clone(),
            &send_message,
        ))
        .await
        {
            Ok(Some(value)) => {
                if let Some(control_type) = get_control_flow_type(&value) {
                    if control_type == CONTROL_RETURN {
                        let return_value = extract_return_value(value);
                        debug!("Returning value from program due to return statement");
                        return ExecutionResult {
                            value: return_value,
                            error: None,
                        };
                    }

                    debug!(
                        "Ignoring control flow signal at program level: {}",
                        control_type
                    );
                } else {
                    debug!("Unexpected value return at program level");
                    return ExecutionResult { value, error: None };
                }
            }
            Ok(None) => {}
            Err(e) => {
                return ExecutionResult {
                    value: serde_json::Value::Null,
                    error: Some(e.to_string()),
                }
            }
        }
    }

    ExecutionResult {
        value: serde_json::Value::Null,
        error: None,
    }
}

const CONTROL_TYPE_KEY: &str = "__control_type";
const CONTROL_CONTINUE: &str = "continue";
const CONTROL_END: &str = "end";
const CONTROL_RETURN: &str = "return";

fn get_control_flow_type(value: &serde_json::Value) -> Option<&str> {
    if let serde_json::Value::Object(map) = value {
        if let Some(serde_json::Value::String(control_type)) = map.get(CONTROL_TYPE_KEY) {
            return Some(control_type);
        }
    }
    None
}

fn extract_return_value(value: serde_json::Value) -> serde_json::Value {
    if let serde_json::Value::Object(map) = &value {
        if map.contains_key(CONTROL_TYPE_KEY) && map.contains_key("value") {
            if let Some(return_value) = map.get("value") {
                return return_value.clone();
            }
        }
    }
    value
}

async fn execute_statement(
    statement: hexput_ast_api::ast_structs::Statement,
    context: &mut ExecutionContext,
    secret_context: Option<&serde_json::Value>,
    function_calls: PendingFunctionCalls,
    function_validations: PendingFunctionValidations,
    send_message: &impl Fn(String) -> futures_util::future::BoxFuture<'static, Result<(), RuntimeError>>,
) -> Result<Option<serde_json::Value>, RuntimeError> {
    let location = match &statement {
        Statement::VariableDeclaration { location, .. } => location.clone(),
        Statement::ExpressionStatement { location, .. } => location.clone(),
        Statement::IfStatement { location, .. } => location.clone(),
        Statement::Block { location, .. } => location.clone(),
        Statement::LoopStatement { location, .. } => location.clone(),
        Statement::CallbackDeclaration { location, .. } => location.clone(),
        Statement::ReturnStatement { location, .. } => location.clone(),
        Statement::EndStatement { location } => location.clone(),
        Statement::ContinueStatement { location } => location.clone(),
    };

    match statement {
        Statement::VariableDeclaration { name, value, .. } => {
            let value_result = match Box::pin(evaluate_expression(
                value,
                context,
                secret_context,
                function_calls,
                function_validations,
                send_message,
            ))
            .await
            {
                Ok(val) => val,
                Err(e) => return Err(add_location_if_needed(e, &location)),
            };
            context.set_variable(name, value_result);
            Ok(None)
        }
        Statement::ExpressionStatement { expression, .. } => {
            match Box::pin(evaluate_expression(
                expression,
                context,
                secret_context,
                function_calls,
                function_validations,
                send_message,
            ))
            .await
            {
                Ok(_) => Ok(None),
                Err(e) => Err(add_location_if_needed(e, &location)),
            }
        }
        Statement::IfStatement {
            condition,
            body,
            else_body,
            ..
        } => {
            let condition_value = match Box::pin(evaluate_expression(
                condition,
                context,
                secret_context,
                function_calls.clone(),
                function_validations.clone(),
                send_message,
            ))
            .await
            {
                Ok(val) => val,
                Err(e) => return Err(add_location_if_needed(e, &location)),
            };

            let is_truthy = match condition_value {
                serde_json::Value::Bool(b) => b,
                serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
                serde_json::Value::String(s) => !s.is_empty(),
                serde_json::Value::Array(a) => !a.is_empty(),
                serde_json::Value::Object(o) => !o.is_empty(),
                serde_json::Value::Null => false,
            };

            if is_truthy {
                match execute_block(
                    body,
                    context,
                    secret_context,
                    function_calls,
                    function_validations,
                    send_message,
                )
                .await
                {
                    Ok(result) => Ok(result),
                    Err(e) => Err(add_location_if_needed(e, &location)),
                }
            } else if let Some(else_block) = else_body {
                match execute_block(
                    else_block,
                    context,
                    secret_context,
                    function_calls,
                    function_validations,
                    send_message,
                )
                .await
                {
                    Ok(result) => Ok(result),
                    Err(e) => Err(add_location_if_needed(e, &location)),
                }
            } else {
                Ok(None)
            }
        }
        Statement::Block { block, .. } => {
            execute_block(
                block,
                context,
                secret_context,
                function_calls,
                function_validations,
                send_message,
            )
            .await
        }
        Statement::LoopStatement {
            variable,
            iterable,
            body,
            ..
        } => {
            let iterable_value = match Box::pin(evaluate_expression(
                iterable,
                context,
                secret_context,
                function_calls.clone(),
                function_validations.clone(),
                send_message,
            ))
            .await
            {
                Ok(val) => val,
                Err(e) => return Err(add_location_if_needed(e, &location)),
            };

            match iterable_value {
                serde_json::Value::Array(items) => {
                    for item in items {
                        context.set_variable(variable.clone(), item);

                        match execute_block(
                            body.clone(),
                            context,
                            secret_context,
                            function_calls.clone(),
                            function_validations.clone(),
                            send_message,
                        )
                        .await
                        {
                            Ok(result) => {
                                if let Some(value) = result {
                                    if let Some(control_type) = get_control_flow_type(&value) {
                                        match control_type {
                                            CONTROL_CONTINUE => {
                                                continue;
                                            }
                                            CONTROL_END => {
                                                break;
                                            }
                                            CONTROL_RETURN => {
                                                return Ok(Some(value));
                                            }
                                            _ => {
                                                return Ok(Some(value));
                                            }
                                        }
                                    } else {
                                        return Ok(Some(value));
                                    }
                                }
                            }
                            Err(e) => return Err(add_location_if_needed(e, &location)),
                        }
                    }
                }
                serde_json::Value::String(s) => {
                    for ch in s.chars() {
                        let char_value = serde_json::Value::String(ch.to_string());
                        context.set_variable(variable.clone(), char_value);

                        let result = execute_block(
                            body.clone(),
                            context,
                            secret_context,
                            function_calls.clone(),
                            function_validations.clone(),
                            send_message,
                        )
                        .await?;

                        if let Some(value) = result {
                            if let Some(control_type) = get_control_flow_type(&value) {
                                match control_type {
                                    CONTROL_CONTINUE => {
                                        continue;
                                    }
                                    CONTROL_END => {
                                        break;
                                    }
                                    CONTROL_RETURN => {
                                        return Ok(Some(value));
                                    }
                                    _ => {
                                        return Ok(Some(value));
                                    }
                                }
                            } else {
                                return Ok(Some(value));
                            }
                        }
                    }
                }
                _ => {
                    return Err(RuntimeError::with_location(
                        format!(
                            "Cannot iterate over value of type: {}",
                            match iterable_value {
                                serde_json::Value::Null => "null",
                                serde_json::Value::Bool(_) => "boolean",
                                serde_json::Value::Number(_) => "number",
                                serde_json::Value::Object(_) => "object",
                                _ => "unknown",
                            }
                        ),
                        location,
                    ))
                }
            }

            Ok(None)
        }
        Statement::CallbackDeclaration {
            name, params, body, ..
        } => {
            let callback = CallbackFunction {
                name: name.clone(),
                params,
                body,
            };
            context.add_callback(callback);
            debug!("Registered callback function: {}", name);
            Ok(None)
        }
        Statement::ReturnStatement { value, .. } => {
            let return_value = Box::pin(evaluate_expression(
                value,
                context,
                secret_context,
                function_calls,
                function_validations,
                send_message,
            ))
            .await?;
            debug!("Processing return statement with value: {:?}", return_value);

            let control_signal = serde_json::json!({
                CONTROL_TYPE_KEY: CONTROL_RETURN,
                "value": return_value
            });

            Ok(Some(control_signal))
        }
        Statement::EndStatement { .. } => {
            debug!("Processing end statement (break)");
            let control_signal = serde_json::json!({
                CONTROL_TYPE_KEY: CONTROL_END
            });

            Ok(Some(control_signal))
        }
        Statement::ContinueStatement { .. } => {
            debug!("Processing continue statement");
            let control_signal = serde_json::json!({
                CONTROL_TYPE_KEY: CONTROL_CONTINUE
            });

            Ok(Some(control_signal))
        }
    }
}

fn add_location_if_needed(
    error: RuntimeError,
    location: &hexput_ast_api::ast_structs::SourceLocation,
) -> RuntimeError {
    match error {
        RuntimeError::ExecutionErrorWithLocation { .. } => error,

        _ => RuntimeError::with_location(error.to_string(), location.clone()),
    }
}

async fn execute_block(
    block: hexput_ast_api::ast_structs::Block,
    context: &mut ExecutionContext,
    secret_context: Option<&serde_json::Value>,
    function_calls: PendingFunctionCalls,
    function_validations: PendingFunctionValidations,
    send_message: &impl Fn(String) -> futures_util::future::BoxFuture<'static, Result<(), RuntimeError>>,
) -> Result<Option<serde_json::Value>, RuntimeError> {
    for statement in block.statements {
        let statement_future = Box::pin(execute_statement(
            statement,
            context,
            secret_context,
            function_calls.clone(),
            function_validations.clone(),
            send_message,
        ));

        let result = statement_future.await?;

        if let Some(value) = result {
            debug!("Propagating control flow or return value from block");
            return Ok(Some(value));
        }
    }

    Ok(None)
}

async fn extract_property_path(
    expression: &hexput_ast_api::ast_structs::Expression,
    context: &mut ExecutionContext,
    secret_context: Option<&serde_json::Value>,
    function_calls: &PendingFunctionCalls,
    function_validations: &PendingFunctionValidations,
    send_message: &impl Fn(String) -> futures_util::future::BoxFuture<'static, Result<(), RuntimeError>>,
) -> Result<Vec<String>, RuntimeError> {
    use hexput_ast_api::ast_structs::Expression;

    let mut path = Vec::new();
    let mut current_expr = expression;

    loop {
        match current_expr {
            Expression::Identifier { name, .. } => {
                path.push(name.clone());
                break;
            }
            Expression::MemberExpression {
                object,
                property,
                property_expr,
                computed,
                ..
            } => {
                if !computed {
                    if let Some(prop) = property {
                        path.push(prop.clone());
                    } else {
                        return Err(RuntimeError::ExecutionError(
                            "Missing property name".to_string(),
                        ));
                    }
                } else if let Some(prop_expr) = property_expr {
                    let prop_value = Box::pin(evaluate_expression(
                        (**prop_expr).clone(),
                        context,
                        secret_context,
                        function_calls.clone(),
                        function_validations.clone(),
                        send_message,
                    ))
                    .await?;

                    let prop_name = match prop_value {
                        serde_json::Value::String(s) => s,
                        serde_json::Value::Number(n) => {
                            if n.is_i64() {
                                n.as_i64().unwrap().to_string()
                            } else if n.is_u64() {
                                n.as_u64().unwrap().to_string()
                            } else {
                                n.as_f64().unwrap_or(0.0).to_string()
                            }
                        }
                        _ => {
                            return Err(RuntimeError::ExecutionError(
                                "Computed property must evaluate to a string or number".to_string(),
                            ))
                        }
                    };

                    path.push(prop_name);
                } else {
                    return Err(RuntimeError::ExecutionError(
                        "Either property or property_expr must be provided".to_string(),
                    ));
                }

                current_expr = object;
            }
            _ => {
                return Err(RuntimeError::ExecutionError(
                    "Unsupported expression type in property path".to_string(),
                ));
            }
        }
    }

    path.reverse();
    Ok(path)
}

fn update_nested_object(
    object: &mut serde_json::Value,
    path: &[String],
    path_index: usize,
    value: serde_json::Value,
) -> Result<(), RuntimeError> {
    if path_index >= path.len() {
        return Err(RuntimeError::ExecutionError(
            "Empty property path".to_string(),
        ));
    }

    let current_prop = &path[path_index];

    if current_prop == FORBIDDEN_KEY {
        return Err(RuntimeError::ExecutionError(format!(
            "Access to the key '{}' is forbidden.",
            FORBIDDEN_KEY
        )));
    }

    let is_array_index = current_prop.parse::<usize>().is_ok();

    if path_index == path.len() - 1 {
        match object {
            serde_json::Value::Object(map) => {
                map.insert(current_prop.clone(), value);
                Ok(())
            }
            serde_json::Value::Array(arr) if is_array_index => {
                let index = current_prop.parse::<usize>().unwrap();

                while arr.len() <= index {
                    arr.push(serde_json::Value::Null);
                }

                arr[index] = value;
                Ok(())
            }
            _ => Err(RuntimeError::ExecutionError(format!(
                "Cannot set index or property '{}' on value of type: {}",
                current_prop,
                match object {
                    serde_json::Value::Array(_) => "array (non-numeric index)",
                    serde_json::Value::String(_) => "string",
                    serde_json::Value::Number(_) => "number",
                    serde_json::Value::Bool(_) => "boolean",
                    serde_json::Value::Null => "null",
                    _ => "unknown",
                }
            ))),
        }
    } else {
        match object {
            serde_json::Value::Object(map) => {
                let next_obj = if is_array_index {
                    map.entry(current_prop.clone())
                        .or_insert_with(|| serde_json::Value::Array(Vec::new()))
                } else {
                    map.entry(current_prop.clone())
                        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
                };

                update_nested_object(next_obj, path, path_index + 1, value)
            }
            serde_json::Value::Array(arr) if is_array_index => {
                let index = current_prop.parse::<usize>().unwrap();

                while arr.len() <= index {
                    arr.push(serde_json::Value::Null);
                }

                let next_is_numeric =
                    path_index + 1 < path.len() && path[path_index + 1].parse::<usize>().is_ok();

                if arr[index].is_null() {
                    if next_is_numeric {
                        arr[index] = serde_json::Value::Array(Vec::new());
                    } else {
                        arr[index] = serde_json::Value::Object(serde_json::Map::new());
                    }
                }

                update_nested_object(&mut arr[index], path, path_index + 1, value)
            }
            _ => Err(RuntimeError::ExecutionError(format!(
                "Cannot access index or property '{}' on non-object/non-array value",
                current_prop
            ))),
        }
    }
}

async fn evaluate_expression(
    expression: hexput_ast_api::ast_structs::Expression,
    context: &mut ExecutionContext,
    secret_context: Option<&serde_json::Value>,
    function_calls: PendingFunctionCalls,
    function_validations: PendingFunctionValidations,
    send_message: &impl Fn(String) -> futures_util::future::BoxFuture<'static, Result<(), RuntimeError>>,
) -> Result<serde_json::Value, RuntimeError> {
    use hexput_ast_api::ast_structs::{Expression, Operator};

    let location = match &expression {
        Expression::StringLiteral { location, .. } => location.clone(),
        Expression::NumberLiteral { location, .. } => location.clone(),
        Expression::Identifier { location, .. } => location.clone(),
        Expression::BinaryExpression { location, .. } => location.clone(),
        Expression::AssignmentExpression { location, .. } => location.clone(),
        Expression::MemberAssignmentExpression { location, .. } => location.clone(),
        Expression::CallExpression { location, .. } => location.clone(),
        Expression::MemberCallExpression { location, .. } => location.clone(),
        Expression::CallbackReference { location, .. } => location.clone(),
        Expression::ArrayExpression { location, .. } => location.clone(),
        Expression::ObjectExpression { location, .. } => location.clone(),
        Expression::MemberExpression { location, .. } => location.clone(),
        Expression::KeysOfExpression { location, .. } => location.clone(),
        Expression::BooleanLiteral { location, .. } => location.clone(),
        Expression::UnaryExpression { location, .. } => location.clone(),
        Expression::NullLiteral { location } => location.clone(),
    };

    match expression {
        Expression::StringLiteral { value, .. } => Ok(serde_json::Value::String(value)),
        Expression::NumberLiteral { value, .. } => Ok(serde_json::Value::Number(
            serde_json::Number::from_f64(value).unwrap_or(serde_json::Number::from(0)),
        )),
        Expression::Identifier { name, .. } => {
            context.get_variable(&name).cloned().ok_or_else(|| {
                RuntimeError::with_location(format!("Undefined variable: {}", name), location)
            })
        }
        Expression::CallExpression {
            callee, arguments, ..
        } => {
            if let Some(callback) = context.get_callback(&callee).cloned() {
                debug!("Executing local callback: {}", callee);
                match execute_callback(
                    callback,
                    arguments,
                    context,
                    secret_context,
                    function_calls,
                    function_validations,
                    send_message,
                )
                .await
                {
                    Ok(val) => Ok(val),
                    Err(e) => Err(add_location_if_needed(e, &location)),
                }
            } else {
                debug!("Checking if remote function exists: {}", callee);

                let check_id = Uuid::new_v4().to_string();

                let (tx, rx) = oneshot::channel::<FunctionExistsResponse>();

                {
                    let mut validations = function_validations.lock().unwrap();
                    validations.insert(check_id.clone(), tx);
                }

                let exists_request = FunctionExistsRequest {
                    id: check_id.clone(),
                    action: "is_function_exists".to_string(),
                    function_name: callee.clone(),
                };

                let request_json = match serde_json::to_string(&exists_request) {
                    Ok(json) => json,
                    Err(e) => {
                        return Err(RuntimeError::with_location(
                            format!("Serialization error: {}", e),
                            location,
                        ))
                    }
                };

                match send_message(request_json).await {
                    Ok(_) => {}
                    Err(e) => return Err(add_location_if_needed(e, &location)),
                }

                let function_exists = match timeout(Duration::from_secs(3), rx).await {
                    Ok(response_result) => match response_result {
                        Ok(response) => response.exists,
                        Err(_) => {
                            debug!(
                                "Function exists check response channel closed for '{}'",
                                callee
                            );
                            false
                        }
                    },
                    Err(_) => {
                        {
                            let mut validations = function_validations.lock().unwrap();
                            validations.remove(&check_id);
                        }
                        debug!("Function exists check timed out for '{}'", callee);
                        false
                    }
                };

                if function_exists {
                    debug!("Remote function '{}' exists, proceeding with call", callee);

                    let call_id = Uuid::new_v4().to_string();

                    let mut evaluated_args = Vec::new();
                    for arg in arguments {
                        match Box::pin(evaluate_expression(
                            arg,
                            context,
                            secret_context,
                            function_calls.clone(),
                            function_validations.clone(),
                            send_message,
                        ))
                        .await
                        {
                            Ok(value) => evaluated_args.push(value),
                            Err(e) => return Err(add_location_if_needed(e, &location)),
                        }
                    }

                    let (tx, rx) = oneshot::channel::<FunctionCallResponse>();

                    {
                        let mut calls = function_calls.lock().unwrap();
                        calls.insert(call_id.clone(), tx);
                    }

                    let request = FunctionCallRequest {
                        id: call_id.clone(),
                        function_name: callee.clone(),
                        arguments: evaluated_args,
                        secret_context: secret_context.cloned(),
                    };

                    let request_json = match serde_json::to_string(&request) {
                        Ok(json) => json,
                        Err(e) => {
                            return Err(RuntimeError::with_location(
                                format!("Serialization error: {}", e),
                                location,
                            ))
                        }
                    };

                    match send_message(request_json).await {
                        Ok(_) => {}
                        Err(e) => return Err(add_location_if_needed(e, &location)),
                    }

                    match timeout(Duration::from_secs(600), rx).await {
                        Ok(response_result) => match response_result {
                            Ok(response) => {
                                if let Some(err) = response.error {
                                    Err(RuntimeError::with_location(
                                        format!("Remote function error: {}", err),
                                        location,
                                    ))
                                } else {
                                    Ok(response.result)
                                }
                            },
                            Err(_) => Err(RuntimeError::with_location(
                                "Function call response channel closed".to_string(),
                                location,
                            )),
                        },
                        Err(_) => {
                            {
                                let mut calls = function_calls.lock().unwrap();
                                calls.remove(&call_id);
                            }

                            warn!("Function call '{}' timed out after 60 seconds", callee);
                            Err(RuntimeError::with_location(
                                format!("Function call '{}' timed out", callee),
                                location,
                            ))
                        }
                    }
                } else {
                    warn!("Remote function '{}' does not exist", callee);
                    Err(RuntimeError::FunctionNotFoundError(format!(
                        "Function '{}' not found",
                        callee
                    )))
                }
            }
        }
        Expression::BinaryExpression {
            left,
            operator,
            right,
            ..
        } => {
            let left_value = match Box::pin(evaluate_expression(
                *left,
                context,
                secret_context,
                function_calls.clone(),
                function_validations.clone(),
                send_message,
            ))
            .await
            {
                Ok(val) => val,
                Err(e) => return Err(add_location_if_needed(e, &location)),
            };

            match operator {
                Operator::And => {
                    let is_left_truthy = match &left_value {
                        serde_json::Value::Bool(b) => *b,
                        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
                        serde_json::Value::String(s) => !s.is_empty(),
                        serde_json::Value::Array(a) => !a.is_empty(),
                        serde_json::Value::Object(o) => !o.is_empty(),
                        serde_json::Value::Null => false,
                    };

                    if !is_left_truthy {
                        return Ok(serde_json::Value::Bool(false));
                    }

                    let right_value = match Box::pin(evaluate_expression(
                        *right,
                        context,
                        secret_context,
                        function_calls.clone(),
                        function_validations.clone(),
                        send_message,
                    ))
                    .await
                    {
                        Ok(val) => val,
                        Err(e) => return Err(add_location_if_needed(e, &location)),
                    };

                    let is_right_truthy = match &right_value {
                        serde_json::Value::Bool(b) => *b,
                        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
                        serde_json::Value::String(s) => !s.is_empty(),
                        serde_json::Value::Array(a) => !a.is_empty(),
                        serde_json::Value::Object(o) => !o.is_empty(),
                        serde_json::Value::Null => false,
                    };

                    Ok(serde_json::Value::Bool(is_right_truthy))
                }
                Operator::Or => {
                    let is_left_truthy = match &left_value {
                        serde_json::Value::Bool(b) => *b,
                        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
                        serde_json::Value::String(s) => !s.is_empty(),
                        serde_json::Value::Array(a) => !a.is_empty(),
                        serde_json::Value::Object(o) => !o.is_empty(),
                        serde_json::Value::Null => false,
                    };

                    if is_left_truthy {
                        return Ok(serde_json::Value::Bool(true));
                    }

                    let right_value = match Box::pin(evaluate_expression(
                        *right,
                        context,
                        secret_context,
                        function_calls.clone(),
                        function_validations.clone(),
                        send_message,
                    ))
                    .await
                    {
                        Ok(val) => val,
                        Err(e) => return Err(add_location_if_needed(e, &location)),
                    };

                    let is_right_truthy = match &right_value {
                        serde_json::Value::Bool(b) => *b,
                        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
                        serde_json::Value::String(s) => !s.is_empty(),
                        serde_json::Value::Array(a) => !a.is_empty(),
                        serde_json::Value::Object(o) => !o.is_empty(),
                        serde_json::Value::Null => false,
                    };

                    Ok(serde_json::Value::Bool(is_right_truthy))
                }
                _ => {
                    let right_value = match Box::pin(evaluate_expression(
                        *right,
                        context,
                        secret_context,
                        function_calls.clone(),
                        function_validations.clone(),
                        send_message,
                    ))
                    .await
                    {
                        Ok(val) => val,
                        Err(e) => return Err(add_location_if_needed(e, &location)),
                    };

                    match operator {
                        Operator::Plus => match (left_value, right_value) {
                            (serde_json::Value::Number(l), serde_json::Value::Number(r)) => {
                                let result = l.as_f64().unwrap_or(0.0) + r.as_f64().unwrap_or(0.0);
                                Ok(serde_json::Value::Number(
                                    serde_json::Number::from_f64(result)
                                        .unwrap_or(serde_json::Number::from(0)),
                                ))
                            }

                            (serde_json::Value::String(l), serde_json::Value::String(r)) => {
                                Ok(serde_json::Value::String(l + &r))
                            }

                            (serde_json::Value::String(l), serde_json::Value::Number(r)) => {
                                let r_str = if r.is_i64() {
                                    r.as_i64().unwrap().to_string()
                                } else if r.is_u64() {
                                    r.as_u64().unwrap().to_string()
                                } else {
                                    r.as_f64().unwrap_or(0.0).to_string()
                                };
                                Ok(serde_json::Value::String(l + &r_str))
                            }

                            (serde_json::Value::Number(l), serde_json::Value::String(r)) => {
                                let l_str = if l.is_i64() {
                                    l.as_i64().unwrap().to_string()
                                } else if l.is_u64() {
                                    l.as_u64().unwrap().to_string()
                                } else {
                                    l.as_f64().unwrap_or(0.0).to_string()
                                };
                                Ok(serde_json::Value::String(l_str + &r))
                            }

                            _ => Err(RuntimeError::with_location(
                                "Invalid operand types for addition".to_string(),
                                location,
                            )),
                        },

                        Operator::Equal => match (&left_value, &right_value) {
                            (serde_json::Value::Null, serde_json::Value::Null) => {
                                Ok(serde_json::Value::Bool(true))
                            }
                            (serde_json::Value::Bool(l), serde_json::Value::Bool(r)) => {
                                Ok(serde_json::Value::Bool(l == r))
                            }
                            (serde_json::Value::Number(l), serde_json::Value::Number(r)) => {
                                let l_f64 = l.as_f64().unwrap_or(0.0);
                                let r_f64 = r.as_f64().unwrap_or(0.0);
                                Ok(serde_json::Value::Bool(
                                    (l_f64 - r_f64).abs() < f64::EPSILON,
                                ))
                            }
                            (serde_json::Value::String(l), serde_json::Value::String(r)) => {
                                Ok(serde_json::Value::Bool(l == r))
                            }

                            _ => Ok(serde_json::Value::Bool(false)),
                        },

                        Operator::NotEqual => match (&left_value, &right_value) {
                            (serde_json::Value::Null, serde_json::Value::Null) => {
                                Ok(serde_json::Value::Bool(false))
                            }
                            (serde_json::Value::Bool(l), serde_json::Value::Bool(r)) => {
                                Ok(serde_json::Value::Bool(l != r))
                            }
                            (serde_json::Value::Number(l), serde_json::Value::Number(r)) => {
                                let l_f64 = l.as_f64().unwrap_or(0.0);
                                let r_f64 = r.as_f64().unwrap_or(0.0);
                                Ok(serde_json::Value::Bool(
                                    (l_f64 - r_f64).abs() >= f64::EPSILON,
                                ))
                            }
                            (serde_json::Value::String(l), serde_json::Value::String(r)) => {
                                Ok(serde_json::Value::Bool(l != r))
                            }

                            _ => Ok(serde_json::Value::Bool(true)),
                        },

                        Operator::Minus => match (&left_value, &right_value) {
                            (serde_json::Value::Number(l), serde_json::Value::Number(r)) => {
                                let result = l.as_f64().unwrap_or(0.0) - r.as_f64().unwrap_or(0.0);
                                Ok(serde_json::Value::Number(
                                    serde_json::Number::from_f64(result)
                                        .unwrap_or(serde_json::Number::from(0)),
                                ))
                            }
                            _ => Err(RuntimeError::with_location(
                                "Invalid operand types for subtraction".to_string(),
                                location,
                            )),
                        },

                        Operator::Less => match (&left_value, &right_value) {
                            (serde_json::Value::Number(l), serde_json::Value::Number(r)) => {
                                let l_f64 = l.as_f64().unwrap_or(0.0);
                                let r_f64 = r.as_f64().unwrap_or(0.0);
                                Ok(serde_json::Value::Bool(l_f64 < r_f64))
                            }
                            (serde_json::Value::String(l), serde_json::Value::String(r)) => {
                                Ok(serde_json::Value::Bool(l < r))
                            }
                            _ => Err(RuntimeError::with_location(
                                "Invalid operand types for less than comparison".to_string(),
                                location,
                            )),
                        },

                        Operator::Greater => match (&left_value, &right_value) {
                            (serde_json::Value::Number(l), serde_json::Value::Number(r)) => {
                                let l_f64 = l.as_f64().unwrap_or(0.0);
                                let r_f64 = r.as_f64().unwrap_or(0.0);
                                Ok(serde_json::Value::Bool(l_f64 > r_f64))
                            }
                            (serde_json::Value::String(l), serde_json::Value::String(r)) => {
                                Ok(serde_json::Value::Bool(l > r))
                            }
                            _ => Err(RuntimeError::with_location(
                                "Invalid operand types for greater than comparison".to_string(),
                                location,
                            )),
                        },

                        Operator::GreaterEqual => match (&left_value, &right_value) {
                            (serde_json::Value::Number(l), serde_json::Value::Number(r)) => {
                                let l_f64 = l.as_f64().unwrap_or(0.0);
                                let r_f64 = r.as_f64().unwrap_or(0.0);
                                Ok(serde_json::Value::Bool(l_f64 >= r_f64))
                            }
                            (serde_json::Value::String(l), serde_json::Value::String(r)) => {
                                Ok(serde_json::Value::Bool(l >= r))
                            }
                            _ => Err(RuntimeError::with_location(
                                "Invalid operand types for greater than or equal comparison".to_string(),
                                location,
                            )),
                        },

                        Operator::LessEqual => match (&left_value, &right_value) {
                            (serde_json::Value::Number(l), serde_json::Value::Number(r)) => {
                                let l_f64 = l.as_f64().unwrap_or(0.0);
                                let r_f64 = r.as_f64().unwrap_or(0.0);
                                Ok(serde_json::Value::Bool(l_f64 <= r_f64))
                            }
                            (serde_json::Value::String(l), serde_json::Value::String(r)) => {
                                Ok(serde_json::Value::Bool(l <= r))
                            }
                            _ => Err(RuntimeError::with_location(
                                "Invalid operand types for less than or equal comparison".to_string(),
                                location,
                            )),
                        },

                        Operator::Multiply => match (&left_value, &right_value) {
                            (serde_json::Value::Number(l), serde_json::Value::Number(r)) => {
                                let result = l.as_f64().unwrap_or(0.0) * r.as_f64().unwrap_or(0.0);
                                Ok(serde_json::Value::Number(
                                    serde_json::Number::from_f64(result)
                                        .unwrap_or(serde_json::Number::from(0)),
                                ))
                            }
                            _ => Err(RuntimeError::with_location(
                                "Invalid operand types for multiplication".to_string(),
                                location,
                            )),
                        },

                        Operator::Divide => match (&left_value, &right_value) {
                            (serde_json::Value::Number(l), serde_json::Value::Number(r)) => {
                                let r_f64 = r.as_f64().unwrap_or(0.0);
                                if r_f64 == 0.0 {
                                    return Err(RuntimeError::with_location(
                                        "Division by zero".to_string(),
                                        location,
                                    ));
                                }
                                let result = l.as_f64().unwrap_or(0.0) / r_f64;
                                Ok(serde_json::Value::Number(
                                    serde_json::Number::from_f64(result)
                                        .unwrap_or(serde_json::Number::from(0)),
                                ))
                            }
                            _ => Err(RuntimeError::with_location(
                                "Invalid operand types for division".to_string(),
                                location,
                            )),
                        },
                        
                        // Add these patterns to handle And and Or operators
                        // These should never be reached as they're handled in the outer match
                        Operator::And => unreachable!("And operator should be handled in the outer match"),
                        Operator::Or => unreachable!("Or operator should be handled in the outer match"),
                    }
                }
            }
        }
        Expression::UnaryExpression {
            operator, operand, ..
        } => {
            let operand_value = match Box::pin(evaluate_expression(
                *operand,
                context,
                secret_context,
                function_calls,
                function_validations,
                send_message,
            ))
            .await
            {
                Ok(val) => val,
                Err(e) => return Err(add_location_if_needed(e, &location)),
            };

            match operator {
                UnaryOperator::Not => {
                    let is_truthy = match &operand_value {
                        serde_json::Value::Bool(b) => *b,
                        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
                        serde_json::Value::String(s) => !s.is_empty(),
                        serde_json::Value::Array(_) => true,
                        serde_json::Value::Object(_) => true,
                        serde_json::Value::Null => false,
                    };

                    Ok(serde_json::Value::Bool(!is_truthy))
                }
            }
        }
        Expression::MemberExpression {
            object,
            property,
            property_expr,
            computed,
            ..
        } => {
            let obj_value = match Box::pin(evaluate_expression(
                *object,
                context,
                secret_context,
                function_calls.clone(),
                function_validations.clone(),
                send_message,
            ))
            .await
            {
                Ok(val) => val,
                Err(e) => return Err(add_location_if_needed(e, &location)),
            };

            if !computed {
                if let Some(prop) = property {
                    if prop == FORBIDDEN_KEY {
                        return Err(RuntimeError::with_location(
                            format!("Access to the key '{}' is forbidden.", FORBIDDEN_KEY),
                            location,
                        ));
                    }
                    match &obj_value {
                        serde_json::Value::Object(map) => {
                            Ok(map.get(&prop).cloned().unwrap_or(serde_json::Value::Null))
                        }

                        serde_json::Value::Array(arr) => {
                            if let Ok(index) = prop.parse::<usize>() {
                                Ok(arr.get(index).cloned().unwrap_or(serde_json::Value::Null))
                            } else {
                                Err(RuntimeError::with_location(
                                    "Array index must be a number".to_string(),
                                    location,
                                ))
                            }
                        }
                        _ => Err(RuntimeError::with_location(
                            "Cannot access property of non-object/non-array".to_string(),
                            location,
                        )),
                    }
                } else {
                    Err(RuntimeError::with_location(
                        "Missing property name".to_string(),
                        location,
                    ))
                }
            } else if let Some(prop_expr) = property_expr {
                let prop_value = match Box::pin(evaluate_expression(
                    *prop_expr,
                    context,
                    secret_context,
                    function_calls.clone(),
                    function_validations.clone(),
                    send_message,
                ))
                .await
                {
                    Ok(val) => val,
                    Err(e) => return Err(add_location_if_needed(e, &location)),
                };

                match prop_value {
                    serde_json::Value::String(s) => {
                        if s == FORBIDDEN_KEY {
                            return Err(RuntimeError::with_location(
                                format!("Access to the key '{}' is forbidden.", FORBIDDEN_KEY),
                                location,
                            ));
                        }
                        match &obj_value {
                            serde_json::Value::Object(map) => {
                                Ok(map.get(&s).cloned().unwrap_or(serde_json::Value::Null))
                            }
                            serde_json::Value::Array(arr) => {
                                if let Ok(index) = s.parse::<usize>() {
                                    Ok(arr.get(index).cloned().unwrap_or(serde_json::Value::Null))
                                } else {
                                    Err(RuntimeError::with_location(
                                        format!("Invalid array index: {}", s),
                                        location,
                                    ))
                                }
                            }
                            _ => Err(RuntimeError::with_location(
                                "Cannot access property of non-object/non-array".to_string(),
                                location,
                            )),
                        }
                    },
                    serde_json::Value::Number(n) => {
                        let key_str = if n.is_i64() {
                            n.as_i64().unwrap().to_string()
                        } else if n.is_u64() {
                            n.as_u64().unwrap().to_string()
                        } else {
                            n.as_f64().unwrap_or(0.0).to_string()
                        };

                        if key_str == FORBIDDEN_KEY {
                             return Err(RuntimeError::with_location(
                                format!("Access to the key '{}' is forbidden.", FORBIDDEN_KEY),
                                location,
                            ));
                        }

                        match &obj_value {
                            serde_json::Value::Array(arr) => {
                                let index = if n.is_u64() {
                                    n.as_u64().unwrap() as usize
                                } else if n.is_i64() {
                                    let i = n.as_i64().unwrap();
                                    if i < 0 {
                                        return Err(RuntimeError::with_location(
                                            "Array index cannot be negative".to_string(),
                                            location,
                                        ));
                                    }
                                    i as usize
                                } else {
                                    let f = n.as_f64().unwrap();
                                    if f < 0.0 || !f.is_finite() {
                                        return Err(RuntimeError::with_location(
                                            "Array index must be a non-negative finite number"
                                                .to_string(),
                                            location,
                                        ));
                                    }
                                    f as usize
                                };

                                Ok(arr.get(index).cloned().unwrap_or(serde_json::Value::Null))
                            }
                            serde_json::Value::Object(map) => {
                                Ok(map.get(&key_str).cloned().unwrap_or(serde_json::Value::Null))
                            }
                            _ => Err(RuntimeError::with_location(
                                "Cannot access property of non-object/non-array".to_string(),
                                location,
                            )),
                        }
                    },
                    _ => Err(RuntimeError::with_location(
                        "Computed property must evaluate to a string or number".to_string(),
                        location,
                    )),
                }
            } else {
                Err(RuntimeError::with_location(
                    "Either property or property_expr must be provided".to_string(),
                    location,
                ))
            }
        }
        Expression::ArrayExpression { elements, .. } => {
            let mut evaluated_elements = Vec::new();

            for element in elements {
                let value = match Box::pin(evaluate_expression(
                    element,
                    context,
                    secret_context,
                    function_calls.clone(),
                    function_validations.clone(),
                    send_message,
                ))
                .await
                {
                    Ok(val) => val,
                    Err(e) => return Err(add_location_if_needed(e, &location)),
                };

                evaluated_elements.push(value);
            }

            Ok(serde_json::Value::Array(evaluated_elements))
        }
        Expression::ObjectExpression { properties, .. } => {
            let mut obj = serde_json::Map::new();

            for property in properties {
                let value = match Box::pin(evaluate_expression(
                    property.value,
                    context,
                    secret_context,
                    function_calls.clone(),
                    function_validations.clone(),
                    send_message,
                ))
                .await
                {
                    Ok(val) => val,
                    Err(e) => return Err(add_location_if_needed(e, &location)),
                };

                obj.insert(property.key, value);
            }

            Ok(serde_json::Value::Object(obj))
        }
        Expression::MemberCallExpression {
            object,
            property,
            property_expr,
            computed,
            arguments,
            ..
        } => {
            let obj = match Box::pin(evaluate_expression(
                *object,
                context,
                secret_context,
                function_calls.clone(),
                function_validations.clone(),
                send_message,
            ))
            .await
            {
                Ok(val) => val,
                Err(e) => return Err(add_location_if_needed(e, &location)),
            };

            let method_name = if !computed {
                if let Some(prop) = property {
                    prop
                } else {
                    return Err(RuntimeError::with_location(
                        "Missing property name for method call".to_string(),
                        location,
                    ));
                }
            } else if let Some(prop_expr) = property_expr {
                let prop_value = match Box::pin(evaluate_expression(
                    *prop_expr,
                    context,
                    secret_context,
                    function_calls.clone(),
                    function_validations.clone(),
                    send_message,
                ))
                .await
                {
                    Ok(val) => val,
                    Err(e) => return Err(add_location_if_needed(e, &location)),
                };

                match prop_value {
                    serde_json::Value::String(s) => s,
                    _ => {
                        return Err(RuntimeError::with_location(
                            "Computed property must evaluate to a string".to_string(),
                            location,
                        ))
                    }
                }
            } else {
                return Err(RuntimeError::with_location(
                    "Either property or property_expr must be provided".to_string(),
                    location,
                ));
            };

            let mut evaluated_args = Vec::new();
            for arg in arguments {
                let value = match Box::pin(evaluate_expression(
                    arg,
                    context,
                    secret_context,
                    function_calls.clone(),
                    function_validations.clone(),
                    send_message,
                ))
                .await
                {
                    Ok(val) => val,
                    Err(e) => return Err(add_location_if_needed(e, &location)),
                };
                evaluated_args.push(value);
            }

            match builtins::execute_builtin_method(&obj, &method_name, &evaluated_args, &location) {
                Ok(Some(result)) => {
                    debug!("Executed built-in method: {}.{}", type_name(&obj), method_name);
                    return Ok(result);
                },
                Ok(None) => {
                    debug!("No built-in method found for {}.{}, checking if remote method exists", type_name(&obj), method_name);
                },
                Err(e) => {
                    return Err(e);
                }
            }

            let check_id = Uuid::new_v4().to_string();
                
            let (tx, rx) = oneshot::channel::<FunctionExistsResponse>();
            
            {
                let mut validations = function_validations.lock().unwrap();
                validations.insert(check_id.clone(), tx);
            }
            
            let exists_request = FunctionExistsRequest {
                id: check_id.clone(),
                action: "is_function_exists".to_string(),
                function_name: method_name.clone(),
            };
            
            let request_json = match serde_json::to_string(&exists_request) {
                Ok(json) => json,
                Err(e) => {
                    return Err(RuntimeError::with_location(
                        format!("Serialization error: {}", e),
                        location,
                    ))
                }
            };
            
            match send_message(request_json).await {
                Ok(_) => {},
                Err(e) => return Err(add_location_if_needed(e, &location)),
            }
            
            let function_exists = match timeout(Duration::from_secs(3), rx).await {
                Ok(response_result) => match response_result {
                    Ok(response) => response.exists,
                    Err(_) => {
                        debug!(
                            "Function exists check response channel closed for '{}'",
                            method_name
                        );
                        false
                    }
                },
                Err(_) => {
                    {
                        let mut validations = function_validations.lock().unwrap();
                        validations.remove(&check_id);
                    }
                    debug!("Function exists check timed out for '{}'", method_name);
                    false
                }
            };
            
            if !function_exists {
                warn!("Remote method '{}' does not exist", method_name);
                return Err(RuntimeError::FunctionNotFoundError(format!(
                    "Method '{}' not found",
                    method_name
                )));
            }
            
            debug!("Remote method '{}' exists, proceeding with call", method_name);

            let mut call_args = vec![obj];
            call_args.extend(evaluated_args);

            let call_id = Uuid::new_v4().to_string();

            let (tx, rx) = oneshot::channel::<FunctionCallResponse>();

            {
                let mut calls = function_calls.lock().unwrap();
                calls.insert(call_id.clone(), tx);
            }

            let request = FunctionCallRequest {
                id: call_id.clone(),
                function_name: method_name.clone(),
                arguments: call_args,
                secret_context: secret_context.cloned(),
            };

            let request_json = match serde_json::to_string(&request) {
                Ok(json) => json,
                Err(e) => {
                    return Err(RuntimeError::with_location(
                        format!("Serialization error: {}", e),
                        location,
                    ))
                }
            };

            match send_message(request_json).await {
                Ok(_) => {}
                Err(e) => return Err(add_location_if_needed(e, &location)),
            }

            match timeout(Duration::from_secs(600), rx).await {
                Ok(response_result) => match response_result {
                    Ok(response) => {
                        if let Some(err) = response.error {
                            Err(RuntimeError::with_location(
                                format!("Remote method error: {}", err),
                                location,
                            ))
                        } else {
                            Ok(response.result)
                        }
                    },
                    Err(_) => Err(RuntimeError::with_location(
                        "Function call response channel closed".to_string(),
                        location,
                    )),
                },
                Err(_) => {
                    {
                        let mut calls = function_calls.lock().unwrap();
                        calls.remove(&call_id);
                    }

                    warn!("Method call '{}' timed out after 60 seconds", method_name);
                    Err(RuntimeError::with_location(
                        format!("Method call '{}' timed out", method_name),
                        location,
                    ))
                }
            }
        }
        Expression::AssignmentExpression { target, value, .. } => {
            let evaluated_value = match Box::pin(evaluate_expression(
                *value,
                context,
                secret_context,
                function_calls.clone(),
                function_validations.clone(),
                send_message,
            ))
            .await
            {
                Ok(val) => val,
                Err(e) => return Err(add_location_if_needed(e, &location)),
            };

            context.set_variable(target, evaluated_value.clone());

            Ok(evaluated_value)
        }
        Expression::MemberAssignmentExpression {
            object,
            property,
            property_expr,
            computed,
            value,
            ..
        } => {
            let value_to_assign = match Box::pin(evaluate_expression(
                *value,
                context,
                secret_context,
                function_calls.clone(),
                function_validations.clone(),
                send_message,
            ))
            .await
            {
                Ok(val) => val,
                Err(e) => return Err(add_location_if_needed(e, &location)),
            };

            let final_prop_name = if !computed {
                if let Some(prop) = property {
                    prop
                } else {
                    return Err(RuntimeError::with_location(
                        "Missing property name for assignment".to_string(),
                        location,
                    ));
                }
            } else if let Some(prop_expr) = property_expr {
                let prop_value = match Box::pin(evaluate_expression(
                    *prop_expr,
                    context,
                    secret_context,
                    function_calls.clone(),
                    function_validations.clone(),
                    send_message,
                ))
                .await
                {
                    Ok(val) => val,
                    Err(e) => return Err(add_location_if_needed(e, &location)),
                };

                match prop_value {
                    serde_json::Value::String(s) => s,
                    serde_json::Value::Number(n) => {
                        if n.is_i64() {
                            n.as_i64().unwrap().to_string()
                        } else if n.is_u64() {
                            n.as_u64().unwrap().to_string()
                        } else {
                            n.as_f64().unwrap_or(0.0).to_string()
                        }
                    }
                    _ => {
                        return Err(RuntimeError::with_location(
                            "Computed property must evaluate to a string or number".to_string(),
                            location,
                        ))
                    }
                }
            } else {
                return Err(RuntimeError::with_location(
                    "Either property or property_expr must be provided".to_string(),
                    location,
                ));
            };

            if final_prop_name == FORBIDDEN_KEY {
                return Err(RuntimeError::with_location(
                    format!("Assignment to the key '{}' is forbidden.", FORBIDDEN_KEY),
                    location,
                ));
            }

            match *object {
                Expression::Identifier { name, .. } => {
                    let mut obj_value = context
                        .get_variable(&name)
                        .cloned()
                        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

                    let is_array_index = final_prop_name.parse::<usize>().is_ok();

                    if is_array_index && obj_value.is_array() {
                        let index = final_prop_name.parse::<usize>().unwrap();
                        if let serde_json::Value::Array(ref mut arr) = obj_value {
                            while arr.len() <= index {
                                arr.push(serde_json::Value::Null);
                            }

                            arr[index] = value_to_assign.clone();
                        }

                        context.set_variable(name, obj_value);
                    } else if let serde_json::Value::Object(ref mut map) = obj_value {
                        map.insert(final_prop_name, value_to_assign.clone());

                        context.set_variable(name, obj_value);
                    } else {
                        return Err(RuntimeError::with_location(
                            format!(
                                "Cannot set property '{}' on value of type: {}",
                                final_prop_name,
                                if obj_value.is_array() {
                                    "array (non-numeric index)"
                                } else {
                                    "non-object"
                                }
                            ),
                            location,
                        ));
                    }
                }

                Expression::MemberExpression { .. } => {
                    let property_path = match Box::pin(extract_property_path(
                        &*object,
                        context,
                        secret_context,
                        &function_calls,
                        &function_validations,
                        send_message,
                    ))
                    .await
                    {
                        Ok(val) => val,
                        Err(e) => return Err(add_location_if_needed(e, &location)),
                    };

                    let root_name = &property_path[0];

                    let is_next_numeric =
                        property_path.len() > 1 && property_path[1].parse::<usize>().is_ok();

                    let mut root_value =
                        context.get_variable(root_name).cloned().unwrap_or_else(|| {
                            if is_next_numeric {
                                serde_json::Value::Array(Vec::new())
                            } else {
                                serde_json::Value::Object(serde_json::Map::new())
                            }
                        });

                    let mut full_path = property_path.clone();
                    full_path.push(final_prop_name);

                    match update_nested_object(
                        &mut root_value,
                        &full_path,
                        1,
                        value_to_assign.clone(),
                    ) {
                        Ok(_) => {}
                        Err(e) => return Err(add_location_if_needed(e, &location)),
                    };

                    context.set_variable(root_name.clone(), root_value);
                }

                _ => {
                    let mut obj_value = match Box::pin(evaluate_expression(
                        *object,
                        context,
                        secret_context,
                        function_calls.clone(),
                        function_validations.clone(),
                        send_message,
                    ))
                    .await
                    {
                        Ok(val) => val,
                        Err(e) => return Err(add_location_if_needed(e, &location)),
                    };

                    let is_array_index = final_prop_name.parse::<usize>().is_ok();

                    if is_array_index && obj_value.is_array() {
                        let index = final_prop_name.parse::<usize>().unwrap();
                        if let serde_json::Value::Array(ref mut arr) = obj_value {
                            while arr.len() <= index {
                                arr.push(serde_json::Value::Null);
                            }

                            arr[index] = value_to_assign.clone();
                        }
                    } else if let serde_json::Value::Object(ref mut map) = obj_value {
                        map.insert(final_prop_name, value_to_assign.clone());
                    } else {
                        return Err(RuntimeError::with_location(
                            format!(
                                "Cannot set property on non-object/non-array or invalid index type"
                            ),
                            location,
                        ));
                    }
                }
            }

            Ok(value_to_assign)
        }
        Expression::KeysOfExpression { object, .. } => {
            let obj_value = match Box::pin(evaluate_expression(
                *object,
                context,
                secret_context,
                function_calls.clone(),
                function_validations.clone(),
                send_message,
            ))
            .await
            {
                Ok(val) => val,
                Err(e) => return Err(add_location_if_needed(e, &location)),
            };

            match obj_value {
                serde_json::Value::Object(map) => {
                    let keys: Vec<serde_json::Value> = map
                        .keys()
                        .filter(|k| k != &FORBIDDEN_KEY)
                        .map(|k| serde_json::Value::String(k.clone()))
                        .collect();
                    Ok(serde_json::Value::Array(keys))
                }
                _ => Err(RuntimeError::with_location(
                    "keysOf can only be applied to objects".to_string(),
                    location,
                )),
            }
        }
        Expression::CallbackReference { name, .. } => {
            if context.get_callback(&name).is_some() {
                Ok(serde_json::json!({
                    "type": "callback_reference",
                    "name": name
                }))
            } else {
                Err(RuntimeError::with_location(
                    format!("Referenced callback '{}' is not defined", name),
                    location,
                ))
            }
        }
        Expression::BooleanLiteral { value, .. } => Ok(serde_json::Value::Bool(value)),

        Expression::NullLiteral { .. } => Ok(serde_json::Value::Null),
    }
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::String(_) => "String",
        Value::Number(_) => "Number",
        Value::Bool(_) => "Boolean",
        Value::Array(_) => "Array",
        Value::Object(_) => "Object",
        Value::Null => "Null",
    }
}

async fn execute_callback(
    callback: CallbackFunction,
    arguments: Vec<hexput_ast_api::ast_structs::Expression>,
    parent_context: &mut ExecutionContext,
    secret_context: Option<&serde_json::Value>,
    function_calls: PendingFunctionCalls,
    function_validations: PendingFunctionValidations,
    send_message: &impl Fn(String) -> futures_util::future::BoxFuture<'static, Result<(), RuntimeError>>,
) -> Result<serde_json::Value, RuntimeError> {
    let mut callback_context = ExecutionContext::with_parent(parent_context);

    if arguments.len() < callback.params.len() {
        return Err(RuntimeError::ExecutionError(format!(
            "Callback '{}' requires {} arguments, but {} were provided",
            callback.name,
            callback.params.len(),
            arguments.len()
        )));
    }

    for (i, param_name) in callback.params.into_iter().enumerate() {
        if i < arguments.len() {
            let arg_value = Box::pin(evaluate_expression(
                arguments[i].clone(),
                parent_context,
                secret_context,
                function_calls.clone(),
                function_validations.clone(),
                send_message,
            ))
            .await?;

            callback_context.set_variable(param_name, arg_value);
        }
    }

    let result = execute_block(
        callback.body,
        &mut callback_context,
        secret_context,
        function_calls,
        function_validations,
        send_message,
    )
    .await?;

    let return_value = match result {
        Some(value) => {
            if let Some(control_type) = get_control_flow_type(&value) {
                if control_type == CONTROL_RETURN {
                    extract_return_value(value)
                } else {
                    serde_json::Value::Null
                }
            } else {
                value
            }
        }
        None => serde_json::Value::Null,
    };

    debug!(
        "Callback '{}' execution complete, return value: {:?}",
        callback.name, return_value
    );
    Ok(return_value)
}
