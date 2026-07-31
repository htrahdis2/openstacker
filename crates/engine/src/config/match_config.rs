//! Rules shared by every player in a match.
//!
//! Where handling is a personal preference, this is the game being played. It is chosen
//! once, applies to everyone, and travels with a replay so a recorded game can be
//! re-simulated under the rules it was actually played under rather than whatever the
//! rules happen to be today.
//!
//! Game modes are data: a mode file supplies one of these, so adding a mode needs no
//! code and no recompile.

use super::desc::{EnumVariant, FieldDesc, FieldKind, Tunable, Unit, field};
use crate::fixed::{ms_to_subticks, ms_to_subticks_nonzero};
use arrayvec::ArrayVec;

/// Maximum stages in a staged gravity curve.
pub const MAX_GRAVITY_STAGES: usize = 16;

/// Longest combo the table can reward. Beyond this the last entry repeats.
pub const COMBO_TABLE_LEN: usize = 21;

/// How fast pieces fall, and whether that changes over time.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "snake_case"))]
pub enum GravityCurve {
    /// Constant speed. `0` means instant, dropping to the floor in one tick.
    Fixed { ms_per_row: u32 },
    /// Speed changes at fixed points in the game. The last stage holds forever.
    Staged {
        stages: ArrayVec<GravityStage, MAX_GRAVITY_STAGES>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GravityStage {
    pub from_tick: u32,
    pub ms_per_row: u32,
}

impl Default for GravityCurve {
    fn default() -> Self {
        GravityCurve::Fixed { ms_per_row: 1000 }
    }
}

impl GravityCurve {
    /// Milliseconds per row at a given tick.
    ///
    /// Stages are scanned in order and the last one whose start has passed wins, so an
    /// unsorted or overlapping stage list still produces one defined answer rather than
    /// depending on iteration order.
    pub fn ms_per_row_at(&self, tick: u32) -> u32 {
        match self {
            GravityCurve::Fixed { ms_per_row } => *ms_per_row,
            GravityCurve::Staged { stages } => {
                let mut best: Option<&GravityStage> = None;
                for s in stages {
                    if s.from_tick <= tick && best.is_none_or(|b| s.from_tick >= b.from_tick) {
                        best = Some(s);
                    }
                }
                best.map(|s| s.ms_per_row).unwrap_or(1000)
            }
        }
    }

    /// Subtick threshold a gravity accumulator must cross to advance one row, or `None`
    /// when gravity is instant.
    pub fn threshold_at(&self, tick: u32) -> Option<u32> {
        match self.ms_per_row_at(tick) {
            0 => None,
            ms => Some(ms_to_subticks_nonzero(ms)),
        }
    }
}

/// When a move or rotation is allowed to restart lock delay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum LockResetMode {
    /// Only descending a row resets the timer.
    Classic,
    /// Any successful move or rotation resets it, up to a cap.
    #[default]
    Extended,
    /// Any successful move or rotation resets it, forever.
    Infinite,
}

impl LockResetMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            LockResetMode::Classic => "classic",
            LockResetMode::Extended => "extended",
            LockResetMode::Infinite => "infinite",
        }
    }
}

/// How a spin is recognised for scoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum SpinRule {
    /// Spins are never recognised.
    None,
    /// T pieces only, by counting occupied corners around the centre.
    #[default]
    ThreeCorner,
    /// T pieces only, by the piece being unable to move after locking.
    Immobile,
    /// Any piece that cannot move after locking.
    AllSpin,
}

impl SpinRule {
    pub const fn as_str(self) -> &'static str {
        match self {
            SpinRule::None => "none",
            SpinRule::ThreeCorner => "three_corner",
            SpinRule::Immobile => "immobile",
            SpinRule::AllSpin => "all_spin",
        }
    }
}

/// Rows sent for each kind of clear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields, default))]
pub struct AttackTable {
    pub single: u8,
    pub double: u8,
    pub triple: u8,
    pub quad: u8,
    pub mini_spin_single: u8,
    pub mini_spin_double: u8,
    pub spin_single: u8,
    pub spin_double: u8,
    pub spin_triple: u8,
    pub perfect_clear: u8,
}

impl Default for AttackTable {
    fn default() -> Self {
        AttackTable {
            single: 0,
            double: 1,
            triple: 2,
            quad: 4,
            mini_spin_single: 0,
            mini_spin_double: 1,
            spin_single: 2,
            spin_double: 4,
            spin_triple: 6,
            perfect_clear: 10,
        }
    }
}

/// Most rows any single clear may send. Generous, since modes are free to be strange.
const MAX_ATTACK: i64 = 40;

/// Build an attack-table descriptor. Every entry has the same shape, so spelling each
/// out longhand would be twelve near-identical blocks.
const fn attack(key: &'static str, label: &'static str, default: i64) -> FieldDesc {
    FieldDesc {
        key,
        label,
        help: "Rows sent to the opponent for this clear.",
        group: "match.attack",
        kind: FieldKind::Int {
            min: 0,
            max: MAX_ATTACK,
            default,
            step: 1,
            unit: Unit::Rows,
        },
    }
}

impl Tunable for AttackTable {
    const FIELDS: &'static [FieldDesc] = &[
        attack("single", "Single", 0),
        attack("double", "Double", 1),
        attack("triple", "Triple", 2),
        attack("quad", "Quad", 4),
        attack("mini_spin_single", "Mini spin single", 0),
        attack("mini_spin_double", "Mini spin double", 1),
        attack("spin_single", "Spin single", 2),
        attack("spin_double", "Spin double", 4),
        attack("spin_triple", "Spin triple", 6),
        attack("perfect_clear", "Perfect clear", 10),
    ];

    fn clamp(&mut self) {
        let f = |k| field(Self::FIELDS, k);
        self.single = f("single").clamp_u8(self.single);
        self.double = f("double").clamp_u8(self.double);
        self.triple = f("triple").clamp_u8(self.triple);
        self.quad = f("quad").clamp_u8(self.quad);
        self.mini_spin_single = f("mini_spin_single").clamp_u8(self.mini_spin_single);
        self.mini_spin_double = f("mini_spin_double").clamp_u8(self.mini_spin_double);
        self.spin_single = f("spin_single").clamp_u8(self.spin_single);
        self.spin_double = f("spin_double").clamp_u8(self.spin_double);
        self.spin_triple = f("spin_triple").clamp_u8(self.spin_triple);
        self.perfect_clear = f("perfect_clear").clamp_u8(self.perfect_clear);
    }
}

const TIMING: &str = "match.timing";
const BOARD: &str = "match.board";
const SCORING: &str = "match.scoring";
const GARBAGE: &str = "match.garbage";

const LOCK_RESET_VARIANTS: &[EnumVariant] = &[
    EnumVariant {
        value: "classic",
        label: "Classic",
        help: "Only descending a row resets lock delay.",
    },
    EnumVariant {
        value: "extended",
        label: "Extended",
        help: "Any move or rotation resets lock delay, up to the reset cap.",
    },
    EnumVariant {
        value: "infinite",
        label: "Infinite",
        help: "Any move or rotation resets lock delay, with no cap.",
    },
];

const SPIN_VARIANTS: &[EnumVariant] = &[
    EnumVariant {
        value: "none",
        label: "None",
        help: "Spins are never recognised.",
    },
    EnumVariant {
        value: "three_corner",
        label: "Three corner",
        help: "T pieces only, judged by occupied corners around the centre.",
    },
    EnumVariant {
        value: "immobile",
        label: "Immobile",
        help: "T pieces only, judged by the piece being unable to move after locking.",
    },
    EnumVariant {
        value: "all_spin",
        label: "All spin",
        help: "Any piece that cannot move after locking counts as a spin.",
    },
];

/// The rules of a match.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields, default))]
pub struct MatchConfig {
    pub gravity: GravityCurve,
    pub lock_delay_ms: u16,
    pub lock_reset_mode: LockResetMode,
    pub lock_reset_cap: u8,
    pub clear_delay_ms: u16,
    pub spawn_delay_ms: u16,

    pub preview_len: u8,
    pub hold_enabled: bool,

    pub spin_detection: SpinRule,
    pub attack_table: AttackTable,
    pub combo_table: ArrayVec<u8, COMBO_TABLE_LEN>,
    pub b2b_bonus: u8,

    pub garbage_delay_ms: u16,
    pub garbage_cap: u8,
    pub garbage_hole_repeat: bool,
}

/// Default combo rewards, indexed by combo count. Saturates at the last entry.
fn default_combo_table() -> ArrayVec<u8, COMBO_TABLE_LEN> {
    let mut v = ArrayVec::new();
    for n in [0u8, 0, 1, 1, 1, 2, 2, 3, 3, 4, 4, 4, 5] {
        v.push(n);
    }
    v
}

impl Default for MatchConfig {
    fn default() -> Self {
        MatchConfig {
            gravity: GravityCurve::default(),
            lock_delay_ms: 500,
            lock_reset_mode: LockResetMode::Extended,
            lock_reset_cap: 15,
            clear_delay_ms: 0,
            spawn_delay_ms: 0,
            preview_len: 5,
            hold_enabled: true,
            spin_detection: SpinRule::ThreeCorner,
            attack_table: AttackTable::default(),
            combo_table: default_combo_table(),
            b2b_bonus: 1,
            garbage_delay_ms: 1000,
            garbage_cap: 8,
            garbage_hole_repeat: true,
        }
    }
}

impl Tunable for MatchConfig {
    const NESTED: &'static [&'static str] = &["gravity", "attack_table", "combo_table"];

    const FIELDS: &'static [FieldDesc] = &[
        FieldDesc {
            key: "lock_delay_ms",
            label: "Lock delay",
            help: "How long a piece rests on the stack before locking.",
            group: TIMING,
            kind: FieldKind::Int {
                min: 0,
                max: 5000,
                default: 500,
                step: 10,
                unit: Unit::Millis,
            },
        },
        FieldDesc {
            key: "lock_reset_mode",
            label: "Lock reset",
            help: "Which actions restart lock delay.",
            group: TIMING,
            kind: FieldKind::Enum {
                variants: LOCK_RESET_VARIANTS,
                default: "extended",
            },
        },
        FieldDesc {
            key: "lock_reset_cap",
            label: "Lock reset cap",
            help: "How many times lock delay may restart before the piece locks \
                   regardless. Only used by the extended reset mode.",
            group: TIMING,
            kind: FieldKind::Int {
                min: 0,
                max: 255,
                default: 15,
                step: 1,
                unit: Unit::Count,
            },
        },
        FieldDesc {
            key: "clear_delay_ms",
            label: "Clear delay",
            help: "Pause after rows clear, before the next piece spawns.",
            group: TIMING,
            kind: FieldKind::Int {
                min: 0,
                max: 1000,
                default: 0,
                step: 10,
                unit: Unit::Millis,
            },
        },
        FieldDesc {
            key: "spawn_delay_ms",
            label: "Spawn delay",
            help: "Pause between a piece locking and the next one appearing.",
            group: TIMING,
            kind: FieldKind::Int {
                min: 0,
                max: 1000,
                default: 0,
                step: 10,
                unit: Unit::Millis,
            },
        },
        FieldDesc {
            key: "preview_len",
            label: "Preview",
            help: "How many upcoming pieces are shown.",
            group: BOARD,
            kind: FieldKind::Int {
                min: 0,
                max: crate::consts::MAX_PREVIEW as i64,
                default: 5,
                step: 1,
                unit: Unit::Count,
            },
        },
        FieldDesc {
            key: "hold_enabled",
            label: "Hold",
            help: "Whether a piece can be set aside for later.",
            group: BOARD,
            kind: FieldKind::Bool { default: true },
        },
        FieldDesc {
            key: "spin_detection",
            label: "Spin detection",
            help: "How a spin is recognised for scoring.",
            group: SCORING,
            kind: FieldKind::Enum {
                variants: SPIN_VARIANTS,
                default: "three_corner",
            },
        },
        FieldDesc {
            key: "b2b_bonus",
            label: "Back-to-back bonus",
            help: "Extra rows sent for chaining hard clears without a plain clear in \
                   between.",
            group: SCORING,
            kind: FieldKind::Int {
                min: 0,
                max: 20,
                default: 1,
                step: 1,
                unit: Unit::Rows,
            },
        },
        FieldDesc {
            key: "garbage_delay_ms",
            label: "Garbage delay",
            help: "How long incoming rows wait before they enter the board. Must stay \
                   comfortably above round-trip latency, or rows arrive after the tick \
                   they were scheduled for.",
            group: GARBAGE,
            kind: FieldKind::Int {
                min: 0,
                max: 10_000,
                default: 1000,
                step: 50,
                unit: Unit::Millis,
            },
        },
        FieldDesc {
            key: "garbage_cap",
            label: "Garbage cap",
            help: "Most rows that can enter the board at once.",
            group: GARBAGE,
            kind: FieldKind::Int {
                min: 1,
                max: crate::consts::BOARD_H as i64,
                default: 8,
                step: 1,
                unit: Unit::Rows,
            },
        },
        FieldDesc {
            key: "garbage_hole_repeat",
            label: "Repeat garbage hole",
            help: "Whether every row in one batch of garbage shares a hole column.",
            group: GARBAGE,
            kind: FieldKind::Bool { default: true },
        },
    ];

    fn clamp(&mut self) {
        let f = |k| field(Self::FIELDS, k);
        self.lock_delay_ms = f("lock_delay_ms").clamp_u16(self.lock_delay_ms);
        self.lock_reset_cap = f("lock_reset_cap").clamp_u8(self.lock_reset_cap);
        self.clear_delay_ms = f("clear_delay_ms").clamp_u16(self.clear_delay_ms);
        self.spawn_delay_ms = f("spawn_delay_ms").clamp_u16(self.spawn_delay_ms);
        self.preview_len = f("preview_len").clamp_u8(self.preview_len);
        self.b2b_bonus = f("b2b_bonus").clamp_u8(self.b2b_bonus);
        self.garbage_delay_ms = f("garbage_delay_ms").clamp_u16(self.garbage_delay_ms);
        self.garbage_cap = f("garbage_cap").clamp_u8(self.garbage_cap);

        // A combo table must never be empty: scoring indexes into it on every clear.
        if self.combo_table.is_empty() {
            self.combo_table = default_combo_table();
        }
    }
}

impl MatchConfig {
    /// Combo reward for a given combo count, saturating at the end of the table.
    pub fn combo_bonus(&self, combo: u8) -> u8 {
        if self.combo_table.is_empty() {
            return 0;
        }
        let i = (combo as usize).min(self.combo_table.len() - 1);
        self.combo_table[i]
    }

    /// Lock delay in subticks.
    pub fn lock_delay_sub(&self) -> u32 {
        ms_to_subticks(self.lock_delay_ms as u32)
    }

    pub fn clear_delay_sub(&self) -> u32 {
        ms_to_subticks(self.clear_delay_ms as u32)
    }

    pub fn spawn_delay_sub(&self) -> u32 {
        ms_to_subticks(self.spawn_delay_ms as u32)
    }

    /// Garbage delay expressed in whole ticks, which is the unit a scheduled arrival
    /// tick is counted in.
    pub fn garbage_delay_ticks(&self) -> u32 {
        ms_to_subticks(self.garbage_delay_ms as u32) / crate::consts::SUBTICK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_inside_their_declared_ranges() {
        for f in MatchConfig::FIELDS {
            assert!(f.kind.default_is_in_range(), "{}", f.key);
        }
    }

    #[cfg(all(feature = "serde", feature = "std"))]
    #[test]
    fn descriptors_cover_every_serde_field() {
        crate::config::assert_descriptors_match_serde_fields(&MatchConfig::default());
        crate::config::assert_descriptors_match_serde_fields(&AttackTable::default());
    }

    #[test]
    fn attack_table_descriptor_defaults_match_the_struct_default() {
        let d = AttackTable::default();
        let int = |k| match field(AttackTable::FIELDS, k).kind {
            FieldKind::Int { default, .. } => default,
            _ => panic!("{k} is not an int field"),
        };
        assert_eq!(int("single"), d.single as i64);
        assert_eq!(int("double"), d.double as i64);
        assert_eq!(int("triple"), d.triple as i64);
        assert_eq!(int("quad"), d.quad as i64);
        assert_eq!(int("mini_spin_single"), d.mini_spin_single as i64);
        assert_eq!(int("mini_spin_double"), d.mini_spin_double as i64);
        assert_eq!(int("spin_single"), d.spin_single as i64);
        assert_eq!(int("spin_double"), d.spin_double as i64);
        assert_eq!(int("spin_triple"), d.spin_triple as i64);
        assert_eq!(int("perfect_clear"), d.perfect_clear as i64);
    }

    #[test]
    fn every_descriptor_default_matches_the_struct_default() {
        let d = MatchConfig::default();
        let g = |k| field(MatchConfig::FIELDS, k);
        let int = |k| match g(k).kind {
            FieldKind::Int { default, .. } => default,
            _ => panic!("{k} is not an int field"),
        };
        assert_eq!(int("lock_delay_ms"), d.lock_delay_ms as i64);
        assert_eq!(int("lock_reset_cap"), d.lock_reset_cap as i64);
        assert_eq!(int("clear_delay_ms"), d.clear_delay_ms as i64);
        assert_eq!(int("spawn_delay_ms"), d.spawn_delay_ms as i64);
        assert_eq!(int("preview_len"), d.preview_len as i64);
        assert_eq!(int("b2b_bonus"), d.b2b_bonus as i64);
        assert_eq!(int("garbage_delay_ms"), d.garbage_delay_ms as i64);
        assert_eq!(int("garbage_cap"), d.garbage_cap as i64);
        match g("lock_reset_mode").kind {
            FieldKind::Enum { default, .. } => {
                assert_eq!(default, d.lock_reset_mode.as_str())
            }
            _ => panic!("not an enum"),
        }
        match g("spin_detection").kind {
            FieldKind::Enum { default, .. } => assert_eq!(default, d.spin_detection.as_str()),
            _ => panic!("not an enum"),
        }
    }

    #[test]
    fn preview_length_cannot_exceed_what_the_queue_can_show() {
        let mut c = MatchConfig {
            preview_len: 200,
            ..Default::default()
        };
        c.clamp();
        assert_eq!(c.preview_len as usize, crate::consts::MAX_PREVIEW);
    }

    #[test]
    fn garbage_cap_cannot_be_zero_or_exceed_the_board() {
        let mut c = MatchConfig {
            garbage_cap: 0,
            ..Default::default()
        };
        c.clamp();
        assert_eq!(c.garbage_cap, 1, "zero would make garbage a no-op");

        let mut c = MatchConfig {
            garbage_cap: 255,
            ..Default::default()
        };
        c.clamp();
        assert_eq!(c.garbage_cap as usize, crate::consts::BOARD_H);
    }

    #[test]
    fn an_empty_combo_table_is_repaired_rather_than_left_to_panic() {
        // Scoring indexes into this on every clear, so an empty table from a malformed
        // mode file would be an index panic in the middle of a match.
        let mut c = MatchConfig {
            combo_table: ArrayVec::new(),
            ..Default::default()
        };
        c.clamp();
        assert!(!c.combo_table.is_empty());
    }

    #[test]
    fn combo_bonus_saturates_past_the_end_of_the_table() {
        let c = MatchConfig::default();
        let last = *c.combo_table.last().unwrap();
        assert_eq!(c.combo_bonus(0), 0);
        assert_eq!(c.combo_bonus(2), 1);
        assert_eq!(c.combo_bonus(u8::MAX), last, "must not index out of bounds");
    }

    #[test]
    fn fixed_gravity_reports_the_same_speed_at_every_tick() {
        let g = GravityCurve::Fixed { ms_per_row: 800 };
        assert_eq!(g.ms_per_row_at(0), 800);
        assert_eq!(g.ms_per_row_at(u32::MAX), 800);
        assert_eq!(g.threshold_at(0), Some(ms_to_subticks_nonzero(800)));
    }

    #[test]
    fn instant_gravity_has_no_threshold() {
        // Zero means "fall to the floor this tick", which is an absence of a threshold
        // rather than a threshold of zero. A zero threshold would loop forever.
        let g = GravityCurve::Fixed { ms_per_row: 0 };
        assert_eq!(g.threshold_at(0), None);
    }

    #[test]
    fn a_gravity_threshold_is_never_zero_for_any_nonzero_speed() {
        for ms in 1..=64u32 {
            let g = GravityCurve::Fixed { ms_per_row: ms };
            assert!(
                g.threshold_at(0).unwrap() >= 1,
                "{ms} ms produced a zero threshold"
            );
        }
    }

    #[test]
    fn staged_gravity_takes_the_latest_stage_that_has_started() {
        let mut stages = ArrayVec::new();
        stages.push(GravityStage {
            from_tick: 0,
            ms_per_row: 1000,
        });
        stages.push(GravityStage {
            from_tick: 600,
            ms_per_row: 500,
        });
        stages.push(GravityStage {
            from_tick: 1200,
            ms_per_row: 100,
        });
        let g = GravityCurve::Staged { stages };

        assert_eq!(g.ms_per_row_at(0), 1000);
        assert_eq!(g.ms_per_row_at(599), 1000);
        assert_eq!(g.ms_per_row_at(600), 500, "a stage starts on its own tick");
        assert_eq!(
            g.ms_per_row_at(1_000_000),
            100,
            "the last stage holds forever"
        );
    }

    #[test]
    fn staged_gravity_is_defined_even_for_an_unsorted_stage_list() {
        // A mode file is hand-written, so the stages can arrive in any order. The result
        // must not depend on that order.
        let mut sorted = ArrayVec::new();
        sorted.push(GravityStage {
            from_tick: 0,
            ms_per_row: 1000,
        });
        sorted.push(GravityStage {
            from_tick: 600,
            ms_per_row: 500,
        });

        let mut shuffled = ArrayVec::new();
        shuffled.push(GravityStage {
            from_tick: 600,
            ms_per_row: 500,
        });
        shuffled.push(GravityStage {
            from_tick: 0,
            ms_per_row: 1000,
        });

        for tick in [0u32, 1, 599, 600, 601, 5000] {
            assert_eq!(
                GravityCurve::Staged {
                    stages: sorted.clone()
                }
                .ms_per_row_at(tick),
                GravityCurve::Staged {
                    stages: shuffled.clone()
                }
                .ms_per_row_at(tick),
                "stage order changed the answer at tick {tick}"
            );
        }
    }

    #[test]
    fn staged_gravity_falls_back_when_no_stage_has_started_yet() {
        let mut stages = ArrayVec::new();
        stages.push(GravityStage {
            from_tick: 100,
            ms_per_row: 500,
        });
        let g = GravityCurve::Staged { stages };
        assert_eq!(g.ms_per_row_at(0), 1000, "must be defined before any stage");
    }

    #[test]
    fn garbage_delay_converts_to_whole_ticks() {
        let c = MatchConfig::default();
        assert_eq!(c.garbage_delay_ticks(), 60, "1000 ms is 60 ticks");
    }

    #[test]
    fn clamping_is_idempotent() {
        let mut c = MatchConfig {
            lock_delay_ms: 60_000,
            preview_len: 99,
            garbage_cap: 0,
            ..Default::default()
        };
        c.clamp();
        let once = c.clone();
        c.clamp();
        assert_eq!(c, once);
    }
}
