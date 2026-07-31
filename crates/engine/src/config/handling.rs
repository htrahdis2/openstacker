//! Per-player input feel.
//!
//! These reach the simulation, so they are frozen when a match starts and travel with
//! every replay. Two peers running the same inputs with different handling produce
//! different boards, which makes this the one category of player preference that an
//! authoritative peer has to know about and agree on.
//!
//! Adding a field here is not free: it changes simulation behaviour, so it invalidates
//! stored replays for verification. The set is deliberately complete up front for that
//! reason.

use super::desc::{EnumVariant, FieldDesc, FieldKind, Tunable, Unit, field};
use crate::fixed::ms_to_subticks;

/// When a rotation held across a spawn should apply to the new piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum IrsMode {
    /// Never pre-rotate.
    Off,
    /// Pre-rotate if the button is held at spawn.
    #[default]
    Hold,
    /// Pre-rotate only if the button was pressed during the preceding delay, so a
    /// button held down from the previous piece does not carry over.
    Tap,
}

impl IrsMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            IrsMode::Off => "off",
            IrsMode::Hold => "hold",
            IrsMode::Tap => "tap",
        }
    }
}

/// Player-chosen input timings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields, default))]
pub struct Handling {
    /// Delay before a held direction starts auto-repeating.
    pub das_ms: u16,
    /// Interval between auto-repeat steps. `0` snaps to the wall in one tick.
    pub arr_ms: u16,
    /// Time per row while soft dropping. `0` snaps to the floor in one tick.
    pub sdf_ms_per_row: u16,
    /// On a direction change, the new direction waits this long instead of a full DAS.
    /// `0` means a direction change re-charges DAS from scratch.
    pub dcd_ms: u16,
    /// After a spawn with a direction still held and DAS already charged, auto-repeat is
    /// suppressed this long, so a new piece does not immediately fly sideways.
    pub das_cut_delay_ms: u16,
    /// Whether a rotation held across a spawn applies to the new piece.
    pub irs: IrsMode,
    /// Whether a hold held across a spawn applies to the new piece.
    pub ihs: bool,
    /// Hard drop is ignored this long after a spawn, so a fast repeat does not
    /// accidentally slam the next piece. `0` disables the guard.
    pub prevent_misdrop_ms: u16,
    /// Whether soft dropping into the floor locks immediately instead of waiting out
    /// lock delay.
    pub soft_drop_lock: bool,
}

impl Default for Handling {
    fn default() -> Self {
        Handling {
            das_ms: 133,
            arr_ms: 0,
            sdf_ms_per_row: 0,
            dcd_ms: 0,
            das_cut_delay_ms: 0,
            irs: IrsMode::Hold,
            ihs: true,
            prevent_misdrop_ms: 0,
            soft_drop_lock: false,
        }
    }
}

const IRS_VARIANTS: &[EnumVariant] = &[
    EnumVariant {
        value: "off",
        label: "Off",
        help: "A rotation held across a spawn does nothing.",
    },
    EnumVariant {
        value: "hold",
        label: "Hold",
        help: "A rotation button held at spawn pre-rotates the new piece.",
    },
    EnumVariant {
        value: "tap",
        label: "Tap",
        help: "Only a rotation pressed during the spawn delay pre-rotates.",
    },
];

const MOVEMENT: &str = "handling.movement";
const DROP: &str = "handling.drop";
const SPAWN: &str = "handling.spawn";

impl Tunable for Handling {
    const FIELDS: &'static [FieldDesc] = &[
        FieldDesc {
            key: "das_ms",
            label: "DAS",
            help: "Delay before a held direction starts repeating. Lower feels faster \
                   but makes single taps harder to place.",
            group: MOVEMENT,
            kind: FieldKind::Int {
                min: 0,
                max: 500,
                default: 133,
                step: 1,
                unit: Unit::Millis,
            },
        },
        FieldDesc {
            key: "arr_ms",
            label: "ARR",
            help: "Time between auto-repeat steps once DAS has charged. Zero moves the \
                   piece to the wall instantly.",
            group: MOVEMENT,
            kind: FieldKind::Int {
                min: 0,
                max: 200,
                default: 0,
                step: 1,
                unit: Unit::Millis,
            },
        },
        FieldDesc {
            key: "dcd_ms",
            label: "DAS cancel on direction change",
            help: "How long the new direction waits after a direction change. Zero \
                   re-charges DAS from scratch.",
            group: MOVEMENT,
            kind: FieldKind::Int {
                min: 0,
                max: 200,
                default: 0,
                step: 1,
                unit: Unit::Millis,
            },
        },
        FieldDesc {
            key: "sdf_ms_per_row",
            label: "Soft drop speed",
            help: "Time to fall one row while soft dropping. Zero drops to the floor \
                   instantly.",
            group: DROP,
            kind: FieldKind::Int {
                min: 0,
                max: 500,
                default: 0,
                step: 1,
                unit: Unit::Millis,
            },
        },
        FieldDesc {
            key: "soft_drop_lock",
            label: "Soft drop locks",
            help: "Lock immediately on reaching the floor under soft drop, skipping \
                   lock delay.",
            group: DROP,
            kind: FieldKind::Bool { default: false },
        },
        FieldDesc {
            key: "das_cut_delay_ms",
            label: "DAS cut delay",
            help: "After a spawn with a direction held, suppress auto-repeat this long \
                   so the new piece does not immediately fly sideways.",
            group: SPAWN,
            kind: FieldKind::Int {
                min: 0,
                max: 200,
                default: 0,
                step: 1,
                unit: Unit::Millis,
            },
        },
        FieldDesc {
            key: "prevent_misdrop_ms",
            label: "Misdrop protection",
            help: "Ignore hard drop this long after a spawn, so a fast repeat does not \
                   slam the next piece. Zero disables it.",
            group: SPAWN,
            kind: FieldKind::Int {
                min: 0,
                max: 200,
                default: 0,
                step: 1,
                unit: Unit::Millis,
            },
        },
        FieldDesc {
            key: "irs",
            label: "Initial rotation",
            help: "Whether a rotation held across a spawn applies to the new piece.",
            group: SPAWN,
            kind: FieldKind::Enum {
                variants: IRS_VARIANTS,
                default: "hold",
            },
        },
        FieldDesc {
            key: "ihs",
            label: "Initial hold",
            help: "Whether a hold held across a spawn applies to the new piece.",
            group: SPAWN,
            kind: FieldKind::Bool { default: true },
        },
    ];

    fn clamp(&mut self) {
        let f = |k| field(Self::FIELDS, k);
        self.das_ms = f("das_ms").clamp_u16(self.das_ms);
        self.arr_ms = f("arr_ms").clamp_u16(self.arr_ms);
        self.sdf_ms_per_row = f("sdf_ms_per_row").clamp_u16(self.sdf_ms_per_row);
        self.dcd_ms = f("dcd_ms").clamp_u16(self.dcd_ms);
        self.das_cut_delay_ms = f("das_cut_delay_ms").clamp_u16(self.das_cut_delay_ms);
        self.prevent_misdrop_ms = f("prevent_misdrop_ms").clamp_u16(self.prevent_misdrop_ms);
        // irs, ihs and soft_drop_lock are closed sets; parsing already rejects anything
        // outside them, so there is nothing to clamp.
    }
}

/// [`Handling`] with every duration pre-converted to subticks.
///
/// The engine holds one of these rather than the millisecond form, so the conversion
/// happens once at construction instead of on every tick. Two peers converting the same
/// [`Handling`] always get the same values: it is integer arithmetic, verified identical
/// on native and wasm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandlingSub {
    pub das: u32,
    pub arr: u32,
    pub sdf_per_row: u32,
    pub dcd: u32,
    pub das_cut_delay: u32,
    pub prevent_misdrop: u32,
    pub irs: IrsMode,
    pub ihs: bool,
    pub soft_drop_lock: bool,
}

impl Handling {
    /// Clamp, then convert every duration to subticks.
    ///
    /// Clamping first is deliberate. An authoritative peer that clamped a player's
    /// values must simulate with the clamped ones and echo those back, or it and the
    /// client diverge immediately and it looks like a desync rather than a settings
    /// disagreement.
    pub fn to_subticks(&self) -> HandlingSub {
        let mut c = *self;
        c.clamp();
        HandlingSub {
            das: ms_to_subticks(c.das_ms as u32),
            arr: ms_to_subticks(c.arr_ms as u32),
            sdf_per_row: ms_to_subticks(c.sdf_ms_per_row as u32),
            dcd: ms_to_subticks(c.dcd_ms as u32),
            das_cut_delay: ms_to_subticks(c.das_cut_delay_ms as u32),
            prevent_misdrop: ms_to_subticks(c.prevent_misdrop_ms as u32),
            irs: c.irs,
            ihs: c.ihs,
            soft_drop_lock: c.soft_drop_lock,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_inside_their_declared_ranges() {
        for f in Handling::FIELDS {
            assert!(f.kind.default_is_in_range(), "{}", f.key);
        }
    }

    #[test]
    fn every_descriptor_default_matches_the_struct_default() {
        // Two sources of truth for a default is one too many. If they disagree, a
        // generated UI shows something different from what the engine actually uses.
        let d = Handling::default();
        let g = |k| field(Handling::FIELDS, k);
        let int = |k| match g(k).kind {
            FieldKind::Int { default, .. } => default,
            _ => panic!("{k} is not an int field"),
        };
        let boolean = |k| match g(k).kind {
            FieldKind::Bool { default } => default,
            _ => panic!("{k} is not a bool field"),
        };
        assert_eq!(int("das_ms"), d.das_ms as i64);
        assert_eq!(int("arr_ms"), d.arr_ms as i64);
        assert_eq!(int("sdf_ms_per_row"), d.sdf_ms_per_row as i64);
        assert_eq!(int("dcd_ms"), d.dcd_ms as i64);
        assert_eq!(int("das_cut_delay_ms"), d.das_cut_delay_ms as i64);
        assert_eq!(int("prevent_misdrop_ms"), d.prevent_misdrop_ms as i64);
        assert_eq!(boolean("ihs"), d.ihs);
        assert_eq!(boolean("soft_drop_lock"), d.soft_drop_lock);
        match g("irs").kind {
            FieldKind::Enum { default, .. } => assert_eq!(default, d.irs.as_str()),
            _ => panic!("irs is not an enum field"),
        }
    }

    #[cfg(all(feature = "serde", feature = "std"))]
    #[test]
    fn descriptors_cover_every_serde_field() {
        crate::config::assert_descriptors_match_serde_fields(&Handling::default());
    }

    #[test]
    fn field_keys_are_unique() {
        for (i, a) in Handling::FIELDS.iter().enumerate() {
            for b in &Handling::FIELDS[i + 1..] {
                assert_ne!(a.key, b.key, "duplicate descriptor key");
            }
        }
    }

    #[test]
    fn out_of_range_values_are_clamped_not_rejected() {
        // These arrive from an untrusted client. Clamping is deterministic, so every
        // peer lands on the same values.
        let mut h = Handling {
            das_ms: u16::MAX,
            arr_ms: u16::MAX,
            sdf_ms_per_row: u16::MAX,
            dcd_ms: u16::MAX,
            das_cut_delay_ms: u16::MAX,
            prevent_misdrop_ms: u16::MAX,
            ..Default::default()
        };
        h.clamp();
        assert_eq!(h.das_ms, 500);
        assert_eq!(h.arr_ms, 200);
        assert_eq!(h.sdf_ms_per_row, 500);
        assert_eq!(h.dcd_ms, 200);
        assert_eq!(h.das_cut_delay_ms, 200);
        assert_eq!(h.prevent_misdrop_ms, 200);
    }

    #[test]
    fn clamping_is_idempotent() {
        let mut h = Handling {
            das_ms: 9999,
            ..Default::default()
        };
        h.clamp();
        let once = h;
        h.clamp();
        assert_eq!(h, once);
    }

    #[test]
    fn zero_das_and_zero_arr_are_legitimate_and_survive_clamping() {
        // Competitively normal, not an error. The bounds must not quietly "fix" them.
        let mut h = Handling {
            das_ms: 0,
            arr_ms: 0,
            sdf_ms_per_row: 0,
            ..Default::default()
        };
        h.clamp();
        assert_eq!((h.das_ms, h.arr_ms, h.sdf_ms_per_row), (0, 0, 0));
    }

    #[test]
    fn subtick_conversion_clamps_before_converting() {
        // An unclamped value converted first would produce a subtick count no clamped
        // peer could ever reach, and the two would diverge on the first held direction.
        let wild = Handling {
            das_ms: u16::MAX,
            ..Default::default()
        };
        let mut clamped = wild;
        clamped.clamp();
        assert_eq!(wild.to_subticks(), clamped.to_subticks());
        assert_eq!(wild.to_subticks().das, ms_to_subticks(500));
    }

    #[test]
    fn default_das_converts_to_the_expected_subtick_count() {
        let s = Handling::default().to_subticks();
        assert_eq!(s.das, ms_to_subticks(133));
        assert_eq!(s.arr, 0, "instant ARR must stay exactly zero");
        assert_eq!(s.sdf_per_row, 0, "instant soft drop must stay exactly zero");
    }
}
