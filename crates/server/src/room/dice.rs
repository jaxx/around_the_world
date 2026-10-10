//! Where die values come from. Only the server rolls (see the `protocol`
//! crate); tests script the values instead.

use rand::{Rng, RngExt};
use rules::Die;

pub trait Dice {
    fn roll(&mut self) -> Die;
}

/// Fair dice from a random number generator.
pub struct RandomDice<R>(pub R);

impl<R: Rng> Dice for RandomDice<R> {
    fn roll(&mut self) -> Die {
        Die::new(self.0.random_range(1..=6)).expect("1..=6 is a valid die")
    }
}

/// Dice that show the given values in order and panic when they run out.
#[cfg(test)]
pub(crate) struct Scripted(pub std::collections::VecDeque<u8>);

#[cfg(test)]
impl Scripted {
    pub(crate) fn new(values: &[u8]) -> Scripted {
        Scripted(values.iter().copied().collect())
    }
}

#[cfg(test)]
impl Dice for Scripted {
    fn roll(&mut self) -> Die {
        let value = self.0.pop_front().expect("the test scripted enough rolls");
        Die::new(value).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn random_dice_show_every_face_and_nothing_else() {
        let mut dice = RandomDice(StdRng::seed_from_u64(7));
        let mut seen = [false; 6];
        for _ in 0..600 {
            seen[dice.roll().value() as usize - 1] = true;
        }
        assert_eq!(seen, [true; 6]);
    }
}
