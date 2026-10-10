//! The state of one seat's connection to its room, and what to do as the
//! socket opens, talks and closes. Pure: the socket loop in
//! [`crate::socket`] feeds it and follows its advice.

use std::time::Duration;

use protocol::{ErrorCode, RoomView, SeatId, ServerMsg, Stage};
use rules::{Color, Die, Event, Game};

/// How often an open connection sends a heartbeat. A free host sleeps after
/// 15 minutes without incoming messages (docs/adr/0002).
pub const HEARTBEAT: Duration = Duration::from_secs(5 * 60);

/// Longest wait between reconnection attempts.
pub const MAX_RETRY: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Opening the socket or waiting for `Welcome`.
    Connecting,
    /// Welcomed; snapshots flow.
    Live,
    /// The connection dropped; trying again (`attempt` counts from 1).
    Retrying { attempt: u32 },
    /// The server will not take this seat back; no more attempts.
    Refused(ErrorCode),
}

/// Something the page must do after a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    None,
    /// The stored seat is no longer valid: drop it.
    ForgetSeat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub status: Status,
    /// This browser's seat, once welcomed.
    pub me: Option<SeatId>,
    /// The latest snapshot.
    pub room: Option<RoomView>,
    /// The last refusal of something this seat tried, until the next snapshot.
    pub problem: Option<ErrorCode>,
    /// The latest roll and who made it, for the die.
    pub last_roll: Option<(Color, Die)>,
    /// Event batches not yet animated, oldest first.
    batches: Vec<Batch>,
    /// Failed connections in a row.
    failures: u32,
}

/// One action's events and the game as it stood before them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Batch {
    pub before: Option<Game>,
    pub events: Vec<Event>,
}

impl Default for Session {
    fn default() -> Session {
        Session {
            status: Status::Connecting,
            me: None,
            room: None,
            problem: None,
            last_roll: None,
            batches: Vec::new(),
            failures: 0,
        }
    }
}

impl Session {
    /// The event batches received since the last call, oldest first.
    pub fn take_batches(&mut self) -> Vec<Batch> {
        std::mem::take(&mut self.batches)
    }

    /// The socket is open and `Hello` has been sent.
    pub fn opened(&mut self) {
        if !matches!(self.status, Status::Refused(_)) {
            self.status = Status::Connecting;
        }
    }

    pub fn received(&mut self, msg: ServerMsg) -> Effect {
        match msg {
            ServerMsg::Welcome { seat } => {
                self.status = Status::Live;
                self.me = Some(seat);
                self.failures = 0;
            }
            ServerMsg::Snapshot { room } => {
                self.room = Some(room);
                self.problem = None;
            }
            ServerMsg::Error {
                code: code @ (ErrorCode::UnknownToken | ErrorCode::ExpectedHello),
            } => {
                self.status = Status::Refused(code);
                if code == ErrorCode::UnknownToken {
                    return Effect::ForgetSeat;
                }
            }
            ServerMsg::Error { code } => self.problem = Some(code),
            ServerMsg::Events { events } => {
                for event in &events {
                    if let &Event::Rolled { color, die } = event {
                        self.last_roll = Some((color, die));
                    }
                }
                // Events come just before their snapshot, so the room still
                // holds the game as it was before them.
                let before = self.room.as_ref().and_then(|room| match &room.stage {
                    Stage::Game { game } => Some(game.clone()),
                    Stage::Lobby => None,
                });
                self.batches.push(Batch { before, events });
            }
        }
        Effect::None
    }

    /// The socket closed (or never opened). How long to wait before trying
    /// again, or `None` to stop.
    pub fn closed(&mut self) -> Option<Duration> {
        if matches!(self.status, Status::Refused(_)) {
            return None;
        }
        self.failures += 1;
        self.status = Status::Retrying {
            attempt: self.failures,
        };
        let wait = Duration::from_secs(1 << (self.failures - 1).min(4));
        Some(wait.min(MAX_RETRY))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::{RoomCode, Stage};
    use rules::RuleError;

    fn room() -> RoomView {
        RoomView {
            code: RoomCode("K7MXQ2".into()),
            host: SeatId(0),
            seats: vec![],
            stage: Stage::Lobby,
            turn_ms_left: None,
        }
    }

    fn live() -> Session {
        let mut s = Session::default();
        s.opened();
        s.received(ServerMsg::Welcome { seat: SeatId(1) });
        s
    }

    #[test]
    fn welcome_makes_the_session_live_and_names_my_seat() {
        let mut s = Session::default();
        assert_eq!(s.status, Status::Connecting);
        s.opened();
        assert_eq!(s.status, Status::Connecting, "not live until welcomed");
        assert_eq!(
            s.received(ServerMsg::Welcome { seat: SeatId(1) }),
            Effect::None
        );
        assert_eq!(s.status, Status::Live);
        assert_eq!(s.me, Some(SeatId(1)));
    }

    #[test]
    fn snapshots_replace_the_room() {
        let mut s = live();
        s.received(ServerMsg::Snapshot { room: room() });
        assert_eq!(s.room, Some(room()));
    }

    #[test]
    fn reconnecting_waits_longer_each_time_up_to_ten_seconds() {
        let mut s = live();
        let waits: Vec<u64> = (0..6).map(|_| s.closed().unwrap().as_secs()).collect();
        assert_eq!(waits, vec![1, 2, 4, 8, 10, 10]);
        assert_eq!(s.status, Status::Retrying { attempt: 6 });
    }

    #[test]
    fn a_welcome_resets_the_waiting() {
        let mut s = live();
        s.closed();
        s.closed();
        s.opened();
        assert_eq!(s.status, Status::Connecting);
        s.received(ServerMsg::Welcome { seat: SeatId(1) });
        assert_eq!(s.closed(), Some(Duration::from_secs(1)));
    }

    #[test]
    fn the_last_snapshot_stays_visible_while_reconnecting() {
        let mut s = live();
        s.received(ServerMsg::Snapshot { room: room() });
        s.closed();
        assert_eq!(s.room, Some(room()));
    }

    #[test]
    fn an_unknown_token_forgets_the_seat_and_stops() {
        let mut s = Session::default();
        s.opened();
        let effect = s.received(ServerMsg::Error {
            code: ErrorCode::UnknownToken,
        });
        assert_eq!(effect, Effect::ForgetSeat);
        assert_eq!(s.status, Status::Refused(ErrorCode::UnknownToken));
        assert_eq!(s.closed(), None);
        assert_eq!(s.status, Status::Refused(ErrorCode::UnknownToken));
    }

    #[test]
    fn a_refused_action_is_shown_until_the_next_snapshot() {
        let mut s = live();
        let effect = s.received(ServerMsg::Error {
            code: ErrorCode::ColorTaken,
        });
        assert_eq!(effect, Effect::None);
        assert_eq!(s.problem, Some(ErrorCode::ColorTaken));
        assert_eq!(s.status, Status::Live);
        s.received(ServerMsg::Snapshot { room: room() });
        assert_eq!(s.problem, None);
    }

    #[test]
    fn rule_errors_are_problems_too() {
        let mut s = live();
        s.received(ServerMsg::Error {
            code: ErrorCode::Rule(RuleError::NotYourTurn),
        });
        assert_eq!(s.problem, Some(ErrorCode::Rule(RuleError::NotYourTurn)));
    }
}

#[cfg(test)]
mod roll_tests {
    use super::*;

    #[test]
    fn the_last_roll_is_remembered_from_events() {
        let mut s = Session::default();
        assert_eq!(s.last_roll, None);
        let four = Die::new(4).unwrap();
        let six = Die::new(6).unwrap();
        s.received(ServerMsg::Events {
            events: vec![
                Event::Rolled {
                    color: Color::Red,
                    die: four,
                },
                Event::NoLegalMove { color: Color::Red },
                Event::Rolled {
                    color: Color::Red,
                    die: six,
                },
            ],
        });
        assert_eq!(s.last_roll, Some((Color::Red, six)));
    }
}

#[cfg(test)]
mod batch_tests {
    use super::*;
    use protocol::RoomCode;
    use rules::Event;

    #[test]
    fn events_are_kept_with_the_game_they_start_from() {
        let game = rules::Game::new(&[Color::Red, Color::Blue]).unwrap();
        let mut s = Session::default();
        s.received(ServerMsg::Snapshot {
            room: RoomView {
                code: RoomCode("K7MXQ2".into()),
                host: SeatId(0),
                seats: vec![],
                stage: Stage::Game { game: game.clone() },
                turn_ms_left: None,
            },
        });
        let events = vec![Event::TurnStarted { color: Color::Blue }];
        s.received(ServerMsg::Events {
            events: events.clone(),
        });
        assert_eq!(
            s.take_batches(),
            vec![Batch {
                before: Some(game),
                events
            }]
        );
        assert_eq!(s.take_batches(), vec![], "taken once");
    }
}
