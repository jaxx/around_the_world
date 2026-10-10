//! Messages between the server and the browser client, as JSON.
//!
//! Joining happens over HTTP ([`CreateRoom`], [`JoinRoom`] → [`Joined`]);
//! play happens over a WebSocket whose first message is
//! [`ClientMsg::Hello`]. The client never sends a die value: the server
//! rolls and turns [`ClientMsg::Roll`] into a `rules::Action`.

use rules::{Color, Event, Game, PieceId, RuleError};
use serde::{Deserialize, Serialize};

/// A room's invite code (kutselink), e.g. `K7MXQ2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoomCode(pub String);

/// The secret that proves which seat a connection belongs to. Kept in the
/// browser per room; never put in a URL.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SeatToken(pub String);

/// A seat in a room, stable for the room's lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SeatId(pub u8);

/// `POST /api/rooms`: create a room; the creator becomes host (võõrustaja).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateRoom {
    pub nickname: String,
}

/// `POST /api/rooms/{code}/join`: take a seat in a room's lobby (ootesaal).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinRoom {
    pub nickname: String,
}

/// Reply to [`CreateRoom`] and [`JoinRoom`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Joined {
    pub code: RoomCode,
    pub token: SeatToken,
    pub seat: SeatId,
}

/// Body of a failed HTTP request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: ErrorCode,
}

/// WebSocket messages from the client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Must be the first message on a connection.
    Hello {
        token: SeatToken,
    },
    /// Swap to a free colour while in the lobby.
    ChooseColor {
        color: Color,
    },
    /// Host only: start the game with 2–4 seats.
    Start,
    Roll,
    Move {
        piece: PieceId,
    },
    /// Give up the seat; lobby only.
    Leave,
    /// Host only, after game over: back to the lobby with the same seats (uus mäng).
    Rematch,
    /// Sent every few minutes; the server ignores it. It only keeps a
    /// sleeping free host awake (docs/adr/0002-heartbeat-for-free-hosting.md).
    Ping,
}

/// WebSocket messages from the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    /// Reply to a valid [`ClientMsg::Hello`]: which seat this connection is.
    Welcome { seat: SeatId },
    /// The whole room, sent after every change.
    Snapshot { room: RoomView },
    /// What the last action did, in order, for animation.
    Events { events: Vec<Event> },
    /// The last message was refused; nothing changed.
    Error { code: ErrorCode },
}

/// Everything a client needs to draw the room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomView {
    pub code: RoomCode,
    pub host: SeatId,
    pub seats: Vec<SeatView>,
    pub stage: Stage,
    /// Time left on the turn timer (käiguaeg), if one is running. Relative,
    /// so the browser's clock does not matter.
    pub turn_ms_left: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatView {
    pub id: SeatId,
    pub nickname: String,
    pub color: Color,
    pub connected: bool,
    /// The server is playing for this seat (eemal).
    pub away: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum Stage {
    /// Ootesaal: seats join, leave and pick colours.
    Lobby,
    /// A game in progress or over; `game.phase()` tells which.
    Game { game: Game },
}

/// Why a request or message was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The game rules refused the action.
    Rule(RuleError),
    /// Not valid JSON for a [`ClientMsg`].
    BadMessage,
    /// The first message on a connection must be [`ClientMsg::Hello`].
    ExpectedHello,
    /// No seat in this room has that token.
    UnknownToken,
    RoomNotFound,
    /// The room already has four seats.
    RoomFull,
    /// The server is at its room limit.
    TooManyRooms,
    /// Too many rooms created from this address recently.
    RateLimited,
    /// Nicknames are 1–20 characters after trimming.
    BadNickname,
    NotHost,
    /// Only possible in the lobby, before the game starts.
    NotInLobby,
    /// Only possible once a game has started.
    NotInGame,
    ColorTaken,
    /// Starting needs at least two seats.
    NotEnoughPlayers,
    /// A rematch is only possible after game over.
    GameNotOver,
}
