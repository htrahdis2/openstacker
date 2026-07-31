//! Deterministic falling-block simulation.
//!
//! This crate holds all of the game's rules and none of its I/O. No filesystem, no
//! clock, no async, no floating point. Those constraints are what make the simulation
//! reproducible bit-for-bit across native and wasm builds, and CI enforces them.
//!
//! # Determinism contract
//!
//! Given the same seed, config, and sequence of [`Buttons`], this crate produces the
//! same sequence of [`TickResult`]s and the same state checksum on every platform and
//! every build. Replays, server-side verification, and desync detection all fall out of
//! that single property.
//!
//! Banned here because they would break it:
//!
//! - `f32`/`f64`: rounding is not guaranteed identical across targets.
//! - `HashMap`: iteration order is not build-stable.
//! - `std::time`: the only clock is the tick counter.
//! - Per-tick allocation: bounded collections only.
//! - `usize` in any value that reaches the checksum or the wire. It is 64-bit on native
//!   and 32-bit on wasm32, so it must never decide a byte width or a serialized field.
//!   Using it for local indices is fine.
//!
//! # Layering
//!
//! ```text
//! engine  <-  config  <-  replay-cli
//! ```
//!
//! `engine` depends on no other workspace crate. Config types and their descriptors live
//! here; reading TOML and emitting JSON belongs to `config`, which is what keeps this
//! crate free of I/O.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

pub mod bag;
pub mod board;
pub mod config;
pub mod consts;
pub mod events;
pub mod fixed;
pub mod garbage;
pub mod input;
pub mod kick;
pub mod piece;
pub mod quad;
pub mod rng;
pub mod scoring;

pub use bag::Bag;
pub use board::Board;
pub use config::{
    Action, AttackTable, Cosmetic, FieldDesc, FieldKind, GravityCurve, Handling, HandlingSub,
    IrsMode, Keybinds, LockResetMode, MatchConfig, SpinRule, Tunable, Unit,
};
pub use consts::{
    BOARD_H, BOARD_W, ENGINE_VER, FULL_ROW, MAX_PREVIEW, SUBTICK, TICK_HZ, VISIBLE_H,
};
pub use events::{Events, Stats, TickResult};
pub use fixed::{ms_to_subticks, subticks_to_centiframes, subticks_to_ms};
pub use garbage::{GarbageQueue, PendingGarbage};
pub use input::{Buttons, Dir};
pub use kick::{Kicked, rotate};
pub use piece::Piece;
pub use quad::{QuadKind, Rot};
pub use rng::SplitMix64;
pub use scoring::Spin;
