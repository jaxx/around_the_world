//! All open rooms by code, plus the limits on creating them. Rooms remove
//! themselves when their task ends.

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use protocol::{ErrorCode, Joined, RoomCode, SeatToken};
use rand::rngs::StdRng;
use rand::{Rng, RngExt};
use tokio::time::Instant;

use crate::room::actor::{self, RoomHandle};
use crate::room::dice::Dice;
use crate::room::state::RoomState;

/// Characters in room codes: no 0/O or 1/I/L to confuse.
pub const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
pub const CODE_LEN: usize = 6;
/// The window for [`Limits::rooms_per_ip`].
pub const RATE_WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Open rooms at once.
    pub max_rooms: usize,
    /// Rooms one address may create per [`RATE_WINDOW`].
    pub rooms_per_ip: usize,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            max_rooms: 500,
            rooms_per_ip: 10,
        }
    }
}

/// Makes the dice for each new room.
pub type DiceFactory = Arc<dyn Fn() -> Box<dyn Dice + Send> + Send + Sync>;

#[derive(Clone)]
pub struct Registry {
    inner: Arc<Mutex<Inner>>,
    limits: Limits,
    dice: DiceFactory,
}

struct Inner {
    rooms: HashMap<RoomCode, RoomHandle>,
    /// Recent room creations per address, oldest first.
    created: HashMap<IpAddr, VecDeque<Instant>>,
    /// For room codes and seat tokens.
    rng: StdRng,
}

impl Registry {
    pub fn new(limits: Limits, dice: DiceFactory) -> Registry {
        Registry {
            inner: Arc::new(Mutex::new(Inner {
                rooms: HashMap::new(),
                created: HashMap::new(),
                rng: rand::make_rng(),
            })),
            limits,
            dice,
        }
    }

    /// Opens a room for `nickname`, who becomes its host. Must run inside
    /// the tokio runtime: it starts the room's task.
    pub fn create(&self, nickname: &str, ip: IpAddr) -> Result<Joined, ErrorCode> {
        let now = Instant::now();
        let mut inner = self.inner.lock().unwrap();
        inner.created.retain(|_, times| {
            while times.front().is_some_and(|&t| now - t >= RATE_WINDOW) {
                times.pop_front();
            }
            !times.is_empty()
        });
        if inner.created.get(&ip).map_or(0, VecDeque::len) >= self.limits.rooms_per_ip {
            return Err(ErrorCode::RateLimited);
        }
        if inner.rooms.len() >= self.limits.max_rooms {
            return Err(ErrorCode::TooManyRooms);
        }

        let code = loop {
            let code = new_code(&mut inner.rng);
            if !inner.rooms.contains_key(&code) {
                break code;
            }
        };
        let token = new_token(&mut inner.rng);
        let (state, seat) = RoomState::new(code.clone(), nickname, token.clone(), now.into_std())?;

        let registry = Arc::downgrade(&self.inner);
        let closing = code.clone();
        let handle = actor::spawn(state, (self.dice)(), move || {
            tracing::info!(code = %closing.0, "room closed");
            if let Some(inner) = registry.upgrade() {
                inner.lock().unwrap().rooms.remove(&closing);
            }
        });
        inner.rooms.insert(code.clone(), handle);
        inner.created.entry(ip).or_default().push_back(now);
        tracing::info!(code = %code.0, rooms = inner.rooms.len(), "room created");
        Ok(Joined { code, token, seat })
    }

    /// Takes a seat in the room's lobby. Codes are case-insensitive.
    pub async fn join(&self, code: &str, nickname: &str) -> Result<Joined, ErrorCode> {
        let code = normalize(code);
        let (handle, token) = {
            let mut inner = self.inner.lock().unwrap();
            let handle = inner
                .rooms
                .get(&code)
                .cloned()
                .ok_or(ErrorCode::RoomNotFound)?;
            (handle, new_token(&mut inner.rng))
        };
        let seat = handle.join(nickname, token.clone()).await?;
        Ok(Joined { code, token, seat })
    }

    /// The room with this code, if it is open. Codes are case-insensitive.
    pub fn room(&self, code: &str) -> Option<RoomHandle> {
        self.inner
            .lock()
            .unwrap()
            .rooms
            .get(&normalize(code))
            .cloned()
    }

    /// Open rooms.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().rooms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn normalize(code: &str) -> RoomCode {
    RoomCode(code.trim().to_ascii_uppercase())
}

fn new_code(rng: &mut StdRng) -> RoomCode {
    RoomCode(
        (0..CODE_LEN)
            .map(|_| CODE_ALPHABET[rng.random_range(0..CODE_ALPHABET.len())] as char)
            .collect(),
    )
}

/// 128 random bits as hex.
fn new_token(rng: &mut StdRng) -> SeatToken {
    SeatToken(format!("{:016x}{:016x}", rng.next_u64(), rng.next_u64()))
}

#[cfg(test)]
mod tests;
