//! What the lobby (ootesaal) offers a seat: colours it may switch to,
//! whether it may start, and the link to share.

use protocol::{RoomCode, RoomView, SeatId};
use rules::Color;

/// Colours `me` can pick: free ones plus its own, in clockwise order.
pub fn choosable_colors(room: &RoomView, me: SeatId) -> Vec<Color> {
    Color::ALL
        .into_iter()
        .filter(|&c| !room.seats.iter().any(|s| s.color == c && s.id != me))
        .collect()
}

/// The host may start once at least two seats are taken.
pub fn can_start(room: &RoomView, me: SeatId) -> bool {
    room.host == me && room.seats.len() >= 2
}

/// The full invite link (kutselink), e.g. `https://example.org/r/K7MXQ2`.
pub fn invite_url(origin: &str, code: &RoomCode) -> String {
    format!(
        "{}{}",
        origin.trim_end_matches('/'),
        crate::route::room_path(code)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::{SeatView, Stage};

    fn seat(id: u8, color: Color) -> SeatView {
        SeatView {
            id: SeatId(id),
            nickname: format!("seat {id}"),
            color,
            connected: true,
            away: false,
        }
    }

    fn room(seats: Vec<SeatView>) -> RoomView {
        RoomView {
            code: RoomCode("K7MXQ2".into()),
            host: SeatId(0),
            seats,
            stage: Stage::Lobby,
            turn_ms_left: None,
        }
    }

    #[test]
    fn free_colours_and_my_own_are_choosable() {
        let r = room(vec![seat(0, Color::Red), seat(1, Color::Green)]);
        assert_eq!(
            choosable_colors(&r, SeatId(1)),
            vec![Color::Yellow, Color::Green, Color::Blue]
        );
    }

    #[test]
    fn only_the_host_with_company_can_start() {
        let alone = room(vec![seat(0, Color::Red)]);
        assert!(!can_start(&alone, SeatId(0)));
        let two = room(vec![seat(0, Color::Red), seat(1, Color::Yellow)]);
        assert!(can_start(&two, SeatId(0)));
        assert!(!can_start(&two, SeatId(1)));
    }

    #[test]
    fn the_invite_link_is_the_origin_plus_the_room_path() {
        let code = RoomCode("K7MXQ2".into());
        assert_eq!(
            invite_url("https://reis.example", &code),
            "https://reis.example/r/K7MXQ2"
        );
        assert_eq!(
            invite_url("http://127.0.0.1:8081/", &code),
            "http://127.0.0.1:8081/r/K7MXQ2"
        );
    }
}
