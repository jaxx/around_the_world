//! The WebSocket to a room: says `Hello`, feeds every message into the
//! [`Session`], sends what the page queues, and reconnects as the session
//! advises. It stops once the page drops its [`Outbox`].

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures::stream::Fuse;
use futures::{FutureExt, SinkExt, StreamExt, select};
use gloo_net::websocket::Message;
use gloo_net::websocket::futures::WebSocket;
use gloo_timers::future::{IntervalStream, TimeoutFuture};
use leptos::prelude::*;
use leptos::task::spawn_local;
use protocol::{ClientMsg, RoomCode, SeatToken, ServerMsg};

use crate::session::{Effect, HEARTBEAT, Session};

/// Queues messages for the room. Dropping every clone closes the socket.
#[derive(Clone)]
pub struct Outbox(UnboundedSender<ClientMsg>);

impl Outbox {
    pub fn send(&self, msg: ClientMsg) {
        let _ = self.0.unbounded_send(msg);
    }
}

/// Opens the room's socket in the background. `forget` runs if the server
/// no longer knows the seat.
pub fn connect(
    code: RoomCode,
    token: SeatToken,
    session: RwSignal<Session>,
    forget: impl Fn() + 'static,
) -> Outbox {
    let (tx, rx) = unbounded();
    spawn_local(run(code, token, session, rx.fuse(), forget));
    Outbox(tx)
}

async fn run(
    code: RoomCode,
    token: SeatToken,
    session: RwSignal<Session>,
    mut outgoing: Fuse<UnboundedReceiver<ClientMsg>>,
    forget: impl Fn(),
) {
    loop {
        if let Ok(socket) = WebSocket::open(&socket_url(&code)) {
            let (mut write, read) = socket.split();
            let mut read = read.fuse();
            let hello = ClientMsg::Hello {
                token: token.clone(),
            };
            if write.send(text(&hello)).await.is_ok() {
                session.try_update(Session::opened);
                // Only messages to the server keep a free host awake, so the
                // heartbeat ticks whatever the server is sending (ADR 0002).
                let mut heartbeat = IntervalStream::new(HEARTBEAT.as_millis() as u32).fuse();
                loop {
                    select! {
                        _ = heartbeat.next() => {
                            if write.send(text(&ClientMsg::Ping)).await.is_err() {
                                break;
                            }
                        }
                        incoming = read.next() => match incoming {
                            Some(Ok(Message::Text(json))) => {
                                let Ok(msg) = serde_json::from_str::<ServerMsg>(&json) else {
                                    continue;
                                };
                                if session.try_update(|s| s.received(msg)) == Some(Effect::ForgetSeat) {
                                    forget();
                                }
                            }
                            Some(Ok(Message::Bytes(_))) => {}
                            Some(Err(_)) | None => break,
                        },
                        queued = outgoing.next() => match queued {
                            Some(msg) => {
                                if write.send(text(&msg)).await.is_err() {
                                    break;
                                }
                            }
                            None => {
                                let _ = write.close().await;
                                return;
                            }
                        },
                    }
                }
            }
        }

        let Some(wait) = session.try_update(Session::closed).flatten() else {
            return;
        };
        // Wait before trying again; anything queued meanwhile is dropped.
        let mut timer = TimeoutFuture::new(wait.as_millis() as u32).fuse();
        loop {
            select! {
                () = timer => break,
                queued = outgoing.next() => if queued.is_none() {
                    return;
                },
            }
        }
    }
}

fn text(msg: &ClientMsg) -> Message {
    Message::Text(serde_json::to_string(msg).expect("client messages serialize"))
}

/// `wss://host/api/ws/{code}` on https pages, `ws://…` otherwise.
fn socket_url(code: &RoomCode) -> String {
    let location = web_sys::window().expect("a browser window").location();
    let scheme = match location.protocol().as_deref() {
        Ok("https:") => "wss:",
        _ => "ws:",
    };
    let host = location.host().unwrap_or_default();
    format!("{scheme}//{host}/api/ws/{}", code.0)
}
