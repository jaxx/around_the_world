//! What happened as the result of an action, for the client to animate.

use serde::{Deserialize, Serialize};

use crate::board::Color;
use crate::die::Die;
use crate::game::PieceId;
use crate::position::Position;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Rolled {
        color: Color,
        die: Die,
    },
    /// The highest starter roll was tied; these colours roll again.
    StarterRollOff {
        contenders: Vec<Color>,
    },
    /// This colour takes the first turn (alustaja).
    StarterChosen {
        color: Color,
    },
    /// The roll gave no piece a legal move.
    NoLegalMove {
        color: Color,
    },
    /// A piece left the nest for its start cell (väljatoomine).
    Entered {
        color: Color,
        piece: PieceId,
    },
    /// A piece moved along its path; `bounced` if the roll was counted back.
    Moved {
        color: Color,
        piece: PieceId,
        from: Position,
        to: Position,
        bounced: bool,
    },
    /// An opponent piece was knocked back to its nest (väljalöömine).
    Captured {
        color: Color,
        piece: PieceId,
    },
    /// A piece reached its target (sihtring) and is finished.
    PieceFinished {
        color: Color,
        piece: PieceId,
    },
    /// All four of the colour's pieces are finished; `place` is 1-based.
    PlayerFinished {
        color: Color,
        place: u8,
    },
    /// Only one player is left unfinished; `placements` is first to last.
    GameOver {
        placements: Vec<Color>,
    },
    /// The turn passed to this colour.
    TurnStarted {
        color: Color,
    },
}
