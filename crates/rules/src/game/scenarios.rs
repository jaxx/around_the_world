//! Turn-loop scenarios set up from arbitrary positions.

use super::*;
use crate::board::Color::{Blue, Red, Yellow};
use crate::position::Position::{Nest, Path};

/// A game where `current` is about to roll, with the given pieces.
fn setup(current: Color, pieces: &[(Color, [Position; 4])]) -> Game {
    let colors: Vec<Color> = pieces.iter().map(|&(color, _)| color).collect();
    let mut game = Game::new(&colors).unwrap();
    for &(color, positions) in pieces {
        game.players
            .iter_mut()
            .find(|p| p.color == color)
            .unwrap()
            .pieces = positions;
    }
    game.current = current;
    game.phase = Phase::AwaitingRoll {
        attempts_left: game.roll_attempts(current),
    };
    game
}

/// Red and blue, all blue pieces in the nest.
fn red_to_roll(red: [Position; 4]) -> Game {
    setup(Red, &[(Red, red), (Blue, [Nest; 4])])
}

fn die(value: u8) -> Die {
    Die::new(value).unwrap()
}

fn roll(game: &Game, value: u8) -> (Game, Vec<Event>) {
    game.apply(game.current(), Action::Roll(die(value)))
        .unwrap()
}

fn move_piece(game: &Game, piece: u8) -> (Game, Vec<Event>) {
    game.apply(game.current(), Action::Move(PieceId(piece)))
        .unwrap()
}

#[test]
fn all_in_nest_gets_three_attempts_then_the_turn_passes() {
    let game = red_to_roll([Nest; 4]);
    assert_eq!(game.phase(), &Phase::AwaitingRoll { attempts_left: 3 });

    let (game, events) = roll(&game, 3);
    assert_eq!(
        events,
        vec![
            Event::Rolled {
                color: Red,
                die: die(3)
            },
            Event::NoLegalMove { color: Red }
        ]
    );
    assert_eq!(game.phase(), &Phase::AwaitingRoll { attempts_left: 2 });

    let (game, _) = roll(&game, 2);
    assert_eq!(game.phase(), &Phase::AwaitingRoll { attempts_left: 1 });

    let (game, events) = roll(&game, 5);
    assert_eq!(events.last(), Some(&Event::TurnStarted { color: Blue }));
    assert_eq!(game.current(), Blue);
    assert_eq!(game.phase(), &Phase::AwaitingRoll { attempts_left: 3 });
}

#[test]
fn six_on_the_third_attempt_enters_and_gives_a_normal_bonus_roll() {
    let (game, _) = roll(&red_to_roll([Nest; 4]), 1);
    let (game, _) = roll(&game, 1);
    let (game, _) = roll(&game, 6);
    assert_eq!(game.phase(), &Phase::AwaitingMove { roll: die(6) });
    assert_eq!(
        game.legal_moves(),
        vec![Move {
            piece: PieceId(0),
            from: Nest,
            to: Path(0)
        }]
    );

    let (game, events) = move_piece(&game, 0);
    assert_eq!(
        events,
        vec![Event::Entered {
            color: Red,
            piece: PieceId(0)
        }]
    );
    assert_eq!(game.pieces(Red), Some([Path(0), Nest, Nest, Nest]));
    assert_eq!(game.current(), Red);
    assert_eq!(game.phase(), &Phase::AwaitingRoll { attempts_left: 1 });
}

#[test]
fn cannot_enter_while_own_piece_is_on_the_start_cell() {
    let game = red_to_roll([Path(0), Nest, Nest, Nest]);
    assert_eq!(game.phase(), &Phase::AwaitingRoll { attempts_left: 1 });
    let (game, _) = roll(&game, 6);
    assert_eq!(
        game.legal_moves(),
        vec![Move {
            piece: PieceId(0),
            from: Path(0),
            to: Path(6)
        }]
    );
}

#[test]
fn a_normal_move_ends_the_turn() {
    let (game, _) = roll(&red_to_roll([Path(10), Nest, Nest, Nest]), 4);
    let (game, events) = move_piece(&game, 0);
    assert_eq!(
        events,
        vec![
            Event::Moved {
                color: Red,
                piece: PieceId(0),
                from: Path(10),
                to: Path(14),
                bounced: false
            },
            Event::TurnStarted { color: Blue },
        ]
    );
    assert_eq!(game.pieces(Red), Some([Path(14), Nest, Nest, Nest]));
    assert_eq!(game.current(), Blue);
}

#[test]
fn a_six_gives_a_bonus_roll_after_moving() {
    let (game, _) = roll(&red_to_roll([Path(10), Nest, Nest, Nest]), 6);
    let (game, _) = move_piece(&game, 0);
    assert_eq!(game.current(), Red);
    assert_eq!(game.phase(), &Phase::AwaitingRoll { attempts_left: 1 });
}

#[test]
fn player_chooses_which_piece_to_move() {
    let (game, _) = roll(&red_to_roll([Path(10), Path(20), Nest, Nest]), 3);
    assert_eq!(game.legal_moves().len(), 2);
    let (game, _) = move_piece(&game, 1);
    assert_eq!(game.pieces(Red), Some([Path(10), Path(23), Nest, Nest]));
}

#[test]
fn a_piece_without_a_legal_move_cannot_be_chosen() {
    let (game, _) = roll(&red_to_roll([Path(10), Nest, Nest, Nest]), 3);
    assert_eq!(
        game.apply(Red, Action::Move(PieceId(1))),
        Err(RuleError::IllegalMove)
    );
    assert_eq!(
        game.apply(Red, Action::Move(PieceId(9))),
        Err(RuleError::IllegalMove)
    );
}

#[test]
fn a_stack_gives_one_move_but_either_piece_may_go() {
    let (game, _) = roll(&red_to_roll([Path(10), Path(10), Nest, Nest]), 2);
    assert_eq!(
        game.legal_moves(),
        vec![Move {
            piece: PieceId(0),
            from: Path(10),
            to: Path(12)
        }]
    );
    let (game, _) = move_piece(&game, 1);
    assert_eq!(game.pieces(Red), Some([Path(10), Path(12), Nest, Nest]));
}

#[test]
fn bounce_is_reported_on_the_move() {
    let (game, _) = roll(&red_to_roll([Path(68), Nest, Nest, Nest]), 5);
    let (_, events) = move_piece(&game, 0);
    assert_eq!(
        events[0],
        Event::Moved {
            color: Red,
            piece: PieceId(0),
            from: Path(68),
            to: Path(67),
            bounced: true
        }
    );
}

#[test]
fn finished_pieces_do_not_move() {
    let (game, _) = roll(&red_to_roll([Path(70), Path(30), Nest, Nest]), 2);
    assert_eq!(
        game.legal_moves(),
        vec![Move {
            piece: PieceId(1),
            from: Path(30),
            to: Path(32)
        }]
    );
}

#[test]
fn pieces_blocked_in_the_lane_get_one_roll_not_three() {
    let game = red_to_roll([Path(68), Path(69), Nest, Nest]);
    assert_eq!(game.phase(), &Phase::AwaitingRoll { attempts_left: 1 });
    let (game, events) = roll(&game, 3);
    assert!(events.contains(&Event::NoLegalMove { color: Red }));
    assert_eq!(game.current(), Blue);
}

#[test]
fn finished_pieces_do_not_count_against_three_attempts() {
    let game = red_to_roll([Path(70), Path(69), Nest, Nest]);
    assert_eq!(game.phase(), &Phase::AwaitingRoll { attempts_left: 3 });
}

#[test]
fn a_six_without_a_legal_move_still_gives_a_bonus_roll() {
    let game = red_to_roll([Path(70), Path(68), Path(66), Path(65)]);
    let (game, events) = roll(&game, 6);
    assert!(events.contains(&Event::NoLegalMove { color: Red }));
    assert_eq!(game.current(), Red);
    assert_eq!(game.phase(), &Phase::AwaitingRoll { attempts_left: 1 });
}

#[test]
fn actions_must_fit_the_phase() {
    let game = red_to_roll([Path(10), Nest, Nest, Nest]);
    assert_eq!(
        game.apply(Red, Action::Move(PieceId(0))),
        Err(RuleError::WrongPhase)
    );
    let (game, _) = roll(&game, 3);
    assert_eq!(
        game.apply(Red, Action::Roll(die(3))),
        Err(RuleError::WrongPhase)
    );
    assert_eq!(
        game.apply(Blue, Action::Move(PieceId(0))),
        Err(RuleError::NotYourTurn)
    );
}

#[test]
fn legal_moves_are_empty_while_awaiting_a_roll() {
    assert!(
        red_to_roll([Path(10), Nest, Nest, Nest])
            .legal_moves()
            .is_empty()
    );
}

// Captures (väljalöömine). Blue's path starts at track cell 48, so blue
// path step `s` is track cell `(48 + s) % 64`: track 14 is blue step 30.

fn red_and_blue(red: [Position; 4], blue: [Position; 4]) -> Game {
    setup(Red, &[(Red, red), (Blue, blue)])
}

#[test]
fn landing_on_an_opponent_sends_it_back_to_its_nest() {
    let game = red_and_blue(
        [Path(10), Nest, Nest, Nest],
        [Path(30), Path(5), Nest, Nest],
    );
    let (game, _) = roll(&game, 4);
    let (game, events) = move_piece(&game, 0);
    assert_eq!(
        events,
        vec![
            Event::Moved {
                color: Red,
                piece: PieceId(0),
                from: Path(10),
                to: Path(14),
                bounced: false
            },
            Event::Captured {
                color: Blue,
                piece: PieceId(0)
            },
            Event::TurnStarted { color: Blue },
        ]
    );
    assert_eq!(game.pieces(Blue), Some([Nest, Path(5), Nest, Nest]));
}

#[test]
fn passing_an_opponent_does_not_capture() {
    let game = red_and_blue([Path(10), Nest, Nest, Nest], [Path(28), Nest, Nest, Nest]);
    let (game, _) = roll(&game, 4);
    let (game, events) = move_piece(&game, 0);
    assert!(!events.iter().any(|e| matches!(e, Event::Captured { .. })));
    assert_eq!(game.pieces(Blue), Some([Path(28), Nest, Nest, Nest]));
}

#[test]
fn only_one_piece_of_a_stack_is_captured() {
    let game = red_and_blue(
        [Path(10), Nest, Nest, Nest],
        [Path(30), Path(30), Nest, Nest],
    );
    let (game, _) = roll(&game, 4);
    let (game, _) = move_piece(&game, 0);
    assert_eq!(game.pieces(Blue), Some([Nest, Path(30), Nest, Nest]));
}

#[test]
fn entering_captures_an_opponent_on_the_start_cell() {
    // Red's start cell is track 0, which is blue step 16.
    let game = red_and_blue([Nest; 4], [Path(16), Nest, Nest, Nest]);
    let (game, _) = roll(&game, 6);
    let (game, events) = move_piece(&game, 0);
    assert_eq!(
        events,
        vec![
            Event::Entered {
                color: Red,
                piece: PieceId(0)
            },
            Event::Captured {
                color: Blue,
                piece: PieceId(0)
            },
        ]
    );
    assert_eq!(game.pieces(Blue), Some([Nest; 4]));
}

#[test]
fn bouncing_back_onto_the_track_captures() {
    // Red's target is 67; 66 + 6 bounces back to step 62, track 62, blue step 14.
    let game = red_and_blue(
        [Path(70), Path(69), Path(68), Path(66)],
        [Path(14), Nest, Nest, Nest],
    );
    let (game, _) = roll(&game, 6);
    let (game, events) = move_piece(&game, 3);
    assert!(events.contains(&Event::Captured {
        color: Blue,
        piece: PieceId(0)
    }));
    assert_eq!(game.pieces(Blue), Some([Nest; 4]));
}

#[test]
fn own_pieces_are_never_captured() {
    let game = red_and_blue([Path(10), Path(14), Nest, Nest], [Nest; 4]);
    let (game, _) = roll(&game, 4);
    let (game, events) = move_piece(&game, 0);
    assert!(!events.iter().any(|e| matches!(e, Event::Captured { .. })));
    assert_eq!(game.pieces(Red), Some([Path(14), Path(14), Nest, Nest]));
}

#[test]
fn a_player_left_with_only_nest_pieces_gets_three_attempts() {
    let game = red_and_blue([Path(10), Nest, Nest, Nest], [Path(30), Nest, Nest, Nest]);
    let (game, _) = roll(&game, 4);
    let (game, _) = move_piece(&game, 0);
    assert_eq!(game.current(), Blue);
    assert_eq!(game.phase(), &Phase::AwaitingRoll { attempts_left: 3 });
}

// Finishing (lõpetamine), placements (koht) and the end of the game.

const DONE: [Position; 4] = [Path(70), Path(69), Path(68), Path(67)];

fn moved(from: u8, to: u8, piece: u8) -> Event {
    Event::Moved {
        color: Red,
        piece: PieceId(piece),
        from: Path(from),
        to: Path(to),
        bounced: false,
    }
}

#[test]
fn reaching_the_target_finishes_the_piece() {
    let (game, _) = roll(&red_to_roll([Path(66), Path(10), Nest, Nest]), 4);
    let (_, events) = move_piece(&game, 0);
    assert_eq!(
        events,
        vec![
            moved(66, 70, 0),
            Event::PieceFinished {
                color: Red,
                piece: PieceId(0)
            },
            Event::TurnStarted { color: Blue },
        ]
    );
}

#[test]
fn a_bounce_does_not_finish_the_piece() {
    let (game, _) = roll(&red_to_roll([Path(68), Path(10), Nest, Nest]), 5);
    let (_, events) = move_piece(&game, 0);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::PieceFinished { .. }))
    );
}

#[test]
fn finishing_a_piece_with_a_six_keeps_the_bonus_roll() {
    let (game, _) = roll(&red_to_roll([Path(64), Path(10), Nest, Nest]), 6);
    let (game, _) = move_piece(&game, 0);
    assert_eq!(game.current(), Red);
    assert_eq!(game.phase(), &Phase::AwaitingRoll { attempts_left: 1 });
}

#[test]
fn finishing_the_last_piece_gives_a_place_and_play_goes_on() {
    let game = setup(
        Red,
        &[
            (Red, [Path(70), Path(69), Path(68), Path(63)]),
            (Yellow, [Path(5), Nest, Nest, Nest]),
            (Blue, [Path(5), Nest, Nest, Nest]),
        ],
    );
    let (game, _) = roll(&game, 4);
    let (game, events) = move_piece(&game, 3);
    assert_eq!(
        events,
        vec![
            moved(63, 67, 3),
            Event::PieceFinished {
                color: Red,
                piece: PieceId(3)
            },
            Event::PlayerFinished {
                color: Red,
                place: 1
            },
            Event::TurnStarted { color: Yellow },
        ]
    );
    assert_eq!(game.placements(), &[Red]);
}

#[test]
fn finishing_with_a_six_ends_the_turn_when_the_player_is_done() {
    let game = setup(
        Red,
        &[
            (Red, [Path(70), Path(69), Path(68), Path(61)]),
            (Yellow, [Path(5), Nest, Nest, Nest]),
            (Blue, [Path(5), Nest, Nest, Nest]),
        ],
    );
    let (game, _) = roll(&game, 6);
    let (game, _) = move_piece(&game, 3);
    assert_eq!(game.current(), Yellow);
}

#[test]
fn finished_players_are_skipped_in_turn_order() {
    let mut game = setup(
        Blue,
        &[
            (Red, DONE),
            (Yellow, [Path(5), Nest, Nest, Nest]),
            (Blue, [Path(5), Nest, Nest, Nest]),
        ],
    );
    game.placements = vec![Red];
    let (game, _) = roll(&game, 2);
    let (game, events) = move_piece(&game, 0);
    assert_eq!(events.last(), Some(&Event::TurnStarted { color: Yellow }));
    assert_eq!(game.current(), Yellow);
}

#[test]
fn game_is_over_when_one_player_is_left_unfinished() {
    let game = red_and_blue(
        [Path(70), Path(69), Path(68), Path(63)],
        [Path(5), Nest, Nest, Nest],
    );
    let (game, _) = roll(&game, 4);
    let (game, events) = move_piece(&game, 3);
    assert_eq!(
        &events[2..],
        &[
            Event::PlayerFinished {
                color: Red,
                place: 1
            },
            Event::GameOver {
                placements: vec![Red, Blue]
            },
        ]
    );
    assert_eq!(game.phase(), &Phase::GameOver);
    assert_eq!(game.placements(), &[Red, Blue]);
    assert_eq!(
        game.apply(Blue, Action::Roll(die(3))),
        Err(RuleError::WrongPhase)
    );
}

#[test]
fn three_players_play_on_until_two_have_finished() {
    let mut game = setup(
        Yellow,
        &[
            (Red, DONE),
            (Yellow, [Path(70), Path(69), Path(68), Path(63)]),
            (Blue, [Path(5), Nest, Nest, Nest]),
        ],
    );
    game.placements = vec![Red];
    let (game, _) = roll(&game, 4);
    let (game, events) = move_piece(&game, 3);
    assert!(events.contains(&Event::PlayerFinished {
        color: Yellow,
        place: 2
    }));
    assert_eq!(game.phase(), &Phase::GameOver);
    assert_eq!(game.placements(), &[Red, Yellow, Blue]);
}

// Shared cells (segaring). Track 14 is red step 14, yellow step 62, blue step 30.

#[test]
fn capturing_one_of_a_stack_leaves_a_shared_cell() {
    let game = red_and_blue(
        [Path(10), Nest, Nest, Nest],
        [Path(30), Path(30), Nest, Nest],
    );
    let (game, _) = roll(&game, 4);
    let (game, _) = move_piece(&game, 0);
    assert_eq!(game.pieces(Red), Some([Path(14), Nest, Nest, Nest]));
    assert_eq!(game.pieces(Blue), Some([Nest, Path(30), Nest, Nest]));
}

#[test]
fn landing_on_a_shared_cell_captures_one_piece_of_every_opponent() {
    let game = setup(
        Red,
        &[
            (Red, [Path(10), Nest, Nest, Nest]),
            (Yellow, [Path(62), Path(62), Nest, Nest]),
            (Blue, [Path(30), Nest, Nest, Nest]),
        ],
    );
    let (game, _) = roll(&game, 4);
    let (game, events) = move_piece(&game, 0);
    assert_eq!(
        &events[1..3],
        &[
            Event::Captured {
                color: Yellow,
                piece: PieceId(0)
            },
            Event::Captured {
                color: Blue,
                piece: PieceId(0)
            },
        ]
    );
    assert_eq!(game.pieces(Yellow), Some([Nest, Path(62), Nest, Nest]));
    assert_eq!(game.pieces(Blue), Some([Nest; 4]));
}

#[test]
fn joining_own_piece_on_a_shared_cell_captures_the_opponent() {
    let game = red_and_blue(
        [Path(14), Path(10), Nest, Nest],
        [Path(30), Nest, Nest, Nest],
    );
    let (game, _) = roll(&game, 4);
    let (game, _) = move_piece(&game, 1);
    assert_eq!(game.pieces(Red), Some([Path(14), Path(14), Nest, Nest]));
    assert_eq!(game.pieces(Blue), Some([Nest; 4]));
}
