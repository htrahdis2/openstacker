//! Machine-readable descriptions of every tunable setting.
//!
//! Each config struct carries a `FIELDS` table describing its own settings: bounds,
//! defaults, units, and help text. That table is the single source of truth, and four
//! things are derived from it rather than written by hand:
//!
//! - `clamp()`, so bounds cannot drift from the doc comments next to them
//! - a generated settings UI, so adding a setting is a one-line change here and no
//!   change at all in the client
//! - a JSON schema for docs and tooling
//! - error messages that suggest the nearest valid key when a config file has a typo
//!
//! The tables are plain `const` data with no dependencies, so they cost nothing at
//! runtime and work in `no_std` builds.

/// What a setting is measured in. Drives the suffix a UI shows next to a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Millis,
    Rows,
    Count,
    None,
}

impl Unit {
    pub const fn suffix(self) -> &'static str {
        match self {
            Unit::Millis => "ms",
            Unit::Rows => "rows",
            Unit::Count => "",
            Unit::None => "",
        }
    }
}

/// One option of an enum-valued setting.
#[derive(Debug, Clone, Copy)]
pub struct EnumVariant {
    pub value: &'static str,
    pub label: &'static str,
    pub help: &'static str,
}

/// The shape and bounds of a setting.
#[derive(Debug, Clone, Copy)]
pub enum FieldKind {
    Int {
        min: i64,
        max: i64,
        default: i64,
        /// Suggested UI increment. Never enforced on parsed values.
        step: i64,
        unit: Unit,
    },
    Bool {
        default: bool,
    },
    Enum {
        variants: &'static [EnumVariant],
        default: &'static str,
    },
    /// A table of numbers, such as the reward for a combo of a given length.
    ///
    /// Every entry shares one set of bounds, and the list has a length cap. A shorter
    /// list is legal: scoring saturates at the last entry rather than indexing past it.
    IntList {
        min: i64,
        max: i64,
        max_len: usize,
        default: &'static [i64],
        unit: Unit,
    },
}

impl FieldKind {
    /// Whether the declared default satisfies the declared bounds. Tested, not assumed.
    pub const fn default_is_in_range(&self) -> bool {
        match self {
            FieldKind::Int {
                min, max, default, ..
            } => *min <= *default && *default <= *max,
            FieldKind::Bool { .. } => true,
            FieldKind::IntList {
                min,
                max,
                max_len,
                default,
                ..
            } => {
                if default.len() > *max_len {
                    return false;
                }
                let mut i = 0;
                while i < default.len() {
                    if default[i] < *min || default[i] > *max {
                        return false;
                    }
                    i += 1;
                }
                true
            }
            FieldKind::Enum { variants, default } => {
                let mut i = 0;
                while i < variants.len() {
                    if const_str_eq(variants[i].value, default) {
                        return true;
                    }
                    i += 1;
                }
                false
            }
        }
    }
}

/// `str` equality usable from a `const fn`.
const fn const_str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// A single tunable setting.
#[derive(Debug, Clone, Copy)]
pub struct FieldDesc {
    /// Must match the serde field name exactly. A test enforces this.
    pub key: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    /// Dotted path used to group settings into UI sections, e.g. `handling.movement`.
    pub group: &'static str,
    pub kind: FieldKind,
}

impl FieldDesc {
    /// Clamp a value to this field's declared range. For a list field this clamps one
    /// entry, since every entry shares the same bounds. Other kinds pass through.
    pub const fn clamp_i64(&self, v: i64) -> i64 {
        match self.kind {
            FieldKind::Int { min, max, .. } | FieldKind::IntList { min, max, .. } => {
                if v < min {
                    min
                } else if v > max {
                    max
                } else {
                    v
                }
            }
            _ => v,
        }
    }

    /// Longest list this field accepts, or 0 if it is not a list.
    pub const fn max_len(&self) -> usize {
        match self.kind {
            FieldKind::IntList { max_len, .. } => max_len,
            _ => 0,
        }
    }

    pub const fn clamp_u16(&self, v: u16) -> u16 {
        self.clamp_i64(v as i64) as u16
    }

    pub const fn clamp_u8(&self, v: u8) -> u8 {
        self.clamp_i64(v as i64) as u8
    }

    pub const fn clamp_u32(&self, v: u32) -> u32 {
        self.clamp_i64(v as i64) as u32
    }
}

/// A config struct that describes its own settings.
pub trait Tunable {
    /// Every scalar setting on this struct.
    const FIELDS: &'static [FieldDesc];

    /// Serde keys that hold nested config rather than a scalar, such as a sub-table or
    /// an enum. They have no single bound, so they are named here instead of in
    /// `FIELDS`, and the drift test accounts for them.
    const NESTED: &'static [&'static str] = &[];

    /// Force every field into its declared range.
    ///
    /// Player-supplied config is clamped rather than rejected: it arrives from an
    /// untrusted client, and clamping is deterministic, so every peer lands on the same
    /// values. Config *files* are validated more strictly by the loader, where an
    /// out-of-range value is an authoring mistake worth reporting.
    fn clamp(&mut self);
}

/// Look up a field by key.
///
/// # Panics
/// If no field has that key. That is a programming error in the descriptor table, and
/// the drift test in each config module catches it before it can reach a caller.
pub fn field(fields: &'static [FieldDesc], key: &str) -> &'static FieldDesc {
    let mut i = 0;
    while i < fields.len() {
        if const_str_eq(fields[i].key, key) {
            return &fields[i];
        }
        i += 1;
    }
    panic!("no descriptor for that config key");
}

/// The valid key with the smallest edit distance to `key`, for "did you mean" errors.
///
/// Returns `None` when nothing is close enough to be a plausible typo.
pub fn nearest_key(fields: &'static [FieldDesc], key: &str) -> Option<&'static str> {
    let mut best: Option<(usize, &'static str)> = None;
    for f in fields {
        let d = edit_distance(f.key, key);
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, f.key));
        }
    }
    let (d, k) = best?;

    // Two different mistakes are worth catching, and they need different thresholds.
    //
    // A slip of the fingers stays within a few edits, so a small bound covers it.
    //
    // Naming a setting with the wrong unit does not. Timing fields here are in
    // milliseconds while players think in frames, so someone will write `das_ticks` for
    // `das_ms`. That is four edits away, but it shares a prefix and is unmistakably the
    // same setting. A shared prefix is strong enough evidence to widen the bound.
    if d <= 3 || (common_prefix_len(k, key) >= 3 && d <= 6) {
        Some(k)
    } else {
        None
    }
}

fn common_prefix_len(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count()
}

/// Levenshtein distance, bounded so it needs no allocation.
fn edit_distance(a: &str, b: &str) -> usize {
    const MAX: usize = 48;
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() > MAX || b.len() > MAX {
        return usize::MAX;
    }
    let mut prev = [0usize; MAX + 1];
    let mut cur = [0usize; MAX + 1];
    for (j, slot) in prev.iter_mut().enumerate().take(b.len() + 1) {
        *slot = j;
    }
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let sub = prev[j - 1] + usize::from(a[i - 1] != b[j - 1]);
            cur[j] = sub.min(prev[j] + 1).min(cur[j - 1] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[FieldDesc] = &[
        FieldDesc {
            key: "das_ms",
            label: "DAS",
            help: "",
            group: "movement",
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
            help: "",
            group: "movement",
            kind: FieldKind::Bool { default: true },
        },
    ];

    #[test]
    fn clamping_respects_the_declared_bounds() {
        let f = field(SAMPLE, "das_ms");
        assert_eq!(f.clamp_i64(-5), 0);
        assert_eq!(f.clamp_i64(9999), 500);
        assert_eq!(f.clamp_i64(133), 133);
        assert_eq!(f.clamp_u16(u16::MAX), 500);
    }

    #[test]
    fn non_integer_fields_pass_through_clamping_unchanged() {
        let f = field(SAMPLE, "arr_ms");
        assert_eq!(f.clamp_i64(42), 42);
    }

    #[test]
    fn defaults_are_inside_their_own_ranges() {
        for f in SAMPLE {
            assert!(f.kind.default_is_in_range(), "{}", f.key);
        }
    }

    #[test]
    fn an_out_of_range_default_is_detected() {
        let bad = FieldKind::Int {
            min: 0,
            max: 10,
            default: 99,
            step: 1,
            unit: Unit::None,
        };
        assert!(!bad.default_is_in_range());
    }

    #[test]
    fn a_list_field_clamps_one_entry_at_a_time() {
        // Every entry shares the field's bounds, so clamping is per entry rather than
        // per list.
        const LIST: &[FieldDesc] = &[FieldDesc {
            key: "combo_table",
            label: "Combo",
            help: "",
            group: "scoring",
            kind: FieldKind::IntList {
                min: 0,
                max: 9,
                max_len: 4,
                default: &[0, 1, 2],
                unit: Unit::Rows,
            },
        }];
        let f = field(LIST, "combo_table");
        assert_eq!(f.clamp_i64(-1), 0);
        assert_eq!(f.clamp_i64(50), 9);
        assert_eq!(f.clamp_u8(3), 3);
        assert_eq!(f.max_len(), 4);
        assert!(f.kind.default_is_in_range());
    }

    #[test]
    fn a_list_default_outside_its_own_bounds_is_detected() {
        assert!(
            !FieldKind::IntList {
                min: 0,
                max: 3,
                max_len: 8,
                default: &[0, 9],
                unit: Unit::None,
            }
            .default_is_in_range()
        );
    }

    #[test]
    fn a_list_default_longer_than_its_cap_is_detected() {
        assert!(
            !FieldKind::IntList {
                min: 0,
                max: 9,
                max_len: 2,
                default: &[1, 1, 1],
                unit: Unit::None,
            }
            .default_is_in_range()
        );
    }

    #[test]
    fn a_field_that_is_not_a_list_has_no_length_cap() {
        assert_eq!(field(SAMPLE, "das_ms").max_len(), 0);
    }

    #[test]
    fn an_enum_default_must_name_a_real_variant() {
        const VARIANTS: &[EnumVariant] = &[EnumVariant {
            value: "classic",
            label: "Classic",
            help: "",
        }];
        assert!(
            FieldKind::Enum {
                variants: VARIANTS,
                default: "classic"
            }
            .default_is_in_range()
        );
        assert!(
            !FieldKind::Enum {
                variants: VARIANTS,
                default: "typo"
            }
            .default_is_in_range()
        );
    }

    #[test]
    fn a_typo_suggests_the_key_it_was_probably_meant_to_be() {
        assert_eq!(nearest_key(SAMPLE, "dasms"), Some("das_ms"));
        assert_eq!(nearest_key(SAMPLE, "das_sm"), Some("das_ms"));
        assert_eq!(nearest_key(SAMPLE, "arr_ns"), Some("arr_ms"));
    }

    #[test]
    fn naming_a_setting_with_the_wrong_unit_is_still_recognised() {
        // Players think in frames while these fields are in milliseconds. Writing
        // `das_ticks` is four edits from `das_ms`, past the plain typo threshold, but it
        // is obviously the same setting and must not be silently unrecognised.
        assert_eq!(nearest_key(SAMPLE, "das_ticks"), Some("das_ms"));
        assert_eq!(nearest_key(SAMPLE, "das_frames"), Some("das_ms"));
    }

    #[test]
    fn nothing_is_suggested_for_a_key_that_is_not_a_plausible_typo() {
        assert_eq!(nearest_key(SAMPLE, "completely_unrelated_setting"), None);
    }

    #[test]
    fn edit_distance_basics() {
        assert_eq!(edit_distance("abc", "abc"), 0);
        assert_eq!(edit_distance("abc", "abd"), 1);
        assert_eq!(edit_distance("abc", ""), 3);
        assert_eq!(edit_distance("", "xy"), 2);
    }

    #[test]
    fn unit_suffixes_are_stable() {
        assert_eq!(Unit::Millis.suffix(), "ms");
        assert_eq!(Unit::None.suffix(), "");
    }
}
