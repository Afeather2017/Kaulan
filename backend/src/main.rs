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
        std::process::exit(1);
    }

    let command = &args[1];
    let music_path = if args.len() > 2 {
        args[2].clone()
    } else {
        env::var("HOME")
            .map(|h| format!("{}/Music", h))
            .unwrap_or_else(|_| String::from("./music"))
    };

    match command.as_str() {
        "run" => {
            tracing::info!("Starting server with music path: {}", music_path);
            // Start the server (spawns in background)
            match start_server(music_path).await {
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
