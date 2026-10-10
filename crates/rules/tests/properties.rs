//! Whole random games, checking the rules' invariants after every action.

use std::collections::HashMap;

use proptest::prelude::*;
use rules::board::{Cell, PATH_END};
use rules::{Action, Color, Die, Event, Game, Phase, PieceId, Position, RuleError};

/// More actions than any random game needs; reaching it means a game stalled.
const MAX_ACTIONS: usize = 100_000;

/// SplitMix64: a tiny deterministic generator, so a failing seed replays.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn die(&mut self) -> Die {
        Die::new(self.below(6) as u8 + 1).unwrap()
    }
}

fn colors_strategy() -> impl Strategy<Value = Vec<Color>> {
    proptest::sample::subsequence(Color::ALL.to_vec(), 2..=4).prop_shuffle()
}

/// Plays one random game to the end, checking invariants after each action.
fn play(colors: &[Color], seed: u64) -> Result<Game, TestCaseError> {
    let mut rng = Rng(seed);
    let mut game = Game::new(colors).unwrap();
    for _ in 0..MAX_ACTIONS {
        let action = match game.phase() {
            Phase::GameOver => return Ok(game),
            Phase::ChoosingStarter { .. } | Phase::AwaitingRoll { .. } => Action::Roll(rng.die()),
            Phase::AwaitingMove { .. } => {
                let moves = game.legal_moves();
                prop_assert!(!moves.is_empty(), "awaiting a move with none legal");
                check_only_legal_pieces_move(&game)?;
                Action::Move(moves[rng.below(moves.len())].piece)
            }
        };
        let (next, events) = game
            .apply(game.current(), action)
            .map_err(|e| TestCaseError::fail(format!("legal action rejected: {e:?}")))?;
        check_invariants(&next, &events)?;
        check_landing(&game, &next, &events)?;
        game = next;
    }
    Err(TestCaseError::fail("game did not end"))
}

/// `apply` accepts exactly the pieces `legal_moves` offers, stacks included.
fn check_only_legal_pieces_move(game: &Game) -> Result<(), TestCaseError> {
    let pieces = game.pieces(game.current()).unwrap();
    let moves = game.legal_moves();
    for (index, &from) in pieces.iter().enumerate() {
        let result = game.apply(game.current(), Action::Move(PieceId(index as u8)));
        let offered = moves.iter().any(|m| m.from == from);
        prop_assert_eq!(result.is_ok(), offered, "piece {} at {:?}", index, from);
        if !offered {
            prop_assert_eq!(result.unwrap_err(), RuleError::IllegalMove);
        }
    }
    Ok(())
}

fn check_invariants(game: &Game, events: &[Event]) -> Result<(), TestCaseError> {
    let mut lane_occupants: HashMap<Cell, usize> = HashMap::new();
    for color in game.colors() {
        let pieces = game.pieces(color).unwrap();
        for &position in &pieces {
            if let Position::Path(step) = position {
                prop_assert!(step <= PATH_END);
                let cell = color.path_cell(step).unwrap();
                if matches!(cell, Cell::Lane(..)) {
                    *lane_occupants.entry(cell).or_default() += 1;
                }
            }
        }

        let all_finished = (0..4).all(|i| game.is_finished(color, PieceId(i)));
        let placed = game.placements().contains(&color);
        let last_place =
            *game.phase() == Phase::GameOver && game.placements().last() == Some(&color);
        prop_assert_eq!(placed && !last_place, all_finished, "{:?} placement", color);
    }

    for (cell, count) in &lane_occupants {
        prop_assert_eq!(*count, 1, "own pieces share {:?}", cell);
    }

    let mut placements = game.placements().to_vec();
    placements.sort();
    placements.dedup();
    prop_assert_eq!(
        placements.len(),
        game.placements().len(),
        "duplicate placement"
    );

    if events.iter().any(|e| matches!(e, Event::GameOver { .. })) {
        prop_assert_eq!(game.placements().len(), game.colors().len());
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn random_games_keep_the_invariants_and_end(colors in colors_strategy(), seed: u64) {
        let game = play(&colors, seed)?;
        prop_assert_eq!(game.phase(), &Phase::GameOver);
        prop_assert_eq!(game.placements().len(), colors.len());
    }
}

/// After a piece lands on the track, every opponent colour that had pieces
/// on that cell has exactly one fewer there (väljalöömine, segaring).
fn check_landing(before: &Game, after: &Game, events: &[Event]) -> Result<(), TestCaseError> {
    let landing = events.iter().find_map(|e| match *e {
        Event::Entered { color, piece } | Event::Moved { color, piece, .. } => Some((color, piece)),
        _ => None,
    });
    let Some((mover, piece)) = landing else {
        return Ok(());
    };
    let Position::Path(step) = after.pieces(mover).unwrap()[piece.0 as usize] else {
        return Err(TestCaseError::fail("a moved piece is in the nest"));
    };
    let cell = mover.path_cell(step).unwrap();
    for color in before.colors().into_iter().filter(|&c| c != mover) {
        let had = pieces_on(before, color, cell);
        let has = pieces_on(after, color, cell);
        prop_assert_eq!(
            has,
            had.saturating_sub(1),
            "{:?} on {:?} after {:?}",
            color,
            cell,
            events
        );
    }
    Ok(())
}

fn pieces_on(game: &Game, color: Color, cell: Cell) -> usize {
    game.pieces(color)
        .unwrap()
        .iter()
        .filter(|&&p| matches!(p, Position::Path(s) if color.path_cell(s) == Some(cell)))
        .count()
}
