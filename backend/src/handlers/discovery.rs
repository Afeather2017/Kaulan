//! Device discovery API handlers
//!
//! This module provides HTTP API endpoints for device discovery.
//! See [docs/device-discovery.md](../../../docs/device-discovery.md) for protocol details.

use actix_web::{get, post, put, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::discovery::discovery::send_discovery_request;
use crate::discovery::types::DiscoveryState;

/// Get discovered devices.
///
/// During an active scan this returns fresh scan-buffer observations so clients
/// can update incrementally. Outside a scan it returns the committed list.
#[get("/api/discovery/devices")]
pub async fn get_discovered_devices(discovery: web::Data<DiscoveryState>) -> impl Responder {
    let devices = discovery.get_visible_devices().await;

    let device_list: Vec<DiscoveredDeviceResponse> = devices
        .into_iter()
        .map(|d| DiscoveredDeviceResponse {
            device_id: d.device_id,
            device_name: d.device_name,
            api_url: d.api_url,
            last_seen_secs_ago: d.last_seen.elapsed().as_secs(),
        })
        .collect();

    HttpResponse::Ok().json(device_list)
}

/// Get this device's info.
#[get("/api/discovery/self")]
pub async fn get_self_device(discovery: web::Data<DiscoveryState>) -> impl Responder {
    let device_name = discovery.get_device_name().await;

    HttpResponse::Ok().json(SelfDeviceResponse {
        device_id: discovery.device_id.clone(),
        device_name,
    })
}

/// Get the persisted runtime setting for periodic LAN announcements.
#[get("/api/discovery/periodic")]
pub async fn get_periodic_discovery(discovery: web::Data<DiscoveryState>) -> impl Responder {
    HttpResponse::Ok().json(PeriodicDiscoveryResponse {
        enabled: discovery.is_periodic_discovery_enabled(),
    })
}

/// Enable or disable periodic LAN announcements without affecting manual scans.
#[put("/api/discovery/periodic")]
pub async fn set_periodic_discovery(
    req: web::Json<SetPeriodicDiscoveryRequest>,
    discovery: web::Data<DiscoveryState>,
) -> impl Responder {
    if let Err(error) = crate::config::save_periodic_discovery_enabled(req.enabled) {
        return HttpResponse::InternalServerError().json(OperationResponse {
            success: false,
            message: format!("Failed to save periodic discovery setting: {}", error),
        });
    }
    discovery.set_periodic_discovery_enabled(req.enabled);
    info!("Periodic device discovery enabled: {}", req.enabled);
    HttpResponse::Ok().json(OperationResponse {
        success: true,
        message: "Periodic discovery setting updated".to_string(),
    })
}

/// Start a manual discovery scan transaction.
#[post("/api/discovery/scan/start")]
pub async fn start_discovery_scan(discovery: web::Data<DiscoveryState>) -> impl Responder {
    discovery.start_scan().await;

    HttpResponse::Ok().json(OperationResponse {
        success: true,
        message: "Discovery scan started".to_string(),
    })
}

/// Send one discovery request packet.
#[post("/api/discovery/request")]
pub async fn request_discovery_once(discovery: web::Data<DiscoveryState>) -> impl Responder {
    match send_discovery_request(&discovery).await {
        Ok(_) => HttpResponse::Ok().json(OperationResponse {
            success: true,
            message: "Discovery request sent".to_string(),
        }),
        Err(e) => {
            warn!("Failed to send discovery request: {}", e);
            HttpResponse::ServiceUnavailable().json(OperationResponse {
                success: false,
                message: format!("Failed to send discovery request: {}", e),
            })
        }
    }
}

/// Finish a manual discovery scan transaction and commit/rollback.
#[post("/api/discovery/scan/finish")]
pub async fn finish_discovery_scan(
    req: web::Json<FinishScanRequest>,
    discovery: web::Data<DiscoveryState>,
) -> impl Responder {
    discovery.finish_scan(req.success).await;

    HttpResponse::Ok().json(OperationResponse {
        success: true,
        message: if req.success {
            "Discovery scan committed".to_string()
        } else {
            "Discovery scan rolled back".to_string()
        },
    })
}

/// Set device name.
#[post("/api/discovery/name")]
pub async fn set_device_name(
    req: web::Json<SetDeviceNameRequest>,
    discovery: web::Data<DiscoveryState>,
) -> impl Responder {
    if req.name.is_empty() || req.name.len() > 64 {
        return HttpResponse::BadRequest().json(SetDeviceNameResponse {
            success: false,
            message: "Device name must be 1-64 characters".to_string(),
        });
    }

    discovery.set_device_name(req.name.clone()).await;

    if let Err(e) = crate::config::set_device_name(&req.name) {
        return HttpResponse::InternalServerError().json(SetDeviceNameResponse {
            success: false,
            message: format!("Failed to save device name: {}", e),
        });
    }

    info!("Device name changed to: {}", req.name);

    HttpResponse::Ok().json(SetDeviceNameResponse {
        success: true,
        message: "Device name updated".to_string(),
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscoveredDeviceResponse {
    pub device_id: String,
    pub device_name: String,
    pub api_url: String,
    pub last_seen_secs_ago: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SelfDeviceResponse {
    pub device_id: String,
    pub device_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PeriodicDiscoveryResponse {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct SetPeriodicDiscoveryRequest {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct SetDeviceNameRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct FinishScanRequest {
    pub success: bool,
}

#[derive(Debug, Serialize)]
pub struct OperationResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SetDeviceNameResponse {
    pub success: bool,
    pub message: String,
}
