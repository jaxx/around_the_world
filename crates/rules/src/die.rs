//! A die value (täring). The rules never roll; the server supplies values.

use serde::{Deserialize, Serialize};

use crate::error::RuleError;

/// A validated die value, 1–6. Serialized as a bare number; out-of-range
/// values are rejected when deserializing too.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Die(u8);

impl Die {
    pub fn new(value: u8) -> Result<Die, RuleError> {
        if (1..=6).contains(&value) {
            Ok(Die(value))
        } else {
            Err(RuleError::InvalidDie)
        }
    }

    pub fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Die {
    type Error = RuleError;

    fn try_from(value: u8) -> Result<Die, RuleError> {
        Die::new(value)
    }
}

impl From<Die> for u8 {
    fn from(die: Die) -> u8 {
        die.0
    }
}
