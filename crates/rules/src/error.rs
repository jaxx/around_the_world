//! Why an action was rejected. A rejected action leaves the game unchanged.

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleError {
    /// A game needs 2–4 distinct colours.
    InvalidPlayers,
    /// A die shows 1–6.
    InvalidDie,
    /// The actor is not the player whose turn it is.
    NotYourTurn,
    /// The action does not fit the current phase.
    WrongPhase,
    /// The chosen piece has no legal move with the current roll.
    IllegalMove,
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            RuleError::InvalidPlayers => "a game needs 2 to 4 distinct colours",
            RuleError::InvalidDie => "a die shows 1 to 6",
            RuleError::NotYourTurn => "it is not your turn",
            RuleError::WrongPhase => "that action does not fit the current phase",
            RuleError::IllegalMove => "that piece has no legal move with this roll",
        })
    }
}

impl std::error::Error for RuleError {}
