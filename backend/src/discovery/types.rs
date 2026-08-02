//! Device discovery types
//!
//! This module defines the data structures for the Kaulan Discovery Protocol (KDP).
//! See [docs/device-discovery.md](../../../docs/device-discovery.md) for protocol specification.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::RwLock;

/// Keep recently seen devices across short explicit scans. The grace period is
/// three periodic-announcement intervals, so a healthy peer is not removed
/// merely because its 10-second announcement missed a 3-second scan window.
pub const DISCOVERY_LIVENESS_GRACE: Duration = Duration::from_secs(30);

/// Discovery message sent via UDP.
///
/// Protocol v1.1 uses identified requests for both manual scans and periodic
/// announcements:
/// - `kaulan-discovery-request`: may include sender identity and API metadata
/// - `kaulan-discovery-response`: immediate unicast response with service metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryMessage {
    /// Message type identifier
    #[serde(rename = "type")]
    pub message_type: String,

    /// Protocol version
    pub version: String,

    /// Unique device identifier (required for response)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,

    /// Human-readable device name (required for response)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,

    /// HTTP API port (required for response)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_port: Option<u16>,

    /// Unix timestamp in milliseconds
    pub timestamp: i64,
}

impl DiscoveryMessage {
    /// Maximum message size in bytes
    pub const MAX_SIZE: usize = 1024;

    /// Request message type identifier
    pub const REQUEST_TYPE: &'static str = "kaulan-discovery-request";

    /// Response message type identifier
    pub const RESPONSE_TYPE: &'static str = "kaulan-discovery-response";

    /// Expected protocol version
    pub const EXPECTED_VERSION: &'static str = "1.1";

    /// Create a new discovery request message.
    pub fn new_request() -> Self {
        Self {
            message_type: Self::REQUEST_TYPE.to_string(),
            version: Self::EXPECTED_VERSION.to_string(),
            device_id: None,
            device_name: None,
            api_port: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Create a request that lets receivers discover the requester directly.
    pub fn new_identified_request(device_id: String, device_name: String, api_port: u16) -> Self {
        Self {
            message_type: Self::REQUEST_TYPE.to_string(),
            version: Self::EXPECTED_VERSION.to_string(),
            device_id: Some(device_id),
            device_name: Some(device_name),
            api_port: Some(api_port),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Create a new discovery response message.
    pub fn new_response(device_id: String, device_name: String, api_port: u16) -> Self {
        Self {
            message_type: Self::RESPONSE_TYPE.to_string(),
            version: Self::EXPECTED_VERSION.to_string(),
            device_id: Some(device_id),
            device_name: Some(device_name),
            api_port: Some(api_port),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Validate this discovery message.
    pub fn validate(&self) -> Result<(), DiscoveryError> {
        if self.version != Self::EXPECTED_VERSION {
            return Err(DiscoveryError::UnsupportedVersion(self.version.clone()));
        }

        match self.message_type.as_str() {
            Self::REQUEST_TYPE => {
                let is_anonymous = self.device_id.is_none()
                    && self.device_name.is_none()
                    && self.api_port.is_none();
                if is_anonymous {
                    return Ok(());
                }
                self.validate_identity()
            }
            Self::RESPONSE_TYPE => self.validate_identity(),
            other => Err(DiscoveryError::InvalidType(other.to_string())),
        }
    }

    fn validate_identity(&self) -> Result<(), DiscoveryError> {
        if self.device_id.as_deref().unwrap_or_default().is_empty() {
            return Err(DiscoveryError::MissingField("device_id".to_string()));
        }
        let device_name = self.device_name.as_deref().unwrap_or_default();
        if device_name.is_empty() || device_name.len() > 64 {
            return Err(DiscoveryError::InvalidDeviceName);
        }
        if self.api_port.unwrap_or(0) == 0 {
            return Err(DiscoveryError::InvalidPort);
        }
        Ok(())
    }
}

/// Discovered device information.
#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    /// Device ID from discovery message
    pub device_id: String,

    /// Human-readable name
    pub device_name: String,

    /// Full HTTP URL (e.g., http://192.168.1.100:2080/api)
    pub api_url: String,

    /// When this device was last seen
    pub last_seen: Instant,

    /// Network address
    pub addr: SocketAddr,
}

impl DiscoveredDevice {
    /// Create a new discovered device entry.
    pub fn new(device_id: String, device_name: String, addr: SocketAddr, api_port: u16) -> Self {
        let ip = addr.ip();
        let api_url = format!("http://{}:{}/api", ip, api_port);

        Self {
            device_id,
            device_name,
            api_url,
            last_seen: Instant::now(),
            addr,
        }
    }
}

/// Shared state for the discovery service.
#[derive(Clone)]
pub struct DiscoveryState {
    /// This device's unique ID
    pub device_id: String,

    /// This device's name (user-configurable)
    pub device_name: Arc<RwLock<String>>,

    /// HTTP API port
    pub api_port: u16,

    /// Shared discovery socket (send+recv on UDP 2082)
    pub socket: Arc<RwLock<Option<Arc<UdpSocket>>>>,

    /// Committed discovered devices map (used by API response)
    pub discovered_devices: Arc<RwLock<HashMap<String, DiscoveredDevice>>>,

    /// Session-only device address book used by playback backends. Unlike the
    /// discovery list, this also contains remembered URLs verified by the
    /// frontend during startup.
    pub resolved_devices: Arc<RwLock<HashMap<String, String>>>,

    /// In-progress scan buffer. Active only between scan start/finish.
    pub scan_buffer: Arc<RwLock<Option<HashMap<String, DiscoveredDevice>>>>,

    /// Backup of committed map for rollback when scan fails.
    pub scan_backup: Arc<RwLock<Option<HashMap<String, DiscoveredDevice>>>>,

    /// Number of callers currently participating in the shared scan window.
    /// The first caller creates the transaction; the last caller commits or
    /// rolls it back.
    pub scan_ref_count: Arc<RwLock<usize>>,

    /// Runtime switch for the low-frequency background announcement task.
    pub periodic_discovery_enabled: Arc<AtomicBool>,
}

impl DiscoveryState {
    /// Create a new discovery state.
    pub fn new(device_id: String, device_name: String, api_port: u16) -> Self {
        Self::new_with_periodic_discovery(device_id, device_name, api_port, true)
    }

    pub fn new_with_periodic_discovery(
        device_id: String,
        device_name: String,
        api_port: u16,
        periodic_discovery_enabled: bool,
    ) -> Self {
        let mut resolved_devices = HashMap::new();
        resolved_devices.insert(
            device_id.clone(),
            format!("http://localhost:{}/api", api_port),
        );
        Self {
            device_id,
            device_name: Arc::new(RwLock::new(device_name)),
            api_port,
            socket: Arc::new(RwLock::new(None)),
            discovered_devices: Arc::new(RwLock::new(HashMap::new())),
            resolved_devices: Arc::new(RwLock::new(resolved_devices)),
            scan_buffer: Arc::new(RwLock::new(None)),
            scan_backup: Arc::new(RwLock::new(None)),
            scan_ref_count: Arc::new(RwLock::new(0)),
            periodic_discovery_enabled: Arc::new(AtomicBool::new(periodic_discovery_enabled)),
        }
    }

    pub fn is_periodic_discovery_enabled(&self) -> bool {
        self.periodic_discovery_enabled.load(Ordering::Relaxed)
    }

    pub fn set_periodic_discovery_enabled(&self, enabled: bool) {
        self.periodic_discovery_enabled
            .store(enabled, Ordering::Relaxed);
    }

    /// Get the current device name.
    pub async fn get_device_name(&self) -> String {
        self.device_name.read().await.clone()
    }

    /// Set the device name.
    pub async fn set_device_name(&self, name: String) {
        *self.device_name.write().await = name;
    }

    /// Set the current shared socket.
    pub async fn set_socket(&self, socket: Arc<UdpSocket>) {
        *self.socket.write().await = Some(socket);
    }

    /// Get the current shared socket.
    pub async fn get_socket(&self) -> Option<Arc<UdpSocket>> {
        self.socket.read().await.clone()
    }

    /// Start a manual scan transaction.
    ///
    /// Creates a new empty scan buffer and stores current committed devices as backup.
    pub async fn start_scan(&self) {
        let mut ref_count = self.scan_ref_count.write().await;
        if *ref_count == 0 {
            let current = self.discovered_devices.read().await.clone();
            *self.scan_backup.write().await = Some(current);
            *self.scan_buffer.write().await = Some(HashMap::new());
        }
        *ref_count = ref_count
            .checked_add(1)
            .expect("discovery scan reference count overflow");
    }

    /// Finish scan transaction and either commit or rollback.
    pub async fn finish_scan(&self, success: bool) -> bool {
        let mut ref_count = self.scan_ref_count.write().await;
        if *ref_count == 0 {
            return false;
        }
        *ref_count = ref_count
            .checked_sub(1)
            .expect("discovery scan reference count underflow");
        if *ref_count > 0 {
            return false;
        }
        drop(ref_count);

        let mut buffer = self.scan_buffer.write().await.take();
        let backup = self.scan_backup.write().await.take();

        if success {
            if let Some(mut next) = buffer.take() {
                if let Some(previous) = backup {
                    for (device_id, device) in previous {
                        if device.last_seen.elapsed() <= DISCOVERY_LIVENESS_GRACE {
                            next.entry(device_id).or_insert(device);
                        }
                    }
                }
                *self.discovered_devices.write().await = next;
            }
            return true;
        }

        if let Some(prev) = backup {
            *self.discovered_devices.write().await = prev;
        }
        true
    }

    /// Add or update a discovered device.
    ///
    /// If scan is active, updates scan buffer only; otherwise updates committed map.
    pub async fn upsert_discovered_device(&self, device: DiscoveredDevice) {
        self.mark_device_resolved(device.device_id.clone(), device.api_url.clone())
            .await;
        let mut scan_guard = self.scan_buffer.write().await;
        if let Some(scan_map) = scan_guard.as_mut() {
            scan_map.insert(device.device_id.clone(), device);
            return;
        }
        drop(scan_guard);

        self.discovered_devices
            .write()
            .await
            .insert(device.device_id.clone(), device);
    }

    /// Record a verified device address for this server lifetime.
    pub async fn mark_device_resolved(&self, device_id: String, api_url: String) {
        let previous = self
            .resolved_devices
            .write()
            .await
            .insert(device_id.clone(), api_url.clone());
        if previous.as_deref() != Some(api_url.as_str()) {
            tracing::info!(
                device_id = %device_id,
                previous_api_url = previous.as_deref().unwrap_or("<unresolved>"),
                api_url = %api_url,
                "Device playback address mapping changed"
            );
        }
    }

    /// Resolve a stable device ID to the URL verified during this session.
    pub async fn resolve_device(&self, device_id: &str) -> Option<String> {
        self.resolved_devices.read().await.get(device_id).cloned()
    }

    /// Get committed discovered devices.
    pub async fn get_devices(&self) -> Vec<DiscoveredDevice> {
        self.discovered_devices
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }

    /// Get devices visible during the current scan.
    ///
    /// While a scan is active, fresh observations override committed entries,
    /// while committed devices remain visible until the shared scan finishes.
    pub async fn get_visible_devices(&self) -> Vec<DiscoveredDevice> {
        let scan = self.scan_buffer.read().await.clone();
        if let Some(scan) = scan {
            let mut visible = self.discovered_devices.read().await.clone();
            visible.extend(scan);
            return visible.into_values().collect();
        }
        self.get_devices().await
    }
}

/// Discovery protocol errors
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("Invalid message type: {0}")]
    InvalidType(String),

    #[error("Unsupported protocol version: {0}")]
    UnsupportedVersion(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid device name (must be 1-64 characters)")]
    InvalidDeviceName,

    #[error("Invalid port number")]
    InvalidPort,

    #[error("Socket unavailable")]
    SocketUnavailable,

    #[error("Message too large")]
    MessageTooLarge,

    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_message_validation() {
        let msg = DiscoveryMessage::new_request();
        assert!(msg.validate().is_ok());
        assert_eq!(msg.message_type, DiscoveryMessage::REQUEST_TYPE);
    }

    #[test]
    fn test_response_message_validation() {
        let msg = DiscoveryMessage::new_response(
            "test-device-id".to_string(),
            "Test Player".to_string(),
            2080,
        );
        assert!(msg.validate().is_ok());
        assert_eq!(msg.message_type, DiscoveryMessage::RESPONSE_TYPE);
    }

    #[test]
    fn test_identified_request_validation() {
        let msg = DiscoveryMessage::new_identified_request(
            "test-device-id".to_string(),
            "Test Player".to_string(),
            2080,
        );
        assert!(msg.validate().is_ok());
    }

    #[test]
    fn test_partial_identified_request_is_rejected() {
        let mut msg = DiscoveryMessage::new_request();
        msg.device_id = Some("test-device-id".to_string());
        assert!(msg.validate().is_err());
    }

    #[tokio::test]
    async fn successful_scan_keeps_recent_device_that_missed_scan_window() {
        let state = DiscoveryState::new("local-id".to_string(), "Local".to_string(), 2080);
        state
            .upsert_discovered_device(DiscoveredDevice::new(
                "peer-id".to_string(),
                "Peer".to_string(),
                "192.168.1.20:2082".parse().unwrap(),
                2080,
            ))
            .await;

        state.start_scan().await;
        state.finish_scan(true).await;

        let devices = state.get_devices().await;
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device_id, "peer-id");
    }

    #[tokio::test]
    async fn successful_scan_removes_device_older_than_liveness_grace() {
        let state = DiscoveryState::new("local-id".to_string(), "Local".to_string(), 2080);
        let mut stale = DiscoveredDevice::new(
            "stale-id".to_string(),
            "Stale Peer".to_string(),
            "192.168.1.21:2082".parse().unwrap(),
            2080,
        );
        stale.last_seen = Instant::now() - DISCOVERY_LIVENESS_GRACE - Duration::from_secs(1);
        state.upsert_discovered_device(stale).await;

        state.start_scan().await;
        state.finish_scan(true).await;

        assert!(state.get_devices().await.is_empty());
    }

    #[tokio::test]
    async fn scan_result_replaces_recent_backup_with_same_device_id() {
        let state = DiscoveryState::new("local-id".to_string(), "Local".to_string(), 2080);
        state
            .upsert_discovered_device(DiscoveredDevice::new(
                "peer-id".to_string(),
                "Peer".to_string(),
                "192.168.1.20:2082".parse().unwrap(),
                2080,
            ))
            .await;

        state.start_scan().await;
        state
            .upsert_discovered_device(DiscoveredDevice::new(
                "peer-id".to_string(),
                "Peer".to_string(),
                "192.168.1.99:2082".parse().unwrap(),
                2080,
            ))
            .await;
        state.finish_scan(true).await;

        let devices = state.get_devices().await;
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].api_url, "http://192.168.1.99:2080/api");
        assert_eq!(
            state.resolve_device("peer-id").await.as_deref(),
            Some("http://192.168.1.99:2080/api")
        );
    }

    #[tokio::test]
    async fn active_scan_devices_are_visible_before_commit() {
        let state = DiscoveryState::new("local-id".to_string(), "Local".to_string(), 2080);
        state.start_scan().await;
        state
            .upsert_discovered_device(DiscoveredDevice::new(
                "peer-id".to_string(),
                "Peer".to_string(),
                "192.168.1.20:2082".parse().unwrap(),
                2080,
            ))
            .await;

        let visible = state.get_visible_devices().await;
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].device_id, "peer-id");
        assert!(state.get_devices().await.is_empty());
    }

    #[tokio::test]
    async fn overlapping_scans_share_one_transaction_until_last_finish() {
        let state = DiscoveryState::new("local-id".to_string(), "Local".to_string(), 2080);
        state.start_scan().await;
        state.start_scan().await;
        state
            .upsert_discovered_device(DiscoveredDevice::new(
                "peer-id".to_string(),
                "Peer".to_string(),
                "192.168.1.20:2082".parse().unwrap(),
                2080,
            ))
            .await;

        assert!(!state.finish_scan(true).await);
        assert!(state.get_devices().await.is_empty());
        assert_eq!(state.get_visible_devices().await.len(), 1);

        assert!(state.finish_scan(true).await);
        assert_eq!(state.get_devices().await.len(), 1);
    }
}
