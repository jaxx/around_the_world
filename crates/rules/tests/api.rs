//! The public surface the server and client rely on: JSON and piece lookups.

use rules::Color::{Blue, Red};
use rules::{Action, Cell, Die, Event, Game, PieceId, RuleError};

fn play(game: &Game, color: rules::Color, action: Action) -> (Game, Vec<Event>) {
    game.apply(color, action).unwrap()
}

fn roll(value: u8) -> Action {
    Action::Roll(Die::new(value).unwrap())
}

/// Red wins the starter roll, then rolls a 6 and enters piece 0.
fn red_entered() -> (Game, Vec<Event>) {
    let game = Game::new(&[Red, Blue]).unwrap();
    let (game, _) = play(&game, Red, roll(5));
    let (game, _) = play(&game, Blue, roll(2));
    let (game, _) = play(&game, Red, roll(6));
    play(&game, Red, Action::Move(PieceId(0)))
}

#[test]
fn die_from_json_is_validated() {
    assert_eq!(
        serde_json::from_str::<Die>("3").unwrap(),
        Die::new(3).unwrap()
    );
    assert!(serde_json::from_str::<Die>("7").is_err());
    assert!(serde_json::from_str::<Die>("0").is_err());
    assert_eq!(serde_json::to_string(&Die::new(4).unwrap()).unwrap(), "4");
}

#[test]
fn game_round_trips_through_json() {
    let (game, _) = red_entered();
    let json = serde_json::to_string(&game).unwrap();
    let back: Game = serde_json::from_str(&json).unwrap();
    assert_eq!(back, game);
    assert_eq!(back.legal_moves(), game.legal_moves());
}

#[test]
fn events_round_trip_through_json() {
    let (_, events) = red_entered();
    let json = serde_json::to_string(&events).unwrap();
    let back: Vec<Event> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, events);
}

#[test]
fn rule_errors_serialize_and_display() {
    let json = serde_json::to_string(&RuleError::NotYourTurn).unwrap();
    assert_eq!(
        serde_json::from_str::<RuleError>(&json).unwrap(),
        RuleError::NotYourTurn
    );
    assert_eq!(RuleError::NotYourTurn.to_string(), "it is not your turn");
}

#[test]
fn a_piece_reports_the_physical_cell_it_is_on() {
    let game = Game::new(&[Red, Blue]).unwrap();
    assert_eq!(game.cell(Red, PieceId(2)), Some(Cell::Nest(Red, 2)));
    assert_eq!(game.cell(Red, PieceId(4)), None);
    assert_eq!(game.cell(rules::Color::Green, PieceId(0)), None);

    let (game, _) = red_entered();
    assert_eq!(game.cell(Red, PieceId(0)), Some(Red.start()));
    assert!(!game.is_finished(Red, PieceId(0)));
}
