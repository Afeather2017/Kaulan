use std::env;

// Import from the library
use kaulan::{start_server, update_database, establish_connection};

/// Initialize tracing subscriber for logging
fn init_tracing() {
    // Set default log level from RUST_LOG env var, or default to info
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .init();
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    init_tracing();

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <run|update> [music_path]", args[0]);
        eprintln!("  run   - Start the web server");
        eprintln!("  update - Scan for new music files and update database");
        eprintln!();
        eprintln!("Music Directory Priority:");
        eprintln!("  1. CLI argument [music_path] (if provided)");
        eprintln!("  2. Config file: ~/.config/kaulan/config.json");
        eprintln!("  3. Environment variable: KAULAN_MUSIC_DIR");
        eprintln!();
        eprintln!("If no music directory is configured, the application will abort.");
        std::process::exit(1);
    }

    let command = &args[1];
    let cli_path = if args.len() > 2 {
        Some(args[2].clone())
    } else {
        None
    };

    match command.as_str() {
        "run" => {
            // Start the server (spawns in background)
            match start_server(cli_path).await {
                Ok(server_info) => {
                    tracing::info!("Server started successfully on: {}", server_info.url());
                    println!("Server started on: {}", server_info.url());
                    // Wait for Ctrl+C to exit
                    println!("Press Ctrl+C to stop the server");
                    tokio::signal::ctrl_c().await?;
                    tracing::info!("Shutting down server...");
                    println!("Shutting down...");
                }
                Err(e) => {
                    tracing::error!("Failed to start server: {}", e);
                    eprintln!("Failed to start server: {}", e);
                    return Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
                }
            }
        }
        "update" => {
            // For update command, we need to get the music path first
            let music_path = if let Some(path) = cli_path {
                path
            } else if let Some(path) = kaulan::load_config() {
                path
            } else if let Ok(path) = env::var("KAULAN_MUSIC_DIR") {
                path
            } else {
                eprintln!("Error: No music directory configured!");
                eprintln!("Please provide music directory via:");
                eprintln!("  1. CLI argument: {} update <music_path>", args[0]);
                eprintln!("  2. Config file: ~/.config/kaulan/config.json");
                eprintln!("  3. Environment variable: KAULAN_MUSIC_DIR");
                std::process::exit(1);
            };

            tracing::info!("Starting database update with music path: {}", music_path);
            // Update the database only
            let db_conn = match establish_connection(&music_path).await {
                Ok(conn) => conn,
                Err(e) => {
                    tracing::error!("Failed to connect to database: {}", e);
                    eprintln!("Failed to connect to database: {}", e);
                    return Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
                }
            };
            match update_database(&music_path, &db_conn).await {
                Ok(_) => {
                    tracing::info!("Database update completed successfully");
                }
                Err(e) => {
                    tracing::error!("Database update failed: {}", e);
                    return Err(e);
                }
            }
        }
        _ => {
            tracing::error!("Unknown command: {}", command);
            eprintln!("Unknown command: {}", command);
            eprintln!("Usage: {} <run|update> [music_path]", args[0]);
            std::process::exit(1);
        }
    }

    Ok(())
}
