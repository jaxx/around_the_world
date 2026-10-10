//! What the browser remembers between visits: each room's seat (token and
//! seat number) and the last nickname used.

use protocol::{RoomCode, SeatId, SeatToken};
use serde::{Deserialize, Serialize};

/// A string key-value store: `localStorage` in the browser, a map in tests.
pub trait KeyValue {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&self, key: &str, value: &str);
    fn remove(&self, key: &str);
}

/// The seat this browser holds in a room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedSeat {
    pub token: SeatToken,
    pub seat: SeatId,
}

pub struct Store<S>(pub S);

impl<S: KeyValue> Store<S> {
    pub fn seat(&self, code: &RoomCode) -> Option<SavedSeat> {
        serde_json::from_str(&self.0.get(&seat_key(code))?).ok()
    }

    pub fn save_seat(&self, code: &RoomCode, seat: &SavedSeat) {
        let json = serde_json::to_string(seat).expect("a seat serializes");
        self.0.set(&seat_key(code), &json);
    }

    /// Drops a seat the server no longer knows.
    pub fn forget_seat(&self, code: &RoomCode) {
        self.0.remove(&seat_key(code));
    }

    pub fn nickname(&self) -> Option<String> {
        self.0.get(NICKNAME_KEY)
    }

    pub fn save_nickname(&self, nickname: &str) {
        self.0.set(NICKNAME_KEY, nickname);
    }
}

const NICKNAME_KEY: &str = "nickname";

fn seat_key(code: &RoomCode) -> String {
    format!("seat:{}", code.0)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[derive(Default)]
    pub(crate) struct Memory(pub RefCell<HashMap<String, String>>);

    impl KeyValue for Memory {
        fn get(&self, key: &str) -> Option<String> {
            self.0.borrow().get(key).cloned()
        }
        fn set(&self, key: &str, value: &str) {
            self.0.borrow_mut().insert(key.into(), value.into());
        }
        fn remove(&self, key: &str) {
            self.0.borrow_mut().remove(key);
        }
    }

    fn code(c: &str) -> RoomCode {
        RoomCode(c.into())
    }

    fn mari() -> SavedSeat {
        SavedSeat {
            token: SeatToken("abc".into()),
            seat: SeatId(2),
        }
    }

    #[test]
    fn a_saved_seat_is_found_again_for_its_room_only() {
        let store = Store(Memory::default());
        assert_eq!(store.seat(&code("K7MXQ2")), None);
        store.save_seat(&code("K7MXQ2"), &mari());
        assert_eq!(store.seat(&code("K7MXQ2")), Some(mari()));
        assert_eq!(store.seat(&code("OTHER1")), None);
    }

    #[test]
    fn seats_are_kept_per_room_under_a_readable_key() {
        let store = Store(Memory::default());
        store.save_seat(&code("K7MXQ2"), &mari());
        assert!(store.0.get("seat:K7MXQ2").is_some());
    }

    #[test]
    fn a_forgotten_seat_is_gone() {
        let store = Store(Memory::default());
        store.save_seat(&code("K7MXQ2"), &mari());
        store.forget_seat(&code("K7MXQ2"));
        assert_eq!(store.seat(&code("K7MXQ2")), None);
    }

    #[test]
    fn a_damaged_entry_counts_as_no_seat() {
        let store = Store(Memory::default());
        store.0.set("seat:K7MXQ2", "{not json");
        assert_eq!(store.seat(&code("K7MXQ2")), None);
    }

    #[test]
    fn the_nickname_is_remembered() {
        let store = Store(Memory::default());
        assert_eq!(store.nickname(), None);
        store.save_nickname("Jüri");
        assert_eq!(store.nickname().as_deref(), Some("Jüri"));
    }
}
