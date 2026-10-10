//! Creating and joining rooms over HTTP.

use gloo_net::http::Request;
use protocol::{ApiError, CreateRoom, ErrorCode, JoinRoom, Joined, RoomCode};

/// Why a request did not produce a seat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// The server answered with a refusal.
    Refused(ErrorCode),
    /// No usable answer: offline, server down, or something unexpected.
    Network,
}

pub async fn create_room(nickname: &str) -> Result<Joined, Failure> {
    let body = CreateRoom {
        nickname: nickname.to_string(),
    };
    post("/api/rooms", &body).await
}

pub async fn join_room(code: &RoomCode, nickname: &str) -> Result<Joined, Failure> {
    let body = JoinRoom {
        nickname: nickname.to_string(),
    };
    post(&format!("/api/rooms/{}/join", code.0), &body).await
}

async fn post(url: &str, body: &impl serde::Serialize) -> Result<Joined, Failure> {
    let response = Request::post(url)
        .json(body)
        .map_err(|_| Failure::Network)?
        .send()
        .await
        .map_err(|_| Failure::Network)?;
    if response.ok() {
        response.json().await.map_err(|_| Failure::Network)
    } else {
        match response.json::<ApiError>().await {
            Ok(error) => Err(Failure::Refused(error.code)),
            Err(_) => Err(Failure::Network),
        }
    }
}
