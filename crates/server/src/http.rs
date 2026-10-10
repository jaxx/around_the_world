//! The HTTP side: creating and joining rooms, and upgrading to the
//! WebSocket a seat plays over.

use std::net::SocketAddr;

use axum::Router;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{ConnectInfo, Json, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use protocol::{ApiError, CreateRoom, ErrorCode, JoinRoom};
use tower_http::services::{ServeDir, ServeFile};

use crate::registry::Registry;
use crate::ws;

/// Largest WebSocket message a client may send.
pub const MAX_MESSAGE: usize = 4 * 1024;

/// All `/api` routes. Serve it with
/// `into_make_service_with_connect_info::<SocketAddr>()`: room creation is
/// rate-limited per client address.
pub fn router(registry: Registry) -> Router {
    Router::new()
        .route("/api/rooms", post(create_room))
        .route("/api/rooms/{code}/join", post(join_room))
        .route("/api/rooms/{code}/ws", get(open_socket))
        .with_state(registry)
}

async fn create_room(
    State(registry): State<Registry>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<CreateRoom>,
) -> Response {
    match registry.create(&body.nickname, addr.ip()) {
        Ok(joined) => (StatusCode::CREATED, Json(joined)).into_response(),
        Err(code) => api_error(code),
    }
}

async fn join_room(
    State(registry): State<Registry>,
    Path(code): Path<String>,
    Json(body): Json<JoinRoom>,
) -> Response {
    match registry.join(&code, &body.nickname).await {
        Ok(joined) => Json(joined).into_response(),
        Err(code) => api_error(code),
    }
}

async fn open_socket(
    State(registry): State<Registry>,
    Path(code): Path<String>,
    upgrade: WebSocketUpgrade,
) -> Response {
    let Some(room) = registry.room(&code) else {
        return api_error(ErrorCode::RoomNotFound);
    };
    upgrade
        .max_message_size(MAX_MESSAGE)
        .max_frame_size(MAX_MESSAGE)
        .on_upgrade(move |socket| ws::session(socket, room))
}

fn api_error(code: ErrorCode) -> Response {
    let status = match code {
        ErrorCode::BadNickname | ErrorCode::BadMessage => StatusCode::BAD_REQUEST,
        ErrorCode::RoomNotFound => StatusCode::NOT_FOUND,
        ErrorCode::RoomFull | ErrorCode::NotInLobby => StatusCode::CONFLICT,
        ErrorCode::RateLimited => StatusCode::TOO_MANY_REQUESTS,
        ErrorCode::TooManyRooms => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::BAD_REQUEST,
    };
    (status, Json(ApiError { code })).into_response()
}

/// The API plus, when `static_dir` is given, the browser client: its files
/// as they are, and `index.html` for invite links (`/r/{code}`).
pub fn app(registry: Registry, static_dir: Option<&std::path::Path>) -> Router {
    let api = router(registry);
    match static_dir {
        None => api,
        Some(dir) => api
            .route_service("/r/{code}", ServeFile::new(dir.join("index.html")))
            .fallback_service(ServeDir::new(dir)),
    }
}
