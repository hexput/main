use hexput::rpc::protocol::{
    Message, ExecutionStart, ExecutionResult, ResponseResult,
};
use serde_json::json;
use std::time::Duration;
use std::sync::atomic::{AtomicU64, Ordering, AtomicBool};
use std::sync::Once;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use futures::{SinkExt, StreamExt};

const SERVER_ADDR: &str = "127.0.0.1:9099";
const WS_URL: &str = "ws://127.0.0.1:9099";
const TEST_TIMEOUT: Duration = Duration::from_secs(10);

static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(2000);
static SERVER_STARTED: AtomicBool = AtomicBool::new(false);
static SERVER_INIT: Once = Once::new();

fn generate_request_id() -> String {
    let id = REQUEST_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("complex_req_{}", id)
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
async fn test_05_complex_example() {
    let (mut write, mut read) = create_client().await;
    
    // Read the 05_complex.hxp file
    let source = std::fs::read_to_string("examples/05_complex.hxp")
        .expect("Failed to read 05_complex.hxp");
    
    let request_id = generate_request_id();
    let id = generate_request_id();
    
    let exec_msg = Message::ExecutionStart(ExecutionStart {
        request_id: request_id.clone(),
        id,
        context_id: "complex_test".to_string(),
        source,
        global_variables: json!({}),
    });
    
    let response = send_and_receive(&mut write, &mut read, exec_msg, request_id).await;
    
    if let Message::ExecutionResult(result) = response {
        match result.result {
            ResponseResult::Success { value } => {
                println!("Execution successful!");
                println!("Result: {}", serde_json::to_string_pretty(&value).unwrap());
                
                // Validate the output structure
                let obj = value.as_object().expect("Result should be an object");
                
                // Check status field
                assert_eq!(obj.get("status").and_then(|v| v.as_str()), Some("complete"), 
                    "status should be 'complete'");
                
                // Check results array exists
                let results = obj.get("results").expect("results field should exist");
                assert!(results.is_array(), "results should be an array");
                let results_arr = results.as_array().unwrap();
                
                // Should have processed 2 items (json and csv, xml was skipped)
                assert_eq!(results_arr.len(), 2, "Should have processed 2 items (json, csv)");
                
                // Check first result (json)
                let first = results_arr[0].as_object().expect("First result should be object");
                assert!(first.contains_key("value"), "Should have value field");
                assert!(first.contains_key("score"), "Should have score field");
                assert!(first.contains_key("metadata"), "JSON should have metadata");
                
                // Check summary
                let summary = obj.get("summary").expect("summary should exist");
                let summary_obj = summary.as_object().expect("summary should be object");
                assert_eq!(summary_obj.get("processed").and_then(|v| v.as_f64()), Some(2.0),
                    "Should have processed 2 items");
                assert_eq!(summary_obj.get("success").and_then(|v| v.as_bool()), Some(true),
                    "success should be true");
                
                // Check config
                let config = obj.get("config").expect("config should exist");
                let config_obj = config.as_object().expect("config should be object");
                assert_eq!(config_obj.get("name").and_then(|v| v.as_str()), Some("DataProcessor"));
                assert_eq!(config_obj.get("version").and_then(|v| v.as_f64()), Some(1.5));
            }
            ResponseResult::Error { message } => {
                panic!("Execution failed: {}\n\nThis suggests a bug in the interpreter or parser. Debug info needed.", message);
            }
        }
    } else {
        panic!("Expected ExecutionResult, got {:?}", response);
    }
}

#[tokio::test]
async fn test_05_complex_with_bytecode() {
    use hexput::rpc::protocol::{CompileBytecode, BytecodeExecutionStart};
    use std::collections::HashMap;
    
    let (mut write, mut read) = create_client().await;
    
    // Read the 05_complex.hxp file
    let source = std::fs::read_to_string("examples/05_complex.hxp")
        .expect("Failed to read 05_complex.hxp");
    
    // Step 1: Compile to bytecode
    let compile_request_id = generate_request_id();
    let compile_id = generate_request_id();
    
    let compile_msg = Message::CompileBytecode(CompileBytecode {
        request_id: compile_request_id.clone(),
        id: compile_id,
        source,
    });
    
    let compile_response = send_and_receive(&mut write, &mut read, compile_msg, compile_request_id).await;
    
    let bytecode = if let Message::CompileBytecodeResponse(resp) = compile_response {
        if resp.bytecode.starts_with("error: ") {
            panic!("Compilation failed: {}", resp.bytecode);
        }
        resp.bytecode
    } else {
        panic!("Expected CompileBytecodeResponse");
    };
    
    // Step 2: Execute bytecode
    let exec_request_id = generate_request_id();
    let exec_id = generate_request_id();
    
    let exec_msg = Message::BytecodeExecutionStart(BytecodeExecutionStart {
        request_id: exec_request_id.clone(),
        id: exec_id,
        context_id: "complex_bytecode_test".to_string(),
        bytecode,
        global_variables: HashMap::new(),
    });
    
    let exec_response = send_and_receive(&mut write, &mut read, exec_msg, exec_request_id).await;
    
    if let Message::ExecutionResult(result) = exec_response {
        match result.result {
            ResponseResult::Success { value } => {
                println!("Bytecode execution successful!");
                
                // Validate the same structure as the direct execution test
                let obj = value.as_object().expect("Result should be an object");
                assert_eq!(obj.get("status").and_then(|v| v.as_str()), Some("complete"));
                
                let results = obj.get("results").expect("results field should exist");
                assert!(results.is_array(), "results should be an array");
                assert_eq!(results.as_array().unwrap().len(), 2);
            }
            ResponseResult::Error { message } => {
                panic!("Bytecode execution failed: {}", message);
            }
        }
    } else {
        panic!("Expected ExecutionResult");
    }
}
