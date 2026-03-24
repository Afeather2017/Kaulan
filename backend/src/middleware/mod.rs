//! HTTP middleware modules.
//!
//! This module provides middleware for:
//! - Access logging for all HTTP requests
//! - Request/response handling

// Placeholder for future middleware implementation
// For now, we'll keep the access logging simple by adding logs directly to handlers

use tracing::info;

/// Simple logging helper for API access
pub fn log_access(method: &str, path: &str, status: u16, elapsed: std::time::Duration) {
    info!(
        "[ACCESS] {} {} - Status: {} - Time: {:?}ms",
        method,
        path,
        status,
        elapsed.as_millis()
    );
}
