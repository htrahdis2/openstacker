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

pub mod desc;

pub use desc::{EnumVariant, FieldDesc, FieldKind, Tunable, Unit, field, nearest_key};
