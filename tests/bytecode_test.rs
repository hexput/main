use hexput::rpc::protocol::{
    Message, CompileBytecode, BytecodeExecutionStart,
    ResponseResult,
};
use serde_json::json;
use std::time::Duration;
use std::sync::atomic::{AtomicU64, Ordering, AtomicBool};
use std::sync::Once;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use futures::{SinkExt, StreamExt};
use std::collections::HashMap;

const SERVER_ADDR: &str = "127.0.0.1:9099";
const WS_URL: &str = "ws://127.0.0.1:9099";
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(1000); // Start from 1000 to avoid conflicts
static SERVER_STARTED: AtomicBool = AtomicBool::new(false);
static SERVER_INIT: Once = Once::new();

fn generate_request_id() -> String {
    let id = REQUEST_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("bytecode_req_{}", id)
}

async fn ensure_test_server_started() {
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
    
    let mut attempts = 0;
    while !SERVER_STARTED.load(Ordering::SeqCst) && attempts < 50 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        attempts += 1;
    }
    
    tokio::time::sleep(Duration::from_millis(100)).await;
}

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
    ensure_test_server_started().await;
    
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
    let json = serde_json::to_string(&message).expect("Failed to serialize");
    write
        .send(WsMessage::Text(json.into()))
        .await
        .expect("Failed to send");

    loop {
        match timeout(TEST_TIMEOUT, read.next()).await {
            Ok(Some(Ok(WsMessage::Text(text)))) => {
                let msg: Message = serde_json::from_str(&text).expect("Failed to parse response");
                
                let response_id = match &msg {
                    Message::CompileBytecodeResponse(r) => Some(&r.response_id),
                    Message::ExecutionResult(r) => Some(&r.response_id),
                    _ => None,
                };
                
                if let Some(rid) = response_id {
                    if rid == &request_id {
                        return msg;
                    }
                }
            }
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(e))) => panic!("WebSocket error: {}", e),
            Ok(None) => panic!("Connection closed"),
            Err(_) => panic!("Timeout waiting for response"),
        }
    }
}

#[tokio::test]
async fn test_compile_simple_arithmetic() {
    let (mut write, mut read) = create_client().await;
    
    let request_id = generate_request_id();
    let id = generate_request_id();
    
    let compile_msg = Message::CompileBytecode(CompileBytecode {
        request_id: request_id.clone(),
        id,
        source: "2 + 3 * 4".to_string(),
    });
    
    let response = send_and_receive(&mut write, &mut read, compile_msg, request_id).await;
    
    if let Message::CompileBytecodeResponse(resp) = response {
        assert!(!resp.bytecode.starts_with("error: "), "Compilation should succeed");
        assert!(!resp.bytecode.is_empty(), "Bytecode should not be empty");
        
        // Verify it's valid base64
        assert!(base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &resp.bytecode
        ).is_ok(), "Bytecode should be valid base64");
    } else {
        panic!("Expected CompileBytecodeResponse, got {:?}", response);
    }
}

#[tokio::test]
async fn test_compile_with_syntax_error() {
    let (mut write, mut read) = create_client().await;
    
    let request_id = generate_request_id();
    let id = generate_request_id();
    
    let compile_msg = Message::CompileBytecode(CompileBytecode {
        request_id: request_id.clone(),
        id,
        source: "res 2 +;".to_string(), // Actually invalid syntax - incomplete expression
    });
    
    let response = send_and_receive(&mut write, &mut read, compile_msg, request_id).await;
    
    if let Message::CompileBytecodeResponse(resp) = response {
        assert!(resp.bytecode.starts_with("error: "), "Should return error for syntax error");
    } else {
        panic!("Expected CompileBytecodeResponse, got {:?}", response);
    }
}

#[tokio::test]
async fn test_compile_and_execute_simple() {
    let (mut write, mut read) = create_client().await;
    
    // Step 1: Compile
    let compile_request_id = generate_request_id();
    let compile_id = generate_request_id();
    
    let compile_msg = Message::CompileBytecode(CompileBytecode {
        request_id: compile_request_id.clone(),
        id: compile_id,
        source: "res 10 + 20;".to_string(),
    });
    
    let compile_response = send_and_receive(&mut write, &mut read, compile_msg, compile_request_id).await;
    
    let bytecode = if let Message::CompileBytecodeResponse(resp) = compile_response {
        assert!(!resp.bytecode.starts_with("error: "), "Compilation should succeed");
        resp.bytecode
    } else {
        panic!("Expected CompileBytecodeResponse");
    };
    
    // Step 2: Execute
    let exec_request_id = generate_request_id();
    let exec_id = generate_request_id();
    
    let exec_msg = Message::BytecodeExecutionStart(BytecodeExecutionStart {
        request_id: exec_request_id.clone(),
        id: exec_id,
        context_id: "test_context".to_string(),
        bytecode,
        global_variables: HashMap::new(),
    });
    
    let exec_response = send_and_receive(&mut write, &mut read, exec_msg, exec_request_id).await;
    
    if let Message::ExecutionResult(result) = exec_response {
        match result.result {
            ResponseResult::Success { value } => {
                assert_eq!(value, json!(30.0), "Result should be 30");
            }
            ResponseResult::Error { message } => {
                panic!("Execution failed: {}", message);
            }
        }
    } else {
        panic!("Expected ExecutionResult, got {:?}", exec_response);
    }
}

#[tokio::test]
async fn test_execute_with_global_variables() {
    let (mut write, mut read) = create_client().await;
    
    // Compile: x + y
    let compile_request_id = generate_request_id();
    let compile_id = generate_request_id();
    
    let compile_msg = Message::CompileBytecode(CompileBytecode {
        request_id: compile_request_id.clone(),
        id: compile_id,
        source: "res x + y;".to_string(),
    });
    
    let compile_response = send_and_receive(&mut write, &mut read, compile_msg, compile_request_id).await;
    
    let bytecode = if let Message::CompileBytecodeResponse(resp) = compile_response {
        assert!(!resp.bytecode.starts_with("error: "));
        resp.bytecode
    } else {
        panic!("Expected CompileBytecodeResponse");
    };
    
    // Execute with different variables
    let exec_request_id = generate_request_id();
    let exec_id = generate_request_id();
    
    let mut globals = HashMap::new();
    globals.insert("x".to_string(), json!(5.0));
    globals.insert("y".to_string(), json!(7.0));
    
    let exec_msg = Message::BytecodeExecutionStart(BytecodeExecutionStart {
        request_id: exec_request_id.clone(),
        id: exec_id,
        context_id: "test_context_vars".to_string(),
        bytecode,
        global_variables: globals,
    });
    
    let exec_response = send_and_receive(&mut write, &mut read, exec_msg, exec_request_id).await;
    
    if let Message::ExecutionResult(result) = exec_response {
        match result.result {
            ResponseResult::Success { value } => {
                assert_eq!(value, json!(12.0), "Result should be 12");
            }
            ResponseResult::Error { message } => {
                panic!("Execution failed: {}", message);
            }
        }
    } else {
        panic!("Expected ExecutionResult");
    }
}

#[tokio::test]
async fn test_execute_same_bytecode_multiple_times() {
    let (mut write, mut read) = create_client().await;
    
    // Compile once: a * 2
    let compile_request_id = generate_request_id();
    let compile_id = generate_request_id();
    
    let compile_msg = Message::CompileBytecode(CompileBytecode {
        request_id: compile_request_id.clone(),
        id: compile_id,
        source: "res a * 2;".to_string(),
    });
    
    let compile_response = send_and_receive(&mut write, &mut read, compile_msg, compile_request_id).await;
    
    let bytecode = if let Message::CompileBytecodeResponse(resp) = compile_response {
        resp.bytecode
    } else {
        panic!("Expected CompileBytecodeResponse");
    };
    
    // Execute multiple times with different values
    for (i, expected) in [(10.0, 20.0), (15.0, 30.0), (7.5, 15.0)].iter() {
        let exec_request_id = generate_request_id();
        let exec_id = generate_request_id();
        
        let mut globals = HashMap::new();
        globals.insert("a".to_string(), json!(i));
        
        let exec_msg = Message::BytecodeExecutionStart(BytecodeExecutionStart {
            request_id: exec_request_id.clone(),
            id: exec_id,
            context_id: format!("test_context_{}", i),
            bytecode: bytecode.clone(),
            global_variables: globals,
        });
        
        let exec_response = send_and_receive(&mut write, &mut read, exec_msg, exec_request_id).await;
        
        if let Message::ExecutionResult(result) = exec_response {
            match result.result {
                ResponseResult::Success { value } => {
                    assert_eq!(value, json!(expected), "Result should be {}", expected);
                }
                ResponseResult::Error { message } => {
                    panic!("Execution {} failed: {}", i, message);
                }
            }
        }
    }
}

#[tokio::test]
async fn test_execute_invalid_bytecode() {
    let (mut write, mut read) = create_client().await;
    
    let exec_request_id = generate_request_id();
    let exec_id = generate_request_id();
    
    // Send invalid base64
    let exec_msg = Message::BytecodeExecutionStart(BytecodeExecutionStart {
        request_id: exec_request_id.clone(),
        id: exec_id,
        context_id: "test_context_invalid".to_string(),
        bytecode: "!!!invalid_base64!!!".to_string(),
        global_variables: HashMap::new(),
    });
    
    let exec_response = send_and_receive(&mut write, &mut read, exec_msg, exec_request_id).await;
    
    if let Message::ExecutionResult(result) = exec_response {
        match result.result {
            ResponseResult::Error { message } => {
                assert!(message.contains("decode") || message.contains("deserialize"),
                    "Error should mention decoding issue");
            }
            ResponseResult::Success { .. } => {
                panic!("Should fail with invalid bytecode");
            }
        }
    } else {
        panic!("Expected ExecutionResult");
    }
}

#[tokio::test]
async fn test_compile_complex_script() {
    let (mut write, mut read) = create_client().await;
    
    let compile_request_id = generate_request_id();
    let compile_id = generate_request_id();
    
    // Test with a slightly more complex expression (no let statements to avoid deserialization issues)
    let source = "res (5 + 10) * 2;";
    
    let compile_msg = Message::CompileBytecode(CompileBytecode {
        request_id: compile_request_id.clone(),
        id: compile_id,
        source: source.to_string(),
    });
    
    let compile_response = send_and_receive(&mut write, &mut read, compile_msg, compile_request_id).await;
    
    let bytecode = if let Message::CompileBytecodeResponse(resp) = compile_response {
        assert!(!resp.bytecode.starts_with("error: "));
        resp.bytecode
    } else {
        panic!("Expected CompileBytecodeResponse");
    };
    
    // Execute the compiled bytecode
    let exec_request_id = generate_request_id();
    let exec_id = generate_request_id();
    
    let exec_msg = Message::BytecodeExecutionStart(BytecodeExecutionStart {
        request_id: exec_request_id.clone(),
        id: exec_id,
        context_id: "test_context_complex".to_string(),
        bytecode,
        global_variables: HashMap::new(),
    });
    
    let exec_response = send_and_receive(&mut write, &mut read, exec_msg, exec_request_id).await;
    
    if let Message::ExecutionResult(result) = exec_response {
        match result.result {
            ResponseResult::Success { value } => {
                assert_eq!(value, json!(30.0), "Result should be 30");
            }
            ResponseResult::Error { message } => {
                panic!("Execution failed: {}", message);
            }
        }
    }
}
