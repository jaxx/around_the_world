//! Setting up a game and choosing who starts (alustaja).

use rules::Color::{Blue, Green, Red, Yellow};
use rules::{Action, Color, Die, Event, Game, Phase, PieceId, Position, RuleError};

fn roll(game: &Game, color: Color, value: u8) -> (Game, Vec<Event>) {
    game.apply(color, Action::Roll(Die::new(value).unwrap()))
        .unwrap()
}

#[test]
fn die_accepts_only_one_to_six() {
    assert_eq!(Die::new(0), Err(RuleError::InvalidDie));
    assert_eq!(Die::new(7), Err(RuleError::InvalidDie));
    for value in 1..=6 {
        assert_eq!(Die::new(value).unwrap().value(), value);
    }
}

#[test]
fn game_needs_two_to_four_distinct_colours() {
    assert_eq!(Game::new(&[Red]), Err(RuleError::InvalidPlayers));
    assert_eq!(Game::new(&[Red, Red]), Err(RuleError::InvalidPlayers));
    assert!(Game::new(&[Red, Blue]).is_ok());
    assert!(Game::new(&[Red, Yellow, Green, Blue]).is_ok());
}

#[test]
fn turn_order_is_clockwise_whatever_order_colours_are_given() {
    let game = Game::new(&[Blue, Green, Red]).unwrap();
    assert_eq!(game.colors(), vec![Red, Green, Blue]);
}

#[test]
fn all_pieces_start_in_the_nest() {
    let game = Game::new(&[Red, Blue]).unwrap();
    assert_eq!(game.pieces(Red), Some([Position::Nest; 4]));
    assert_eq!(game.pieces(Yellow), None);
}

#[test]
fn starter_rolls_go_clockwise() {
    let game = Game::new(&[Blue, Red]).unwrap();
    assert_eq!(game.current(), Red);
    let (game, events) = roll(&game, Red, 3);
    assert_eq!(
        events,
        vec![Event::Rolled {
            color: Red,
            die: Die::new(3).unwrap()
        }]
    );
    assert_eq!(game.current(), Blue);
}

#[test]
fn rolling_out_of_turn_is_rejected() {
    let game = Game::new(&[Red, Blue]).unwrap();
    assert_eq!(
        game.apply(Blue, Action::Roll(Die::new(6).unwrap())),
        Err(RuleError::NotYourTurn)
    );
}

#[test]
fn moving_while_choosing_starter_is_rejected() {
    let game = Game::new(&[Red, Blue]).unwrap();
    assert_eq!(
        game.apply(Red, Action::Move(PieceId(0))),
        Err(RuleError::WrongPhase)
    );
}

#[test]
fn highest_roll_starts_with_three_attempts() {
    let game = Game::new(&[Red, Blue]).unwrap();
    let (game, _) = roll(&game, Red, 3);
    let (game, events) = roll(&game, Blue, 5);
    assert_eq!(events.last(), Some(&Event::StarterChosen { color: Blue }));
    assert_eq!(game.current(), Blue);
    assert_eq!(game.phase(), &Phase::AwaitingRoll { attempts_left: 3 });
}

#[test]
fn tied_highest_rolls_roll_again_among_themselves() {
    let game = Game::new(&[Red, Yellow, Blue]).unwrap();
    let (game, _) = roll(&game, Red, 5);
    let (game, _) = roll(&game, Yellow, 2);
    let (game, events) = roll(&game, Blue, 5);
    assert_eq!(
        events.last(),
        Some(&Event::StarterRollOff {
            contenders: vec![Red, Blue]
        })
    );
    assert_eq!(game.current(), Red);

    let (game, _) = roll(&game, Red, 1);
    assert_eq!(game.current(), Blue, "yellow is out of the roll-off");
    let (game, events) = roll(&game, Blue, 4);
    assert_eq!(events.last(), Some(&Event::StarterChosen { color: Blue }));
    assert_eq!(game.current(), Blue);
}

#[test]
fn apply_leaves_the_original_game_unchanged() {
    let game = Game::new(&[Red, Blue]).unwrap();
    let before = game.clone();
    let _ = roll(&game, Red, 4);
    assert_eq!(game, before);
}
