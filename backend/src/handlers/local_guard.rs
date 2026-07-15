//! Loopback-only guard for endpoints that expose filesystem-path based reads.
//!
//! See [`reject_non_local_peer`] for the threat model.

use actix_web::{HttpRequest, HttpResponse};
use tracing::warn;

/// Reject non-loopback peers for endpoints that take an absolute filesystem
/// path via `?p=` (or carry launch-handoff state). Returns `Some(403)` when
/// the request came from a non-local peer, `None` otherwise.
///
/// # Threat model
///
/// `/api/music/path`, `/api/music/path/cover`, `/api/lyrics/path`, and the
/// `/api/launch/*` endpoints expose reads of arbitrary audio files based on
/// a caller-supplied path (subject to the extension whitelist in the music /
/// cover / lyrics handlers). The DB-id endpoints (`/api/music/id/{id}` etc.)
/// only read files the scanner previously indexed, so they remain safe to
/// expose to the LAN for remote playback. The path-based endpoints widen the
/// read surface to any file the backend process can reach, so they are
/// gated to loopback only.
///
/// # Why `peer_addr` is reliable here
///
/// Actix-web sets `peer_addr` from the TCP connection. There is no
/// `X-Forwarded-For` parsing — the check is on the actual TCP peer, so it
/// cannot be spoofed by a header. On Android the in-process webview hits
/// the backend via `tauri.localhost` → 127.0.0.1, and `adb forward` forwards
/// through loopback too, so legitimate traffic always passes.
pub(crate) fn reject_non_local_peer(req: &HttpRequest) -> Option<HttpResponse> {
    let is_local = req
        .peer_addr()
        .map(|addr| addr.ip().is_loopback())
        .unwrap_or(false);
    if is_local {
        None
    } else {
        let peer = req.peer_addr().map(|a| a.to_string()).unwrap_or_default();
        warn!(
            "Rejected non-local peer ({}) on protected launch/path endpoint: {}",
            peer,
            req.path()
        );
        Some(HttpResponse::Forbidden().body("Local access only"))
    }
}
