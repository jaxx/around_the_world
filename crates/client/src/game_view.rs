//! What the game screen shows, worked out from a [`Game`]: where each piece
//! is drawn, which moves this seat may make, and the turn timer.

use rules::{Cell, Color, Game, Phase, PieceId, Position};

/// One piece to draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PieceMark {
    pub color: Color,
    pub piece: PieceId,
    pub cell: Cell,
    /// This piece's place among those on the same cell, and how many there are.
    pub nth: usize,
    pub of: usize,
    /// Reached its target (lõpetanud nupp).
    pub finished: bool,
}

/// A move this seat can make by picking `piece`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub piece: PieceId,
    pub from: Cell,
    pub to: Cell,
}

/// Every piece of every colour, with stacks and shared cells numbered.
pub fn piece_marks(game: &Game) -> Vec<PieceMark> {
    let mut marks: Vec<PieceMark> = Vec::new();
    for color in game.colors() {
        let pieces = game.pieces(color).expect("the game's own colours");
        for (index, position) in pieces.into_iter().enumerate() {
            let piece = PieceId(index as u8);
            marks.push(PieceMark {
                color,
                piece,
                cell: cell_of(color, piece, position),
                nth: 0,
                of: 1,
                finished: game.is_finished(color, piece),
            });
        }
    }
    number_stacks(&mut marks);
    marks
}

/// Numbers the pieces sharing each cell, in order.
fn number_stacks(marks: &mut [PieceMark]) {
    for i in 0..marks.len() {
        let cell = marks[i].cell;
        marks[i].nth = marks[..i].iter().filter(|m| m.cell == cell).count();
        marks[i].of = marks.iter().filter(|m| m.cell == cell).count();
    }
}

/// The moves `me` can choose from right now: empty unless it is `me`'s turn
/// and the roll is in.
pub fn my_targets(game: &Game, me: Color) -> Vec<Target> {
    if game.current() != me {
        return Vec::new();
    }
    game.legal_moves()
        .into_iter()
        .map(|m| Target {
            piece: m.piece,
            from: cell_of(me, m.piece, m.from),
            to: cell_of(me, m.piece, m.to),
        })
        .collect()
}

/// Whether `me` should roll now (including the starter roll).
pub fn can_roll(game: &Game, me: Color) -> bool {
    game.current() == me
        && matches!(
            game.phase(),
            Phase::ChoosingStarter { .. } | Phase::AwaitingRoll { .. }
        )
}

/// Whole seconds left on the turn timer, rounded up, `elapsed_ms` after a
/// snapshot that said `ms_left`.
pub fn seconds_left(ms_left: u32, elapsed_ms: u32) -> u32 {
    ms_left.saturating_sub(elapsed_ms).div_ceil(1000)
}

fn cell_of(color: Color, piece: PieceId, position: Position) -> Cell {
    match position {
        Position::Nest => Cell::Nest(color, piece.0),
        Position::Path(step) => color.path_cell(step).expect("positions stay on the path"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A two-player game (red, blue) from positions, as the server sends it.
    fn game(
        red: serde_json::Value,
        blue: serde_json::Value,
        current: &str,
        phase: serde_json::Value,
    ) -> Game {
        serde_json::from_value(json!({
            "players": [
                {"color": "Red", "pieces": red},
                {"color": "Blue", "pieces": blue}
            ],
            "current": current,
            "phase": phase,
            "placements": []
        }))
        .unwrap()
    }

    fn nest() -> serde_json::Value {
        json!(["Nest", "Nest", "Nest", "Nest"])
    }

    #[test]
    fn nest_pieces_sit_on_their_own_nest_cells() {
        let g = game(
            nest(),
            nest(),
            "Red",
            json!({"AwaitingRoll": {"attempts_left": 3}}),
        );
        let marks = piece_marks(&g);
        assert_eq!(marks.len(), 8);
        let red2 = marks
            .iter()
            .find(|m| m.color == Color::Red && m.piece == PieceId(2))
            .unwrap();
        assert_eq!(red2.cell, Cell::Nest(Color::Red, 2));
        assert_eq!((red2.nth, red2.of), (0, 1));
    }

    #[test]
    fn stacked_and_shared_cells_number_their_pieces() {
        // Red step 14 is track 14; blue step 30 is track (48 + 30) % 64 = 14.
        let g = game(
            json!([{"Path": 14}, {"Path": 14}, "Nest", "Nest"]),
            json!([{"Path": 30}, "Nest", "Nest", "Nest"]),
            "Red",
            json!({"AwaitingRoll": {"attempts_left": 1}}),
        );
        let on_14: Vec<(usize, usize)> = piece_marks(&g)
            .into_iter()
            .filter(|m| m.cell == Cell::Track(14))
            .map(|m| (m.nth, m.of))
            .collect();
        assert_eq!(on_14, vec![(0, 3), (1, 3), (2, 3)]);
    }

    #[test]
    fn finished_pieces_are_marked() {
        let g = game(
            json!([{"Path": 70}, {"Path": 66}, "Nest", "Nest"]),
            nest(),
            "Red",
            json!({"AwaitingRoll": {"attempts_left": 1}}),
        );
        let marks = piece_marks(&g);
        assert!(
            marks
                .iter()
                .any(|m| m.cell == Cell::Lane(Color::Red, 6) && m.finished)
        );
        assert!(
            marks
                .iter()
                .any(|m| m.cell == Cell::Lane(Color::Red, 2) && !m.finished)
        );
    }

    #[test]
    fn my_targets_are_my_legal_moves_as_cells() {
        let g = game(
            json!([{"Path": 10}, "Nest", "Nest", "Nest"]),
            nest(),
            "Red",
            json!({"AwaitingMove": {"roll": 6}}),
        );
        assert_eq!(
            my_targets(&g, Color::Red),
            vec![
                Target {
                    piece: PieceId(0),
                    from: Cell::Track(10),
                    to: Cell::Track(16)
                },
                Target {
                    piece: PieceId(1),
                    from: Cell::Nest(Color::Red, 1),
                    to: Cell::Track(0)
                },
            ]
        );
        assert_eq!(my_targets(&g, Color::Blue), vec![], "not blue's turn");
    }

    #[test]
    fn nothing_to_move_before_rolling() {
        let g = game(
            json!([{"Path": 10}, "Nest", "Nest", "Nest"]),
            nest(),
            "Red",
            json!({"AwaitingRoll": {"attempts_left": 1}}),
        );
        assert_eq!(my_targets(&g, Color::Red), vec![]);
    }

    #[test]
    fn rolling_is_for_the_current_seat_while_a_roll_is_awaited() {
        let awaiting_roll = game(
            nest(),
            nest(),
            "Red",
            json!({"AwaitingRoll": {"attempts_left": 3}}),
        );
        assert!(can_roll(&awaiting_roll, Color::Red));
        assert!(!can_roll(&awaiting_roll, Color::Blue));
        let starter = game(
            nest(),
            nest(),
            "Blue",
            json!({"ChoosingStarter": {"contenders": ["Red", "Blue"], "rolls": [["Red", 4]]}}),
        );
        assert!(can_roll(&starter, Color::Blue));
        let moving = game(
            json!([{"Path": 10}, "Nest", "Nest", "Nest"]),
            nest(),
            "Red",
            json!({"AwaitingMove": {"roll": 3}}),
        );
        assert!(!can_roll(&moving, Color::Red));
    }

    #[test]
    fn the_countdown_rounds_up_and_stops_at_zero() {
        assert_eq!(seconds_left(30_000, 0), 30);
        assert_eq!(seconds_left(30_000, 1), 30);
        assert_eq!(seconds_left(30_000, 1_000), 29);
        assert_eq!(seconds_left(25_400, 0), 26);
        assert_eq!(seconds_left(1_000, 5_000), 0);
    }
}

/// The dots on a die face, as (column, row) on a 3 × 3 grid.
pub fn pips(value: u8) -> Vec<(u8, u8)> {
    let corners_a = [(0, 0), (2, 2)];
    let corners_b = [(2, 0), (0, 2)];
    let sides = [(0, 1), (2, 1)];
    let middle = [(1, 1)];
    let parts: &[&[(u8, u8)]] = match value {
        1 => &[&middle],
        2 => &[&corners_a],
        3 => &[&corners_a, &middle],
        4 => &[&corners_a, &corners_b],
        5 => &[&corners_a, &corners_b, &middle],
        _ => &[&corners_a, &corners_b, &sides],
    };
    parts.concat()
}

#[cfg(test)]
mod pip_tests {
    use super::*;

    #[test]
    fn a_face_shows_as_many_dots_as_its_value() {
        for value in 1..=6 {
            let dots = pips(value);
            assert_eq!(dots.len(), value as usize);
            assert!(dots.iter().all(|&(c, r)| c < 3 && r < 3));
            let mut unique = dots.clone();
            unique.sort();
            unique.dedup();
            assert_eq!(unique.len(), dots.len(), "{value}: no dot twice");
        }
    }

    #[test]
    fn odd_faces_have_a_middle_dot_even_ones_do_not() {
        for value in 1..=6 {
            assert_eq!(pips(value).contains(&(1, 1)), value % 2 == 1, "{value}");
        }
    }
}

/// Like [`piece_marks`], with some pieces shown elsewhere (mid-animation).
pub fn marks_with(game: &Game, overrides: &[(Color, PieceId, Cell)]) -> Vec<PieceMark> {
    let mut marks = piece_marks(game);
    for mark in &mut marks {
        if let Some(&(_, _, cell)) = overrides
            .iter()
            .find(|&&(c, p, _)| c == mark.color && p == mark.piece)
        {
            mark.cell = cell;
        }
    }
    number_stacks(&mut marks);
    marks
}

#[cfg(test)]
mod override_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn overridden_pieces_are_drawn_where_the_animation_has_them() {
        let game: Game = serde_json::from_value(json!({
            "players": [
                {"color": "Red", "pieces": [{"Path": 10}, {"Path": 12}, "Nest", "Nest"]},
                {"color": "Blue", "pieces": ["Nest", "Nest", "Nest", "Nest"]}
            ],
            "current": "Red",
            "phase": {"AwaitingRoll": {"attempts_left": 1}},
            "placements": []
        }))
        .unwrap();
        let marks = marks_with(&game, &[(Color::Red, PieceId(0), Cell::Track(12))]);
        let on_12: Vec<(PieceId, usize, usize)> = marks
            .iter()
            .filter(|m| m.cell == Cell::Track(12))
            .map(|m| (m.piece, m.nth, m.of))
            .collect();
        assert_eq!(on_12, vec![(PieceId(0), 0, 2), (PieceId(1), 1, 2)]);
        assert!(!marks.iter().any(|m| m.cell == Cell::Track(10)));
    }
}

/// Final placements (koht) as place, nickname and colour, first place first.
pub fn standings(room: &protocol::RoomView, game: &Game) -> Vec<(usize, String, Color)> {
    game.placements()
        .iter()
        .enumerate()
        .map(|(i, &color)| {
            let name = room
                .seats
                .iter()
                .find(|s| s.color == color)
                .map(|s| s.nickname.clone())
                .unwrap_or_default();
            (i + 1, name, color)
        })
        .collect()
}

#[cfg(test)]
mod standings_tests {
    use super::*;
    use protocol::{RoomCode, RoomView, SeatId, SeatView, Stage};
    use serde_json::json;

    #[test]
    fn placements_are_listed_with_names() {
        let game: Game = serde_json::from_value(json!({
            "players": [
                {"color": "Red", "pieces": ["Nest", "Nest", "Nest", "Nest"]},
                {"color": "Blue", "pieces": [{"Path": 70}, {"Path": 69}, {"Path": 68}, {"Path": 67}]}
            ],
            "current": "Blue",
            "phase": "GameOver",
            "placements": ["Blue", "Red"]
        }))
        .unwrap();
        let seat = |id, nickname: &str, color| SeatView {
            id: SeatId(id),
            nickname: nickname.into(),
            color,
            connected: true,
            away: false,
        };
        let room = RoomView {
            code: RoomCode("K7MXQ2".into()),
            host: SeatId(0),
            seats: vec![seat(0, "Mari", Color::Red), seat(1, "Jüri", Color::Blue)],
            stage: Stage::Game { game: game.clone() },
            turn_ms_left: None,
        };
        assert_eq!(
            standings(&room, &game),
            vec![
                (1, "Jüri".to_string(), Color::Blue),
                (2, "Mari".to_string(), Color::Red)
            ]
        );
    }
}
