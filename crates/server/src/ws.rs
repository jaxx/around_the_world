//! One WebSocket session: the `Hello` handshake, then relaying messages
//! between the client and its room.

use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use protocol::{ClientMsg, ErrorCode, ServerMsg};
use tokio::sync::mpsc;

use crate::room::actor::{OUTBOX_LEN, RoomHandle};

/// How long a new socket has to say `Hello`.
pub const HELLO_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) async fn session(mut socket: WebSocket, room: RoomHandle) {
    let Some(token) = hello(&mut socket).await else {
        refuse(socket, ErrorCode::ExpectedHello).await;
        return;
    };
    let (outbox, mut inbox) = mpsc::channel(OUTBOX_LEN);
    let conn = match room.connect(token, outbox).await {
        Ok(conn) => {
            tracing::debug!(?conn, "websocket connected");
            conn
        }
        Err(code) => {
            tracing::debug!(?code, "websocket refused");
            refuse(socket, code).await;
            return;
        }
    };

    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Text(text))) => match serde_json::from_str::<ClientMsg>(&text) {
                    Ok(msg) => room.send(conn, msg).await,
                    Err(_) => {
                        if send(&mut socket, &bad_message()).await.is_err() {
                            break;
                        }
                    }
                },
                Some(Ok(Message::Binary(_))) => {
                    if send(&mut socket, &bad_message()).await.is_err() {
                        break;
                    }
                }
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
            },
            outgoing = inbox.recv() => match outgoing {
                Some(msg) => {
                    if send(&mut socket, &msg).await.is_err() {
                        break;
                    }
                }
                // Replaced by a newer connection, or the room closed.
                None => break,
            },
        }
    }
    tracing::debug!(?conn, "websocket closed");
    room.disconnect(conn).await;
    let _ = socket.send(Message::Close(None)).await;
}

/// The token from the first message, if it is a `Hello` sent in time.
async fn hello(socket: &mut WebSocket) -> Option<protocol::SeatToken> {
    match tokio::time::timeout(HELLO_TIMEOUT, socket.recv()).await {
        Ok(Some(Ok(Message::Text(text)))) => match serde_json::from_str(&text) {
            Ok(ClientMsg::Hello { token }) => Some(token),
            _ => None,
        },
        _ => None,
    }
}

async fn refuse(mut socket: WebSocket, code: ErrorCode) {
    let _ = send(&mut socket, &ServerMsg::Error { code }).await;
    let _ = socket.send(Message::Close(None)).await;
}

fn bad_message() -> ServerMsg {
    ServerMsg::Error {
        code: ErrorCode::BadMessage,
    }
}

async fn send(socket: &mut WebSocket, msg: &ServerMsg) -> Result<(), axum::Error> {
    let text = serde_json::to_string(msg).expect("server messages serialize");
    socket.send(Message::Text(text.into())).await
}
