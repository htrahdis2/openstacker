//! Player input.
//!
//! Input is **buttons held this tick**, never actions. A caller cannot express "I moved
//! left 10 times this frame" because movement is not in this type. Turning "LEFT was
//! held for these 30 consecutive ticks" into column movements is the engine's job, since
//! DAS, ARR and SDF all live inside the engine.
//!
//! That is more than an ergonomic choice. Because movement is not expressible, a remote
//! client cannot claim impossible movement. The worst it can do is emit button patterns
//! a human could not produce, which is a statistics problem rather than a correctness
//! one.

use bitflags::bitflags;

bitflags! {
    /// Buttons held during a single tick.
    ///
    /// `LEFT`, `RIGHT` and `SOFT_DROP` are level-triggered: the engine acts on them for
    /// as long as they are held. `CW`, `CCW`, `FLIP`, `HOLD` and `HARD_DROP` are
    /// edge-triggered, firing on the tick they are first seen and not again until
    /// released. The engine tracks the previous tick's buttons internally, so this
    /// distinction is invisible to the caller.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct Buttons: u8 {
        const LEFT      = 1 << 0;
        const RIGHT     = 1 << 1;
        const CW        = 1 << 2;
        const CCW       = 1 << 3;
        /// 180° rotation.
        const FLIP      = 1 << 4;
        const HOLD      = 1 << 5;
        const SOFT_DROP = 1 << 6;
        const HARD_DROP = 1 << 7;
    }
}

impl Buttons {
    /// Buttons that fire once on press rather than acting while held.
    pub const EDGE_TRIGGERED: Buttons = Buttons::CW
        .union(Buttons::CCW)
        .union(Buttons::FLIP)
        .union(Buttons::HOLD)
        .union(Buttons::HARD_DROP);

    /// The set newly pressed this tick, given the previous tick's buttons.
    #[inline]
    pub const fn pressed_since(self, prev: Buttons) -> Buttons {
        Buttons::from_bits_retain(self.bits() & !prev.bits())
    }

    /// The set released this tick, given the previous tick's buttons.
    #[inline]
    pub const fn released_since(self, prev: Buttons) -> Buttons {
        Buttons::from_bits_retain(!self.bits() & prev.bits())
    }
}

/// A horizontal movement direction. Distinct from `Buttons` because the DAS state
/// machine tracks exactly one charging direction, never both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
}

impl Dir {
    /// Column delta applied by one step in this direction.
    #[inline]
    pub const fn dx(self) -> i8 {
        match self {
            Dir::Left => -1,
            Dir::Right => 1,
        }
    }

    #[inline]
    pub const fn opposite(self) -> Dir {
        match self {
            Dir::Left => Dir::Right,
            Dir::Right => Dir::Left,
        }
    }

    /// Stable encoding for the state checksum. `None` is 0.
    #[inline]
    pub const fn checksum_code(this: Option<Dir>) -> u8 {
        match this {
            None => 0,
            Some(Dir::Left) => 1,
            Some(Dir::Right) => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_button_occupies_a_distinct_bit() {
        let all = Buttons::all();
        assert_eq!(
            all.bits().count_ones(),
            8,
            "all eight bits should be claimed"
        );
    }

    #[test]
    fn edge_detection_fires_only_on_the_transition() {
        let prev = Buttons::LEFT;
        let now = Buttons::LEFT | Buttons::CW;
        assert_eq!(now.pressed_since(prev), Buttons::CW);
        assert_eq!(now.released_since(prev), Buttons::empty());

        // Holding CW for a second tick must not fire again.
        assert_eq!(now.pressed_since(now), Buttons::empty());
    }

    #[test]
    fn release_detection_is_the_mirror_of_press() {
        let prev = Buttons::LEFT | Buttons::HARD_DROP;
        let now = Buttons::RIGHT;
        assert_eq!(now.released_since(prev), Buttons::LEFT | Buttons::HARD_DROP);
        assert_eq!(now.pressed_since(prev), Buttons::RIGHT);
    }

    #[test]
    fn edge_triggered_set_is_exactly_the_documented_five() {
        assert_eq!(
            Buttons::EDGE_TRIGGERED,
            Buttons::CW | Buttons::CCW | Buttons::FLIP | Buttons::HOLD | Buttons::HARD_DROP
        );
        // The level-triggered remainder is the movement/soft-drop set.
        assert_eq!(
            Buttons::all() - Buttons::EDGE_TRIGGERED,
            Buttons::LEFT | Buttons::RIGHT | Buttons::SOFT_DROP
        );
    }

    #[test]
    fn direction_deltas_and_inverses() {
        assert_eq!(Dir::Left.dx(), -1);
        assert_eq!(Dir::Right.dx(), 1);
        assert_eq!(Dir::Left.opposite(), Dir::Right);
        assert_eq!(Dir::Right.opposite().opposite(), Dir::Right);
    }

    #[test]
    fn direction_checksum_codes_are_distinct_and_stable() {
        assert_eq!(Dir::checksum_code(None), 0);
        assert_eq!(Dir::checksum_code(Some(Dir::Left)), 1);
        assert_eq!(Dir::checksum_code(Some(Dir::Right)), 2);
    }
}
