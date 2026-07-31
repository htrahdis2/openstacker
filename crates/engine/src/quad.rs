//! Piece kinds and rotation states.
//!
//! The seven kinds are named for the letters their shapes resemble. That is a
//! description of geometry and says nothing about how they are presented on screen.

/// One of the seven quad shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[repr(u8)]
pub enum QuadKind {
    I = 0,
    O = 1,
    T = 2,
    S = 3,
    Z = 4,
    J = 5,
    L = 6,
}

impl QuadKind {
    /// All seven kinds, in the canonical order used to seed a bag.
    pub const ALL: [QuadKind; 7] = [
        QuadKind::I,
        QuadKind::O,
        QuadKind::T,
        QuadKind::S,
        QuadKind::Z,
        QuadKind::J,
        QuadKind::L,
    ];

    pub const COUNT: usize = 7;

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub const fn from_index(i: usize) -> Option<QuadKind> {
        match i {
            0 => Some(QuadKind::I),
            1 => Some(QuadKind::O),
            2 => Some(QuadKind::T),
            3 => Some(QuadKind::S),
            4 => Some(QuadKind::Z),
            5 => Some(QuadKind::J),
            6 => Some(QuadKind::L),
            _ => None,
        }
    }

    /// Single-character label, for the ASCII renderer and input scripts.
    #[inline]
    pub const fn label(self) -> char {
        match self {
            QuadKind::I => 'I',
            QuadKind::O => 'O',
            QuadKind::T => 'T',
            QuadKind::S => 'S',
            QuadKind::Z => 'Z',
            QuadKind::J => 'J',
            QuadKind::L => 'L',
        }
    }

    /// Render-channel color index. Never read by game logic.
    ///
    /// These are opaque indices, not colors. What they actually look like is decided
    /// entirely by the client's skin data.
    #[inline]
    pub const fn color(self) -> u8 {
        self as u8 + 1 // 0 is reserved for empty
    }

    /// Whether this kind kicks at all. `O` is rotationally symmetric and never does.
    #[inline]
    pub const fn kicks(self) -> bool {
        !matches!(self, QuadKind::O)
    }
}

/// Rotation state. `R0` is spawn orientation; increasing is clockwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Rot {
    R0 = 0,
    R1 = 1,
    R2 = 2,
    R3 = 3,
}

impl Rot {
    pub const ALL: [Rot; 4] = [Rot::R0, Rot::R1, Rot::R2, Rot::R3];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub const fn from_index(i: usize) -> Rot {
        match i & 3 {
            0 => Rot::R0,
            1 => Rot::R1,
            2 => Rot::R2,
            _ => Rot::R3,
        }
    }

    #[inline]
    pub const fn cw(self) -> Rot {
        Rot::from_index(self as usize + 1)
    }

    #[inline]
    pub const fn ccw(self) -> Rot {
        Rot::from_index(self as usize + 3)
    }

    #[inline]
    pub const fn flip(self) -> Rot {
        Rot::from_index(self as usize + 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_kinds_are_distinct_and_round_trip_through_their_index() {
        for (i, k) in QuadKind::ALL.iter().enumerate() {
            assert_eq!(k.index(), i);
            assert_eq!(QuadKind::from_index(i), Some(*k));
        }
        assert_eq!(QuadKind::from_index(7), None);
        assert_eq!(QuadKind::ALL.len(), QuadKind::COUNT);
    }

    #[test]
    fn color_indices_are_distinct_and_never_collide_with_empty_or_garbage() {
        use crate::consts::{COLOR_EMPTY, COLOR_GARBAGE};
        let mut seen = [false; 256];
        for k in QuadKind::ALL {
            let c = k.color();
            assert_ne!(c, COLOR_EMPTY, "{k:?} collides with the empty marker");
            assert_ne!(c, COLOR_GARBAGE, "{k:?} collides with the garbage marker");
            assert!(!seen[c as usize], "duplicate color index for {k:?}");
            seen[c as usize] = true;
        }
    }

    #[test]
    fn labels_are_unique() {
        let mut labels: [char; 7] = QuadKind::ALL.map(|k| k.label());
        labels.sort_unstable();
        for w in labels.windows(2) {
            assert_ne!(w[0], w[1]);
        }
    }

    #[test]
    fn only_o_is_exempt_from_kicking() {
        for k in QuadKind::ALL {
            assert_eq!(k.kicks(), k != QuadKind::O);
        }
    }

    #[test]
    fn four_clockwise_rotations_return_to_spawn() {
        for r in Rot::ALL {
            assert_eq!(r.cw().cw().cw().cw(), r);
            assert_eq!(r.ccw().ccw().ccw().ccw(), r);
            assert_eq!(r.flip().flip(), r);
        }
    }

    #[test]
    fn cw_and_ccw_are_inverses_and_flip_is_two_of_either() {
        for r in Rot::ALL {
            assert_eq!(r.cw().ccw(), r);
            assert_eq!(r.cw().cw(), r.flip());
            assert_eq!(r.ccw().ccw(), r.flip());
        }
    }
}
