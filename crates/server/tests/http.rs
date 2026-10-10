//! Creating and joining rooms over HTTP.

mod common;

use std::net::SocketAddr;

use axum::Router;
use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use protocol::{ApiError, ErrorCode, Joined, SeatId};
use serde_json::{Value, json};
use server::registry::Limits;
use tower::ServiceExt;

fn app(limits: Limits) -> Router {
    server::http::router(common::registry(limits))
        .layer(MockConnectInfo(SocketAddr::from(([10, 0, 0, 1], 5000))))
}

async fn post(app: &Router, uri: &str, body: Value) -> (StatusCode, Value) {
    post_raw(app, uri, body.to_string()).await
}

async fn post_raw(app: &Router, uri: &str, body: String) -> (StatusCode, Value) {
    let request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn joined(body: Value) -> Joined {
    serde_json::from_value(body).unwrap()
}

fn error(body: Value) -> ErrorCode {
    serde_json::from_value::<ApiError>(body).unwrap().code
}

#[tokio::test]
async fn creating_a_room_returns_the_hosts_seat() {
    let app = app(Limits::default());
    let (status, body) = post(&app, "/api/rooms", json!({"nickname": "Mari"})).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(joined(body).seat, SeatId(0));
}

#[tokio::test]
async fn a_bad_nickname_is_a_bad_request() {
    let app = app(Limits::default());
    let (status, body) = post(&app, "/api/rooms", json!({"nickname": " "})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error(body), ErrorCode::BadNickname);
}

#[tokio::test]
async fn joining_takes_the_next_seat() {
    let app = app(Limits::default());
    let (_, body) = post(&app, "/api/rooms", json!({"nickname": "Mari"})).await;
    let code = joined(body).code.0.to_lowercase();
    let (status, body) = post(
        &app,
        &format!("/api/rooms/{code}/join"),
        json!({"nickname": "Jüri"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(joined(body).seat, SeatId(1));
}

#[tokio::test]
async fn joining_an_unknown_room_is_not_found() {
    let app = app(Limits::default());
    let (status, body) = post(&app, "/api/rooms/ZZZZZZ/join", json!({"nickname": "Jüri"})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(error(body), ErrorCode::RoomNotFound);
}

#[tokio::test]
async fn joining_a_full_room_is_a_conflict() {
    let app = app(Limits::default());
    let (_, body) = post(&app, "/api/rooms", json!({"nickname": "Mari"})).await;
    let uri = format!("/api/rooms/{}/join", joined(body).code.0);
    for name in ["b", "c", "d"] {
        post(&app, &uri, json!({"nickname": name})).await;
    }
    let (status, body) = post(&app, &uri, json!({"nickname": "e"})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error(body), ErrorCode::RoomFull);
}

#[tokio::test]
async fn too_many_rooms_from_one_address_is_rate_limited() {
    let app = app(Limits {
        max_rooms: 100,
        rooms_per_ip: 1,
    });
    post(&app, "/api/rooms", json!({"nickname": "Mari"})).await;
    let (status, body) = post(&app, "/api/rooms", json!({"nickname": "Mari"})).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(error(body), ErrorCode::RateLimited);
}

#[tokio::test]
async fn the_server_room_limit_is_service_unavailable() {
    let app = app(Limits {
        max_rooms: 1,
        rooms_per_ip: 10,
    });
    post(&app, "/api/rooms", json!({"nickname": "Mari"})).await;
    let (status, body) = post(&app, "/api/rooms", json!({"nickname": "Mari"})).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error(body), ErrorCode::TooManyRooms);
}

#[tokio::test]
async fn malformed_json_is_a_client_error() {
    let app = app(Limits::default());
    let (status, _) = post_raw(&app, "/api/rooms", "{nick".to_string()).await;
    assert!(status.is_client_error());
}
