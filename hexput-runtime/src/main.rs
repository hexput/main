pub mod error;
pub mod handler;
pub mod messages;
pub mod server;
pub mod builtins;

use std::vec;

use clap::{builder::Str, Parser};
use tracing::info;
use tracing_subscriber::{FmtSubscriber, EnvFilter};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "127.0.0.1")]
    address: String,

    #[arg(short, long, default_value = "9001")]
    port: u16,

    #[arg(short, long)]
    debug: bool,
    
    #[arg(short, long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let args = Args::parse();
    
    let log_level_str = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        if args.debug {
            "debug".to_string()
        } else {
            args.log_level.clone()
        }
    });
    
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            // Create filter from provided log level or default
            EnvFilter::new(format!(
                "hexput_runtime={},tokio=info,runtime=info", 
                log_level_str
            ))
        });
    
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .with_ansi(true)
        .finish();
    
    tracing::subscriber::set_global_default(subscriber)?;

    let server_address = format!("{}:{}", args.address, args.port);

    info!(
        "Starting Hexput Runtime WebSocket server on {}",
        server_address
    );

    let config = server::ServerConfig {
        address: server_address,
    };

    match server::run_server(config).await {
        Ok(_) => info!("Server shut down gracefully"),
        Err(e) => {
            eprintln!("Server error: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}
