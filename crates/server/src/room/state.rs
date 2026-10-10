//! Pure room logic: seats, host, colours, the game and its turn timer. No
//! I/O, clock or randomness of its own; the room actor passes `now` and the
//! dice in, calls [`RoomState::tick`] at [`RoomState::next_deadline`], and
//! broadcasts a fresh [`RoomView`] after every change.

use std::time::{Duration, Instant};

use protocol::{
    ClientMsg, ErrorCode, RoomCode, RoomView, SeatId, SeatToken, SeatView, Stage as ViewStage,
};
use rules::{Action, Color, Die, Event, Game, Phase, PieceId};

use crate::room::dice::Dice;

/// Most seats in a room: one per colour.
pub const MAX_SEATS: usize = 4;
/// Longest nickname, in characters, after trimming.
pub const MAX_NICKNAME: usize = 20;
/// How long a lobby seat may stay disconnected before it is given up.
pub const LOBBY_GRACE: Duration = Duration::from_secs(30);
/// How long a seat may stay disconnected mid-game before it is away (eemal).
pub const GAME_GRACE: Duration = Duration::from_secs(10);
/// The turn timer (käiguaeg).
pub const TURN_TIME: Duration = Duration::from_secs(30);
/// Pause before the server plays for an away seat or a single legal move.
pub const AUTO_DELAY: Duration = Duration::from_secs(1);
/// Timeouts in a row after which a seat is away.
pub const AWAY_AFTER_TIMEOUTS: u8 = 3;
/// How long a room lives after its game ends or its last connection drops.
pub const ROOM_TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone)]
struct Seat {
    id: SeatId,
    token: SeatToken,
    nickname: String,
    color: Color,
    connected: bool,
    /// When the connection dropped, while it stays dropped.
    disconnected_at: Option<Instant>,
    /// The server plays for this seat (eemal).
    away: bool,
    /// Turn-timer timeouts in a row.
    timeouts: u8,
}

#[derive(Debug, Clone)]
enum Stage {
    /// Ootesaal.
    Lobby,
    Game(Game),
}

/// When the server acts for the current seat, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Deadline {
    at: Instant,
    kind: DeadlineKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeadlineKind {
    /// The turn timer: running out counts against the seat.
    Timeout,
    /// The seat is away or has a single legal move: the server just plays.
    Auto,
}

#[derive(Debug, Clone)]
pub struct RoomState {
    code: RoomCode,
    /// In join order, which is also host succession order.
    seats: Vec<Seat>,
    host: SeatId,
    stage: Stage,
    deadline: Option<Deadline>,
    /// When the current game ended.
    finished_at: Option<Instant>,
}

impl RoomState {
    /// A new room in the lobby; its creator takes the first seat and is host.
    pub fn new(
        code: RoomCode,
        nickname: &str,
        token: SeatToken,
        now: Instant,
    ) -> Result<(RoomState, SeatId), ErrorCode> {
        let mut room = RoomState {
            code,
            seats: Vec::new(),
            host: SeatId(0),
            stage: Stage::Lobby,
            deadline: None,
            finished_at: None,
        };
        let seat = room.join(nickname, token, now)?;
        room.host = seat;
        Ok((room, seat))
    }

    /// Takes a free seat in the lobby, with the first free colour clockwise.
    pub fn join(
        &mut self,
        nickname: &str,
        token: SeatToken,
        now: Instant,
    ) -> Result<SeatId, ErrorCode> {
        if !matches!(self.stage, Stage::Lobby) {
            return Err(ErrorCode::NotInLobby);
        }
        let nickname = nickname.trim();
        if nickname.is_empty() || nickname.chars().count() > MAX_NICKNAME {
            return Err(ErrorCode::BadNickname);
        }
        if self.seats.len() >= MAX_SEATS {
            return Err(ErrorCode::RoomFull);
        }
        let id = (0..)
            .map(SeatId)
            .find(|id| self.seats.iter().all(|s| s.id != *id))
            .expect("fewer than four seats are taken");
        let color = Color::ALL
            .into_iter()
            .find(|&c| self.seats.iter().all(|s| s.color != c))
            .expect("fewer than four colours are taken");
        self.seats.push(Seat {
            id,
            token,
            nickname: nickname.to_string(),
            color,
            connected: false,
            disconnected_at: Some(now),
            away: false,
            timeouts: 0,
        });
        Ok(id)
    }

    /// The seat holding `token`, if any.
    pub fn seat_for(&self, token: &SeatToken) -> Option<SeatId> {
        self.seats.iter().find(|s| &s.token == token).map(|s| s.id)
    }

    /// Applies a client message from `seat`. `Hello` is the shell's business.
    pub fn handle(
        &mut self,
        seat: SeatId,
        msg: ClientMsg,
        now: Instant,
        dice: &mut (impl Dice + ?Sized),
    ) -> Result<Vec<Event>, ErrorCode> {
        match (&self.stage, msg) {
            (_, ClientMsg::Hello { .. }) => Err(ErrorCode::BadMessage),
            (Stage::Lobby, ClientMsg::ChooseColor { color }) => self.choose_color(seat, color),
            (Stage::Lobby, ClientMsg::Leave) => {
                self.remove_seat(seat);
                Ok(Vec::new())
            }
            (Stage::Lobby, ClientMsg::Start) => self.start(seat, now),
            (Stage::Lobby, ClientMsg::Roll | ClientMsg::Move { .. }) => Err(ErrorCode::NotInGame),
            (Stage::Lobby, ClientMsg::Rematch) => Err(ErrorCode::GameNotOver),
            (
                Stage::Game(_),
                ClientMsg::ChooseColor { .. } | ClientMsg::Leave | ClientMsg::Start,
            ) => Err(ErrorCode::NotInLobby),
            (Stage::Game(_), ClientMsg::Roll) => self.play(seat, None, now, dice),
            (Stage::Game(_), ClientMsg::Move { piece }) => self.play(seat, Some(piece), now, dice),
            (Stage::Game(_), ClientMsg::Rematch) => self.rematch(seat),
        }
    }

    fn choose_color(&mut self, seat: SeatId, color: Color) -> Result<Vec<Event>, ErrorCode> {
        if self.seats.iter().any(|s| s.color == color && s.id != seat) {
            return Err(ErrorCode::ColorTaken);
        }
        self.seat_mut(seat).color = color;
        Ok(Vec::new())
    }

    fn start(&mut self, seat: SeatId, now: Instant) -> Result<Vec<Event>, ErrorCode> {
        if seat != self.host {
            return Err(ErrorCode::NotHost);
        }
        if self.seats.len() < 2 {
            return Err(ErrorCode::NotEnoughPlayers);
        }
        let colors: Vec<Color> = self.seats.iter().map(|s| s.color).collect();
        let game = Game::new(&colors).map_err(ErrorCode::Rule)?;
        self.stage = Stage::Game(game);
        self.reschedule(now);
        Ok(Vec::new())
    }

    /// A seat's own roll (`piece` is `None`) or move. Acting clears away.
    fn play(
        &mut self,
        seat: SeatId,
        piece: Option<PieceId>,
        now: Instant,
        dice: &mut (impl Dice + ?Sized),
    ) -> Result<Vec<Event>, ErrorCode> {
        let Stage::Game(game) = &self.stage else {
            unreachable!("caller checked the stage");
        };
        let color = self.seat(seat).color;
        let action = match piece {
            Some(piece) => Action::Move(piece),
            None => {
                // Check the roll is allowed before taking a value from the dice.
                let probe = Die::new(1).expect("1 is a valid die");
                game.apply(color, Action::Roll(probe))
                    .map_err(ErrorCode::Rule)?;
                Action::Roll(dice.roll())
            }
        };
        let (next, events) = game.apply(color, action).map_err(ErrorCode::Rule)?;
        let seat = self.seat_mut(seat);
        seat.timeouts = 0;
        seat.away = false;
        self.advance(next, &events, now);
        Ok(events)
    }

    /// What the server does when the deadline falls due: roll, or play the
    /// first legal move.
    fn auto_play(
        &mut self,
        kind: DeadlineKind,
        now: Instant,
        dice: &mut (impl Dice + ?Sized),
    ) -> Vec<Event> {
        let Stage::Game(game) = &self.stage else {
            unreachable!("deadlines are only set during a game");
        };
        let color = game.current();
        let action = match game.phase() {
            Phase::AwaitingMove { .. } => Action::Move(game.legal_moves()[0].piece),
            _ => Action::Roll(dice.roll()),
        };
        let (next, events) = game
            .apply(color, action)
            .expect("the server only plays legal actions");
        if kind == DeadlineKind::Timeout {
            let seat = self.seat_of_mut(color);
            seat.timeouts += 1;
            if seat.timeouts >= AWAY_AFTER_TIMEOUTS {
                seat.away = true;
            }
        }
        self.advance(next, &events, now);
        events
    }

    fn advance(&mut self, next: Game, events: &[Event], now: Instant) {
        self.stage = Stage::Game(next);
        if events.iter().any(|e| matches!(e, Event::GameOver { .. })) {
            self.finished_at = Some(now);
        }
        self.reschedule(now);
    }

    fn rematch(&mut self, seat: SeatId) -> Result<Vec<Event>, ErrorCode> {
        let Stage::Game(game) = &self.stage else {
            unreachable!("caller checked the stage");
        };
        if *game.phase() != Phase::GameOver {
            return Err(ErrorCode::GameNotOver);
        }
        if seat != self.host {
            return Err(ErrorCode::NotHost);
        }
        self.stage = Stage::Lobby;
        self.deadline = None;
        self.finished_at = None;
        for seat in &mut self.seats {
            seat.away = false;
            seat.timeouts = 0;
        }
        Ok(Vec::new())
    }

    /// Re-arms the turn timer for the current state of play.
    fn reschedule(&mut self, now: Instant) {
        self.deadline = match &self.stage {
            Stage::Game(game) if *game.phase() != Phase::GameOver => {
                let away = self.seat_of(game.current()).away;
                let forced = matches!(game.phase(), Phase::AwaitingMove { .. })
                    && game.legal_moves().len() == 1;
                let (kind, wait) = if away || forced {
                    (DeadlineKind::Auto, AUTO_DELAY)
                } else {
                    (DeadlineKind::Timeout, TURN_TIME)
                };
                Some(Deadline {
                    at: now + wait,
                    kind,
                })
            }
            _ => None,
        };
    }

    /// Removes a seat; if it was the host, the earliest remaining seat hosts.
    fn remove_seat(&mut self, seat: SeatId) {
        self.seats.retain(|s| s.id != seat);
        if seat == self.host
            && let Some(next) = self.seats.first()
        {
            self.host = next.id;
        }
    }

    /// The seat's WebSocket said hello. Reconnecting clears away.
    pub fn connect(&mut self, seat: SeatId, now: Instant) {
        let seat = self.seat_mut(seat);
        seat.connected = true;
        seat.disconnected_at = None;
        if seat.away {
            seat.away = false;
            seat.timeouts = 0;
            let color = seat.color;
            if self.is_current(color) {
                self.reschedule(now);
            }
        }
    }

    pub fn disconnect(&mut self, seat: SeatId, now: Instant) {
        let seat = self.seat_mut(seat);
        seat.connected = false;
        seat.disconnected_at = Some(now);
    }

    /// Applies everything due by `now`: lobby seats past [`LOBBY_GRACE`] are
    /// given up, game seats past [`GAME_GRACE`] go away, and an expired turn
    /// timer makes the server act. Returns the game events that produced.
    pub fn tick(&mut self, now: Instant, dice: &mut (impl Dice + ?Sized)) -> Vec<Event> {
        match &self.stage {
            Stage::Lobby => {
                let expired: Vec<SeatId> = self
                    .seats
                    .iter()
                    .filter(|s| s.disconnected_at.is_some_and(|at| now >= at + LOBBY_GRACE))
                    .map(|s| s.id)
                    .collect();
                for seat in expired {
                    self.remove_seat(seat);
                }
                Vec::new()
            }
            Stage::Game(game) => {
                let current = game.current();
                let mut current_went_away = false;
                for seat in &mut self.seats {
                    if !seat.away
                        && seat
                            .disconnected_at
                            .is_some_and(|at| now >= at + GAME_GRACE)
                    {
                        seat.away = true;
                        current_went_away |= seat.color == current;
                    }
                }
                if current_went_away {
                    self.reschedule(now);
                }
                match self.deadline {
                    Some(deadline) if deadline.at <= now => {
                        self.auto_play(deadline.kind, now, dice)
                    }
                    _ => Vec::new(),
                }
            }
        }
    }

    /// When [`tick`](Self::tick) next has something to do.
    pub fn next_deadline(&self) -> Option<Instant> {
        let grace = match self.stage {
            Stage::Lobby => LOBBY_GRACE,
            Stage::Game(_) => GAME_GRACE,
        };
        let graces = self
            .seats
            .iter()
            .filter(|s| !s.away)
            .filter_map(|s| s.disconnected_at.map(|at| at + grace));
        graces.chain(self.deadline.map(|d| d.at)).min()
    }

    /// When the room may be dropped: [`ROOM_TTL`] after its game ended or
    /// after its last connection dropped, whichever is sooner.
    pub fn expires_at(&self) -> Option<Instant> {
        let ended = self.finished_at;
        let abandoned = if self.seats.iter().all(|s| !s.connected) {
            self.seats.iter().filter_map(|s| s.disconnected_at).max()
        } else {
            None
        };
        [ended, abandoned]
            .into_iter()
            .flatten()
            .min()
            .map(|at| at + ROOM_TTL)
    }

    pub fn has_seat(&self, seat: SeatId) -> bool {
        self.seats.iter().any(|s| s.id == seat)
    }

    /// No seats left; the room can be dropped.
    pub fn is_empty(&self) -> bool {
        self.seats.is_empty()
    }

    /// The room as clients see it.
    pub fn view(&self, now: Instant) -> RoomView {
        RoomView {
            code: self.code.clone(),
            host: self.host,
            seats: self
                .seats
                .iter()
                .map(|s| SeatView {
                    id: s.id,
                    nickname: s.nickname.clone(),
                    color: s.color,
                    connected: s.connected,
                    away: s.away,
                })
                .collect(),
            stage: match &self.stage {
                Stage::Lobby => ViewStage::Lobby,
                Stage::Game(game) => ViewStage::Game { game: game.clone() },
            },
            turn_ms_left: self
                .deadline
                .filter(|d| d.kind == DeadlineKind::Timeout)
                .map(|d| d.at.saturating_duration_since(now).as_millis() as u32),
        }
    }

    fn is_current(&self, color: Color) -> bool {
        matches!(&self.stage, Stage::Game(game) if game.current() == color)
    }

    fn seat(&self, seat: SeatId) -> &Seat {
        self.seats
            .iter()
            .find(|s| s.id == seat)
            .expect("the shell only passes seats it got from this room")
    }

    fn seat_mut(&mut self, seat: SeatId) -> &mut Seat {
        self.seats
            .iter_mut()
            .find(|s| s.id == seat)
            .expect("the shell only passes seats it got from this room")
    }

    fn seat_of(&self, color: Color) -> &Seat {
        self.seats
            .iter()
            .find(|s| s.color == color)
            .expect("every game colour has a seat")
    }

    fn seat_of_mut(&mut self, color: Color) -> &mut Seat {
        self.seats
            .iter_mut()
            .find(|s| s.color == color)
            .expect("every game colour has a seat")
    }
}

#[cfg(test)]
mod game_tests;
#[cfg(test)]
mod tests;
