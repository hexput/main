// Unix domain socket example
// Build: cargo build --example unix_socket_demo
// Run server: cargo run --example unix_socket_demo server
// Run client: cargo run --example unix_socket_demo client

#[cfg(unix)]
use hexput::transport::unix_socket::{UnixSocketServer, UnixSocketTransport};
#[cfg(unix)]
use hexput::transport::Transport;
use hexput::rpc::{Message, Request, Response};

#[cfg(unix)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} [server|client]", args[0]);
        std::process::exit(1);
    }

    let socket_path = "/tmp/hexput_demo.sock";

    match args[1].as_str() {
        "server" => run_server(socket_path).await,
        "client" => run_client(socket_path).await,
        _ => {
            eprintln!("Invalid command. Use 'server' or 'client'");
            std::process::exit(1);
        }
    }
}

#[cfg(unix)]
async fn run_server(socket_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Unix socket server on {}", socket_path);
    
    let mut server = UnixSocketServer::bind(socket_path).await?;
    println!("Server listening...");

    loop {
        let mut transport = server.accept().await?;
        println!("Client connected");

        tokio::spawn(async move {
            loop {
                match transport.recv().await {
                    Ok(Some(Message::Request(request))) => {
                        println!("Received request: {} ({})", request.function, request.id);
                        
                        // Simulate processing
                        let result = match request.function.as_str() {
                            "add" => {
                                let a = request.args[0].as_f64().unwrap_or(0.0);
                                let b = request.args[1].as_f64().unwrap_or(0.0);
                                serde_json::json!(a + b)
                            }
                            "greet" => {
                                let name = request.args[0].as_str().unwrap_or("World");
                                serde_json::json!(format!("Hello, {}!", name))
                            }
                            _ => serde_json::json!(null),
                        };

                        let response = Message::Response(Response::success(request.id, result));
                        
                        if let Err(e) = transport.send(response).await {
                            eprintln!("Failed to send response: {}", e);
                            break;
                        }
                    }
                    Ok(None) => {
                        println!("Client disconnected");
                        break;
                    }
                    Err(e) => {
                        eprintln!("Error receiving: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });
    }
}

#[cfg(unix)]
async fn run_client(socket_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Connecting to Unix socket at {}", socket_path);
    
    let mut transport = UnixSocketTransport::connect(socket_path).await?;
    println!("Connected!");

    // Test 1: Addition
    println!("\n--- Test 1: Addition ---");
    let request = Message::Request(Request::new(
        "req_1".to_string(),
        "add".to_string(),
        vec![serde_json::json!(15), serde_json::json!(27)]
    ));
    
    transport.send(request).await?;
    
    if let Some(Message::Response(response)) = transport.recv().await? {
        println!("Result: {:?}", response.result);
    }

    // Test 2: Greeting
    println!("\n--- Test 2: Greeting ---");
    let request = Message::Request(Request::new(
        "req_2".to_string(),
        "greet".to_string(),
        vec![serde_json::json!("Hexput User")]
    ));
    
    transport.send(request).await?;
    
    if let Some(Message::Response(response)) = transport.recv().await? {
        println!("Result: {:?}", response.result);
    }

    println!("\nTests complete!");
    transport.close().await?;
    
    Ok(())
}

#[cfg(not(unix))]
fn main() {
    eprintln!("This example is only available on Unix-like systems (Linux, macOS)");
    std::process::exit(1);
}
