//! Game state and the single entry point for changing it: `Game::apply`.

use serde::{Deserialize, Serialize};

use crate::board::{Cell, Color, NEST_LEN};
use crate::die::Die;
use crate::error::RuleError;
use crate::event::Event;
use crate::position::{Position, destination, is_finished};

/// One of a player's four pieces, 0–3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PieceId(pub u8);

/// What a player does. Deliberately not deserializable: `Roll` carries the
/// die value, which only the server may supply, so client messages must be
/// translated by the server rather than decoded into an `Action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Roll(Die),
    Move(PieceId),
}

/// A legal move for the current roll. Pieces stacked on one cell give a
/// single move, listed under the lowest piece id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Move {
    pub piece: PieceId,
    pub from: Position,
    pub to: Position,
}

#[cfg(test)]
mod scenarios;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    /// Every contender rolls once, clockwise; the highest starts, ties roll again.
    ChoosingStarter {
        contenders: Vec<Color>,
        rolls: Vec<(Color, Die)>,
    },
    /// The current player must roll; `attempts_left` is 3 under the
    /// three-attempts rule (kolm katset), otherwise 1.
    AwaitingRoll {
        attempts_left: u8,
    },
    /// The current player must choose a piece to move by `roll`.
    AwaitingMove {
        roll: Die,
    },
    GameOver,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PlayerState {
    color: Color,
    pieces: [Position; NEST_LEN as usize],
}

/// A game in progress. Deserializing does not re-validate the state: only
/// decode games the server itself produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    /// Players in clockwise turn order.
    players: Vec<PlayerState>,
    current: Color,
    phase: Phase,
    /// Colours that have finished all pieces, first place first (koht).
    placements: Vec<Color>,
}

impl Game {
    /// A new game for 2–4 distinct colours, all pieces in their nests,
    /// starting with the roll for the first player.
    pub fn new(colors: &[Color]) -> Result<Game, RuleError> {
        let mut order = colors.to_vec();
        order.sort();
        order.dedup();
        if order.len() != colors.len() || !(2..=4).contains(&order.len()) {
            return Err(RuleError::InvalidPlayers);
        }
        Ok(Game {
            players: order
                .iter()
                .map(|&color| PlayerState {
                    color,
                    pieces: [Position::Nest; NEST_LEN as usize],
                })
                .collect(),
            current: order[0],
            phase: Phase::ChoosingStarter {
                contenders: order,
                rolls: Vec::new(),
            },
            placements: Vec::new(),
        })
    }

    /// Colours in clockwise turn order.
    pub fn colors(&self) -> Vec<Color> {
        self.players.iter().map(|p| p.color).collect()
    }

    /// The colour expected to act next.
    pub fn current(&self) -> Color {
        self.current
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    /// Finished colours in finishing order; complete once the game is over.
    pub fn placements(&self) -> &[Color] {
        &self.placements
    }

    /// The colour's four pieces, or `None` if it is not in this game.
    pub fn pieces(&self, color: Color) -> Option<[Position; NEST_LEN as usize]> {
        self.players
            .iter()
            .find(|p| p.color == color)
            .map(|p| p.pieces)
    }

    /// The physical cell a piece is on; a nest piece sits on the nest cell
    /// matching its id. `None` for an unknown colour or piece.
    pub fn cell(&self, color: Color, piece: PieceId) -> Option<Cell> {
        match *self.pieces(color)?.get(piece.0 as usize)? {
            Position::Nest => Some(Cell::Nest(color, piece.0)),
            Position::Path(step) => color.path_cell(step),
        }
    }

    /// Whether the piece has reached its target (lõpetanud nupp).
    pub fn is_finished(&self, color: Color, piece: PieceId) -> bool {
        self.pieces(color)
            .and_then(|pieces| {
                let &position = pieces.get(piece.0 as usize)?;
                Some(finished(position, &pieces))
            })
            .unwrap_or(false)
    }

    /// The current player's legal moves while a move is awaited, else empty.
    pub fn legal_moves(&self) -> Vec<Move> {
        match self.phase {
            Phase::AwaitingMove { roll } => self.moves_for(self.current, roll),
            _ => Vec::new(),
        }
    }

    fn moves_for(&self, color: Color, roll: Die) -> Vec<Move> {
        let pieces = self.player(color).pieces;
        let mut moves: Vec<Move> = Vec::new();
        for (index, &from) in pieces.iter().enumerate() {
            if moves.iter().any(|m| m.from == from) {
                continue;
            }
            if let Some((to, _)) = piece_destination(&pieces, index, roll) {
                moves.push(Move {
                    piece: PieceId(index as u8),
                    from,
                    to,
                });
            }
        }
        moves
    }

    /// Rolls allowed before the turn passes without a move: three while all
    /// the colour's unfinished pieces are in the nest (kolm katset), else one.
    fn roll_attempts(&self, color: Color) -> u8 {
        let pieces = self.player(color).pieces;
        let all_waiting = pieces
            .iter()
            .all(|&p| p == Position::Nest || finished(p, &pieces));
        if all_waiting { 3 } else { 1 }
    }

    fn player(&self, color: Color) -> &PlayerState {
        self.players
            .iter()
            .find(|p| p.color == color)
            .expect("colour is in the game")
    }

    fn player_mut(&mut self, color: Color) -> &mut PlayerState {
        self.players
            .iter_mut()
            .find(|p| p.color == color)
            .expect("colour is in the game")
    }

    /// The game after `actor` performs `action`, with what happened.
    /// The original game is never changed.
    #[must_use = "apply returns the new game; the original is unchanged"]
    pub fn apply(&self, actor: Color, action: Action) -> Result<(Game, Vec<Event>), RuleError> {
        let mut next = self.clone();
        let events = next.act(actor, action)?;
        Ok((next, events))
    }

    fn act(&mut self, actor: Color, action: Action) -> Result<Vec<Event>, RuleError> {
        if self.phase == Phase::GameOver {
            return Err(RuleError::WrongPhase);
        }
        if actor != self.current {
            return Err(RuleError::NotYourTurn);
        }
        match (&self.phase, action) {
            (Phase::ChoosingStarter { .. }, Action::Roll(die)) => Ok(self.roll_for_starter(die)),
            (&Phase::AwaitingRoll { attempts_left }, Action::Roll(die)) => {
                Ok(self.roll(attempts_left, die))
            }
            (&Phase::AwaitingMove { roll }, Action::Move(piece)) => self.move_piece(roll, piece),
            _ => Err(RuleError::WrongPhase),
        }
    }

    fn roll(&mut self, attempts_left: u8, die: Die) -> Vec<Event> {
        let color = self.current;
        let mut events = vec![Event::Rolled { color, die }];
        if !self.moves_for(color, die).is_empty() {
            self.phase = Phase::AwaitingMove { roll: die };
            return events;
        }
        events.push(Event::NoLegalMove { color });
        if die.value() == 6 {
            self.phase = Phase::AwaitingRoll {
                attempts_left: self.roll_attempts(color),
            };
        } else if attempts_left > 1 {
            self.phase = Phase::AwaitingRoll {
                attempts_left: attempts_left - 1,
            };
        } else {
            self.end_turn(&mut events);
        }
        events
    }

    fn move_piece(&mut self, roll: Die, piece: PieceId) -> Result<Vec<Event>, RuleError> {
        let color = self.current;
        let pieces = self.player(color).pieces;
        let index = piece.0 as usize;
        if index >= pieces.len() {
            return Err(RuleError::IllegalMove);
        }
        let (to, bounced) =
            piece_destination(&pieces, index, roll).ok_or(RuleError::IllegalMove)?;
        let from = pieces[index];
        self.player_mut(color).pieces[index] = to;

        let mut events = vec![match from {
            Position::Nest => Event::Entered { color, piece },
            Position::Path(_) => Event::Moved {
                color,
                piece,
                from,
                to,
                bounced,
            },
        }];
        if let Position::Path(step) = to {
            self.capture_at(color, step, &mut events);
            let pieces = self.player(color).pieces;
            if is_finished(step, &pieces) {
                events.push(Event::PieceFinished { color, piece });
                if pieces.iter().all(|&p| finished(p, &pieces)) {
                    self.finish_player(color, &mut events);
                }
            }
        }
        if self.phase == Phase::GameOver {
            // Placements are final; nobody acts again.
        } else if roll.value() == 6 && !self.placements.contains(&color) {
            self.phase = Phase::AwaitingRoll {
                attempts_left: self.roll_attempts(color),
            };
        } else {
            self.end_turn(&mut events);
        }
        Ok(events)
    }

    /// Gives `color` the next place, ending the game when only one player
    /// is left unfinished; that player takes the last place.
    fn finish_player(&mut self, color: Color, events: &mut Vec<Event>) {
        self.placements.push(color);
        events.push(Event::PlayerFinished {
            color,
            place: self.placements.len() as u8,
        });
        let unfinished: Vec<Color> = self
            .colors()
            .into_iter()
            .filter(|c| !self.placements.contains(c))
            .collect();
        if let [last] = unfinished[..] {
            self.placements.push(last);
            self.phase = Phase::GameOver;
            events.push(Event::GameOver {
                placements: self.placements.clone(),
            });
        }
    }

    /// Sends one piece of every opponent colour on the track cell at
    /// `mover`'s path `step` back to its nest; any others stay (segaring).
    /// Lanes are single-colour, so only the track counts.
    fn capture_at(&mut self, mover: Color, step: u8, events: &mut Vec<Event>) {
        let Some(cell @ Cell::Track(_)) = mover.path_cell(step) else {
            return;
        };
        for player in self.players.iter_mut().filter(|p| p.color != mover) {
            let hit = player.pieces.iter().position(|&p| match p {
                Position::Path(s) => player.color.path_cell(s) == Some(cell),
                Position::Nest => false,
            });
            if let Some(index) = hit {
                player.pieces[index] = Position::Nest;
                events.push(Event::Captured {
                    color: player.color,
                    piece: PieceId(index as u8),
                });
            }
        }
    }

    fn end_turn(&mut self, events: &mut Vec<Event>) {
        let index = self
            .players
            .iter()
            .position(|p| p.color == self.current)
            .expect("current plays");
        let next = (1..=self.players.len())
            .map(|offset| self.players[(index + offset) % self.players.len()].color)
            .find(|c| !self.placements.contains(c))
            .expect("an unfinished player remains while the game is on");
        self.current = next;
        self.phase = Phase::AwaitingRoll {
            attempts_left: self.roll_attempts(next),
        };
        events.push(Event::TurnStarted { color: next });
    }

    fn roll_for_starter(&mut self, die: Die) -> Vec<Event> {
        let Phase::ChoosingStarter { contenders, rolls } = &mut self.phase else {
            unreachable!("caller checked the phase");
        };
        rolls.push((self.current, die));
        let mut events = vec![Event::Rolled {
            color: self.current,
            die,
        }];

        if rolls.len() < contenders.len() {
            self.current = contenders[rolls.len()];
            return events;
        }

        let best = rolls
            .iter()
            .map(|&(_, die)| die)
            .max()
            .expect("contenders rolled");
        let top: Vec<Color> = rolls
            .iter()
            .filter(|&&(_, die)| die == best)
            .map(|&(c, _)| c)
            .collect();
        self.current = top[0];
        if let [starter] = top[..] {
            self.phase = Phase::AwaitingRoll {
                attempts_left: self.roll_attempts(starter),
            };
            events.push(Event::StarterChosen { color: starter });
        } else {
            events.push(Event::StarterRollOff {
                contenders: top.clone(),
            });
            self.phase = Phase::ChoosingStarter {
                contenders: top,
                rolls: Vec::new(),
            };
        }
        events
    }
}

/// Where piece `index` of `pieces` goes with `roll`, and whether it bounced;
/// `None` if it has no legal move.
fn piece_destination(pieces: &[Position], index: usize, roll: Die) -> Option<(Position, bool)> {
    let others: Vec<Position> = pieces
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != index)
        .map(|(_, &p)| p)
        .collect();
    match pieces[index] {
        Position::Nest => (roll.value() == 6 && !others.contains(&Position::Path(0)))
            .then_some((Position::Path(0), false)),
        Position::Path(step) if is_finished(step, pieces) => None,
        Position::Path(step) => {
            destination(step, roll.value(), &others).map(|d| (Position::Path(d.step), d.bounced))
        }
    }
}

fn finished(position: Position, pieces: &[Position]) -> bool {
    matches!(position, Position::Path(step) if is_finished(step, pieces))
}
