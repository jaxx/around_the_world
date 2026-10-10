//! Playing over a real WebSocket against a server on a local port.

mod common;

use common::Ws;
use protocol::{ClientMsg, ErrorCode, Joined, SeatId, SeatToken, ServerMsg, Stage};
use rules::Color;
use server::http::MAX_MESSAGE;
use server::registry::{Limits, Registry};

/// A server with one room: Mari (host) and Jüri.
async fn room() -> (std::net::SocketAddr, Joined, Joined) {
    let registry: Registry = common::registry(Limits::default());
    let mari = registry.create("Mari", [10, 0, 0, 1].into()).unwrap();
    let juri = registry.join(&mari.code.0, "Jüri").await.unwrap();
    (common::serve(registry).await, mari, juri)
}

/// Opens a socket for `seat` and reads its welcome and first snapshot.
async fn hello(addr: std::net::SocketAddr, seat: &Joined) -> Ws {
    let mut ws = Ws::open(addr, &seat.code.0).await;
    ws.send(&ClientMsg::Hello {
        token: seat.token.clone(),
    })
    .await;
    assert_eq!(
        ws.recv().await,
        Some(ServerMsg::Welcome { seat: seat.seat })
    );
    ws.snapshot().await;
    ws
}

#[tokio::test]
async fn hello_is_answered_with_welcome_and_a_snapshot() {
    let (addr, mari, _) = room().await;
    let mut ws = Ws::open(addr, &mari.code.0).await;
    ws.send(&ClientMsg::Hello {
        token: mari.token.clone(),
    })
    .await;
    assert_eq!(
        ws.recv().await,
        Some(ServerMsg::Welcome { seat: SeatId(0) })
    );
    let view = ws.snapshot().await;
    assert!(view.seats[0].connected);
}

#[tokio::test]
async fn the_first_message_must_be_hello() {
    let (addr, mari, _) = room().await;
    let mut ws = Ws::open(addr, &mari.code.0).await;
    ws.send(&ClientMsg::Roll).await;
    assert_eq!(
        ws.recv().await,
        Some(ServerMsg::Error {
            code: ErrorCode::ExpectedHello
        })
    );
    assert_eq!(ws.recv().await, None, "the server closes the socket");
}

#[tokio::test]
async fn an_unknown_token_is_refused() {
    let (addr, mari, _) = room().await;
    let mut ws = Ws::open(addr, &mari.code.0).await;
    ws.send(&ClientMsg::Hello {
        token: SeatToken("nope".into()),
    })
    .await;
    assert_eq!(
        ws.recv().await,
        Some(ServerMsg::Error {
            code: ErrorCode::UnknownToken
        })
    );
    assert_eq!(ws.recv().await, None);
}

#[tokio::test]
async fn a_bad_message_is_reported_but_the_socket_stays_open() {
    let (addr, mari, _) = room().await;
    let mut ws = hello(addr, &mari).await;
    ws.send_raw("not json".into()).await;
    assert_eq!(
        ws.recv().await,
        Some(ServerMsg::Error {
            code: ErrorCode::BadMessage
        })
    );
    ws.send(&ClientMsg::ChooseColor { color: Color::Blue })
        .await;
    assert_eq!(ws.snapshot().await.seats[0].color, Color::Blue);
}

#[tokio::test]
async fn an_oversized_message_closes_the_socket() {
    let (addr, mari, _) = room().await;
    let mut ws = hello(addr, &mari).await;
    ws.send_raw("x".repeat(MAX_MESSAGE + 1)).await;
    assert_eq!(ws.recv().await, None);
}

#[tokio::test]
async fn the_host_starting_reaches_everyone() {
    let (addr, mari, juri) = room().await;
    let mut host = hello(addr, &mari).await;
    let mut guest = hello(addr, &juri).await;
    host.snapshot().await; // Jüri connected.

    host.send(&ClientMsg::Start).await;
    for ws in [&mut host, &mut guest] {
        assert!(matches!(ws.snapshot().await.stage, Stage::Game { .. }));
    }
}

#[tokio::test]
async fn closing_the_socket_disconnects_the_seat() {
    let (addr, mari, juri) = room().await;
    let mut host = hello(addr, &mari).await;
    let guest = hello(addr, &juri).await;
    host.snapshot().await; // Jüri connected.
    drop(guest);
    assert!(!host.snapshot().await.seats[1].connected);
}

#[tokio::test]
async fn an_unknown_room_has_no_socket() {
    let addr = common::serve(common::registry(Limits::default())).await;
    let url = format!("ws://{addr}/api/rooms/ZZZZZZ/ws");
    let err = tokio_tungstenite::connect_async(url).await.unwrap_err();
    let tokio_tungstenite::tungstenite::Error::Http(response) = err else {
        panic!("expected an HTTP refusal, got {err:?}");
    };
    assert_eq!(response.status(), 404);
}
