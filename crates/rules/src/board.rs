//! Board geometry: the 64-cell track, home lanes and nests, and how a
//! colour's path maps onto physical cells. No pixel coordinates here.

use serde::{Deserialize, Serialize};

/// Number of cells on the track (rada).
pub const TRACK_LEN: u8 = 64;
/// Track cells per arm (haru), including its centre cell.
pub const ARM_LEN: u8 = 16;
/// Cells in each home lane (kodutee).
pub const LANE_LEN: u8 = 7;
/// Cells in each nest (pesa).
pub const NEST_LEN: u8 = 4;
/// Last step of a colour's path: the end of its home lane.
pub const PATH_END: u8 = TRACK_LEN + LANE_LEN - 1;

/// A player's colour, in clockwise order starting top-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Color {
    Red,
    Yellow,
    Green,
    Blue,
}

impl Color {
    pub const ALL: [Color; 4] = [Color::Red, Color::Yellow, Color::Green, Color::Blue];

    pub fn index(self) -> u8 {
        self as u8
    }

    /// The colour's start cell (stardiring) on the track.
    pub fn start(self) -> Cell {
        Cell::Track(self.start_index())
    }

    /// The colour's corner cell (nurgaring): the last track cell of its path.
    pub fn corner(self) -> Cell {
        Cell::Track((self.start_index() + TRACK_LEN - 1) % TRACK_LEN)
    }

    /// The physical cell at `step` along this colour's path: steps
    /// `0..TRACK_LEN` are the track starting at the start cell, steps
    /// `TRACK_LEN..=PATH_END` are the home lane. `None` past the lane end.
    pub fn path_cell(self, step: u8) -> Option<Cell> {
        match step {
            0..TRACK_LEN => Some(Cell::Track((self.start_index() + step) % TRACK_LEN)),
            TRACK_LEN..=PATH_END => Some(Cell::Lane(self, step - TRACK_LEN)),
            _ => None,
        }
    }

    fn start_index(self) -> u8 {
        self.index() * ARM_LEN
    }
}

/// One physical circle on the board (ring).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Cell {
    /// Track cell, numbered clockwise from red's start cell.
    Track(u8),
    /// Home lane cell, 0 at the corner end, `LANE_LEN - 1` at the centre.
    Lane(Color, u8),
    /// Nest cell.
    Nest(Color, u8),
}

/// The four centre cells (keskring) shared by neighbouring arms.
pub const CENTER_CELLS: [Cell; 4] = [
    Cell::Track(7),
    Cell::Track(23),
    Cell::Track(39),
    Cell::Track(55),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn start_cells_are_sixteen_apart_clockwise() {
        assert_eq!(Color::Red.start(), Cell::Track(0));
        assert_eq!(Color::Yellow.start(), Cell::Track(16));
        assert_eq!(Color::Green.start(), Cell::Track(32));
        assert_eq!(Color::Blue.start(), Cell::Track(48));
    }

    #[test]
    fn corner_cell_is_just_before_own_start() {
        assert_eq!(Color::Red.corner(), Cell::Track(63));
        assert_eq!(Color::Yellow.corner(), Cell::Track(15));
        assert_eq!(Color::Blue.corner(), Cell::Track(47));
    }

    #[test]
    fn path_starts_at_start_cell_and_reaches_corner_at_step_63() {
        for color in Color::ALL {
            assert_eq!(color.path_cell(0), Some(color.start()));
            assert_eq!(color.path_cell(63), Some(color.corner()));
        }
    }

    #[test]
    fn path_turns_into_own_lane_after_corner() {
        for color in Color::ALL {
            assert_eq!(color.path_cell(64), Some(Cell::Lane(color, 0)));
            assert_eq!(
                color.path_cell(PATH_END),
                Some(Cell::Lane(color, LANE_LEN - 1))
            );
            assert_eq!(color.path_cell(PATH_END + 1), None);
        }
    }

    #[test]
    fn path_is_seventy_steps_from_start_to_lane_end() {
        assert_eq!(PATH_END, 70);
    }

    #[test]
    fn every_path_visits_each_track_cell_exactly_once() {
        for color in Color::ALL {
            let cells: HashSet<Cell> = (0..TRACK_LEN)
                .map(|s| color.path_cell(s).unwrap())
                .collect();
            assert_eq!(cells.len(), TRACK_LEN as usize);
            assert!(
                cells
                    .iter()
                    .all(|c| matches!(c, Cell::Track(i) if *i < TRACK_LEN))
            );
        }
    }

    #[test]
    fn seventh_step_is_a_centre_cell_for_every_colour() {
        for color in Color::ALL {
            assert!(CENTER_CELLS.contains(&color.path_cell(7).unwrap()));
        }
    }

    #[test]
    fn next_colours_start_is_one_arm_further_along_the_path() {
        assert_eq!(Color::Red.path_cell(ARM_LEN), Some(Color::Yellow.start()));
        assert_eq!(Color::Blue.path_cell(ARM_LEN), Some(Color::Red.start()));
    }
}
