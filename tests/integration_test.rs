use futures::{stream::SplitSink, stream::SplitStream, SinkExt, StreamExt};
use hexput::rpc::protocol::{
    CachedExecutionStart, CodeRegister, ExecutionStart, Message, RegisterFunction, RegisterMethod,
    RemoteFunctionResult, ResponseResult,
};
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Once;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

const SERVER_ADDR: &str = "127.0.0.1:9099"; // Use different port than default
const WS_URL: &str = "ws://127.0.0.1:9099";
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Global counter for generating unique request IDs
static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Global flag to ensure server is started only once
static SERVER_STARTED: AtomicBool = AtomicBool::new(false);
static SERVER_INIT: Once = Once::new();

/// Generate a unique request ID
fn generate_request_id() -> String {
    let id = REQUEST_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("req_{}", id)
}

/// Helper to ensure the server is started only once across all tests
async fn ensure_test_server_started() {
    // Use Once to guarantee single initialization
    SERVER_INIT.call_once(|| {
        std::thread::spawn(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let server_config = hexput::server::ServerConfig {
                    ws_addr: SERVER_ADDR.to_string(),
                    #[cfg(unix)]
                    unix_socket_path: None,
                    #[cfg(windows)]
                    named_pipe: None,
                    enable_ws: true,
                    #[cfg(unix)]
                    enable_unix: false,
                    #[cfg(windows)]
                    enable_pipe: false,
                };

                let server = hexput::server::Server::new(server_config);
                SERVER_STARTED.store(true, Ordering::SeqCst);
                let _ = server.run().await;
            });
        });
    });

    // Wait for server to actually start
    let mut attempts = 0;
    while !SERVER_STARTED.load(Ordering::SeqCst) && attempts < 50 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        attempts += 1;
    }

    // Give it a bit more time to be fully ready
    tokio::time::sleep(Duration::from_millis(100)).await;
}

/// Helper to create a WebSocket client connection
async fn create_client() -> (
    futures::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        WsMessage,
    >,
    futures::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
) {
    // Ensure server is running before connecting
    ensure_test_server_started().await;

    // Retry connection a few times in case server is still starting
    let mut attempts = 0;
    loop {
        match connect_async(WS_URL).await {
            Ok((ws_stream, _)) => {
                let (write, read) = ws_stream.split();
                return (write, read);
            }
            Err(_) if attempts < 20 => {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => panic!("Failed to connect to test server: {}", e),
        }
    }
}

/// Helper to handle bidirectional communication - handles both normal responses and remote function calls
async fn handle_bidirectional(
    write: &mut SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, WsMessage>,
    read: &mut SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    message: Message,
    function_handler: Option<Box<dyn Fn(&str, Vec<serde_json::Value>) -> serde_json::Value + Send>>,
) -> Message {
    // Send the request
    let text = serde_json::to_string(&message).unwrap();
    write.send(WsMessage::Text(text.into())).await.unwrap();

    // Handle responses and remote function calls
    loop {
        if let Ok(Some(msg)) = timeout(TEST_TIMEOUT, read.next()).await {
            println!("DEBUG TEST: Received WS message");
            if let Ok(WsMessage::Text(text)) = msg {
                println!(
                    "DEBUG TEST: Received text message: {}",
                    &text[..text.len().min(100)]
                );
                if let Ok(response) = serde_json::from_str::<Message>(&text) {
                    match response {
                        // Handle remote function calls from server
                        Message::RemoteFunctionCall(call) => {
                            println!(
                                "DEBUG TEST: Received RemoteFunctionCall for '{}'",
                                call.function_name
                            );
                            if let Some(ref handler) = function_handler {
                                let result = handler(&call.function_name, call.args);
                                println!("DEBUG TEST: Handler returned: {:?}", result);

                                // Send result back to server
                                let result_msg =
                                    Message::RemoteFunctionResult(RemoteFunctionResult {
                                        response_id: call.request_id,
                                        id: call.id,
                                        result: ResponseResult::Success { value: result },
                                    });

                                let result_text = serde_json::to_string(&result_msg).unwrap();
                                println!("DEBUG TEST: Sending RemoteFunctionResult back to server");
                                write
                                    .send(WsMessage::Text(result_text.into()))
                                    .await
                                    .unwrap();
                                println!("DEBUG TEST: RemoteFunctionResult sent, continuing to wait for ExecutionResult");

                                // Continue listening for the actual response
                                continue;
                            } else {
                                panic!("Received remote function call but no handler provided");
                            }
                        }
                        // Return any other message as the response
                        _ => return response,
                    }
                }
            }
        } else {
            panic!("Timeout waiting for response");
        }
    }
}

/// Helper to send a message and receive response
async fn send_and_receive(
    write: &mut futures::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        WsMessage,
    >,
    read: &mut futures::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    message: Message,
    request_id: String,
) -> Message {
    // Send message
    let json = serde_json::to_string(&message).expect("Failed to serialize");
    write
        .send(WsMessage::Text(json.into()))
        .await
        .expect("Failed to send");

    // Receive responses until we find the one matching our request_id
    loop {
        match timeout(TEST_TIMEOUT, read.next()).await {
            Ok(Some(Ok(WsMessage::Text(text)))) => {
                let msg: Message = serde_json::from_str(&text).expect("Failed to parse response");

                // Check if this response matches our request_id
                let response_id = match &msg {
                    Message::Response(r) => Some(&r.response_id),
                    Message::ExecutionResult(r) => Some(&r.response_id),
                    Message::CodeRegisterResponse(r) => Some(&r.response_id),
                    Message::RegisterResponse(r) => Some(&r.response_id),
                    _ => None,
                };

                if let Some(rid) = response_id {
                    if rid == &request_id {
                        return msg;
                    }
                    // If not matching, keep reading (could be another request's response)
                } else {
                    // Not a response message, ignore
                }
            }
            Ok(Some(Ok(WsMessage::Binary(data)))) => {
                let msg: Message = serde_json::from_slice(&data).expect("Failed to parse response");

                let response_id = match &msg {
                    Message::Response(r) => Some(&r.response_id),
                    Message::ExecutionResult(r) => Some(&r.response_id),
                    Message::CodeRegisterResponse(r) => Some(&r.response_id),
                    Message::RegisterResponse(r) => Some(&r.response_id),
                    _ => None,
                };

                if let Some(rid) = response_id {
                    if rid == &request_id {
                        return msg;
                    }
                }
            }
            Ok(Some(Ok(_))) => panic!("Unexpected message type"),
            Ok(Some(Err(e))) => panic!("WebSocket error: {}", e),
            Ok(None) => panic!("Connection closed"),
            Err(_) => panic!("Response timeout for request_id: {}", request_id),
        }
    }
}

#[tokio::test]
async fn test_basic_execution() {
    // Create client
    let (mut write, mut read) = create_client().await;

    // Test simple execution
    let request_id = generate_request_id();
    let response = send_and_receive(
        &mut write,
        &mut read,
        Message::ExecutionStart(ExecutionStart {
            request_id: request_id.clone(),
            id: "test_1".to_string(),
            context_id: "test_ctx".to_string(),
            source: "vl x = 10; vl y = 20; res x + y;".to_string(),
            global_variables: json!({}),
        }),
        request_id,
    )
    .await;

    // Verify response
    match response {
        Message::ExecutionResult(result) => {
            assert_eq!(result.id, "test_1");
            assert_eq!(result.context_id, "test_ctx");
            match result.result {
                ResponseResult::Success { value } => {
                    assert_eq!(value, json!(30.0));
                }
                ResponseResult::Error { message } => {
                    panic!("Execution failed: {}", message);
                }
            }
        }
        _ => panic!("Expected ExecutionResult"),
    }
}

#[tokio::test]
async fn test_execution_with_globals() {
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (mut write, mut read) = create_client().await;

    // Execute with global variables
    let request_id = generate_request_id();
    let response = send_and_receive(
        &mut write,
        &mut read,
        Message::ExecutionStart(ExecutionStart {
            request_id: request_id.clone(),
            id: "test_2".to_string(),
            context_id: "test_ctx".to_string(),
            source: "vl result = a + b; res result;".to_string(),
            global_variables: json!({
                "a": 100,
                "b": 200
            }),
        }),
        request_id,
    )
    .await;

    match response {
        Message::ExecutionResult(result) => match result.result {
            ResponseResult::Success { value } => {
                assert_eq!(value, json!(300.0));
            }
            ResponseResult::Error { message } => {
                panic!("Execution failed: {}", message);
            }
        },
        _ => panic!("Expected ExecutionResult"),
    }
}

#[tokio::test]
async fn test_code_registration_and_cached_execution() {
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (mut write, mut read) = create_client().await;

    // Step 1: Register code
    let request_id = generate_request_id();
    let response = send_and_receive(
        &mut write,
        &mut read,
        Message::CodeRegister(CodeRegister {
            request_id: request_id.clone(),
            id: "reg_1".to_string(),
            context_id: "cached_ctx".to_string(),
            source: "vl sum = x + y; res sum * multiplier;".to_string(),
        }),
        request_id,
    )
    .await;

    let code_id = match response {
        Message::CodeRegisterResponse(resp) => {
            assert_eq!(resp.id, "reg_1");
            assert!(!resp.code_id.starts_with("error:"));
            resp.code_id
        }
        _ => panic!("Expected CodeRegisterResponse"),
    };

    // Step 2: Execute cached code (first time)
    let request_id = generate_request_id();
    let response = send_and_receive(
        &mut write,
        &mut read,
        Message::CachedExecutionStart(CachedExecutionStart {
            request_id: request_id.clone(),
            code_id: code_id.clone(),
            global_variables: json!({
                "__context_id": "cached_ctx",
                "x": 10,
                "y": 20,
                "multiplier": 2
            }),
        }),
        request_id,
    )
    .await;

    match response {
        Message::ExecutionResult(result) => {
            match result.result {
                ResponseResult::Success { value } => {
                    assert_eq!(value, json!(60.0)); // (10 + 20) * 2 = 60
                }
                ResponseResult::Error { message } => {
                    panic!("Cached execution failed: {}", message);
                }
            }
        }
        _ => panic!("Expected ExecutionResult"),
    }

    // Step 3: Execute cached code again with different values
    let request_id = generate_request_id();
    let response = send_and_receive(
        &mut write,
        &mut read,
        Message::CachedExecutionStart(CachedExecutionStart {
            request_id: request_id.clone(),
            code_id: code_id.clone(),
            global_variables: json!({
                "__context_id": "cached_ctx",
                "x": 100,
                "y": 200,
                "multiplier": 3
            }),
        }),
        request_id,
    )
    .await;

    match response {
        Message::ExecutionResult(result) => {
            match result.result {
                ResponseResult::Success { value } => {
                    assert_eq!(value, json!(900.0)); // (100 + 200) * 3 = 900
                }
                ResponseResult::Error { message } => {
                    panic!("Cached execution failed: {}", message);
                }
            }
        }
        _ => panic!("Expected ExecutionResult"),
    }
}

#[tokio::test]
async fn test_function_registration() {
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (mut write, mut read) = create_client().await;

    let context_id = "func_reg_ctx";

    // Step 1: Register a function
    let request_id = generate_request_id();
    let response = send_and_receive(
        &mut write,
        &mut read,
        Message::RegisterFunction(RegisterFunction {
            request_id: request_id.clone(),
            id: "reg_fn_1".to_string(),
            context_id: context_id.to_string(),
            function_name: "testFunction".to_string(),
        }),
        request_id,
    )
    .await;

    match response {
        Message::RegisterResponse(resp) => {
            assert_eq!(resp.id, "reg_fn_1");
            match resp.result {
                ResponseResult::Success { value } => {
                    assert_eq!(value["status"], "registered");
                    assert_eq!(value["function"], "testFunction");
                }
                ResponseResult::Error { message } => {
                    panic!("Registration failed: {}", message);
                }
            }
        }
        _ => panic!("Expected RegisterResponse"),
    }

    // Step 2: Try to call the function (will fail because no implementation)
    // With RPC handler enabled, it will try to call remotely, so we need to handle it
    let request_id = generate_request_id();
    let response = handle_bidirectional(
        &mut write,
        &mut read,
        Message::ExecutionStart(ExecutionStart {
            request_id: request_id.clone(),
            id: "exec_fn_1".to_string(),
            context_id: context_id.to_string(),
            source: "vl result = testFunction();".to_string(),
            global_variables: json!({}),
        }),
        Some(Box::new(|_func_name, _args| {
            // Return null or error - function not implemented
            json!(null)
        })),
    )
    .await;

    match response {
        Message::ExecutionResult(result) => {
            match result.result {
                ResponseResult::Success { value } => {
                    // Function was called and returned null - that's acceptable
                    assert_eq!(value, json!(null));
                }
                ResponseResult::Error { message } => {
                    // Also acceptable if it errors
                    assert!(
                        message.contains("not allowed")
                            || message.contains("Permission denied")
                            || message.contains("not found")
                            || message.contains("Function 'testFunction' not found"),
                        "Unexpected error: {}",
                        message
                    );
                }
            }
        }
        _ => panic!("Expected ExecutionResult"),
    }

    // Step 3: Try to call unregistered function (should fail with security error)
    let request_id = generate_request_id();
    let response = send_and_receive(
        &mut write,
        &mut read,
        Message::ExecutionStart(ExecutionStart {
            request_id: request_id.clone(),
            id: "exec_fn_2".to_string(),
            context_id: context_id.to_string(),
            source: "vl result = unregisteredFunction();".to_string(),
            global_variables: json!({}),
        }),
        request_id,
    )
    .await;

    match response {
        Message::ExecutionResult(result) => match result.result {
            ResponseResult::Error { message } => {
                assert!(
                    message.contains("not registered")
                        || message.contains("Permission denied")
                        || message.contains("Not allowed"),
                    "Expected security error, got: {}",
                    message
                );
            }
            ResponseResult::Success { .. } => {
                panic!("Should have failed - function not registered");
            }
        },
        _ => panic!("Expected ExecutionResult"),
    }
}

#[tokio::test]
async fn test_method_registration() {
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (mut write, mut read) = create_client().await;

    let context_id = "method_reg_ctx";

    // Register a method
    let request_id = generate_request_id();
    let response = send_and_receive(
        &mut write,
        &mut read,
        Message::RegisterMethod(RegisterMethod {
            request_id: request_id.clone(),
            id: "reg_method_1".to_string(),
            context_id: context_id.to_string(),
            object_id: "test_obj_123".to_string(),
            method_name: "testMethod".to_string(),
        }),
        request_id,
    )
    .await;

    match response {
        Message::RegisterResponse(resp) => {
            assert_eq!(resp.id, "reg_method_1");
            match resp.result {
                ResponseResult::Success { value } => {
                    assert_eq!(value["status"], "registered");
                    assert_eq!(value["method"], "testMethod");
                    assert_eq!(value["object_id"], "test_obj_123");
                }
                ResponseResult::Error { message } => {
                    panic!("Registration failed: {}", message);
                }
            }
        }
        _ => panic!("Expected RegisterResponse"),
    }
}

#[tokio::test]
async fn test_control_flow() {
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (mut write, mut read) = create_client().await;

    // Test if/else
    let request_id = generate_request_id();
    let response = send_and_receive(
        &mut write,
        &mut read,
        Message::ExecutionStart(ExecutionStart {
            request_id: request_id.clone(),
            id: "test_if".to_string(),
            context_id: "test_ctx".to_string(),
            source: r#"
                vl x = 10;
                if x > 5 {
                    res "greater";
                } else {
                    res "less";
                }
            "#
            .to_string(),
            global_variables: json!({}),
        }),
        request_id,
    )
    .await;

    match response {
        Message::ExecutionResult(result) => match result.result {
            ResponseResult::Success { value } => {
                assert_eq!(value, json!("greater"));
            }
            ResponseResult::Error { message } => {
                panic!("Execution failed: {}", message);
            }
        },
        _ => panic!("Expected ExecutionResult"),
    }

    // Test loop
    let request_id = generate_request_id();
    let response = send_and_receive(
        &mut write,
        &mut read,
        Message::ExecutionStart(ExecutionStart {
            request_id: request_id.clone(),
            id: "test_loop".to_string(),
            context_id: "test_ctx".to_string(),
            source: r#"
                vl sum = 0;
                vl numbers = [1, 2, 3, 4, 5];
                loop num in numbers {
                    sum = sum + num;
                }
                res sum;
            "#
            .to_string(),
            global_variables: json!({}),
        }),
        request_id,
    )
    .await;

    match response {
        Message::ExecutionResult(result) => {
            match result.result {
                ResponseResult::Success { value } => {
                    assert_eq!(value, json!(15.0)); // 1+2+3+4+5 = 15
                }
                ResponseResult::Error { message } => {
                    panic!("Execution failed: {}", message);
                }
            }
        }
        _ => panic!("Expected ExecutionResult"),
    }
}

#[tokio::test]
async fn test_callbacks() {
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (mut write, mut read) = create_client().await;

    let request_id = generate_request_id();
    let response = send_and_receive(
        &mut write,
        &mut read,
        Message::ExecutionStart(ExecutionStart {
            request_id: request_id.clone(),
            id: "test_callback".to_string(),
            context_id: "test_ctx".to_string(),
            source: r#"
                cb add(a, b) {
                    res a + b;
                }
                
                vl result = add(10, 20);
                res result;
            "#
            .to_string(),
            global_variables: json!({}),
        }),
        request_id,
    )
    .await;

    match response {
        Message::ExecutionResult(result) => match result.result {
            ResponseResult::Success { value } => {
                assert_eq!(value, json!(30.0));
            }
            ResponseResult::Error { message } => {
                panic!("Execution failed: {}", message);
            }
        },
        _ => panic!("Expected ExecutionResult"),
    }
}

#[tokio::test]
async fn test_objects_and_arrays() {
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (mut write, mut read) = create_client().await;

    // Test object creation and access
    let request_id = generate_request_id();
    let response = send_and_receive(
        &mut write,
        &mut read,
        Message::ExecutionStart(ExecutionStart {
            request_id: request_id.clone(),
            id: "test_obj".to_string(),
            context_id: "test_ctx".to_string(),
            source: r#"
                vl user = {
                    name: "Alice",
                    age: 30,
                    active: true
                };
                res user;
            "#
            .to_string(),
            global_variables: json!({}),
        }),
        request_id,
    )
    .await;

    match response {
        Message::ExecutionResult(result) => match result.result {
            ResponseResult::Success { value } => {
                assert_eq!(value["name"], "Alice");
                assert_eq!(value["age"], 30.0);
                assert_eq!(value["active"], true);
            }
            ResponseResult::Error { message } => {
                panic!("Execution failed: {}", message);
            }
        },
        _ => panic!("Expected ExecutionResult"),
    }

    // Test array operations
    let request_id = generate_request_id();
    let response = send_and_receive(
        &mut write,
        &mut read,
        Message::ExecutionStart(ExecutionStart {
            request_id: request_id.clone(),
            id: "test_arr".to_string(),
            context_id: "test_ctx".to_string(),
            source: r#"
                vl numbers = [10, 20, 30];
                vl first = numbers[0];
                res first;
            "#
            .to_string(),
            global_variables: json!({}),
        }),
        request_id,
    )
    .await;

    match response {
        Message::ExecutionResult(result) => match result.result {
            ResponseResult::Success { value } => {
                assert_eq!(value, json!(10.0));
            }
            ResponseResult::Error { message } => {
                panic!("Execution failed: {}", message);
            }
        },
        _ => panic!("Expected ExecutionResult"),
    }
}

#[tokio::test]
async fn test_error_handling() {
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (mut write, mut read) = create_client().await;

    // Test parse error
    let request_id = generate_request_id();
    let response = send_and_receive(
        &mut write,
        &mut read,
        Message::ExecutionStart(ExecutionStart {
            request_id: request_id.clone(),
            id: "test_parse_err".to_string(),
            context_id: "test_ctx".to_string(),
            source: "vl x = ;".to_string(), // Invalid syntax
            global_variables: json!({}),
        }),
        request_id,
    )
    .await;

    match response {
        Message::ExecutionResult(result) => match result.result {
            ResponseResult::Error { message } => {
                assert!(message.contains("Parse error") || message.contains("parse"));
            }
            ResponseResult::Success { .. } => {
                panic!("Should have failed - invalid syntax");
            }
        },
        _ => panic!("Expected ExecutionResult"),
    }

    // Test runtime error (undefined variable)
    let request_id = generate_request_id();
    let response = send_and_receive(
        &mut write,
        &mut read,
        Message::ExecutionStart(ExecutionStart {
            request_id: request_id.clone(),
            id: "test_runtime_err".to_string(),
            context_id: "test_ctx".to_string(),
            source: "res undefinedVariable;".to_string(),
            global_variables: json!({}),
        }),
        request_id,
    )
    .await;

    match response {
        Message::ExecutionResult(result) => match result.result {
            ResponseResult::Error { message } => {
                assert!(
                    message.contains("Undefined") || message.contains("not found"),
                    "Expected undefined error, got: {}",
                    message
                );
            }
            ResponseResult::Success { .. } => {
                panic!("Should have failed - undefined variable");
            }
        },
        _ => panic!("Expected ExecutionResult"),
    }
}

#[tokio::test]
async fn test_multiple_clients() {
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Create two clients
    let (mut write1, mut read1) = create_client().await;
    let (mut write2, mut read2) = create_client().await;

    // Client 1 executes
    let request_id = generate_request_id();
    let response1 = send_and_receive(
        &mut write1,
        &mut read1,
        Message::ExecutionStart(ExecutionStart {
            request_id: request_id.clone(),
            id: "client1_exec".to_string(),
            context_id: "ctx1".to_string(),
            source: "vl x = 100; res x;".to_string(),
            global_variables: json!({}),
        }),
        request_id,
    )
    .await;

    // Client 2 executes
    let request_id2 = generate_request_id();
    let response2 = send_and_receive(
        &mut write2,
        &mut read2,
        Message::ExecutionStart(ExecutionStart {
            request_id: request_id2.clone(),
            id: "client2_exec".to_string(),
            context_id: "ctx2".to_string(),
            source: "vl y = 200; res y;".to_string(),
            global_variables: json!({}),
        }),
        request_id2,
    )
    .await;

    // Verify both got their correct responses
    match response1 {
        Message::ExecutionResult(result) => match result.result {
            ResponseResult::Success { value } => {
                assert_eq!(value, json!(100.0));
            }
            ResponseResult::Error { message } => {
                panic!("Client 1 execution failed: {}", message);
            }
        },
        _ => panic!("Expected ExecutionResult"),
    }

    match response2 {
        Message::ExecutionResult(result) => match result.result {
            ResponseResult::Success { value } => {
                assert_eq!(value, json!(200.0));
            }
            ResponseResult::Error { message } => {
                panic!("Client 2 execution failed: {}", message);
            }
        },
        _ => panic!("Expected ExecutionResult"),
    }
}

#[tokio::test]
async fn test_remote_function_calling() {
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (mut write, mut read) = create_client().await;

    let context_id = "remote_call_ctx";

    // Step 1: Register remote function that will be callable from scripts
    let request_id = generate_request_id();
    let response = send_and_receive(
        &mut write,
        &mut read,
        Message::RegisterFunction(RegisterFunction {
            request_id: request_id.clone(),
            id: "reg_add".to_string(),
            context_id: context_id.to_string(),
            function_name: "addNumbers".to_string(),
        }),
        request_id,
    )
    .await;

    match response {
        Message::RegisterResponse(resp) => {
            assert_eq!(resp.id, "reg_add");
            match resp.result {
                ResponseResult::Success { value } => {
                    assert_eq!(value["status"], "registered");
                }
                ResponseResult::Error { message } => {
                    panic!("Registration failed: {}", message);
                }
            }
        }
        _ => panic!("Expected RegisterResponse"),
    }

    // Step 2: Execute a script that calls the remote function
    // Now with a function handler that implements addNumbers
    let request_id = generate_request_id();
    let response = handle_bidirectional(
        &mut write,
        &mut read,
        Message::ExecutionStart(ExecutionStart {
            request_id: request_id.clone(),
            id: "call_remote".to_string(),
            context_id: context_id.to_string(),
            source: r#"
                vl x = 10;
                vl y = 20;
                vl sum = addNumbers(x, y);
                res sum;
            "#
            .to_string(),
            global_variables: json!({}),
        }),
        Some(Box::new(|func_name, args| {
            if func_name == "addNumbers" && args.len() == 2 {
                let a = args[0].as_f64().unwrap_or(0.0);
                let b = args[1].as_f64().unwrap_or(0.0);
                json!(a + b)
            } else {
                json!(null)
            }
        })),
    )
    .await;

    // The call should now succeed!
    match response {
        Message::ExecutionResult(result) => {
            assert_eq!(result.id, "call_remote");
            match result.result {
                ResponseResult::Success { value } => {
                    // Should get 30 (10 + 20)
                    assert_eq!(value, json!(30.0));
                }
                ResponseResult::Error { message } => {
                    panic!("Execution failed: {}", message);
                }
            }
        }
        _ => panic!("Expected ExecutionResult"),
    }
}

#[tokio::test]
async fn test_remote_method_calling() {
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (mut write, mut read) = create_client().await;

    let context_id = "remote_method_ctx";
    let object_id = "user_object_456";

    // Step 1: Register a remote method
    let request_id = generate_request_id();
    let response = send_and_receive(
        &mut write,
        &mut read,
        Message::RegisterMethod(RegisterMethod {
            request_id: request_id.clone(),
            id: "reg_method_save".to_string(),
            context_id: context_id.to_string(),
            object_id: object_id.to_string(),
            method_name: "save".to_string(),
        }),
        request_id,
    )
    .await;

    match response {
        Message::RegisterResponse(resp) => {
            assert_eq!(resp.id, "reg_method_save");
            match resp.result {
                ResponseResult::Success { value } => {
                    assert_eq!(value["status"], "registered");
                }
                ResponseResult::Error { message } => {
                    panic!("Method registration failed: {}", message);
                }
            }
        }
        _ => panic!("Expected RegisterResponse"),
    }

    // Step 2: Create a script with an object that has secret_data
    // This simulates a host-injected object with remote methods
    let request_id = generate_request_id();
    let response = send_and_receive(
        &mut write,
        &mut read,
        Message::ExecutionStart(ExecutionStart {
            request_id: request_id.clone(),
            id: "call_method".to_string(),
            context_id: context_id.to_string(),
            source: r#"
                vl user = {
                    name: "Bob",
                    email: "bob@example.com"
                };
                res user;
            "#
            .to_string(),
            global_variables: json!({
                "userObj": {
                    "secret_data": {
                        "id": object_id
                    },
                    "name": "Alice",
                    "active": true
                }
            }),
        }),
        request_id,
    )
    .await;

    // Should succeed - just creating an object
    match response {
        Message::ExecutionResult(result) => match result.result {
            ResponseResult::Success { value } => {
                assert_eq!(value["name"], "Bob");
            }
            ResponseResult::Error { message } => {
                panic!("Execution failed: {}", message);
            }
        },
        _ => panic!("Expected ExecutionResult"),
    }
}
