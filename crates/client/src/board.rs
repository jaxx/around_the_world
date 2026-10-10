//! The board drawn as SVG from [`crate::layout`]: circles, colours, the
//! arrows of each corner loop, the pieces, and this seat's possible moves.

use leptos::prelude::*;
use rules::board::TRACK_LEN;
use rules::{Cell, Color, PieceId};

use crate::game_view::{PieceMark, Target};
use crate::layout::{Point, RADIUS, VIEW, all_cells, cell_center, stack_offset};
use crate::text;

/// The board with `marks` on it. `mine`'s pieces that start one of
/// `targets` are highlighted and clickable; destinations get a ring.
#[component]
pub fn Board(
    marks: Signal<Vec<PieceMark>>,
    targets: Signal<Vec<Target>>,
    mine: Signal<Option<Color>>,
    on_pick: Callback<PieceId>,
) -> impl IntoView {
    let destinations = move || {
        targets
            .get()
            .into_iter()
            .map(|t| {
                let p = cell_center(t.to);
                view! { <circle class="destination" cx=p.x cy=p.y r=RADIUS + 5.0 /> }
            })
            .collect_view()
    };
    // Each piece is one lasting element, moved by a CSS transition, so hops
    // and flights home are drawn smoothly.
    let ids =
        move || marks.with(|marks| marks.iter().map(|m| (m.color, m.piece)).collect::<Vec<_>>());
    let piece = move |(color, piece): (Color, PieceId)| {
        let mark = Memo::new(move |_| {
            marks.with(|marks| {
                marks
                    .iter()
                    .find(|m| m.color == color && m.piece == piece)
                    .copied()
            })
        });
        // Only my own pieces can be picked: on a shared cell an opponent's
        // piece sits on the same circle as mine.
        let pick = Memo::new(move |_| {
            let mark = mark.get()?;
            if mine.get() != Some(color) {
                return None;
            }
            targets.with(|t| t.iter().find(|t| t.from == mark.cell).map(|t| t.piece))
        });
        let place = move || {
            let Some(mark) = mark.get() else {
                return String::new();
            };
            let center = cell_center(mark.cell);
            let offset = stack_offset(mark.nth, mark.of);
            format!(
                "translate({}px, {}px)",
                center.x + offset.x,
                center.y + offset.y
            )
        };
        let radius = move || match mark.get() {
            Some(m) if m.of > 1 => RADIUS * 0.5,
            _ => RADIUS * 0.68,
        };
        let class = move || {
            let mark = mark.get();
            format!(
                "piece {}{}{}{}",
                color_class(color),
                if pick.get().is_some() { " movable" } else { "" },
                if mark.is_some_and(|m| m.finished) {
                    " finished"
                } else {
                    ""
                },
                if matches!(mark.map(|m| m.cell), Some(Cell::Nest(..))) {
                    " home"
                } else {
                    ""
                },
            )
        };
        view! {
            <g
                class=class
                style:transform=place
                on:click=move |_| {
                    if let Some(piece) = pick.get_untracked() {
                        on_pick.run(piece);
                    }
                }
            >
                <title>{text::color_name(color)}</title>
                <circle class="piece-rim" cx="0" cy="0" r=move || radius() + 3.0 />
                <circle class="piece-body" cx="0" cy="0" r=radius />
            </g>
        }
    };
    let board = static_board();
    let view_box = format!("0 0 {VIEW} {VIEW}");
    view! {
        <svg class="board" viewBox=view_box role="img" aria-label=text::BOARD>
            {board}
            <g class="destinations">{destinations}</g>
            <g class="pieces">
                <For each=ids key=|id| *id children=piece />
            </g>
        </svg>
    }
}

/// Paper, frame, circles and corner arrows: everything that never changes.
fn static_board() -> impl IntoView {
    let circles = all_cells()
        .into_iter()
        .map(|cell| {
            let p = cell_center(cell);
            view! { <circle cx=p.x cy=p.y r=RADIUS class=cell_class(cell) /> }
        })
        .collect_view();
    let arrows = Color::ALL
        .into_iter()
        .flat_map(corner_loop)
        .map(|(from, to)| arrow(cell_center(from), cell_center(to)))
        .collect_view();
    let inner = VIEW - 16.0;
    view! {
        <g class="static">
            <rect class="board-paper" x="0" y="0" width=VIEW height=VIEW />
            <rect class="board-frame" x="8" y="8" width=inner height=inner />
            {circles}
            {arrows}
        </g>
    }
}

/// The CSS classes of a circle: what it is and whose colour it shows.
pub fn cell_class(cell: Cell) -> String {
    match cell {
        Cell::Track(i) => match Color::ALL.into_iter().find(|c| c.start() == Cell::Track(i)) {
            Some(color) => format!("cell start {}", color_class(color)),
            None => "cell track".to_string(),
        },
        Cell::Lane(color, _) => format!("cell lane {}", color_class(color)),
        Cell::Nest(color, _) => format!("cell nest {}", color_class(color)),
    }
}

pub fn color_class(color: Color) -> &'static str {
    match color {
        Color::Red => "red",
        Color::Yellow => "yellow",
        Color::Green => "green",
        Color::Blue => "blue",
    }
}

/// The three turns of a colour's corner: outward row's end → corner cell →
/// start cell → first inward cell.
fn corner_loop(color: Color) -> [(Cell, Cell); 3] {
    let start = color.start();
    let Cell::Track(s) = start else {
        unreachable!("start cells are on the track");
    };
    let track = |offset: u8| Cell::Track((s + TRACK_LEN - 2 + offset) % TRACK_LEN);
    [(track(0), track(1)), (track(1), start), (start, track(3))]
}

/// A small white arrow inside the circle at `from`, pointing at `to`.
fn arrow(from: Point, to: Point) -> impl IntoView {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let len = (dx * dx + dy * dy).sqrt();
    let (ux, uy) = (dx / len, dy / len);
    let reach = RADIUS * 0.6;
    let (tail, tip) = (
        (from.x - ux * reach, from.y - uy * reach),
        (from.x + ux * reach, from.y + uy * reach),
    );
    let head = |turn: f32| {
        let (c, s) = (turn.cos(), turn.sin());
        let (bx, by) = (-ux * c + uy * s, -ux * s - uy * c);
        (tip.0 + bx * reach * 0.6, tip.1 + by * reach * 0.6)
    };
    let (l, r) = (head(0.6), head(-0.6));
    let points = format!("{},{} {},{} {},{}", l.0, l.1, tip.0, tip.1, r.0, r.1);
    view! {
        <g class="arrow">
            <line x1=tail.0 y1=tail.1 x2=tip.0 y2=tip.1 />
            <polyline points=points />
        </g>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circles_are_classed_by_kind_and_colour() {
        assert_eq!(cell_class(Cell::Track(5)), "cell track");
        assert_eq!(cell_class(Cell::Track(16)), "cell start yellow");
        assert_eq!(cell_class(Cell::Lane(Color::Green, 3)), "cell lane green");
        assert_eq!(cell_class(Cell::Nest(Color::Blue, 0)), "cell nest blue");
    }

    #[test]
    fn the_corner_loop_runs_into_the_start_cell() {
        assert_eq!(
            corner_loop(Color::Red),
            [
                (Cell::Track(62), Cell::Track(63)),
                (Cell::Track(63), Cell::Track(0)),
                (Cell::Track(0), Cell::Track(1)),
            ]
        );
    }
}
