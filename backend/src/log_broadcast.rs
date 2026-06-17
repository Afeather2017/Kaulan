//! Log broadcast module for streaming logs to TCP clients
//!
//! This module provides a custom tracing layer that broadcasts log lines
//! to connected TCP clients on port 2081. This is useful for debugging on
//! devices where accessing logs directly is difficult.
//!
//! # Usage
//!
//! ```bash
//! nc device-ip 2081
//! ```
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
//! │  Tracing Macros │────▶│  Custom Layer    │────▶│  TCP Clients    │
//! │  (info!, warn!) │     │  (broadcast)     │     │  (nc, telnet)   │
//! └─────────────────┘     └──────────────────┘     └─────────────────┘
//!                                │
//!                                ▼
//!                         ┌──────────────────┐
//!                         │  Standard        │
//!                         │  Console Output  │
//!                         └──────────────────┘
//! ```

use std::io;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::Subscriber;
use tracing_subscriber::{fmt, registry, Layer};

/// Global log broadcaster state
#[derive(Clone)]
pub struct LogBroadcaster {
    sender: broadcast::Sender<String>,
}

impl LogBroadcaster {
    /// Initialize a new log broadcaster with the given capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Subscribe to log broadcasts
    /// Returns a receiver that will receive log lines sent after subscription
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.sender.subscribe()
    }

    /// Send a log line to all subscribers
    /// If there are no subscribers or the channel is full, the log line is dropped
    pub fn send(&self, line: String) {
        let _ = self.sender.send(line);
        // Ignore errors: no receivers or channel full (both expected in some cases)
    }
}

/// A custom writer that broadcasts formatted log lines
///
/// The writer is wrapped in Arc<Mutex<>> so it can be shared
/// and mutated from multiple threads (each log event creates a new write)
struct BroadcastWriter {
    broadcaster: Arc<LogBroadcaster>,
}

impl io::Write for BroadcastWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let line = String::from_utf8_lossy(buf).to_string();
        self.broadcaster.send(line);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Create a broadcast layer for the tracing subscriber
///
/// This creates a fmt::Layer that writes to our broadcast writer,
/// sending formatted log lines to all connected TCP clients.
pub fn create_broadcast_layer<S>(
    broadcaster: Arc<LogBroadcaster>,
) -> Box<dyn Layer<S> + Send + Sync>
where
    S: Subscriber + for<'a> registry::LookupSpan<'a>,
{
    let broadcaster_clone = broadcaster.clone();

    // Use a closure that returns a new writer each time
    let layer = fmt::layer()
        .with_writer(move || BroadcastWriter {
            broadcaster: broadcaster_clone.clone(),
        })
        .with_level(true)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .compact();

    Box::new(layer)
}

/// Start the TCP log streaming server
///
/// Listens on `0.0.0.0:2081` and streams raw log lines to connected clients.
/// Each client sees only logs generated after their connection (no historical logs).
///
/// # Arguments
/// * `broadcaster` - The log broadcaster to subscribe to
///
/// # Behavior
/// - Accepts multiple simultaneous connections
/// - Each connection gets its own receiver (broadcast channel supports multiple receivers)
/// - Sends plain text log lines, one per line
/// - Handles connection close gracefully
/// - Non-blocking: if a client can't keep up, logs are dropped for that client
pub async fn start_log_server(broadcaster: Arc<LogBroadcaster>) {
    let addr = "0.0.0.0:2081";
    tracing::info!("Starting log streaming server on {}", addr);

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => {
            tracing::info!("Log streaming server listening on {}", addr);
            l
        }
        Err(e) => {
            tracing::error!("Failed to bind log streaming server to {}: {}", addr, e);
            return;
        }
    };

    // Spawn the TCP accept loop in the background
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((socket, peer_addr)) => {
                    tracing::debug!("New log streaming client connected from {}", peer_addr);

                    let broadcaster = broadcaster.clone();
                    tokio::spawn(async move {
                        handle_client(socket, broadcaster).await;
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to accept log streaming client: {}", e);
                }
            }
        }
    });
}

/// Handle a single TCP client connection
async fn handle_client(socket: tokio::net::TcpStream, broadcaster: Arc<LogBroadcaster>) {
    // Subscribe to log broadcasts
    let mut receiver = broadcaster.subscribe();

    // Use a buffered writer for better performance
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (mut reader, mut writer) = socket.into_split();

    // Spawn a task to monitor for client disconnect
    let disconnect_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let disconnect_signal_clone = disconnect_signal.clone();

    tokio::spawn(async move {
        // Read from the socket (ignoring data) to detect disconnect
        let mut buffer = [0u8; 1];
        while reader.read(&mut buffer).await.is_ok() {
            // Client is still connected
        }
        disconnect_signal_clone.store(true, std::sync::atomic::Ordering::Release);
    });

    // Stream logs to the client
    loop {
        // Check if client disconnected
        if disconnect_signal.load(std::sync::atomic::Ordering::Acquire) {
            break;
        }

        match receiver.recv().await {
            Ok(log_line) => {
                if let Err(e) = writer.write_all(log_line.as_bytes()).await {
                    tracing::debug!("Log streaming client disconnected: {}", e);
                    break;
                }
                // Flush to ensure logs are sent immediately
                if writer.flush().await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                // Client couldn't keep up, missed n messages
                tracing::debug!("Log streaming client lagged, missed {} messages", n);
                // Continue receiving new messages
            }
            Err(broadcast::error::RecvError::Closed) => {
                // Server shutting down
                break;
            }
        }
    }
}
