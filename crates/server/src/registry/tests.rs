//! Creating and finding rooms, and the limits on doing so.

use super::*;
use crate::room::dice::Scripted;
use crate::room::state::LOBBY_GRACE;
use std::net::Ipv4Addr;

fn registry(limits: Limits) -> Registry {
    Registry::new(limits, Arc::new(|| Box::new(Scripted::new(&[]))))
}

fn ip(last: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(10, 0, 0, last))
}

#[tokio::test(start_paused = true)]
async fn creating_a_room_seats_the_host() {
    let rooms = registry(Limits::default());
    let joined = rooms.create("Mari", ip(1)).unwrap();
    assert_eq!(joined.seat, protocol::SeatId(0));
    assert_eq!(joined.code.0.len(), CODE_LEN);
    assert!(joined.code.0.bytes().all(|b| CODE_ALPHABET.contains(&b)));
    assert_eq!(joined.token.0.len(), 32, "128 bits as hex");
    assert!(joined.token.0.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(rooms.len(), 1);
}

#[tokio::test(start_paused = true)]
async fn joining_by_code_ignores_case() {
    let rooms = registry(Limits::default());
    let host = rooms.create("Mari", ip(1)).unwrap();
    let guest = rooms
        .join(&host.code.0.to_lowercase(), "Jüri")
        .await
        .unwrap();
    assert_eq!(guest.seat, protocol::SeatId(1));
    assert_eq!(guest.code, host.code);
    assert_ne!(guest.token, host.token);
    assert!(rooms.room(&host.code.0.to_lowercase()).is_some());
}

#[tokio::test(start_paused = true)]
async fn unknown_codes_are_not_found() {
    let rooms = registry(Limits::default());
    assert_eq!(
        rooms.join("ZZZZZZ", "Jüri").await,
        Err(ErrorCode::RoomNotFound)
    );
    assert!(rooms.room("ZZZZZZ").is_none());
}

#[tokio::test(start_paused = true)]
async fn a_bad_nickname_opens_no_room() {
    let rooms = registry(Limits::default());
    assert_eq!(rooms.create("  ", ip(1)), Err(ErrorCode::BadNickname));
    assert!(rooms.is_empty());
}

#[tokio::test(start_paused = true)]
async fn the_number_of_rooms_is_limited() {
    let rooms = registry(Limits {
        max_rooms: 2,
        rooms_per_ip: 10,
    });
    rooms.create("a", ip(1)).unwrap();
    rooms.create("b", ip(2)).unwrap();
    assert_eq!(rooms.create("c", ip(3)), Err(ErrorCode::TooManyRooms));
}

#[tokio::test(start_paused = true)]
async fn room_creation_is_rate_limited_per_address() {
    let rooms = registry(Limits {
        max_rooms: 100,
        rooms_per_ip: 2,
    });
    rooms.create("a", ip(1)).unwrap();
    rooms.create("b", ip(1)).unwrap();
    assert_eq!(rooms.create("c", ip(1)), Err(ErrorCode::RateLimited));
    assert!(
        rooms.create("d", ip(2)).is_ok(),
        "other addresses are unaffected"
    );

    tokio::time::advance(RATE_WINDOW).await;
    assert!(rooms.create("e", ip(1)).is_ok());
}

#[tokio::test(start_paused = true)]
async fn rooms_remove_themselves_when_they_close() {
    let rooms = registry(Limits::default());
    rooms.create("Mari", ip(1)).unwrap();
    // Nobody connects, so the lobby gives the seat up and the room closes.
    tokio::time::sleep(LOBBY_GRACE + Duration::from_secs(1)).await;
    assert!(rooms.is_empty());
}
