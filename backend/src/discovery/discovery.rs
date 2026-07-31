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

/// Low enough traffic for LAN use while still repairing changed IPs quickly.
pub const PERIODIC_DISCOVERY_INTERVAL: Duration = Duration::from_secs(10);

/// Start UDP discovery listener.
///
/// Behavior:
/// - Listens on UDP port 2082 (via shared socket)
/// - On request packet: replies immediately with unicast response
/// - On response packet: updates discovered devices
/// - Recreates socket on receive errors (network changes)
pub async fn start_discovery_listener(socket: Arc<UdpSocket>, state: Arc<DiscoveryState>) {
    info!("Starting discovery listener");

    // The caller (server::start_server) publishes the socket before spawning
    // this task so the periodic announcer can send immediately. Don't re-set
    // it here: doing so would race a `recreate_socket` swap if one were in
    // flight on another task (there isn't today, but the invariant matters).

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

    let msg = DiscoveryMessage::new_identified_request(
        state.device_id.clone(),
        state.get_device_name().await,
        state.api_port,
    );
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

/// Periodically announce this server so peers that cannot broadcast, such as
/// some router-hosted servers, can learn it from the incoming request.
pub async fn run_periodic_discovery(state: Arc<DiscoveryState>) {
    loop {
        if state.is_periodic_discovery_enabled() {
            if let Err(error) = send_discovery_request(&state).await {
                warn!("Failed to send periodic discovery request: {}", error);
            } else {
                debug!("Sent periodic discovery request");
            }
        }
        sleep(PERIODIC_DISCOVERY_INTERVAL).await;
    }
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
            // Recording the requester is best-effort: a malformed-but-valid
            // request must still get a unicast response so the peer learns
            // about us. Validation has already rejected partial identification,
            // so an error here is unexpected — log it and move on.
            if let Err(error) = record_identified_requester(&msg, addr, state).await {
                debug!("Skipped recording requester at {}: {}", addr, error);
            }
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

async fn record_identified_requester(
    msg: &DiscoveryMessage,
    addr: std::net::SocketAddr,
    state: &Arc<DiscoveryState>,
) -> Result<(), DiscoveryError> {
    // Anonymous v1.1 requests from older Kaulan versions remain compatible.
    let Some(device_id) = msg.device_id.clone() else {
        return Ok(());
    };
    if device_id == state.device_id {
        return Ok(());
    }
    let device_name = msg
        .device_name
        .clone()
        .ok_or_else(|| DiscoveryError::MissingField("device_name".to_string()))?;
    let api_port = msg
        .api_port
        .ok_or_else(|| DiscoveryError::MissingField("api_port".to_string()))?;
    let device = DiscoveredDevice::new(device_id, device_name.clone(), addr, api_port);
    let api_url = device.api_url.clone();
    state.upsert_discovered_device(device).await;
    info!("Discovered requester: {} at {}", device_name, api_url);
    Ok(())
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

    /// Regression coverage for issue #35. The receiver must learn an
    /// identified requester even when the receiver cannot broadcast.
    /// See `docs/device-discovery.md`.
    #[tokio::test]
    async fn identified_requester_is_recorded_from_source_address() {
        let state = Arc::new(DiscoveryState::new(
            "router-id".to_string(),
            "Router".to_string(),
            2080,
        ));
        let request = DiscoveryMessage::new_identified_request(
            "client-id".to_string(),
            "Phone".to_string(),
            3080,
        );
        let addr = "192.168.1.23:2082".parse().unwrap();

        record_identified_requester(&request, addr, &state)
            .await
            .unwrap();

        let devices = state.get_devices().await;
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device_id, "client-id");
        assert_eq!(devices[0].api_url, "http://192.168.1.23:3080/api");
    }
}
