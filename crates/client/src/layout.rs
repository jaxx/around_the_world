//! Where every circle (ring) of the X-shaped board sits in the SVG, derived
//! from the board's shape rather than drawn by hand.
//!
//! Each arm (haru) has three parallel rows along a diagonal: the home lane
//! in the middle, the outward row on the side of the previous colour and
//! the inward row on the side of the next. Neighbouring arms' rows cross at
//! the four centre cells. The view box is `0..VIEW` both ways, y down.

use rules::Cell;
use rules::Color;
use rules::board::{LANE_LEN, NEST_LEN, TRACK_LEN};

/// Width and height of the SVG view box.
pub const VIEW: f32 = 1000.0;
/// Radius of a circle.
pub const RADIUS: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// Distance between neighbouring circles along a row.
const STEP: f32 = 62.0;
/// Distance of the outward and inward rows from the arm's centre line; also
/// how far out along the arm the centre cells sit.
const ROW: f32 = 74.0;
/// Distance of a nest's middle from the middle of the board.
const NEST_REACH: f32 = 390.0;
/// Half the spacing of the 2 × 2 nest circles.
const NEST_HALF: f32 = 36.0;

/// The centre of a circle on the board.
pub fn cell_center(cell: Cell) -> Point {
    match cell {
        Cell::Track(i) => {
            // Arm A owns track cells 16A-8 ..= 16A+7: its outward row, corner,
            // start, inward row and the centre cell ahead of it.
            let arm = Color::ALL[(((i + 8) / 16) % 4) as usize];
            let (along, side) = match (i + 8) % 16 {
                j @ 0..=6 => (ROW + (j + 1) as f32 * STEP, -ROW),
                7 => (ROW + 7.0 * STEP, 0.0),
                8 => (ROW + 7.0 * STEP, ROW),
                j => (ROW + (15 - j) as f32 * STEP, ROW),
            };
            on_arm(arm, along, side)
        }
        Cell::Lane(color, j) => on_arm(color, ROW + (6 - j) as f32 * STEP, 0.0),
        Cell::Nest(color, slot) => {
            let (out, ahead) = (outward(color), outward(next(color)));
            let toward = ((out.0 + ahead.0) / SQRT_2, (out.1 + ahead.1) / SQRT_2);
            let across = (-toward.1, toward.0);
            let (a, b) = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)][slot as usize];
            Point {
                x: VIEW / 2.0 + NEST_REACH * toward.0 + NEST_HALF * (a * across.0 + b * toward.0),
                y: VIEW / 2.0 + NEST_REACH * toward.1 + NEST_HALF * (a * across.1 + b * toward.1),
            }
        }
    }
}

/// Where the `nth` of `of` pieces on one circle is drawn, relative to the
/// circle's centre: alone in the middle, otherwise spread around it.
pub fn stack_offset(nth: usize, of: usize) -> Point {
    if of <= 1 {
        return Point { x: 0.0, y: 0.0 };
    }
    let turn = std::f32::consts::TAU * nth as f32 / of as f32 - std::f32::consts::FRAC_PI_2;
    let reach = RADIUS * 0.45;
    Point {
        x: reach * turn.cos(),
        y: reach * turn.sin(),
    }
}

/// Every circle on the board: 64 track, 4 × 7 lane and 4 × 4 nest cells.
pub fn all_cells() -> Vec<Cell> {
    let track = (0..TRACK_LEN).map(Cell::Track);
    let lanes = Color::ALL
        .into_iter()
        .flat_map(|c| (0..LANE_LEN).map(move |j| Cell::Lane(c, j)));
    let nests = Color::ALL
        .into_iter()
        .flat_map(|c| (0..NEST_LEN).map(move |s| Cell::Nest(c, s)));
    track.chain(lanes).chain(nests).collect()
}

const SQRT_2: f32 = std::f32::consts::SQRT_2;

/// The point `along` the arm's centre line from the middle of the board,
/// moved `side` towards the next colour's arm.
fn on_arm(arm: Color, along: f32, side: f32) -> Point {
    let (u, v) = (outward(arm), outward(next(arm)));
    Point {
        x: VIEW / 2.0 + along * u.0 + side * v.0,
        y: VIEW / 2.0 + along * u.1 + side * v.1,
    }
}

/// Unit vector from the middle of the board out along the colour's arm.
fn outward(color: Color) -> (f32, f32) {
    let h = 1.0 / SQRT_2;
    match color {
        Color::Red => (-h, -h),
        Color::Yellow => (h, -h),
        Color::Green => (h, h),
        Color::Blue => (-h, h),
    }
}

fn next(color: Color) -> Color {
    Color::ALL[(color.index() as usize + 1) % 4]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rules::board::{CENTER_CELLS, PATH_END};

    fn distance(a: Point, b: Point) -> f32 {
        ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
    }

    /// Quarter turn clockwise around the middle of the board (y points down).
    fn rotate(p: Point) -> Point {
        let (dx, dy) = (p.x - VIEW / 2.0, p.y - VIEW / 2.0);
        Point {
            x: VIEW / 2.0 - dy,
            y: VIEW / 2.0 + dx,
        }
    }

    fn close(a: Point, b: Point) -> bool {
        distance(a, b) < 0.01
    }

    #[test]
    fn the_board_has_every_circle_once() {
        let cells = all_cells();
        assert_eq!(cells.len(), 64 + 4 * 7 + 4 * 4);
        for (i, a) in cells.iter().enumerate() {
            assert!(!cells[i + 1..].contains(a), "{a:?} listed twice");
        }
    }

    #[test]
    fn no_two_circles_overlap() {
        let cells = all_cells();
        for (i, &a) in cells.iter().enumerate() {
            for &b in &cells[i + 1..] {
                let gap = distance(cell_center(a), cell_center(b)) - 2.0 * RADIUS;
                assert!(gap >= 4.0, "{a:?} and {b:?} are {gap} apart");
            }
        }
    }

    #[test]
    fn every_circle_fits_on_the_board() {
        for cell in all_cells() {
            let p = cell_center(cell);
            for v in [p.x, p.y] {
                assert!((RADIUS..=VIEW - RADIUS).contains(&v), "{cell:?} at {p:?}");
            }
        }
    }

    #[test]
    fn each_step_along_a_path_goes_to_a_neighbouring_circle() {
        for color in Color::ALL {
            for step in 0..PATH_END {
                let here = cell_center(color.path_cell(step).unwrap());
                let next = cell_center(color.path_cell(step + 1).unwrap());
                let d = distance(here, next);
                assert!(d < 4.0 * RADIUS, "{color:?} step {step} jumps {d}");
            }
        }
    }

    #[test]
    fn the_track_closes_into_a_loop() {
        let last = cell_center(Cell::Track(TRACK_LEN - 1));
        assert!(distance(last, cell_center(Cell::Track(0))) < 4.0 * RADIUS);
    }

    #[test]
    fn each_arm_is_the_previous_one_turned_a_quarter_clockwise() {
        for (from, to) in [
            (Color::Red, Color::Yellow),
            (Color::Yellow, Color::Green),
            (Color::Green, Color::Blue),
            (Color::Blue, Color::Red),
        ] {
            for step in 0..=PATH_END {
                let a = cell_center(from.path_cell(step).unwrap());
                let b = cell_center(to.path_cell(step).unwrap());
                assert!(close(rotate(a), b), "{from:?}→{to:?} step {step}");
            }
            for slot in 0..NEST_LEN {
                let a = cell_center(Cell::Nest(from, slot));
                let b = cell_center(Cell::Nest(to, slot));
                assert!(close(rotate(a), b), "{from:?}→{to:?} nest {slot}");
            }
        }
    }

    #[test]
    fn red_starts_top_left_with_its_nest_at_the_top() {
        let start = cell_center(Color::Red.start());
        assert!(start.x < VIEW / 4.0 && start.y < VIEW / 4.0, "{start:?}");
        for slot in 0..NEST_LEN {
            let nest = cell_center(Cell::Nest(Color::Red, slot));
            assert!(nest.y < VIEW / 4.0, "{nest:?}");
            assert!((nest.x - VIEW / 2.0).abs() < VIEW / 8.0, "{nest:?}");
        }
    }

    #[test]
    fn the_centre_cells_ring_the_middle() {
        let middle = Point {
            x: VIEW / 2.0,
            y: VIEW / 2.0,
        };
        let radii: Vec<f32> = CENTER_CELLS
            .iter()
            .map(|&c| distance(cell_center(c), middle))
            .collect();
        for r in &radii {
            assert!((r - radii[0]).abs() < 0.01);
        }
        let nearest_other = all_cells()
            .into_iter()
            .filter(|c| !CENTER_CELLS.contains(c) && matches!(c, Cell::Track(_)))
            .map(|c| distance(cell_center(c), middle))
            .fold(f32::MAX, f32::min);
        assert!(
            radii[0] < nearest_other,
            "centre cells are the innermost track cells"
        );
    }

    #[test]
    fn the_lane_runs_from_the_corner_to_the_middle() {
        let middle = Point {
            x: VIEW / 2.0,
            y: VIEW / 2.0,
        };
        let corner = cell_center(Color::Red.corner());
        let first = cell_center(Cell::Lane(Color::Red, 0));
        let last = cell_center(Cell::Lane(Color::Red, 6));
        assert!(distance(first, corner) < 4.0 * RADIUS);
        assert!(distance(last, middle) < distance(first, middle));
    }
}

#[cfg(test)]
mod stack_tests {
    use super::*;

    #[test]
    fn a_lone_piece_sits_in_the_middle() {
        assert_eq!(stack_offset(0, 1), Point { x: 0.0, y: 0.0 });
    }

    #[test]
    fn stacked_pieces_are_spread_apart_but_stay_near_their_circle() {
        for of in 2..=8 {
            let spots: Vec<Point> = (0..of).map(|n| stack_offset(n, of)).collect();
            for (i, a) in spots.iter().enumerate() {
                assert!(
                    (a.x * a.x + a.y * a.y).sqrt() <= RADIUS,
                    "{of} pieces: {a:?}"
                );
                for b in &spots[i + 1..] {
                    let d = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
                    assert!(d > 4.0, "{of} pieces too close: {a:?} {b:?}");
                }
            }
        }
    }
}
