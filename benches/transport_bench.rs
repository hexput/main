use criterion::{criterion_group, criterion_main, Criterion};
use hexput::rpc::protocol::{
    CachedExecutionStart, CodeRegister, ExecutionStart, Message, RegisterFunction,
};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::time::timeout;

const SERVER_ADDR: &str = "127.0.0.1:9199"; // Different port for benchmarks
const WS_URL: &str = "ws://127.0.0.1:9199";
const TEST_TIMEOUT: Duration = Duration::from_secs(10);

#[cfg(unix)]
const UNIX_SOCKET_PATH: &str = "/tmp/hexput_bench.sock";

static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

fn generate_request_id() -> String {
    let id = REQUEST_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("bench_req_{}", id)
}

async fn start_bench_server() -> tokio::task::JoinHandle<()> {
    let server_config = hexput::server::ServerConfig {
        ws_addr: SERVER_ADDR.to_string(),
        #[cfg(unix)]
        unix_socket_path: Some(UNIX_SOCKET_PATH.to_string()),
        #[cfg(windows)]
        named_pipe: Some("\\\\.\\pipe\\hexput_bench".to_string()),
        enable_ws: true,
        #[cfg(unix)]
        enable_unix: true,
        #[cfg(windows)]
        enable_pipe: true,
    };

    tokio::spawn(async move {
        let server = hexput::server::Server::new(server_config);
        let _ = server.run().await;
    })
}

async fn create_ws_client() -> (
    futures::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        tokio_tungstenite::tungstenite::Message,
    >,
    futures::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
) {
    use futures::StreamExt;
    use tokio_tungstenite::connect_async;

    let mut attempts = 0;
    loop {
        match connect_async(WS_URL).await {
            Ok((ws_stream, _)) => {
                return ws_stream.split();
            }
            Err(_) if attempts < 30 => {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => panic!("Failed to connect: {}", e),
        }
    }
}

#[cfg(unix)]
async fn create_unix_client() -> hexput::transport::unix_socket::UnixSocketTransport {
    use hexput::transport::unix_socket::UnixSocketTransport;

    let mut attempts = 0;
    loop {
        match UnixSocketTransport::connect(UNIX_SOCKET_PATH).await {
            Ok(transport) => return transport,
            Err(_) if attempts < 30 => {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => panic!("Failed to connect to Unix socket: {}", e),
        }
    }
}

async fn ws_send_receive(
    write: &mut futures::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        tokio_tungstenite::tungstenite::Message,
    >,
    read: &mut futures::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    message: Message,
    request_id: String,
) -> Message {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message as WsMessage;

    let json = serde_json::to_string(&message).unwrap();
    write.send(WsMessage::Text(json.into())).await.unwrap();

    loop {
        match timeout(TEST_TIMEOUT, read.next()).await {
            Ok(Some(Ok(WsMessage::Text(text)))) => {
                let msg: Message = serde_json::from_str(&text).unwrap();

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
            Ok(Some(Ok(WsMessage::Binary(data)))) => {
                let msg: Message = serde_json::from_slice(&data).unwrap();

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
            _ => panic!("Timeout or connection closed"),
        }
    }
}

#[cfg(unix)]
async fn unix_send_receive(
    transport: &mut hexput::transport::unix_socket::UnixSocketTransport,
    message: Message,
    request_id: String,
) -> Message {
    use hexput::transport::Transport;

    transport.send(message).await.unwrap();

    loop {
        match timeout(TEST_TIMEOUT, transport.recv()).await {
            Ok(Ok(Some(msg))) => {
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
            _ => panic!("Timeout or connection closed"),
        }
    }
}

fn bench_websocket_basic_execution(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    // Start server once
    runtime.block_on(async {
        let _server = start_bench_server().await;
        tokio::time::sleep(Duration::from_millis(500)).await;
    });

    c.bench_function("ws_basic_execution", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let (mut write, mut read) = create_ws_client().await;

                let request_id = generate_request_id();
                let _response = ws_send_receive(
                    &mut write,
                    &mut read,
                    Message::ExecutionStart(ExecutionStart {
                        request_id: request_id.clone(),
                        id: "bench_1".to_string(),
                        context_id: "bench_ctx".to_string(),
                        source: "vl x = 10; vl y = 20; res x + y;".to_string(),
                        global_variables: json!({}),
                    }),
                    request_id,
                )
                .await;
            })
        });
    });
}

fn bench_websocket_cached_execution(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("ws_cached_execution", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let (mut write, mut read) = create_ws_client().await;

                // Register code
                let reg_id = generate_request_id();
                let response = ws_send_receive(
                    &mut write,
                    &mut read,
                    Message::CodeRegister(CodeRegister {
                        request_id: reg_id.clone(),
                        id: "reg".to_string(),
                        context_id: "bench_ctx".to_string(),
                        source: "vl result = x * 2; res result;".to_string(),
                    }),
                    reg_id,
                )
                .await;

                let code_id = match response {
                    Message::CodeRegisterResponse(resp) => resp.code_id,
                    _ => panic!("Expected CodeRegisterResponse"),
                };

                // Execute cached
                let exec_id = generate_request_id();
                let _response = ws_send_receive(
                    &mut write,
                    &mut read,
                    Message::CachedExecutionStart(CachedExecutionStart {
                        request_id: exec_id.clone(),
                        code_id,
                        global_variables: json!({
                            "__context_id": "bench_ctx",
                            "x": 42
                        }),
                    }),
                    exec_id,
                )
                .await;
            })
        });
    });
}

fn bench_websocket_function_registration(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("ws_function_registration", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let (mut write, mut read) = create_ws_client().await;

                let request_id = generate_request_id();
                let _response = ws_send_receive(
                    &mut write,
                    &mut read,
                    Message::RegisterFunction(RegisterFunction {
                        request_id: request_id.clone(),
                        id: "reg_fn".to_string(),
                        context_id: "bench_ctx".to_string(),
                        function_name: "benchFunc".to_string(),
                    }),
                    request_id,
                )
                .await;
            })
        });
    });
}

fn bench_websocket_complex_execution(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("ws_complex_execution", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let (mut write, mut read) = create_ws_client().await;

                let request_id = generate_request_id();
                let _response = ws_send_receive(
                    &mut write,
                    &mut read,
                    Message::ExecutionStart(ExecutionStart {
                        request_id: request_id.clone(),
                        id: "bench_complex".to_string(),
                        context_id: "bench_ctx".to_string(),
                        source: r#"
                        vl data = {
                            values: [10, 20, 30, 40, 50],
                            multiplier: 2
                        };
                        
                        vl result = [];
                        vl index = 0;
                        
                        loop val in data.values {
                            vl processed = val * data.multiplier;
                            result[index] = processed;
                            index = index + 1;
                        }
                        
                        res result;
                    "#
                        .to_string(),
                        global_variables: json!({}),
                    }),
                    request_id,
                )
                .await;
            })
        });
    });
}

fn bench_websocket_remote_function_call(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("ws_remote_function_call", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let (mut write, mut read) = create_ws_client().await;

                // Register a function
                let reg_id = generate_request_id();
                let _reg_response = ws_send_receive(
                    &mut write,
                    &mut read,
                    Message::RegisterFunction(RegisterFunction {
                        request_id: reg_id.clone(),
                        id: "reg_fn".to_string(),
                        context_id: "bench_remote_ctx".to_string(),
                        function_name: "remoteAdd".to_string(),
                    }),
                    reg_id,
                )
                .await;

                // Call the remote function via script execution
                let exec_id = generate_request_id();
                let _response = ws_send_receive(
                    &mut write,
                    &mut read,
                    Message::ExecutionStart(ExecutionStart {
                        request_id: exec_id.clone(),
                        id: "call_remote".to_string(),
                        context_id: "bench_remote_ctx".to_string(),
                        source: "vl result = remoteAdd(10, 20); res result;".to_string(),
                        global_variables: json!({}),
                    }),
                    exec_id,
                )
                .await;
            })
        });
    });
}

#[cfg(unix)]
fn bench_unix_socket_basic_execution(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("unix_basic_execution", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let mut transport = create_unix_client().await;

                let request_id = generate_request_id();
                let _response = unix_send_receive(
                    &mut transport,
                    Message::ExecutionStart(ExecutionStart {
                        request_id: request_id.clone(),
                        id: "bench_1".to_string(),
                        context_id: "bench_ctx".to_string(),
                        source: "vl x = 10; vl y = 20; res x + y;".to_string(),
                        global_variables: json!({}),
                    }),
                    request_id,
                )
                .await;
            })
        });
    });
}

#[cfg(unix)]
fn bench_unix_socket_cached_execution(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("unix_cached_execution", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let mut transport = create_unix_client().await;

                // Register code
                let reg_id = generate_request_id();
                let response = unix_send_receive(
                    &mut transport,
                    Message::CodeRegister(CodeRegister {
                        request_id: reg_id.clone(),
                        id: "reg".to_string(),
                        context_id: "bench_ctx".to_string(),
                        source: "vl result = x * 2; res result;".to_string(),
                    }),
                    reg_id,
                )
                .await;

                let code_id = match response {
                    Message::CodeRegisterResponse(resp) => resp.code_id,
                    _ => panic!("Expected CodeRegisterResponse"),
                };

                // Execute cached
                let exec_id = generate_request_id();
                let _response = unix_send_receive(
                    &mut transport,
                    Message::CachedExecutionStart(CachedExecutionStart {
                        request_id: exec_id.clone(),
                        code_id,
                        global_variables: json!({
                            "__context_id": "bench_ctx",
                            "x": 42
                        }),
                    }),
                    exec_id,
                )
                .await;
            })
        });
    });
}

#[cfg(unix)]
fn bench_unix_socket_complex_execution(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("unix_complex_execution", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let mut transport = create_unix_client().await;

                let request_id = generate_request_id();
                let _response = unix_send_receive(
                    &mut transport,
                    Message::ExecutionStart(ExecutionStart {
                        request_id: request_id.clone(),
                        id: "bench_complex".to_string(),
                        context_id: "bench_ctx".to_string(),
                        source: r#"
                        vl data = {
                            values: [10, 20, 30, 40, 50],
                            multiplier: 2
                        };
                        
                        vl result = [];
                        vl index = 0;
                        
                        loop val in data.values {
                            vl processed = val * data.multiplier;
                            result[index] = processed;
                            index = index + 1;
                        }
                        
                        res result;
                    "#
                        .to_string(),
                        global_variables: json!({}),
                    }),
                    request_id,
                )
                .await;
            })
        });
    });
}

#[cfg(unix)]
fn bench_unix_socket_remote_function_call(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("unix_remote_function_call", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let mut transport = create_unix_client().await;

                // Register a function
                let reg_id = generate_request_id();
                let _reg_response = unix_send_receive(
                    &mut transport,
                    Message::RegisterFunction(RegisterFunction {
                        request_id: reg_id.clone(),
                        id: "reg_fn".to_string(),
                        context_id: "bench_remote_ctx".to_string(),
                        function_name: "remoteAdd".to_string(),
                    }),
                    reg_id,
                )
                .await;

                // Call the remote function via script execution
                let exec_id = generate_request_id();
                let _response = unix_send_receive(
                    &mut transport,
                    Message::ExecutionStart(ExecutionStart {
                        request_id: exec_id.clone(),
                        id: "call_remote".to_string(),
                        context_id: "bench_remote_ctx".to_string(),
                        source: "vl result = remoteAdd(10, 20); res result;".to_string(),
                        global_variables: json!({}),
                    }),
                    exec_id,
                )
                .await;
            })
        });
    });
}

#[cfg(unix)]
criterion_group!(
    transport_benches,
    bench_websocket_basic_execution,
    bench_websocket_cached_execution,
    bench_websocket_function_registration,
    bench_websocket_complex_execution,
    bench_websocket_remote_function_call,
    bench_unix_socket_basic_execution,
    bench_unix_socket_cached_execution,
    bench_unix_socket_complex_execution,
    bench_unix_socket_remote_function_call
);

#[cfg(not(unix))]
criterion_group!(
    transport_benches,
    bench_websocket_basic_execution,
    bench_websocket_cached_execution,
    bench_websocket_function_registration,
    bench_websocket_complex_execution,
    bench_websocket_remote_function_call
);

criterion_main!(transport_benches);
