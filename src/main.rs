use hexput::server::{Server, ServerConfig};
use hexput::VERSION;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let mut config = ServerConfig::default();
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--ws-addr" => {
                i += 1;
                if i < args.len() {
                    config.ws_addr = args[i].clone();
                }
            }
            "--no-ws" => {
                config.enable_ws = false;
            }
            #[cfg(unix)]
            "--unix-socket" => {
                i += 1;
                if i < args.len() {
                    config.unix_socket_path = Some(args[i].clone());
                }
            }
            #[cfg(unix)]
            "--no-unix" => {
                config.enable_unix = false;
            }
            #[cfg(windows)]
            "--pipe" => {
                i += 1;
                if i < args.len() {
                    config.named_pipe = Some(args[i].clone());
                }
            }
            #[cfg(windows)]
            "--no-pipe" => {
                config.enable_pipe = false;
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            "--version" | "-v" => {
                println!("Hexput v{}", VERSION);
                return Ok(());
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                eprintln!("Use --help for usage information");
                std::process::exit(1);
            }
        }
        i += 1;
    }
    
    println!("╔══════════════════════════════════════════════════════╗");
    println!("║   Hexput Runtime Server v{}                      ║", VERSION);
    println!("║   Deterministic, Synchronous, Sandboxed RPC         ║");
    println!("╚══════════════════════════════════════════════════════╝");
    println!();
    
    // Create and run server
    let server = Server::new(config);
    server.run().await
}

fn print_help() {
    println!("Hexput Runtime Server v{}", VERSION);
    println!();
    println!("USAGE:");
    println!("    hexput [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    --ws-addr <ADDR>       WebSocket bind address (default: 0.0.0.0:9091)");
    println!("    --no-ws                Disable WebSocket server");
    
    #[cfg(unix)]
    {
        println!("    --unix-socket <PATH>   Unix domain socket path (default: /tmp/.s.HXP.9091)");
        println!("    --no-unix              Disable Unix domain socket");
    }
    
    #[cfg(windows)]
    {
        println!("    --pipe <NAME>          Named pipe name (default: \\\\.\\pipe\\hexput)");
        println!("    --no-pipe              Disable named pipe");
    }
    
    println!("    -h, --help             Show this help message");
    println!("    -v, --version          Show version information");
    println!();
    println!("EXAMPLES:");
    println!("    hexput                           # Start with default settings");
    println!("    hexput --ws-addr 127.0.0.1:8080  # Custom WebSocket address");
    
    #[cfg(unix)]
    println!("    hexput --no-unix                 # Disable Unix socket");
    
    println!();
}
