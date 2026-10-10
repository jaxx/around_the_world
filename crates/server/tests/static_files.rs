//! Serving the browser client next to the API.

mod common;

use std::net::SocketAddr;
use std::path::Path;

use axum::Router;
use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use server::registry::Limits;
use tower::ServiceExt;

/// A client build: an index page and one asset.
fn client_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), "<!doctype html>index").unwrap();
    std::fs::write(dir.path().join("app.js"), "console.log(1)").unwrap();
    dir
}

fn app(static_dir: Option<&Path>) -> Router {
    server::http::app(common::registry(Limits::default()), static_dir)
        .layer(MockConnectInfo(SocketAddr::from(([10, 0, 0, 1], 5000))))
}

async fn get(app: &Router, uri: &str) -> (StatusCode, String) {
    let response = app
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn the_client_is_served_from_the_static_dir() {
    let dir = client_dir();
    let app = app(Some(dir.path()));
    assert_eq!(
        get(&app, "/").await,
        (StatusCode::OK, "<!doctype html>index".into())
    );
    assert_eq!(
        get(&app, "/app.js").await,
        (StatusCode::OK, "console.log(1)".into())
    );
    assert_eq!(get(&app, "/missing.js").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn invite_links_open_the_client() {
    let dir = client_dir();
    let app = app(Some(dir.path()));
    assert_eq!(
        get(&app, "/r/K7MXQ2").await,
        (StatusCode::OK, "<!doctype html>index".into())
    );
}

#[tokio::test]
async fn the_api_still_answers_next_to_the_client() {
    let dir = client_dir();
    let app = app(Some(dir.path()));
    let request = Request::post("/api/rooms/ZZZZZZ/join")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"nickname":"Jüri"}"#))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn without_a_static_dir_only_the_api_is_served() {
    let app = app(None);
    assert_eq!(get(&app, "/").await.0, StatusCode::NOT_FOUND);
    assert_eq!(get(&app, "/r/K7MXQ2").await.0, StatusCode::NOT_FOUND);
}
