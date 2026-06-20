//! Integration tests for backend static frontend hosting.
//!
//! See docs/static-frontend-serving.md for the deployment layout and request
//! flow covered by these tests.

use actix_web::{http::StatusCode, test, web, App};
use kaulan::server::{serve_static_frontend, StaticFrontendConfig};
use std::fs;

fn write_test_dist(root: &std::path::Path) {
    fs::create_dir_all(root.join("assets")).expect("Failed to create assets directory");
    fs::write(
        root.join("index.html"),
        r#"<!doctype html><html><body><div id="app">Kaulan</div></body></html>"#,
    )
    .expect("Failed to write index.html");
    fs::write(root.join("assets/app.js"), "console.log('kaulan')\n")
        .expect("Failed to write app.js");
}

#[actix_web::test]
async fn test_serves_frontend_index_at_root() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    write_test_dist(temp_dir.path());

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(StaticFrontendConfig {
                dist_dir: Some(temp_dir.path().to_path_buf()),
            }))
            .service(serve_static_frontend),
    )
    .await;

    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    assert!(
        String::from_utf8_lossy(&body).contains("Kaulan"),
        "Expected root request to return the frontend index"
    );
}

#[actix_web::test]
async fn test_serves_frontend_assets() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    write_test_dist(temp_dir.path());

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(StaticFrontendConfig {
                dist_dir: Some(temp_dir.path().to_path_buf()),
            }))
            .service(serve_static_frontend),
    )
    .await;

    let req = test::TestRequest::get().uri("/assets/app.js").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    assert!(
        String::from_utf8_lossy(&body).contains("console.log"),
        "Expected asset request to return the built asset"
    );
}

#[actix_web::test]
async fn test_spa_routes_fall_back_to_index() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    write_test_dist(temp_dir.path());

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(StaticFrontendConfig {
                dist_dir: Some(temp_dir.path().to_path_buf()),
            }))
            .service(serve_static_frontend),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/library/playlist")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    assert!(
        String::from_utf8_lossy(&body).contains("Kaulan"),
        "Expected unknown browser route to return the frontend index"
    );
}

#[actix_web::test]
async fn test_missing_assets_do_not_fall_back_to_index() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    write_test_dist(temp_dir.path());

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(StaticFrontendConfig {
                dist_dir: Some(temp_dir.path().to_path_buf()),
            }))
            .service(serve_static_frontend),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/assets/missing.js")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = test::read_body(resp).await;
    assert!(
        body.is_empty(),
        "Expected missing asset request to stay 404 instead of returning index.html"
    );
}

#[actix_web::test]
async fn test_api_paths_do_not_return_frontend_index() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    write_test_dist(temp_dir.path());

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(StaticFrontendConfig {
                dist_dir: Some(temp_dir.path().to_path_buf()),
            }))
            .service(serve_static_frontend),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/not-a-real-route")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = test::read_body(resp).await;
    assert!(
        body.is_empty(),
        "Expected API 404 to stay empty instead of returning index.html"
    );
}

#[actix_web::test]
async fn test_missing_frontend_dist_returns_not_found() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(StaticFrontendConfig { dist_dir: None }))
            .service(serve_static_frontend),
    )
    .await;

    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = test::read_body(resp).await;
    assert!(
        String::from_utf8_lossy(&body).contains("Frontend build not found"),
        "Expected a useful missing-build message"
    );
}
