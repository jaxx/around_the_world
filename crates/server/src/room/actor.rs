//! One tokio task per room. It owns the [`RoomState`], its dice and its
//! connections; everything else talks to it through a [`RoomHandle`].

use std::collections::HashMap;
use std::time::Instant;

use protocol::{ClientMsg, ErrorCode, SeatId, SeatToken, ServerMsg};
use rules::Event;
use tokio::sync::{mpsc, oneshot};

use crate::room::dice::Dice;
use crate::room::state::RoomState;

/// Messages queued for one connection before it counts as too slow and is
/// dropped.
pub const OUTBOX_LEN: usize = 64;

/// One open WebSocket, as far as its room is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnId(u64);

enum Command {
    Join {
        nickname: String,
        token: SeatToken,
        reply: oneshot::Sender<Result<SeatId, ErrorCode>>,
    },
    Connect {
        token: SeatToken,
        outbox: mpsc::Sender<ServerMsg>,
        reply: oneshot::Sender<Result<ConnId, ErrorCode>>,
    },
    Message {
        conn: ConnId,
        msg: ClientMsg,
    },
    Disconnect {
        conn: ConnId,
    },
}

/// A cheap, cloneable way to reach a room's task.
#[derive(Clone)]
pub struct RoomHandle {
    tx: mpsc::Sender<Command>,
}

impl RoomHandle {
    /// Takes a lobby seat. `RoomNotFound` if the room has closed.
    pub async fn join(&self, nickname: &str, token: SeatToken) -> Result<SeatId, ErrorCode> {
        let (reply, answer) = oneshot::channel();
        self.ask(
            Command::Join {
                nickname: nickname.to_string(),
                token,
                reply,
            },
            answer,
        )
        .await
    }

    /// Attaches a connection to the seat holding `token`. The room sends it
    /// `Welcome` and from then on every snapshot, event and its own errors.
    pub async fn connect(
        &self,
        token: SeatToken,
        outbox: mpsc::Sender<ServerMsg>,
    ) -> Result<ConnId, ErrorCode> {
        let (reply, answer) = oneshot::channel();
        self.ask(
            Command::Connect {
                token,
                outbox,
                reply,
            },
            answer,
        )
        .await
    }

    /// Passes on a message from a connected client.
    pub async fn send(&self, conn: ConnId, msg: ClientMsg) {
        let _ = self.tx.send(Command::Message { conn, msg }).await;
    }

    /// The connection closed.
    pub async fn disconnect(&self, conn: ConnId) {
        let _ = self.tx.send(Command::Disconnect { conn }).await;
    }

    async fn ask<T>(
        &self,
        command: Command,
        answer: oneshot::Receiver<Result<T, ErrorCode>>,
    ) -> Result<T, ErrorCode> {
        self.tx
            .send(command)
            .await
            .map_err(|_| ErrorCode::RoomNotFound)?;
        answer.await.unwrap_or(Err(ErrorCode::RoomNotFound))
    }
}

/// Starts the room's task. `on_exit` runs once the room is empty or has
/// expired, just before the task ends.
pub fn spawn(
    state: RoomState,
    dice: Box<dyn Dice + Send>,
    on_exit: impl FnOnce() + Send + 'static,
) -> RoomHandle {
    let (tx, rx) = mpsc::channel(OUTBOX_LEN);
    let actor = Actor {
        state,
        dice,
        conns: HashMap::new(),
        next_conn: 0,
    };
    tokio::spawn(actor.run(rx, on_exit));
    RoomHandle { tx }
}

struct Conn {
    seat: SeatId,
    outbox: mpsc::Sender<ServerMsg>,
}

struct Actor {
    state: RoomState,
    dice: Box<dyn Dice + Send>,
    conns: HashMap<ConnId, Conn>,
    next_conn: u64,
}

impl Actor {
    async fn run(mut self, mut rx: mpsc::Receiver<Command>, on_exit: impl FnOnce()) {
        loop {
            let wake = [self.state.next_deadline(), self.state.expires_at()]
                .into_iter()
                .flatten()
                .min();
            tokio::select! {
                command = rx.recv() => match command {
                    Some(command) => self.command(command),
                    None => break,
                },
                () = sleep_until(wake) => self.tick(),
            }
            let closed =
                self.state.is_empty() || self.state.expires_at().is_some_and(|at| at <= now());
            if closed {
                break;
            }
        }
        rx.close();
        on_exit();
    }

    fn command(&mut self, command: Command) {
        let now = now();
        match command {
            Command::Join {
                nickname,
                token,
                reply,
            } => {
                let joined = self.state.join(&nickname, token, now);
                let ok = joined.is_ok();
                let _ = reply.send(joined);
                if ok {
                    self.broadcast_snapshot();
                }
            }
            Command::Connect {
                token,
                outbox,
                reply,
            } => {
                let Some(seat) = self.state.seat_for(&token) else {
                    let _ = reply.send(Err(ErrorCode::UnknownToken));
                    return;
                };
                self.conns.retain(|_, c| c.seat != seat);
                let conn = ConnId(self.next_conn);
                self.next_conn += 1;
                self.conns.insert(conn, Conn { seat, outbox });
                self.state.connect(seat, now);
                let _ = reply.send(Ok(conn));
                self.deliver(conn, ServerMsg::Welcome { seat });
                self.broadcast_snapshot();
            }
            Command::Message { conn, msg } => {
                let Some(seat) = self.conns.get(&conn).map(|c| c.seat) else {
                    return;
                };
                match self.state.handle(seat, msg, now, &mut *self.dice) {
                    Ok(events) => self.broadcast_change(events),
                    Err(code) => self.deliver(conn, ServerMsg::Error { code }),
                }
            }
            Command::Disconnect { conn } => {
                if let Some(conn) = self.conns.remove(&conn) {
                    self.state.disconnect(conn.seat, now);
                    self.broadcast_snapshot();
                }
            }
        }
    }

    fn tick(&mut self) {
        let events = self.state.tick(now(), &mut *self.dice);
        self.broadcast_change(events);
    }

    fn broadcast_change(&mut self, events: Vec<Event>) {
        // Seats can disappear (leaving, lobby grace); their connections go too.
        let state = &self.state;
        self.conns.retain(|_, c| state.has_seat(c.seat));
        if !events.is_empty() {
            self.broadcast(ServerMsg::Events { events });
        }
        self.broadcast_snapshot();
    }

    fn broadcast_snapshot(&mut self) {
        let room = self.state.view(now());
        self.broadcast(ServerMsg::Snapshot { room });
    }

    fn broadcast(&mut self, msg: ServerMsg) {
        let conns: Vec<ConnId> = self.conns.keys().copied().collect();
        for conn in conns {
            self.deliver(conn, msg.clone());
        }
    }

    /// Queues `msg` for one connection. A connection that is gone or too far
    /// behind is dropped and its seat counts as disconnected.
    fn deliver(&mut self, conn: ConnId, msg: ServerMsg) {
        let Some(target) = self.conns.get(&conn) else {
            return;
        };
        if target.outbox.try_send(msg).is_err() {
            let seat = target.seat;
            self.conns.remove(&conn);
            self.state.disconnect(seat, now());
        }
    }
}

/// The current time, from tokio's clock so tests can pause it.
fn now() -> Instant {
    tokio::time::Instant::now().into_std()
}

async fn sleep_until(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(tokio::time::Instant::from_std(at)).await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod tests;
