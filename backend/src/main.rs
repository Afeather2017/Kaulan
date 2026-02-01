use std::env;

// Import from the library
use kaulan::{start_server, update_database, establish_connection};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
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
            // Start the server (spawns in background)
            match start_server(music_path).await {
                Ok(server_info) => {
                    println!("Server started on: {}", server_info.url());
                    // Wait for Ctrl+C to exit
                    println!("Press Ctrl+C to stop the server");
                    tokio::signal::ctrl_c().await?;
                    println!("Shutting down...");
                }
                Err(e) => {
                    eprintln!("Failed to start server: {}", e);
                    return Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
                }
            }
        }
        "update" => {
            // Update the database only
            let db_conn = match establish_connection(&music_path).await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("Failed to connect to database: {}", e);
                    return Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
                }
            };
            update_database(&music_path, &db_conn).await?;
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            eprintln!("Usage: {} <run|update> [music_path]", args[0]);
            std::process::exit(1);
        }
    }

    Ok(())
}
