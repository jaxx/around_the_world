//! Playing a game in a room: server dice, turn timer (käiguaeg), away seats
//! (eemal), automatic moves, rematch (uus mäng) and room expiry.

use super::*;
use crate::room::dice::Scripted;
use protocol::Stage as ViewStage;
use rules::Color::{Red, Yellow};
use rules::{Die, PieceId, Position, RuleError};
use serde_json::json;

/// A started two-seat game: Mari (seat 0, red, host) and Jüri (seat 1,
/// yellow), both connected, with a fake clock and scripted dice.
struct Table {
    room: RoomState,
    now: Instant,
    dice: Scripted,
}

fn started(dice: &[u8]) -> Table {
    let now = Instant::now();
    let (mut room, _) =
        RoomState::new(RoomCode("ABCDEF".into()), "Mari", tok("mari"), now).unwrap();
    room.join("Jüri", tok("juri"), now).unwrap();
    room.connect(SeatId(0), now);
    room.connect(SeatId(1), now);
    let mut table = Table {
        room,
        now,
        dice: Scripted::new(dice),
    };
    table.act(0, ClientMsg::Start).unwrap();
    table
}

fn tok(name: &str) -> SeatToken {
    SeatToken(name.to_string())
}

fn die(value: u8) -> Die {
    Die::new(value).unwrap()
}

impl Table {
    fn act(&mut self, seat: u8, msg: ClientMsg) -> Result<Vec<Event>, ErrorCode> {
        self.room
            .handle(SeatId(seat), msg, self.now, &mut self.dice)
    }

    fn roll(&mut self, seat: u8) -> Vec<Event> {
        self.act(seat, ClientMsg::Roll).unwrap()
    }

    /// Lets `by` pass and applies whatever fell due.
    fn wait(&mut self, by: Duration) -> Vec<Event> {
        self.now += by;
        self.room.tick(self.now, &mut self.dice)
    }

    /// Jumps to the next deadline and applies it.
    fn expire(&mut self) -> Vec<Event> {
        self.now = self.room.next_deadline().expect("something is due");
        self.room.tick(self.now, &mut self.dice)
    }

    fn game(&self) -> Game {
        match self.room.view(self.now).stage {
            ViewStage::Game { game } => game,
            ViewStage::Lobby => panic!("no game in the lobby"),
        }
    }

    fn away(&self, seat: u8) -> bool {
        self.room.view(self.now).seats[seat as usize].away
    }

    fn ms_left(&self) -> Option<u32> {
        self.room.view(self.now).turn_ms_left
    }

    /// Red (Mari) wins the starter roll 6 against Jüri's 1 by acting at once.
    fn red_starts(&mut self) {
        self.roll(0);
        self.roll(1);
        assert_eq!(self.game().current(), Red);
    }

    /// Replaces the game, as if play had reached this position.
    fn inject(&mut self, game: serde_json::Value) {
        self.room.stage = Stage::Game(serde_json::from_value(game).unwrap());
        self.room.reschedule(self.now);
    }
}

#[test]
fn rolls_use_the_servers_dice() {
    let mut t = started(&[5]);
    assert_eq!(
        t.roll(0),
        vec![Event::Rolled {
            color: Red,
            die: die(5)
        }]
    );
}

#[test]
fn rule_errors_are_passed_on() {
    let mut t = started(&[]);
    assert_eq!(
        t.act(1, ClientMsg::Roll),
        Err(ErrorCode::Rule(RuleError::NotYourTurn))
    );
    assert_eq!(
        t.act(0, ClientMsg::Move { piece: PieceId(0) }),
        Err(ErrorCode::Rule(RuleError::WrongPhase))
    );
}

#[test]
fn the_turn_timer_counts_down() {
    let mut t = started(&[]);
    assert_eq!(t.ms_left(), Some(30_000));
    t.now += Duration::from_secs(10);
    assert_eq!(t.ms_left(), Some(20_000));
}

#[test]
fn a_timeout_rolls_for_the_player() {
    let mut t = started(&[4]);
    assert!(t.wait(TURN_TIME - Duration::from_millis(1)).is_empty());
    assert_eq!(
        t.wait(Duration::from_millis(1)),
        vec![Event::Rolled {
            color: Red,
            die: die(4)
        }]
    );
    assert_eq!(t.game().current(), Yellow);
    assert!(!t.away(0));
}

/// Red times out the starter roll (6), Jüri rolls 1, then Red times out two
/// of its three attempts (2, 3): three timeouts in a row.
fn red_timed_out_three_times() -> Table {
    let mut t = started(&[6, 1, 2, 3, 4, 5]);
    t.expire();
    t.roll(1);
    t.expire();
    assert!(!t.away(0), "two timeouts are not enough");
    t.expire();
    t
}

#[test]
fn three_timeouts_in_a_row_make_a_seat_away() {
    let t = red_timed_out_three_times();
    assert!(t.away(0));
    assert_eq!(t.room.next_deadline(), Some(t.now + AUTO_DELAY));
    assert_eq!(t.ms_left(), None, "no countdown while the server plays");
}

#[test]
fn an_away_seat_is_played_after_a_short_pause() {
    let mut t = red_timed_out_three_times();
    let events = t.wait(AUTO_DELAY);
    assert_eq!(
        events[0],
        Event::Rolled {
            color: Red,
            die: die(4)
        }
    );
    assert_eq!(t.game().current(), Yellow, "red's third attempt failed");
}

#[test]
fn an_own_action_resets_the_timeout_count() {
    let mut t = started(&[6, 1, 2, 3, 4]);
    t.expire();
    t.roll(1);
    t.expire();
    t.roll(0);
    t.expire();
    assert!(!t.away(0));
}

#[test]
fn acting_clears_away() {
    let mut t = red_timed_out_three_times();
    t.roll(0);
    assert!(!t.away(0));
    assert_eq!(t.game().current(), Yellow);
    assert_eq!(t.ms_left(), Some(30_000));
}

#[test]
fn a_seat_disconnected_past_the_grace_is_away_until_it_reconnects() {
    let mut t = started(&[]);
    t.room.disconnect(SeatId(0), t.now);
    assert_eq!(t.room.next_deadline(), Some(t.now + GAME_GRACE));
    t.wait(GAME_GRACE - Duration::from_millis(1));
    assert!(!t.away(0));
    t.wait(Duration::from_millis(1));
    assert!(t.away(0));
    assert_eq!(t.room.next_deadline(), Some(t.now + AUTO_DELAY));

    t.room.connect(SeatId(0), t.now);
    assert!(!t.away(0));
    assert_eq!(t.ms_left(), Some(30_000));
}

#[test]
fn another_seat_reconnecting_does_not_reset_the_turn_timer() {
    let mut t = started(&[]);
    t.room.disconnect(SeatId(1), t.now);
    t.now += Duration::from_secs(5);
    t.room.connect(SeatId(1), t.now);
    assert_eq!(t.ms_left(), Some(25_000));
}

#[test]
fn a_single_legal_move_is_played_after_a_short_pause() {
    let mut t = started(&[6, 1, 6]);
    t.red_starts();
    t.roll(0);
    assert_eq!(t.game().legal_moves().len(), 1);
    assert_eq!(t.room.next_deadline(), Some(t.now + AUTO_DELAY));
    assert_eq!(t.ms_left(), None);

    let events = t.wait(AUTO_DELAY);
    assert_eq!(
        events,
        vec![Event::Entered {
            color: Red,
            piece: PieceId(0)
        }]
    );
    assert_eq!(t.room.seats[0].timeouts, 0, "not the player's fault");
    assert_eq!(t.ms_left(), Some(30_000), "bonus roll, back on the clock");
}

#[test]
fn a_timeout_while_choosing_plays_the_first_legal_move() {
    let mut t = started(&[6, 1, 6, 6, 6]);
    t.red_starts();
    t.roll(0);
    t.act(0, ClientMsg::Move { piece: PieceId(0) }).unwrap();
    t.roll(0);
    t.act(0, ClientMsg::Move { piece: PieceId(0) }).unwrap();
    t.roll(0);
    assert_eq!(t.game().legal_moves().len(), 2);

    let events = t.wait(TURN_TIME);
    assert_eq!(
        events[0],
        Event::Moved {
            color: Red,
            piece: PieceId(0),
            from: Position::Path(6),
            to: Position::Path(12),
            bounced: false
        }
    );
    assert_eq!(t.room.seats[0].timeouts, 1);
}

/// Red needs a 4 with its last piece to finish; yellow is all in the nest.
fn red_about_to_win() -> serde_json::Value {
    json!({
        "players": [
            {"color": "Red", "pieces": [{"Path": 70}, {"Path": 69}, {"Path": 68}, {"Path": 63}]},
            {"color": "Yellow", "pieces": ["Nest", "Nest", "Nest", "Nest"]}
        ],
        "current": "Red",
        "phase": {"AwaitingMove": {"roll": 4}},
        "placements": []
    })
}

#[test]
fn game_over_stops_the_timer_and_starts_the_room_ttl() {
    let mut t = started(&[]);
    t.inject(red_about_to_win());
    assert_eq!(t.room.expires_at(), None);
    let events = t.act(0, ClientMsg::Move { piece: PieceId(3) }).unwrap();
    assert!(events.contains(&Event::GameOver {
        placements: vec![Red, Yellow]
    }));
    assert_eq!(t.room.next_deadline(), None);
    assert_eq!(t.ms_left(), None);
    assert_eq!(t.room.expires_at(), Some(t.now + ROOM_TTL));
}

#[test]
fn rematch_is_for_the_host_after_game_over() {
    let mut t = started(&[]);
    assert_eq!(t.act(0, ClientMsg::Rematch), Err(ErrorCode::GameNotOver));
    t.inject(red_about_to_win());
    t.act(0, ClientMsg::Move { piece: PieceId(3) }).unwrap();
    assert_eq!(t.act(1, ClientMsg::Rematch), Err(ErrorCode::NotHost));

    assert_eq!(t.act(0, ClientMsg::Rematch), Ok(vec![]));
    let view = t.room.view(t.now);
    assert_eq!(view.stage, ViewStage::Lobby);
    assert_eq!(view.seats.len(), 2);
    assert_eq!(t.room.expires_at(), None);
    assert_eq!(t.room.next_deadline(), None);
}

#[test]
fn a_room_expires_after_its_last_connection_drops() {
    let mut t = started(&[]);
    t.room.disconnect(SeatId(0), t.now);
    assert_eq!(t.room.expires_at(), None, "someone is still connected");
    t.now += Duration::from_secs(60);
    t.room.disconnect(SeatId(1), t.now);
    assert_eq!(t.room.expires_at(), Some(t.now + ROOM_TTL));
    t.room.connect(SeatId(1), t.now);
    assert_eq!(t.room.expires_at(), None);
}

#[test]
fn the_lobby_wakes_up_for_its_grace() {
    let now = Instant::now();
    let (mut room, _) =
        RoomState::new(RoomCode("ABCDEF".into()), "Mari", tok("mari"), now).unwrap();
    assert_eq!(room.next_deadline(), Some(now + LOBBY_GRACE));
    room.connect(SeatId(0), now);
    assert_eq!(room.next_deadline(), None);
}
