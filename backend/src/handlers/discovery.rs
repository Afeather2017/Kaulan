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

/// Resolve a stable device ID to its URL for the current server session.
#[get("/api/discovery/resolutions/{device_id}")]
pub async fn get_device_resolution(
    device_id: web::Path<String>,
    discovery: web::Data<DiscoveryState>,
) -> impl Responder {
    let device_id = device_id.into_inner();
    match discovery.resolve_device(&device_id).await {
        Some(api_url) => HttpResponse::Ok().json(DeviceResolutionResponse { device_id, api_url }),
        None => {
            // The local device always plays through localhost. (It is also
            // seeded into the resolution map at startup, so this branch is a
            // defensive guarantee that local playback keeps working even if the
            // map is ever cleared.) Any other id not observed this session is
            // genuinely unresolved → 404, so the web/Android player skips the
            // song instead of playing a foreign song id against the local
            // server and 404'ing at the music endpoint.
            if discovery.device_id == device_id {
                HttpResponse::Ok().json(DeviceResolutionResponse {
                    device_id,
                    api_url: format!("http://localhost:{}/api", discovery.api_port),
                })
            } else {
                HttpResponse::NotFound().finish()
            }
        }
    }
}

/// Publish a device URL already verified through `/discovery/self`.
#[put("/api/discovery/resolutions/{device_id}")]
pub async fn set_device_resolution(
    device_id: web::Path<String>,
    req: web::Json<SetDeviceResolutionRequest>,
    discovery: web::Data<DiscoveryState>,
) -> impl Responder {
    let device_id = device_id.into_inner();
    if device_id.trim().is_empty()
        || !(req.api_url.starts_with("http://") || req.api_url.starts_with("https://"))
    {
        return HttpResponse::BadRequest().json(OperationResponse {
            success: false,
            message: "A device ID and HTTP(S) API URL are required".to_string(),
        });
    }

    discovery
        .mark_device_resolved(device_id.clone(), req.api_url.clone())
        .await;
    info!(device_id = %device_id, "Device playback address resolved");
    HttpResponse::Ok().json(DeviceResolutionResponse {
        device_id,
        api_url: req.api_url.clone(),
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
pub struct DeviceResolutionResponse {
    pub device_id: String,
    pub api_url: String,
}

#[derive(Debug, Deserialize)]
pub struct SetDeviceResolutionRequest {
    pub api_url: String,
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

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{http::StatusCode, test, App};

    #[actix_web::test]
    async fn resolution_api_is_session_scoped_and_seeded_with_local_device() {
        let state = web::Data::new(DiscoveryState::new(
            "local-device".to_string(),
            "Local".to_string(),
            2080,
        ));
        let app = test::init_service(
            App::new()
                .app_data(state)
                .service(get_device_resolution)
                .service(set_device_resolution),
        )
        .await;

        let local = test::TestRequest::get()
            .uri("/api/discovery/resolutions/local-device")
            .to_request();
        let local: DeviceResolutionResponse = test::call_and_read_body_json(&app, local).await;
        assert_eq!(local.api_url, "http://localhost:2080/api");

        let missing = test::TestRequest::get()
            .uri("/api/discovery/resolutions/missing")
            .to_request();
        let missing_resp = test::call_service(&app, missing).await;
        assert_eq!(missing_resp.status(), StatusCode::NOT_FOUND);

        let mark = test::TestRequest::put()
            .uri("/api/discovery/resolutions/remote-device")
            .set_json(serde_json::json!({"api_url": "http://192.168.1.20:2080/api"}))
            .to_request();
        assert_eq!(
            test::call_service(&app, mark).await.status(),
            StatusCode::OK
        );

        let resolved = test::TestRequest::get()
            .uri("/api/discovery/resolutions/remote-device")
            .to_request();
        let resolved: DeviceResolutionResponse =
            test::call_and_read_body_json(&app, resolved).await;
        assert_eq!(resolved.api_url, "http://192.168.1.20:2080/api");
    }
}
