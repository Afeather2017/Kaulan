use std::env;

// Import from the library (init_tracing is now in the library)
use kaulan::{
    cli::{apply_standalone_auth, parse_cli_options, CliOptions},
    establish_connection, init_tracing, start_log_server, start_server, update_database_with_roots,
};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize tracing with broadcast support
    let broadcaster = init_tracing();

    let args: Vec<String> = env::args().collect();
    let program = args.first().map(String::as_str).unwrap_or("kaulan");

    if args.len() < 2 {
        eprintln!("{}", CliOptions::usage(program));
        eprintln!("Music Directory Priority:");
        eprintln!("  1. CLI argument [music_path] (if provided)");
        eprintln!("  2. Config file: ~/.config/kaulan/config.json");
        eprintln!("  3. Environment variable: KAULAN_MUSIC_DIR");
        eprintln!();
        eprintln!("If no music directory is configured, the application will abort.");
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "missing command",
        ));
    }

    let Some(command) = args.get(1) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "missing command",
        ));
    };
    let cli_args = args.get(2..).unwrap_or(&[]);
    let cli_options = match parse_cli_options(cli_args) {
        Ok(options) => options,
        Err(err) => {
            eprintln!("{err}");
            eprintln!();
            eprintln!("{}", CliOptions::usage(program));
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, err));
        }
    };

    if let Err(err) = apply_standalone_auth(&cli_options) {
        tracing::error!("Failed to apply standalone provider auth: {}", err);
        eprintln!("Failed to apply standalone provider auth: {}", err);
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, err));
    };

    match command.as_str() {
        "run" => {
            // Start the log streaming server on port 2081
            tokio::spawn(start_log_server(broadcaster));

            // Start the server (spawns in background)
            match start_server(cli_options.music_path.clone()).await {
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
                    return Err(std::io::Error::other(e.to_string()));
                }
            }
        }
        "update" => {
            // For update command, we need to get the music path first
            let music_path = if let Some(path) = cli_options.music_path.clone() {
                path
            } else if let Some(path) = kaulan::load_config() {
                path
            } else if let Ok(path) = env::var("KAULAN_MUSIC_DIR") {
                path
            } else {
                eprintln!("Error: No music directory configured!");
                eprintln!("Please provide music directory via:");
                eprintln!("  1. CLI argument: {} update <music_path>", program);
                eprintln!("  2. Config file: ~/.config/kaulan/config.json");
                eprintln!("  3. Environment variable: KAULAN_MUSIC_DIR");
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "no music directory configured",
                ));
            };

            tracing::info!("Starting database update with music path: {}", music_path);
            // Update the database only
            let db_conn = match establish_connection(&music_path).await {
                Ok(conn) => conn,
                Err(e) => {
                    tracing::error!("Failed to connect to database: {}", e);
                    eprintln!("Failed to connect to database: {}", e);
                    return Err(std::io::Error::other(e.to_string()));
                }
            };
            let download_root =
                env::var("KAULAN_DOWNLOAD_ROOT").unwrap_or_else(|_| music_path.clone());
            let library_roots = [music_path.as_str(), download_root.as_str()];
            match update_database_with_roots(&library_roots, &db_conn).await {
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
            eprintln!("{}", CliOptions::usage(program));
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("unknown command: {command}"),
            ));
        }
    }

    Ok(())
}
