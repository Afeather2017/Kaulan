//! UDP listener for device discovery.
//!
//! This module handles receiving discovery packets and responding to
//! request messages for manual on-demand scanning.

use crate::discovery::socket::get_broadcast_addr;
use crate::discovery::types::{DiscoveredDevice, DiscoveryError, DiscoveryMessage, DiscoveryState};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Maximum receive buffer size
const RECV_BUFFER_SIZE: usize = 1024;

/// Start UDP discovery listener.
///
/// Behavior:
/// - Listens on UDP port 2082 (via shared socket)
/// - On request packet: replies immediately with unicast response
/// - On response packet: updates discovered devices
/// - Recreates socket on receive errors (network changes)
pub async fn start_discovery_listener(socket: Arc<UdpSocket>, state: Arc<DiscoveryState>) {
    info!("Starting discovery listener");

    state.set_socket(socket.clone()).await;

    let mut buf = [0u8; RECV_BUFFER_SIZE];
    let mut current_socket = socket;

    info!("Discovery listener listening on UDP port 2082");

    loop {
        match current_socket.recv_from(&mut buf).await {
            Ok((len, addr)) => {
                // Process packet in background to avoid blocking receive loop
                let state_clone = state.clone();
                let Some(data) = buf.get(..len).map(<[u8]>::to_vec) else {
                    continue;
                };
                tokio::spawn(async move {
                    if let Err(e) = handle_discovery_packet(&data, addr, &state_clone).await {
                        debug!("Invalid discovery packet from {}: {}", addr, e);
                    }
                });
            }
            Err(e) => {
                warn!("Listener socket error: {}, recreating socket...", e);
                sleep(Duration::from_secs(1)).await;
                match recreate_socket().await {
                    Some(new_socket) => {
                        state.set_socket(new_socket.clone()).await;
                        current_socket = new_socket;
                    }
                    None => {
                        error!("Failed to recreate socket, will retry");
                        continue;
                    }
                };
                debug!("Socket recreated, resuming listening");
            }
        }
    }
}

/// Send one discovery request broadcast packet.
pub async fn send_discovery_request(state: &DiscoveryState) -> Result<(), DiscoveryError> {
    let socket = state
        .get_socket()
        .await
        .ok_or(DiscoveryError::SocketUnavailable)?;

    let msg = DiscoveryMessage::new_request();
    let payload = serde_json::to_vec(&msg)?;

    if payload.len() > DiscoveryMessage::MAX_SIZE {
        return Err(DiscoveryError::MessageTooLarge);
    }

    socket
        .send_to(&payload, get_broadcast_addr())
        .await
        .map_err(|_| DiscoveryError::SocketUnavailable)?;

    Ok(())
}

/// Recreate the UDP socket after network change.
async fn recreate_socket() -> Option<Arc<UdpSocket>> {
    crate::discovery::socket::create_discovery_socket().await
}

/// Handle a received discovery packet.
async fn handle_discovery_packet(
    data: &[u8],
    addr: std::net::SocketAddr,
    state: &Arc<DiscoveryState>,
) -> Result<(), DiscoveryError> {
    let msg: DiscoveryMessage = serde_json::from_slice(data)?;
    msg.validate()?;

    match msg.message_type.as_str() {
        DiscoveryMessage::REQUEST_TYPE => {
            respond_to_request(addr, state).await;
            Ok(())
        }
        DiscoveryMessage::RESPONSE_TYPE => {
            let device_id = msg
                .device_id
                .clone()
                .ok_or(DiscoveryError::MissingField("device_id".to_string()))?;

            // Ignore our own responses.
            if device_id == state.device_id {
                return Ok(());
            }

            let device_name = msg
                .device_name
                .clone()
                .ok_or(DiscoveryError::MissingField("device_name".to_string()))?;

            let api_port = msg
                .api_port
                .ok_or(DiscoveryError::MissingField("api_port".to_string()))?;

            let device = DiscoveredDevice::new(device_id, device_name.clone(), addr, api_port);
            let api_url = device.api_url.clone();
            state.upsert_discovered_device(device).await;

            info!("Discovered device: {} at {}", device_name, api_url);
            Ok(())
        }
        other => Err(DiscoveryError::InvalidType(other.to_string())),
    }
}

async fn respond_to_request(requester_addr: std::net::SocketAddr, state: &Arc<DiscoveryState>) {
    let socket = match state.get_socket().await {
        Some(socket) => socket,
        None => {
            warn!("Cannot respond to discovery request, socket unavailable");
            return;
        }
    };

    let device_name = state.get_device_name().await;
    let response =
        DiscoveryMessage::new_response(state.device_id.clone(), device_name, state.api_port);
    let payload = match serde_json::to_vec(&response) {
        Ok(payload) => payload,
        Err(e) => {
            error!("Failed to serialize discovery response: {}", e);
            return;
        }
    };

    if payload.len() > DiscoveryMessage::MAX_SIZE {
        warn!("Discovery response too large: {} bytes", payload.len());
        return;
    }

    if let Err(e) = socket.send_to(&payload, requester_addr).await {
        warn!(
            "Failed to send discovery response to {}: {}",
            requester_addr, e
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_message_validation() {
        let msg = DiscoveryMessage::new_request();
        assert!(msg.validate().is_ok());

        let msg =
            DiscoveryMessage::new_response("test-id".to_string(), "Test Player".to_string(), 2080);
        assert!(msg.validate().is_ok());
    }
}
