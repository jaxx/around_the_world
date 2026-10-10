//! Lobby (ootesaal) behaviour.

use super::*;
use crate::room::dice::Scripted;
use protocol::Stage as ViewStage;
use rules::Color::{Blue, Green, Red, Yellow};
use rules::Phase;

/// Dice for lobby tests, which never roll.
fn no_dice() -> Scripted {
    Scripted::new(&[])
}

fn token(name: &str) -> SeatToken {
    SeatToken(name.to_string())
}

/// A lobby created by "Mari" at `now`.
fn lobby(now: Instant) -> RoomState {
    RoomState::new(RoomCode("ABCDEF".into()), "Mari", token("mari"), now)
        .unwrap()
        .0
}

fn colors(room: &RoomState, now: Instant) -> Vec<(SeatId, Color)> {
    room.view(now)
        .seats
        .iter()
        .map(|s| (s.id, s.color))
        .collect()
}

#[test]
fn creator_takes_the_first_seat_as_host_with_red() {
    let now = Instant::now();
    let (room, seat) =
        RoomState::new(RoomCode("ABCDEF".into()), "Mari", token("mari"), now).unwrap();
    assert_eq!(seat, SeatId(0));
    let view = room.view(now);
    assert_eq!(view.code, RoomCode("ABCDEF".into()));
    assert_eq!(view.host, SeatId(0));
    assert_eq!(view.stage, ViewStage::Lobby);
    assert_eq!(view.turn_ms_left, None);
    assert_eq!(view.seats.len(), 1);
    let mari = &view.seats[0];
    assert_eq!((mari.nickname.as_str(), mari.color), ("Mari", Red));
    assert!(
        !mari.connected,
        "connected only once the WebSocket says hello"
    );
    assert!(!mari.away);
}

#[test]
fn joiners_get_the_first_free_colour_clockwise() {
    let now = Instant::now();
    let mut room = lobby(now);
    assert_eq!(room.join("Jüri", token("juri"), now), Ok(SeatId(1)));
    room.handle(
        SeatId(1),
        ClientMsg::ChooseColor { color: Green },
        now,
        &mut no_dice(),
    )
    .unwrap();
    assert_eq!(room.join("Kati", token("kati"), now), Ok(SeatId(2)));
    assert_eq!(
        colors(&room, now),
        vec![(SeatId(0), Red), (SeatId(1), Green), (SeatId(2), Yellow)]
    );
}

#[test]
fn nicknames_are_trimmed_and_one_to_twenty_characters() {
    let now = Instant::now();
    let mut room = lobby(now);
    assert_eq!(
        room.join("   ", token("a"), now),
        Err(ErrorCode::BadNickname)
    );
    assert_eq!(
        room.join(&"x".repeat(21), token("b"), now),
        Err(ErrorCode::BadNickname)
    );
    assert_eq!(room.join(&"õ".repeat(20), token("c"), now), Ok(SeatId(1)));
    room.join("  Kati  ", token("d"), now).unwrap();
    assert_eq!(room.view(now).seats[2].nickname, "Kati");
    assert!(RoomState::new(RoomCode("X".into()), "", token("e"), now).is_err());
}

#[test]
fn a_room_holds_four_seats() {
    let now = Instant::now();
    let mut room = lobby(now);
    for name in ["b", "c", "d"] {
        room.join(name, token(name), now).unwrap();
    }
    assert_eq!(room.join("e", token("e"), now), Err(ErrorCode::RoomFull));
}

#[test]
fn seats_are_found_by_token() {
    let now = Instant::now();
    let mut room = lobby(now);
    room.join("Jüri", token("juri"), now).unwrap();
    assert_eq!(room.seat_for(&token("juri")), Some(SeatId(1)));
    assert_eq!(room.seat_for(&token("nobody")), None);
}

#[test]
fn a_free_colour_can_be_chosen_but_not_a_taken_one() {
    let now = Instant::now();
    let mut room = lobby(now);
    room.join("Jüri", token("juri"), now).unwrap();
    assert_eq!(
        room.handle(
            SeatId(1),
            ClientMsg::ChooseColor { color: Red },
            now,
            &mut no_dice()
        ),
        Err(ErrorCode::ColorTaken)
    );
    assert_eq!(
        room.handle(
            SeatId(1),
            ClientMsg::ChooseColor { color: Blue },
            now,
            &mut no_dice()
        ),
        Ok(vec![])
    );
    assert_eq!(
        room.handle(
            SeatId(1),
            ClientMsg::ChooseColor { color: Blue },
            now,
            &mut no_dice()
        ),
        Ok(vec![])
    );
    assert_eq!(
        colors(&room, now),
        vec![(SeatId(0), Red), (SeatId(1), Blue)]
    );
}

#[test]
fn leaving_frees_the_seat_and_its_colour() {
    let now = Instant::now();
    let mut room = lobby(now);
    room.join("Jüri", token("juri"), now).unwrap();
    room.join("Kati", token("kati"), now).unwrap();
    room.handle(SeatId(1), ClientMsg::Leave, now, &mut no_dice())
        .unwrap();
    assert_eq!(room.seat_for(&token("juri")), None);
    assert_eq!(
        room.join("Mati", token("mati"), now),
        Ok(SeatId(1)),
        "lowest free seat id"
    );
    assert_eq!(
        colors(&room, now),
        vec![(SeatId(0), Red), (SeatId(2), Green), (SeatId(1), Yellow)]
    );
}

#[test]
fn when_the_host_leaves_the_next_seat_in_join_order_becomes_host() {
    let now = Instant::now();
    let mut room = lobby(now);
    room.join("Jüri", token("juri"), now).unwrap();
    room.join("Kati", token("kati"), now).unwrap();
    room.handle(SeatId(0), ClientMsg::Leave, now, &mut no_dice())
        .unwrap();
    assert_eq!(room.view(now).host, SeatId(1));
}

#[test]
fn when_everyone_leaves_the_room_is_empty() {
    let now = Instant::now();
    let mut room = lobby(now);
    room.handle(SeatId(0), ClientMsg::Leave, now, &mut no_dice())
        .unwrap();
    assert!(room.is_empty());
}

#[test]
fn only_the_host_can_start_and_only_with_two_seats() {
    let now = Instant::now();
    let mut room = lobby(now);
    assert_eq!(
        room.handle(SeatId(0), ClientMsg::Start, now, &mut no_dice()),
        Err(ErrorCode::NotEnoughPlayers)
    );
    room.join("Jüri", token("juri"), now).unwrap();
    assert_eq!(
        room.handle(SeatId(1), ClientMsg::Start, now, &mut no_dice()),
        Err(ErrorCode::NotHost)
    );
    assert_eq!(
        room.handle(SeatId(0), ClientMsg::Start, now, &mut no_dice()),
        Ok(vec![])
    );

    let ViewStage::Game { game } = room.view(now).stage else {
        panic!("game should have started");
    };
    assert_eq!(game.colors(), vec![Red, Yellow]);
    assert!(matches!(game.phase(), Phase::ChoosingStarter { .. }));
}

#[test]
fn the_lobby_closes_when_the_game_starts() {
    let now = Instant::now();
    let mut room = lobby(now);
    room.join("Jüri", token("juri"), now).unwrap();
    room.handle(SeatId(0), ClientMsg::Start, now, &mut no_dice())
        .unwrap();
    assert_eq!(
        room.join("Kati", token("kati"), now),
        Err(ErrorCode::NotInLobby)
    );
    assert_eq!(
        room.handle(SeatId(1), ClientMsg::Leave, now, &mut no_dice()),
        Err(ErrorCode::NotInLobby)
    );
    assert_eq!(
        room.handle(
            SeatId(1),
            ClientMsg::ChooseColor { color: Blue },
            now,
            &mut no_dice()
        ),
        Err(ErrorCode::NotInLobby)
    );
    assert_eq!(
        room.handle(SeatId(0), ClientMsg::Start, now, &mut no_dice()),
        Err(ErrorCode::NotInLobby)
    );
}

#[test]
fn game_actions_are_refused_in_the_lobby() {
    let now = Instant::now();
    let mut room = lobby(now);
    assert_eq!(
        room.handle(SeatId(0), ClientMsg::Roll, now, &mut no_dice()),
        Err(ErrorCode::NotInGame)
    );
    assert_eq!(
        room.handle(
            SeatId(0),
            ClientMsg::Move {
                piece: rules::PieceId(0)
            },
            now,
            &mut no_dice()
        ),
        Err(ErrorCode::NotInGame)
    );
    assert_eq!(
        room.handle(SeatId(0), ClientMsg::Rematch, now, &mut no_dice()),
        Err(ErrorCode::GameNotOver)
    );
}

#[test]
fn connection_state_is_shown() {
    let now = Instant::now();
    let mut room = lobby(now);
    room.connect(SeatId(0), now);
    assert!(room.view(now).seats[0].connected);
    room.disconnect(SeatId(0), now);
    assert!(!room.view(now).seats[0].connected);
}

#[test]
fn a_lobby_seat_disconnected_past_the_grace_is_given_up() {
    let start = Instant::now();
    let mut room = lobby(start);
    room.join("Jüri", token("juri"), start).unwrap();
    room.connect(SeatId(0), start);
    room.connect(SeatId(1), start);
    room.disconnect(SeatId(0), start);

    room.tick(
        start + LOBBY_GRACE - Duration::from_millis(1),
        &mut no_dice(),
    );
    assert_eq!(room.view(start).seats.len(), 2, "still within the grace");

    room.tick(start + LOBBY_GRACE, &mut no_dice());
    let view = room.view(start);
    assert_eq!(view.seats.len(), 1);
    assert_eq!(view.host, SeatId(1), "host passes on");
}

#[test]
fn reconnecting_within_the_grace_keeps_the_seat() {
    let start = Instant::now();
    let mut room = lobby(start);
    room.connect(SeatId(0), start);
    room.disconnect(SeatId(0), start);
    room.connect(SeatId(0), start);
    room.tick(start + LOBBY_GRACE * 2, &mut no_dice());
    assert_eq!(room.view(start).seats.len(), 1);
}

#[test]
fn a_seat_that_never_connected_is_given_up_after_the_grace() {
    let start = Instant::now();
    let mut room = lobby(start);
    room.join("Jüri", token("juri"), start).unwrap();
    room.tick(
        start + LOBBY_GRACE + Duration::from_millis(1),
        &mut no_dice(),
    );
    assert!(room.is_empty(), "nobody ever opened the WebSocket");
}
