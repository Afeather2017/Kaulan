//! Shared UDP socket for device discovery
//!
//! This module creates a single UDP socket that is shared between
//! the broadcast sender and discovery listener via Arc wrapping.
//!
//! Tokio's UdpSocket does not implement Clone, but both send_to() and
//! recv_from() take &self, so we can share the socket via Arc.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::sleep;
use tracing::{error, info};

/// Bind address for the discovery socket (listen on all interfaces)
const BIND_ADDR: &str = "0.0.0.0:2082";

/// Retry delay when socket creation fails
const SOCKET_RETRY_DELAY: Duration = Duration::from_secs(5);

/// Create and configure the shared UDP discovery socket
///
/// This function creates a single UDP socket bound to port 2082,
/// wrapped in Arc for sharing between sender and listener tasks.
///
/// The socket will:
/// - Bind to 0.0.0.0:2082 to receive from all interfaces
/// - Enable SO_BROADCAST to send to 255.255.255.255
/// - Retry on bind failure (every 5 seconds)
///
/// # Returns
/// * `Some(Arc<UdpSocket>)` - Socket successfully created and configured
/// * `None` - Failed after retries (should not happen under normal circumstances)
pub async fn create_discovery_socket() -> Option<Arc<UdpSocket>> {
    loop {
        match UdpSocket::bind(BIND_ADDR).await {
            Ok(socket) => {
                info!("UDP discovery socket bound to {}", BIND_ADDR);

                // Enable broadcast mode
                if let Err(e) = socket.set_broadcast(true) {
                    error!("Failed to set SO_BROADCAST: {}", e);
                    // This is critical - broadcast won't work without it
                    // But continue anyway - some platforms may handle this differently
                }

                return Some(Arc::new(socket));
            }
            Err(e) => {
                error!(
                    "Failed to bind discovery socket to {}: {}, retrying in {:?}",
                    BIND_ADDR, e, SOCKET_RETRY_DELAY
                );
                sleep(SOCKET_RETRY_DELAY).await;
                // Continue retrying
            }
        }
    }
}

/// Get the broadcast address for discovery messages
///
/// # Returns
/// The broadcast socket address (255.255.255.255:2082)
pub fn get_broadcast_addr() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::BROADCAST, 2082))
}
