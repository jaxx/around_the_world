//! Shared helpers: a registry with seeded dice, a real server on a free
//! port, and a small WebSocket client speaking the protocol.

#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use protocol::{ClientMsg, RoomView, ServerMsg};
use rand::SeedableRng;
use rand::rngs::StdRng;
use server::registry::{Limits, Registry};
use server::room::dice::RandomDice;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

pub fn registry(limits: Limits) -> Registry {
    Registry::new(
        limits,
        Arc::new(|| Box::new(RandomDice(StdRng::seed_from_u64(1)))),
    )
}

/// Serves the API on a free local port.
pub async fn serve(registry: Registry) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = server::http::router(registry).into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

pub struct Ws(WebSocketStream<MaybeTlsStream<TcpStream>>);

impl Ws {
    pub async fn open(addr: SocketAddr, code: &str) -> Ws {
        let url = format!("ws://{addr}/api/ws/{code}");
        Ws(connect_async(url).await.expect("websocket opens").0)
    }

    pub async fn send(&mut self, msg: &ClientMsg) {
        self.send_raw(serde_json::to_string(msg).unwrap()).await;
    }

    pub async fn send_raw(&mut self, text: String) {
        self.0.send(Message::Text(text.into())).await.unwrap();
    }

    /// The next protocol message, or `None` once the server has closed.
    pub async fn recv(&mut self) -> Option<ServerMsg> {
        loop {
            let next = tokio::time::timeout(Duration::from_secs(5), self.0.next())
                .await
                .expect("the server answers within 5 s");
            match next? {
                Ok(Message::Text(text)) => return Some(serde_json::from_str(&text).unwrap()),
                Ok(Message::Close(_)) | Err(_) => return None,
                Ok(_) => continue,
            }
        }
    }

    pub async fn snapshot(&mut self) -> RoomView {
        match self.recv().await {
            Some(ServerMsg::Snapshot { room }) => room,
            other => panic!("expected a snapshot, got {other:?}"),
        }
    }
}
