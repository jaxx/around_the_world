//! Pure game rules for «Reis ümber maailma». No I/O, randomness or clock:
//! the server supplies die values, see docs/adr/0001-pure-shared-rules-crate.md.
//!
//! Start with [`Game::new`], then drive it with [`Game::apply`]; the client
//! highlights [`Game::legal_moves`] and draws pieces with [`Game::cell`] and
//! the geometry in [`board`].

pub mod board;
mod die;
mod error;
mod event;
mod game;
mod position;

pub use board::{Cell, Color};
pub use die::Die;
pub use error::RuleError;
pub use event::Event;
pub use game::{Action, Game, Move, Phase, PieceId};
pub use position::Position;
