// Named pipe example for Windows
// Build: cargo build --example named_pipe_demo
// Run server: cargo run --example named_pipe_demo server
// Run client: cargo run --example named_pipe_demo client

#[cfg(windows)]
use hexput::transport::named_pipe::NamedPipeTransport;
#[cfg(windows)]
use hexput::transport::Transport;
use hexput::rpc::{Message, Request, Response};

#[cfg(windows)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} [server|client]", args[0]);
        std::process::exit(1);
    }

    let pipe_name = r"\\.\pipe\hexput_demo";

    match args[1].as_str() {
        "server" => run_server(pipe_name).await,
        "client" => run_client(pipe_name).await,
        _ => {
            eprintln!("Invalid command. Use 'server' or 'client'");
            std::process::exit(1);
        }
    }
}

#[cfg(windows)]
async fn run_server(pipe_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting named pipe server: {}", pipe_name);
    
    let builder = NamedPipeTransport::create_server(pipe_name)?;
    let mut listener = builder.build()?;
    println!("Server listening...");

    loop {
        let mut transport = listener.accept().await?;
        println!("Client connected");

        tokio::spawn(async move {
            loop {
                match transport.recv().await {
                    Ok(Some(Message::Request(request))) => {
                        println!("Received request: {} ({})", request.function, request.id);
                        
                        // Simulate processing
                        let result = match request.function.as_str() {
                            "multiply" => {
                                let a = request.args[0].as_f64().unwrap_or(0.0);
                                let b = request.args[1].as_f64().unwrap_or(0.0);
                                serde_json::json!(a * b)
                            }
                            "echo" => request.args[0].clone(),
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

#[cfg(windows)]
async fn run_client(pipe_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Connecting to named pipe: {}", pipe_name);
    
    let mut transport = NamedPipeTransport::connect(pipe_name).await?;
    println!("Connected!");

    // Test 1: Multiplication
    println!("\n--- Test 1: Multiplication ---");
    let request = Message::Request(Request::new(
        "req_1".to_string(),
        "multiply".to_string(),
        vec![serde_json::json!(7), serde_json::json!(6)]
    ));
    
    transport.send(request).await?;
    
    if let Some(Message::Response(response)) = transport.recv().await? {
        println!("Result: {:?}", response.result);
    }

    // Test 2: Echo
    println!("\n--- Test 2: Echo ---");
    let request = Message::Request(Request::new(
        "req_2".to_string(),
        "echo".to_string(),
        vec![serde_json::json!({"message": "Hello from Windows!"})]
    ));
    
    transport.send(request).await?;
    
    if let Some(Message::Response(response)) = transport.recv().await? {
        println!("Result: {:?}", response.result);
    }

    println!("\nTests complete!");
    transport.close().await?;
    
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This example is only available on Windows");
    std::process::exit(1);
}
