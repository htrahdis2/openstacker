//! Player settings that never reach the simulation.
//!
//! Keybinds resolve to [`Buttons`] before any engine call, and cosmetics stop at the
//! render layer. Neither is ever sent to a peer, because neither can change what
//! happens: two players with opposite key layouts and different skins running the same
//! inputs produce bit-identical boards.
//!
//! They live here anyway so that one settings UI can be generated from one descriptor
//! format, instead of the client inventing a second system for the half of the settings
//! the engine does not care about.
//!
//! [`Buttons`]: crate::Buttons

use super::desc::{EnumVariant, FieldDesc, FieldKind, Tunable, Unit, field};
use crate::input::Buttons;

/// An action a key can be bound to.
///
/// This is the same set as [`Buttons`], named for humans. The engine only ever sees the
/// resolved bitflags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Action {
    MoveLeft,
    MoveRight,
    RotateCw,
    RotateCcw,
    RotateFlip,
    Hold,
    SoftDrop,
    HardDrop,
}

impl Action {
    pub const ALL: [Action; 8] = [
        Action::MoveLeft,
        Action::MoveRight,
        Action::RotateCw,
        Action::RotateCcw,
        Action::RotateFlip,
        Action::Hold,
        Action::SoftDrop,
        Action::HardDrop,
    ];

    /// The button this action resolves to. This is the only crossing point between
    /// keybinds and the simulation, and it happens in the client before any engine call.
    pub const fn button(self) -> Buttons {
        match self {
            Action::MoveLeft => Buttons::LEFT,
            Action::MoveRight => Buttons::RIGHT,
            Action::RotateCw => Buttons::CW,
            Action::RotateCcw => Buttons::CCW,
            Action::RotateFlip => Buttons::FLIP,
            Action::Hold => Buttons::HOLD,
            Action::SoftDrop => Buttons::SOFT_DROP,
            Action::HardDrop => Buttons::HARD_DROP,
        }
    }

    pub const fn key(self) -> &'static str {
        match self {
            Action::MoveLeft => "move_left",
            Action::MoveRight => "move_right",
            Action::RotateCw => "rotate_cw",
            Action::RotateCcw => "rotate_ccw",
            Action::RotateFlip => "rotate_flip",
            Action::Hold => "hold",
            Action::SoftDrop => "soft_drop",
            Action::HardDrop => "hard_drop",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Action::MoveLeft => "Move left",
            Action::MoveRight => "Move right",
            Action::RotateCw => "Rotate clockwise",
            Action::RotateCcw => "Rotate counter-clockwise",
            Action::RotateFlip => "Rotate 180",
            Action::Hold => "Hold",
            Action::SoftDrop => "Soft drop",
            Action::HardDrop => "Hard drop",
        }
    }
}

const BINDS: &str = "keybinds";
const DISPLAY: &str = "cosmetic.display";
const AUDIO: &str = "cosmetic.audio";

/// Which physical key triggers each action.
///
/// Values are platform key names, kept as opaque strings so the engine never has to know
/// anything about input hardware.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields, default))]
pub struct Keybinds {
    pub move_left: &'static str,
    pub move_right: &'static str,
    pub rotate_cw: &'static str,
    pub rotate_ccw: &'static str,
    pub rotate_flip: &'static str,
    pub hold: &'static str,
    pub soft_drop: &'static str,
    pub hard_drop: &'static str,
}

impl Default for Keybinds {
    fn default() -> Self {
        Keybinds {
            move_left: "ArrowLeft",
            move_right: "ArrowRight",
            rotate_cw: "ArrowUp",
            rotate_ccw: "KeyZ",
            rotate_flip: "KeyA",
            hold: "ShiftLeft",
            soft_drop: "ArrowDown",
            hard_drop: "Space",
        }
    }
}

impl Tunable for Keybinds {
    const FIELDS: &'static [FieldDesc] = &[
        binding("move_left", "Move left"),
        binding("move_right", "Move right"),
        binding("rotate_cw", "Rotate clockwise"),
        binding("rotate_ccw", "Rotate counter-clockwise"),
        binding("rotate_flip", "Rotate 180"),
        binding("hold", "Hold"),
        binding("soft_drop", "Soft drop"),
        binding("hard_drop", "Hard drop"),
    ];

    fn clamp(&mut self) {
        // A key name is whatever the platform calls it. There is no range to enforce,
        // and rejecting an unfamiliar name would break on the next browser or OS that
        // spells one differently.
    }
}

/// A key binding descriptor. Bindings have no numeric range, so they are described as a
/// free enum with no variants: a UI renders them as a key-capture control.
const fn binding(key: &'static str, label: &'static str) -> FieldDesc {
    FieldDesc {
        key,
        label,
        help: "Press a key to rebind this action.",
        group: BINDS,
        kind: FieldKind::Enum {
            variants: &[],
            default: "",
        },
    }
}

const SKIN_VARIANTS: &[EnumVariant] = &[
    EnumVariant {
        value: "default",
        label: "Default",
        help: "The bundled theme.",
    },
    EnumVariant {
        value: "mono",
        label: "Monochrome",
        help: "One color for every piece.",
    },
    EnumVariant {
        value: "high_contrast",
        label: "High contrast",
        help: "Maximum separation between adjacent pieces.",
    },
];

/// Presentation only. Never leaves the render layer, never sent to a peer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields, default))]
pub struct Cosmetic {
    pub skin: &'static str,
    /// Ghost piece opacity, as a percentage. `0` hides it.
    pub ghost_opacity: u8,
    /// Board scale, as a percentage of the default size.
    pub board_scale: u8,
    pub show_grid: bool,
    pub music_volume: u8,
    pub effects_volume: u8,
}

impl Default for Cosmetic {
    fn default() -> Self {
        Cosmetic {
            skin: "default",
            ghost_opacity: 35,
            board_scale: 100,
            show_grid: true,
            music_volume: 50,
            effects_volume: 70,
        }
    }
}

impl Tunable for Cosmetic {
    const FIELDS: &'static [FieldDesc] = &[
        FieldDesc {
            key: "skin",
            label: "Skin",
            help: "Which bundled theme to draw with.",
            group: DISPLAY,
            kind: FieldKind::Enum {
                variants: SKIN_VARIANTS,
                default: "default",
            },
        },
        FieldDesc {
            key: "ghost_opacity",
            label: "Ghost opacity",
            help: "How visible the landing preview is. Zero hides it.",
            group: DISPLAY,
            kind: FieldKind::Int {
                min: 0,
                max: 100,
                default: 35,
                step: 5,
                unit: Unit::Count,
            },
        },
        FieldDesc {
            key: "board_scale",
            label: "Board scale",
            help: "Board size, as a percentage of the default.",
            group: DISPLAY,
            kind: FieldKind::Int {
                min: 50,
                max: 200,
                default: 100,
                step: 5,
                unit: Unit::Count,
            },
        },
        FieldDesc {
            key: "show_grid",
            label: "Show grid",
            help: "Draw column and row guides behind the stack.",
            group: DISPLAY,
            kind: FieldKind::Bool { default: true },
        },
        FieldDesc {
            key: "music_volume",
            label: "Music",
            help: "",
            group: AUDIO,
            kind: FieldKind::Int {
                min: 0,
                max: 100,
                default: 50,
                step: 5,
                unit: Unit::Count,
            },
        },
        FieldDesc {
            key: "effects_volume",
            label: "Effects",
            help: "",
            group: AUDIO,
            kind: FieldKind::Int {
                min: 0,
                max: 100,
                default: 70,
                step: 5,
                unit: Unit::Count,
            },
        },
    ];

    fn clamp(&mut self) {
        let f = |k| field(Self::FIELDS, k);
        self.ghost_opacity = f("ghost_opacity").clamp_u8(self.ghost_opacity);
        self.board_scale = f("board_scale").clamp_u8(self.board_scale);
        self.music_volume = f("music_volume").clamp_u8(self.music_volume);
        self.effects_volume = f("effects_volume").clamp_u8(self.effects_volume);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_maps_to_exactly_one_distinct_button() {
        let mut seen = Buttons::empty();
        for a in Action::ALL {
            let b = a.button();
            assert_eq!(b.bits().count_ones(), 1, "{a:?} must be one button");
            assert!(!seen.contains(b), "{a:?} duplicates another action");
            seen |= b;
        }
        assert_eq!(seen, Buttons::all(), "every button must be bindable");
    }

    #[test]
    fn action_keys_match_the_keybind_descriptor_keys() {
        // The settings UI joins these two by key. If they drift, an action silently
        // becomes unbindable.
        for a in Action::ALL {
            assert!(
                Keybinds::FIELDS.iter().any(|f| f.key == a.key()),
                "{a:?} has no keybind descriptor",
            );
        }
        assert_eq!(Keybinds::FIELDS.len(), Action::ALL.len());
    }

    #[cfg(all(feature = "serde", feature = "std"))]
    #[test]
    fn descriptors_cover_every_serde_field() {
        crate::config::assert_descriptors_match_serde_fields(&Keybinds::default());
        crate::config::assert_descriptors_match_serde_fields(&Cosmetic::default());
    }

    #[test]
    fn cosmetic_defaults_are_inside_their_declared_ranges() {
        for f in Cosmetic::FIELDS {
            assert!(f.kind.default_is_in_range(), "{}", f.key);
        }
    }

    #[test]
    fn cosmetic_values_are_clamped() {
        let mut c = Cosmetic {
            ghost_opacity: 255,
            board_scale: 1,
            music_volume: 255,
            effects_volume: 255,
            ..Default::default()
        };
        c.clamp();
        assert_eq!(c.ghost_opacity, 100);
        assert_eq!(c.board_scale, 50);
        assert_eq!(c.music_volume, 100);
        assert_eq!(c.effects_volume, 100);
    }

    #[test]
    fn default_keybinds_are_all_distinct() {
        let d = Keybinds::default();
        let keys = [
            d.move_left,
            d.move_right,
            d.rotate_cw,
            d.rotate_ccw,
            d.rotate_flip,
            d.hold,
            d.soft_drop,
            d.hard_drop,
        ];
        for (i, a) in keys.iter().enumerate() {
            for b in &keys[i + 1..] {
                assert_ne!(a, b, "two actions share a default key");
            }
        }
    }

    #[test]
    fn clamping_keybinds_leaves_them_alone() {
        // Key names come from the platform. Rejecting an unfamiliar one would break on
        // the next browser that spells a key differently.
        let mut k = Keybinds {
            move_left: "SomeKeyThisBuildHasNeverHeardOf",
            ..Default::default()
        };
        let before = k.clone();
        k.clamp();
        assert_eq!(k, before);
    }
}
