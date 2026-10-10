//! Where a piece is, counted along its own colour's path, and where a roll
//! takes it: bounce (tagasilugemine) and home-lane blocking included.

use serde::{Deserialize, Serialize};

use crate::board::{PATH_END, TRACK_LEN};

/// A piece's place, relative to its own colour's path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Position {
    /// In the nest (pesa).
    Nest,
    /// Steps from the start cell: `0..TRACK_LEN` on the track,
    /// `TRACK_LEN..=PATH_END` in the home lane.
    Path(u8),
}

/// Where a move ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Destination {
    pub step: u8,
    /// The roll overshot the target and the rest was counted back.
    pub bounced: bool,
}

/// The target (sihtring) for a colour's next piece: the deepest lane step
/// not already filled by the block of pieces at the lane end.
pub(crate) fn target(own: &[Position]) -> u8 {
    let mut target = PATH_END;
    while target >= TRACK_LEN && occupied(target, own) {
        target -= 1;
    }
    target
}

/// Whether the piece at `step` is finished: it and every lane step after it
/// are held by its own colour. `own` is all four of the colour's pieces.
pub(crate) fn is_finished(step: u8, own: &[Position]) -> bool {
    step >= TRACK_LEN && (step..=PATH_END).all(|s| occupied(s, own))
}

/// Where a piece at path `from` lands after moving `roll` steps, or `None`
/// if the move is illegal because it passes or lands on one of `others`
/// (the colour's other pieces) in the home lane.
pub(crate) fn destination(from: u8, roll: u8, others: &[Position]) -> Option<Destination> {
    let target = target(others);
    debug_assert!(from < target, "a finished piece cannot move");
    let mut step = from;
    let mut bounced = false;
    for _ in 0..roll {
        if step == target {
            bounced = true;
        }
        step = if bounced { step - 1 } else { step + 1 };
        if step >= TRACK_LEN && occupied(step, others) {
            return None;
        }
    }
    Some(Destination { step, bounced })
}

fn occupied(step: u8, pieces: &[Position]) -> bool {
    pieces.contains(&Position::Path(step))
}

#[cfg(test)]
mod tests {
    use super::*;
    use Position::{Nest, Path};

    fn lands(step: u8) -> Option<Destination> {
        Some(Destination {
            step,
            bounced: false,
        })
    }

    fn bounces_to(step: u8) -> Option<Destination> {
        Some(Destination {
            step,
            bounced: true,
        })
    }

    #[test]
    fn target_is_lane_end_when_nothing_finished() {
        assert_eq!(target(&[Nest, Path(10), Path(69)]), 70);
    }

    #[test]
    fn target_moves_back_past_the_finished_block() {
        assert_eq!(target(&[Path(70)]), 69);
        assert_eq!(target(&[Path(70), Path(69), Path(68), Nest]), 67);
    }

    #[test]
    fn finished_means_part_of_the_block_at_the_lane_end() {
        let own = [Path(70), Path(69), Path(67), Path(5)];
        assert!(is_finished(70, &own));
        assert!(is_finished(69, &own));
        assert!(!is_finished(67, &own));
        assert!(!is_finished(5, &own));
    }

    #[test]
    fn moves_forward_along_the_track() {
        assert_eq!(destination(10, 4, &[]), lands(14));
    }

    #[test]
    fn passing_and_landing_on_own_piece_on_the_track_is_allowed() {
        assert_eq!(destination(10, 4, &[Path(12), Path(14)]), lands(14));
    }

    #[test]
    fn turns_into_the_lane_after_the_corner_cell() {
        assert_eq!(destination(63, 1, &[]), lands(64));
        assert_eq!(destination(60, 6, &[]), lands(66));
    }

    #[test]
    fn reaches_the_lane_end_exactly() {
        assert_eq!(destination(66, 4, &[]), lands(70));
    }

    #[test]
    fn overshoot_is_counted_back() {
        assert_eq!(destination(68, 5, &[]), bounces_to(67));
    }

    #[test]
    fn exact_target_shrinks_as_pieces_finish() {
        assert_eq!(destination(65, 4, &[Path(70)]), lands(69));
        assert_eq!(destination(65, 5, &[Path(70)]), bounces_to(68));
    }

    #[test]
    fn bounce_can_leave_the_lane_back_onto_the_track() {
        let finished = [Path(70), Path(69), Path(68)];
        assert_eq!(destination(66, 6, &finished), bounces_to(62));
    }

    #[test]
    fn cannot_pass_own_piece_in_the_lane() {
        assert_eq!(destination(64, 3, &[Path(66)]), None);
        assert_eq!(destination(64, 3, &[Path(65)]), None);
    }

    #[test]
    fn cannot_land_on_own_piece_in_the_lane() {
        assert_eq!(destination(63, 3, &[Path(66)]), None);
    }

    #[test]
    fn cannot_turn_into_the_lane_past_own_piece() {
        assert_eq!(destination(62, 4, &[Path(65)]), None);
    }

    #[test]
    fn bounce_cannot_pass_or_land_on_own_piece() {
        assert_eq!(destination(69, 4, &[Path(67)]), None);
        assert_eq!(destination(69, 5, &[Path(67)]), None);
        assert_eq!(destination(69, 3, &[Path(67)]), bounces_to(68));
    }

    #[test]
    fn nest_pieces_never_block() {
        assert_eq!(destination(63, 3, &[Nest, Nest]), lands(66));
    }
}
