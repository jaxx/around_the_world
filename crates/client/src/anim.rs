//! Turns what happened (events) into what to show, beat by beat: the die
//! rolling, pieces hopping circle by circle, captured pieces flying home,
//! and the event log's lines in step with them.

use rules::board::{PATH_END, TRACK_LEN};
use rules::{Cell, Color, Die, Event, Game, PieceId, Position};

use crate::text;

/// Showing the die roll.
pub const ROLL_MS: u32 = 600;
/// One hop to the next circle.
pub const STEP_MS: u32 = 120;
/// A captured piece flying back to its nest.
pub const FLY_MS: u32 = 400;

#[derive(Debug, Clone, PartialEq)]
pub enum Beat {
    Roll {
        color: Color,
        die: Die,
    },
    Place {
        color: Color,
        piece: PieceId,
        cell: Cell,
    },
    Log(String),
}

/// A beat and how long to hold it before the next.
#[derive(Debug, Clone, PartialEq)]
pub struct Timed {
    pub beat: Beat,
    pub ms: u32,
}

/// The circles a piece passes through, ending where it lands. `target` is
/// its colour's target (sihtring) when the move began; a bounce turns there.
pub fn path(color: Color, from: Position, to: Position, bounced: bool, target: u8) -> Vec<Cell> {
    let cell = |step: u8| color.path_cell(step).expect("moves stay on the path");
    let Position::Path(end) = to else {
        return Vec::new();
    };
    let start = match from {
        Position::Nest => return vec![cell(end)],
        Position::Path(start) => start,
    };
    if !bounced {
        return (start + 1..=end).map(cell).collect();
    }
    (start + 1..=target)
        .chain((end..target).rev())
        .map(cell)
        .collect()
}

/// The beats for one batch of events, starting from `before`. `name` gives
/// a colour's player's nickname for the log.
pub fn plan(before: &Game, events: &[Event], name: &dyn Fn(Color) -> String) -> Vec<Timed> {
    let mut beats = Vec::new();
    let log = |beats: &mut Vec<Timed>, line: String| {
        beats.push(Timed {
            beat: Beat::Log(line),
            ms: 0,
        })
    };
    let hop = |beats: &mut Vec<Timed>, color, piece, cell, ms| {
        beats.push(Timed {
            beat: Beat::Place { color, piece, cell },
            ms,
        });
    };
    // Who moved last in this batch: captures name them.
    let mut mover = None;
    for event in events {
        match event {
            &Event::Rolled { color, die } => {
                log(
                    &mut beats,
                    format!("{} viskas {}", name(color), die.value()),
                );
                beats.push(Timed {
                    beat: Beat::Roll { color, die },
                    ms: ROLL_MS,
                });
            }
            &Event::NoLegalMove { color } => {
                log(&mut beats, format!("{}: käiku pole", name(color)))
            }
            &Event::Entered { color, piece } => {
                log(&mut beats, format!("{} tõi nupu välja", name(color)));
                hop(&mut beats, color, piece, color.start(), STEP_MS);
                mover = Some(color);
            }
            &Event::Moved {
                color,
                piece,
                from,
                to,
                bounced,
            } => {
                for cell in path(color, from, to, bounced, target_of(before, color)) {
                    hop(&mut beats, color, piece, cell, STEP_MS);
                }
                mover = Some(color);
            }
            &Event::Captured { color, piece } => {
                if let Some(mover) = mover {
                    let line = format!(
                        "{} lõi {} nupu välja",
                        name(mover),
                        text::color_genitive(color)
                    );
                    log(&mut beats, line);
                }
                hop(&mut beats, color, piece, Cell::Nest(color, piece.0), FLY_MS);
            }
            &Event::PieceFinished { color, .. } => {
                log(&mut beats, format!("{} viis nupu koju", name(color)));
            }
            &Event::PlayerFinished { color, place } => {
                log(
                    &mut beats,
                    format!("{} lõpetas: {}", name(color), text::place(place as usize)),
                );
            }
            Event::GameOver { .. } => log(&mut beats, text::GAME_OVER.to_string()),
            &Event::StarterChosen { color } => log(&mut beats, format!("{} alustab", name(color))),
            Event::StarterRollOff { contenders } => {
                let names: Vec<String> = contenders.iter().map(|&c| name(c)).collect();
                log(
                    &mut beats,
                    format!("Viik! Uuesti viskavad: {}", names.join(", ")),
                );
            }
            Event::TurnStarted { .. } => {}
        }
    }
    beats
}

/// A colour's target in `game`: the lane end, less its finished pieces.
fn target_of(game: &Game, color: Color) -> u8 {
    let finished = (0..4)
        .filter(|&i| game.is_finished(color, PieceId(i)))
        .count() as u8;
    debug_assert!(PATH_END - finished >= TRACK_LEN);
    PATH_END - finished
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn red_and_blue(red: serde_json::Value, blue: serde_json::Value) -> Game {
        serde_json::from_value(json!({
            "players": [
                {"color": "Red", "pieces": red},
                {"color": "Blue", "pieces": blue}
            ],
            "current": "Red",
            "phase": {"AwaitingRoll": {"attempts_left": 1}},
            "placements": []
        }))
        .unwrap()
    }

    fn name(color: Color) -> String {
        match color {
            Color::Red => "Mari".into(),
            _ => "Jüri".into(),
        }
    }

    fn die(v: u8) -> Die {
        Die::new(v).unwrap()
    }

    fn place(color: Color, piece: u8, cell: Cell) -> Timed {
        Timed {
            beat: Beat::Place {
                color,
                piece: PieceId(piece),
                cell,
            },
            ms: STEP_MS,
        }
    }

    #[test]
    fn a_move_hops_through_every_circle() {
        let cells = path(
            Color::Red,
            Position::Path(10),
            Position::Path(13),
            false,
            70,
        );
        assert_eq!(
            cells,
            vec![Cell::Track(11), Cell::Track(12), Cell::Track(13)]
        );
    }

    #[test]
    fn a_move_turns_into_the_lane_after_the_corner() {
        let cells = path(
            Color::Red,
            Position::Path(62),
            Position::Path(65),
            false,
            70,
        );
        assert_eq!(
            cells,
            vec![
                Cell::Track(63),
                Cell::Lane(Color::Red, 0),
                Cell::Lane(Color::Red, 1)
            ]
        );
    }

    #[test]
    fn entering_lands_on_the_start_cell() {
        let cells = path(Color::Blue, Position::Nest, Position::Path(0), false, 70);
        assert_eq!(cells, vec![Cell::Track(48)]);
    }

    #[test]
    fn a_bounce_goes_to_the_target_and_back() {
        // 68 + 5 with target 70: 69, 70, then back 69, 68, 67.
        let cells = path(Color::Red, Position::Path(68), Position::Path(67), true, 70);
        let lane = |j| Cell::Lane(Color::Red, j);
        assert_eq!(cells, vec![lane(5), lane(6), lane(5), lane(4), lane(3)]);
    }

    #[test]
    fn a_bounce_turns_at_a_nearer_target_once_pieces_have_finished() {
        // Target 69: 67 + 4 goes 68, 69, then back 68, 67.
        let cells = path(Color::Red, Position::Path(67), Position::Path(67), true, 69);
        let lane = |j| Cell::Lane(Color::Red, j);
        assert_eq!(cells, vec![lane(4), lane(5), lane(4), lane(3)]);
    }

    #[test]
    fn a_roll_then_a_move_is_played_out_with_the_log_in_step() {
        let before = red_and_blue(
            json!([{"Path": 10}, "Nest", "Nest", "Nest"]),
            json!(["Nest", "Nest", "Nest", "Nest"]),
        );
        let events = vec![
            Event::Rolled {
                color: Color::Red,
                die: die(3),
            },
            Event::Moved {
                color: Color::Red,
                piece: PieceId(0),
                from: Position::Path(10),
                to: Position::Path(13),
                bounced: false,
            },
            Event::TurnStarted { color: Color::Blue },
        ];
        assert_eq!(
            plan(&before, &events, &name),
            vec![
                Timed {
                    beat: Beat::Log("Mari viskas 3".into()),
                    ms: 0
                },
                Timed {
                    beat: Beat::Roll {
                        color: Color::Red,
                        die: die(3)
                    },
                    ms: ROLL_MS
                },
                place(Color::Red, 0, Cell::Track(11)),
                place(Color::Red, 0, Cell::Track(12)),
                place(Color::Red, 0, Cell::Track(13)),
            ]
        );
    }

    #[test]
    fn a_captured_piece_flies_home_after_the_capturing_move() {
        // Red step 14 is track 14 = blue step 30.
        let before = red_and_blue(
            json!([{"Path": 13}, "Nest", "Nest", "Nest"]),
            json!([{"Path": 30}, "Nest", "Nest", "Nest"]),
        );
        let events = vec![
            Event::Moved {
                color: Color::Red,
                piece: PieceId(0),
                from: Position::Path(13),
                to: Position::Path(14),
                bounced: false,
            },
            Event::Captured {
                color: Color::Blue,
                piece: PieceId(0),
            },
        ];
        let beats = plan(&before, &events, &name);
        assert_eq!(beats[0], place(Color::Red, 0, Cell::Track(14)));
        assert_eq!(
            beats[1],
            Timed {
                beat: Beat::Log("Mari lõi sinise nupu välja".into()),
                ms: 0
            }
        );
        assert_eq!(
            beats[2],
            Timed {
                beat: Beat::Place {
                    color: Color::Blue,
                    piece: PieceId(0),
                    cell: Cell::Nest(Color::Blue, 0)
                },
                ms: FLY_MS
            }
        );
    }

    #[test]
    fn the_bounce_target_comes_from_the_position_before_the_move() {
        // One red piece finished at 70, so the target is 69.
        let before = red_and_blue(
            json!([{"Path": 70}, {"Path": 67}, "Nest", "Nest"]),
            json!(["Nest", "Nest", "Nest", "Nest"]),
        );
        let events = vec![Event::Moved {
            color: Color::Red,
            piece: PieceId(1),
            from: Position::Path(67),
            to: Position::Path(67),
            bounced: true,
        }];
        let cells: Vec<Cell> = plan(&before, &events, &name)
            .into_iter()
            .filter_map(|t| match t.beat {
                Beat::Place { cell, .. } => Some(cell),
                _ => None,
            })
            .collect();
        let lane = |j| Cell::Lane(Color::Red, j);
        assert_eq!(cells, vec![lane(4), lane(5), lane(4), lane(3)]);
    }

    #[test]
    fn notable_events_are_logged_in_estonian() {
        let before = red_and_blue(
            json!(["Nest", "Nest", "Nest", "Nest"]),
            json!(["Nest", "Nest", "Nest", "Nest"]),
        );
        let logged = |events: Vec<Event>| -> Vec<String> {
            plan(&before, &events, &name)
                .into_iter()
                .filter_map(|t| match t.beat {
                    Beat::Log(line) => Some(line),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(
            logged(vec![Event::NoLegalMove { color: Color::Red }]),
            vec!["Mari: käiku pole"]
        );
        assert_eq!(
            logged(vec![Event::Entered {
                color: Color::Red,
                piece: PieceId(0)
            }]),
            vec!["Mari tõi nupu välja"]
        );
        assert_eq!(
            logged(vec![Event::StarterChosen { color: Color::Blue }]),
            vec!["Jüri alustab"]
        );
        assert_eq!(
            logged(vec![Event::StarterRollOff {
                contenders: vec![Color::Red, Color::Blue]
            }]),
            vec!["Viik! Uuesti viskavad: Mari, Jüri"]
        );
        assert_eq!(
            logged(vec![Event::PieceFinished {
                color: Color::Red,
                piece: PieceId(0)
            }]),
            vec!["Mari viis nupu koju"]
        );
        assert_eq!(
            logged(vec![Event::PlayerFinished {
                color: Color::Red,
                place: 1
            }]),
            vec!["Mari lõpetas: 1. koht"]
        );
        assert_eq!(
            logged(vec![Event::GameOver {
                placements: vec![Color::Red, Color::Blue]
            }]),
            vec!["Mäng läbi!"]
        );
        assert_eq!(
            logged(vec![Event::TurnStarted { color: Color::Blue }]),
            Vec::<String>::new()
        );
    }
}
