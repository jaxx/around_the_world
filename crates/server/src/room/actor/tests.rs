//! The room task, driven through its handle with tokio's paused clock: when
//! every task is idle the clock jumps to the next timer, so a 30-minute
//! expiry takes no real time.

use super::*;
use crate::room::dice::{RandomDice, Scripted};
use crate::room::state::ROOM_TTL;
use protocol::{RoomCode, RoomView, Stage};
use rand::SeedableRng;
use rand::rngs::StdRng;
use rules::{Color, Die, Event};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::time::Instant;

fn tok(name: &str) -> SeatToken {
    SeatToken(name.to_string())
}

/// A lobby with Mari (seat 0, host) and Jüri (seat 1), neither connected.
/// The returned receiver fires when the room's task ends.
async fn room(dice: Box<dyn Dice + Send>) -> (RoomHandle, oneshot::Receiver<()>) {
    let now = Instant::now().into_std();
    let (state, _) = RoomState::new(RoomCode("ABCDEF".into()), "Mari", tok("mari"), now).unwrap();
    let (exited, on_exit) = oneshot::channel();
    let handle = spawn(state, dice, move || {
        let _ = exited.send(());
    });
    handle.join("Jüri", tok("juri")).await.unwrap();
    (handle, on_exit)
}

struct Client {
    conn: ConnId,
    rx: mpsc::Receiver<ServerMsg>,
}

impl Client {
    async fn next(&mut self) -> ServerMsg {
        self.rx.recv().await.expect("connection open")
    }

    async fn snapshot(&mut self) -> RoomView {
        match self.next().await {
            ServerMsg::Snapshot { room } => room,
            other => panic!("expected a snapshot, got {other:?}"),
        }
    }

    fn nothing_queued(&mut self) -> bool {
        matches!(self.rx.try_recv(), Err(TryRecvError::Empty))
    }
}

async fn connect(handle: &RoomHandle, name: &str) -> Client {
    let (outbox, rx) = mpsc::channel(OUTBOX_LEN);
    let conn = handle.connect(tok(name), outbox).await.unwrap();
    Client { conn, rx }
}

/// Mari and Jüri both connected, each having read their welcome and the
/// snapshots so far.
async fn both_connected(handle: &RoomHandle) -> (Client, Client) {
    let mut mari = connect(handle, "mari").await;
    assert_eq!(mari.next().await, ServerMsg::Welcome { seat: SeatId(0) });
    mari.snapshot().await;
    let mut juri = connect(handle, "juri").await;
    assert_eq!(juri.next().await, ServerMsg::Welcome { seat: SeatId(1) });
    juri.snapshot().await;
    mari.snapshot().await;
    (mari, juri)
}

#[tokio::test(start_paused = true)]
async fn connecting_welcomes_the_seat_and_tells_everyone() {
    let (handle, _exit) = room(Box::new(Scripted::new(&[]))).await;
    let mut mari = connect(&handle, "mari").await;
    assert_eq!(mari.next().await, ServerMsg::Welcome { seat: SeatId(0) });
    assert!(mari.snapshot().await.seats[0].connected);

    let mut juri = connect(&handle, "juri").await;
    assert_eq!(juri.next().await, ServerMsg::Welcome { seat: SeatId(1) });
    assert!(juri.snapshot().await.seats[1].connected);
    assert!(mari.snapshot().await.seats[1].connected);
}

#[tokio::test(start_paused = true)]
async fn an_unknown_token_is_refused() {
    let (handle, _exit) = room(Box::new(Scripted::new(&[]))).await;
    let (outbox, _rx) = mpsc::channel(OUTBOX_LEN);
    assert_eq!(
        handle.connect(tok("nobody"), outbox).await,
        Err(ErrorCode::UnknownToken)
    );
}

#[tokio::test(start_paused = true)]
async fn errors_go_only_to_the_sender() {
    let (handle, _exit) = room(Box::new(Scripted::new(&[]))).await;
    let (mut mari, mut juri) = both_connected(&handle).await;
    handle.send(juri.conn, ClientMsg::Start).await;
    assert_eq!(
        juri.next().await,
        ServerMsg::Error {
            code: ErrorCode::NotHost
        }
    );
    assert!(mari.nothing_queued());
}

#[tokio::test(start_paused = true)]
async fn changes_and_events_go_to_everyone() {
    let (handle, _exit) = room(Box::new(Scripted::new(&[6]))).await;
    let (mut mari, mut juri) = both_connected(&handle).await;
    handle.send(mari.conn, ClientMsg::Start).await;
    for client in [&mut mari, &mut juri] {
        assert!(matches!(client.snapshot().await.stage, Stage::Game { .. }));
    }

    handle.send(mari.conn, ClientMsg::Roll).await;
    let rolled = ServerMsg::Events {
        events: vec![Event::Rolled {
            color: Color::Red,
            die: Die::new(6).unwrap(),
        }],
    };
    for client in [&mut mari, &mut juri] {
        assert_eq!(client.next().await, rolled);
        client.snapshot().await;
    }
}

#[tokio::test(start_paused = true)]
async fn the_turn_timer_fires_on_its_own() {
    let (handle, _exit) = room(Box::new(Scripted::new(&[2]))).await;
    let (mut mari, _juri) = both_connected(&handle).await;
    handle.send(mari.conn, ClientMsg::Start).await;
    mari.snapshot().await;

    let started = Instant::now();
    let ServerMsg::Events { events } = mari.next().await else {
        panic!("expected the timeout roll");
    };
    assert_eq!(
        events,
        vec![Event::Rolled {
            color: Color::Red,
            die: Die::new(2).unwrap()
        }]
    );
    assert!(started.elapsed() >= crate::room::state::TURN_TIME);
}

#[tokio::test(start_paused = true)]
async fn a_new_connection_replaces_the_old_one() {
    let (handle, _exit) = room(Box::new(Scripted::new(&[]))).await;
    let mut first = connect(&handle, "mari").await;
    let mut second = connect(&handle, "mari").await;
    assert_eq!(second.next().await, ServerMsg::Welcome { seat: SeatId(0) });
    while first.rx.recv().await.is_some() {}

    handle.disconnect(first.conn).await;
    let view = second.snapshot().await;
    assert!(
        view.seats[0].connected,
        "the stale connection's close is ignored"
    );
}

#[tokio::test(start_paused = true)]
async fn disconnecting_is_shown_to_the_others() {
    let (handle, _exit) = room(Box::new(Scripted::new(&[]))).await;
    let (mut mari, juri) = both_connected(&handle).await;
    handle.disconnect(juri.conn).await;
    assert!(!mari.snapshot().await.seats[1].connected);
}

#[tokio::test(start_paused = true)]
async fn the_room_closes_when_everyone_has_left() {
    let (handle, exit) = room(Box::new(Scripted::new(&[]))).await;
    let (mari, juri) = both_connected(&handle).await;
    handle.send(juri.conn, ClientMsg::Leave).await;
    handle.send(mari.conn, ClientMsg::Leave).await;
    exit.await.unwrap();
    assert_eq!(
        handle.join("Kati", tok("kati")).await,
        Err(ErrorCode::RoomNotFound)
    );
}

#[tokio::test(start_paused = true)]
async fn an_abandoned_game_closes_after_the_room_ttl() {
    let dice = RandomDice(StdRng::seed_from_u64(1));
    let (handle, exit) = room(Box::new(dice)).await;
    let (mari, juri) = both_connected(&handle).await;
    handle.send(mari.conn, ClientMsg::Start).await;
    handle.disconnect(mari.conn).await;
    handle.disconnect(juri.conn).await;

    let left = Instant::now();
    exit.await.unwrap();
    assert!(left.elapsed() >= ROOM_TTL);
}
