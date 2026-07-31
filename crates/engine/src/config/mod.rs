//! Configuration types and their self-descriptions.
//!
//! Config splits four ways, by who owns it and whether it reaches the simulation:
//!
//! | Struct         | Owner            | Reaches the sim |
//! |----------------|------------------|-----------------|
//! | `MatchConfig`  | the match         | yes             |
//! | `Handling`     | the player        | yes             |
//! | `Keybinds`     | the player        | no              |
//! | `Cosmetic`     | the player        | no              |
//!
//! Only the first two are read by the engine. Keybinds resolve to [`Buttons`] before any
//! engine call, and cosmetics stop at the render layer. Keeping that line sharp is what
//! lets two peers with wildly different key layouts and skins stay bit-identical.
//!
//! These are types and descriptors only. Reading TOML, layering overrides and emitting
//! JSON all live outside this crate, so the engine keeps doing no I/O.
//!
//! [`Buttons`]: crate::Buttons

pub mod client;
pub mod desc;
pub mod handling;
pub mod match_config;

pub use client::{Action, Cosmetic, Keybinds};
pub use desc::{EnumVariant, FieldDesc, FieldKind, Tunable, Unit, field, nearest_key};
pub use handling::{Handling, HandlingSub, IrsMode};
pub use match_config::{
    AttackTable, GravityCurve, GravityStage, LockResetMode, MatchConfig, SpinRule,
};

/// Assert that a config struct's descriptor table and its serde fields describe exactly
/// the same set of settings.
///
/// This is the guard that makes the descriptor table trustworthy. Without it the table
/// is just a parallel copy of the struct, and parallel copies drift: a field gets added
/// and silently has no bounds, no help text, and no control in the settings UI. The
/// check runs against real serde output, so it cannot be fooled by a stale comment.
///
/// Nested settings such as sub-tables and enums have no single bound, so they are listed
/// in [`Tunable::NESTED`] instead of `FIELDS` and are accounted for here.
#[cfg(all(test, feature = "serde", feature = "std"))]
pub(crate) fn assert_descriptors_match_serde_fields<T>(value: &T)
where
    T: serde::Serialize + Tunable,
{
    let json = serde_json::to_value(value).expect("config should serialize");
    let object = json
        .as_object()
        .expect("a config struct should serialize to an object");

    let described: Vec<&str> = T::FIELDS
        .iter()
        .map(|f| f.key)
        .chain(T::NESTED.iter().copied())
        .collect();

    for key in object.keys() {
        assert!(
            described.contains(&key.as_str()),
            "serde field `{key}` has no descriptor, so it would have no bounds and no \
             control in a generated settings UI",
        );
    }
    for key in &described {
        assert!(
            object.contains_key(*key),
            "descriptor `{key}` names no serde field; it was probably renamed",
        );
    }
}
