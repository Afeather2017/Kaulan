//! Launch-file handoff endpoints.
//!
//! When the OS launches Kaulan as the default music app (double-click a `.mp3`
//! in the file manager), the Tauri shell captures the file path and pushes it
//! into the [`crate::launch_broker`] singleton. The frontend then consumes it
//! via [`get_launch_pending`] and plays the file.
//!
//! Two runtime cases feed the same broker:
//! - **Cold start** — Tauri sets `KAULAN_LAUNCH_FILE` env before the backend
//!   starts; the backend drains it into the broker on boot.
//! - **Warm start** — the single-instance plugin forwards argv to the running
//!   instance, which calls [`crate::set_pending_launch_file`] directly.
//!
//! Frontend subscribes to [`get_launch_events`] (SSE) so warm-start pushes
//! arrive without polling. Cold-start seeds (which happened before the SSE
//! connection opened) are caught by a one-shot GET on mount.
//!
//! Related documentation: `docs/default-music-app.md`

use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use futures::stream;
use futures::StreamExt;
use serde::Serialize;

use crate::handlers::local_guard::reject_non_local_peer;

/// Response body for `GET /api/launch/pending`.
#[derive(Serialize)]
struct LaunchPendingResponse {
    /// The absolute filesystem path the OS launched Kaulan with, or `null` if
    /// no pending file (or already consumed by a previous GET).
    path: Option<String>,
    /// Optional friendly filename (Android `content://` URIs come with a
    /// `_display_name` from ContentResolver; desktop leaves this `null` since
    /// the path itself ends in a filename).
    display_name: Option<String>,
}

/// Atomically take the pending launch file path.
///
/// Returns `{path: "/abs/path/to.mp3", display_name: null}` if a file is
/// pending (set by the Tauri shell on launch), or `{path: null, display_name:
/// null}` otherwise. Either way the stash is cleared — the frontend gets
/// exactly one shot at each pending launch.
///
/// Loopback-only — the stashed path is a hint to the local frontend about
/// what the OS launched. A remote caller has no business reading it.
#[get("/api/launch/pending")]
pub async fn get_launch_pending(req: HttpRequest) -> impl Responder {
    if let Some(reject) = reject_non_local_peer(&req) {
        return reject;
    }
    let path = crate::launch_broker().take_path();
    let display_name = crate::launch_broker().take_display_name();
    HttpResponse::Ok().json(LaunchPendingResponse { path, display_name })
}

/// Server-Sent Events stream that pushes a `data: {}\n\n` event each time the
/// Tauri shell stashes a new launch file (warm-start case).
///
/// The browser's `EventSource` API auto-reconnects on disconnect, so the
/// frontend can rely on this for the lifetime of the page. An initial
/// `: connected` comment is emitted so the browser sees a 200 immediately and
/// doesn't retry.
///
/// Loopback-only — same rationale as [`get_launch_pending`].
#[get("/api/launch/events")]
pub async fn get_launch_events(req: HttpRequest) -> impl Responder {
    if let Some(reject) = reject_non_local_peer(&req) {
        return reject;
    }
    let rx = crate::launch_broker().subscribe();

    let initial = stream::once(async {
        Ok::<_, std::io::Error>(web::Bytes::from_static(b": connected\n\n"))
    });
    let events = stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(()) => {
                    return Some((
                        Ok::<_, std::io::Error>(web::Bytes::from_static(b"data: {}\n\n")),
                        rx,
                    ));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // Missed events while the receiver was slow. Each event is
                    // just a () ping, so losing some is harmless — keep
                    // delivering future launches.
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    // No senders left. The broker holds a Sender for the
                    // process lifetime, so this only happens at shutdown —
                    // end the stream.
                    return None;
                }
            }
        }
    });
    let body = initial.chain(events);

    HttpResponse::Ok()
        .insert_header(("Content-Type", "text/event-stream"))
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .streaming(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch_broker;
    use actix_web::{test as actix_test, App};

    /// Tests share the singleton broker, so serialize them. (`OnceLock` can't
    /// be reset, and parallel tests would race on `set_path`/`take_path`.)
    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Reset the broker state between tests so they don't leak into each other.
    fn reset_broker() {
        launch_broker().take_path();
        launch_broker().take_display_name();
    }

    /// Loopback peer address for tests of endpoints guarded by
    /// [`reject_non_local_peer`]. `TestRequest` defaults `peer_addr` to `None`,
    /// which the guard treats as non-local — preset this so the guard passes.
    fn local_peer() -> std::net::SocketAddr {
        "127.0.0.1:0".parse().unwrap()
    }

    #[actix_web::test]
    async fn launch_pending_returns_and_clears_stashed_path() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_broker();
        crate::set_pending_launch_file("/tmp/song.mp3".to_string());

        let app = actix_test::init_service(App::new().service(get_launch_pending)).await;
        let req = actix_test::TestRequest::get()
            .peer_addr(local_peer())
            .uri("/api/launch/pending")
            .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: serde_json::Value = actix_test::read_body_json(resp).await;
        assert_eq!(body["path"], "/tmp/song.mp3");
        // Desktop path-only call leaves display_name null.
        assert!(body["display_name"].is_null());
    }

    #[actix_web::test]
    async fn launch_pending_returns_display_name_when_set() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_broker();
        crate::set_pending_launch_file_with_name(
            "content://media/external/audio/media/42".to_string(),
            Some("song.mp3".to_string()),
        );

        let app = actix_test::init_service(App::new().service(get_launch_pending)).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .peer_addr(local_peer())
                .uri("/api/launch/pending")
                .to_request(),
        )
        .await;
        let body: serde_json::Value = actix_test::read_body_json(resp).await;
        assert_eq!(body["path"], "content://media/external/audio/media/42");
        assert_eq!(body["display_name"], "song.mp3");
    }

    #[actix_web::test]
    async fn launch_pending_returns_null_after_take() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_broker();
        crate::set_pending_launch_file("/tmp/other.flac".to_string());

        let app = actix_test::init_service(App::new().service(get_launch_pending)).await;
        let _ = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .peer_addr(local_peer())
                .uri("/api/launch/pending")
                .to_request(),
        )
        .await;

        // Second call must return null — the stash was consumed.
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .peer_addr(local_peer())
                .uri("/api/launch/pending")
                .to_request(),
        )
        .await;
        let body: serde_json::Value = actix_test::read_body_json(resp).await;
        assert!(body["path"].is_null());
        assert!(body["display_name"].is_null());
    }

    #[actix_web::test]
    async fn launch_pending_returns_null_when_nothing_stashed() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_broker();
        let app = actix_test::init_service(App::new().service(get_launch_pending)).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .peer_addr(local_peer())
                .uri("/api/launch/pending")
                .to_request(),
        )
        .await;
        let body: serde_json::Value = actix_test::read_body_json(resp).await;
        assert!(body["path"].is_null());
        assert!(body["display_name"].is_null());
    }

    /// Loopback guard: a non-local peer must be rejected with 403 so the
    /// stashed path (a hint about what the OS launched locally) is not
    /// readable from the LAN.
    #[actix_web::test]
    async fn launch_pending_rejects_non_local_peer() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_broker();
        crate::set_pending_launch_file("/tmp/secret.mp3".to_string());

        let app = actix_test::init_service(App::new().service(get_launch_pending)).await;
        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .peer_addr("192.168.1.5:1234".parse().unwrap())
                .uri("/api/launch/pending")
                .to_request(),
        )
        .await;
        assert_eq!(resp.status().as_u16(), 403);
        // And the stash must be preserved (not consumed) on rejection.
        assert_eq!(
            launch_broker().take_path(),
            Some("/tmp/secret.mp3".to_string())
        );
    }

    /// Broker-level test: `subscribe()` then `set_path()` delivers a notification.
    /// This is the warm-start path the SSE handler forwards to clients.
    #[actix_web::test]
    async fn broker_subscribe_receives_notification_on_set_path() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_broker();
        let mut rx = launch_broker().subscribe();
        crate::set_pending_launch_file("/tmp/warm.flac".to_string());
        let received = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv()).await;
        assert!(received.is_ok(), "subscriber should receive a notification");
        // Inner Result<(), RecvError> — Ok means the broadcast delivered.
        assert!(
            received.unwrap().is_ok(),
            "broadcast recv should succeed, not lag or close"
        );
        assert_eq!(
            launch_broker().take_path(),
            Some("/tmp/warm.flac".to_string())
        );
        assert_eq!(launch_broker().take_display_name(), None);
    }
}
