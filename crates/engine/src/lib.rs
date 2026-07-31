//! Deterministic falling-block simulation.
//!
//! This crate is the whole of the game's rules and none of its I/O. It has no
//! filesystem access, no clock, no async, and no floating point. Those are hard
//! constraints, enforced in CI by a grep guard, because they are what make the
//! simulation reproducible bit-for-bit across native and wasm builds.
//!
//! # Determinism contract
//!
//! Given the same `(seed, MatchConfig, Handling)` and the same sequence of [`Buttons`],
//! this crate produces the same sequence of [`TickResult`]s and the same
//! [`checksum`](Engine::checksum) on every platform and every build. That property is
//! what buys free replays, free server verification, and free desync detection.
//!
//! Things that would break it, and are therefore banned here:
//!
//! - floating point (`f32`/`f64`) — rounding is not guaranteed identical across targets
//! - `HashMap` — iteration order is not build-stable
//! - `std::time` — the simulation's only clock is its own tick counter
//! - per-tick allocation — bounded collections only
//!
//! # Layering
//!
//! ```text
//! engine  ←  config  ←  replay-cli
//! ```
//!
//! `engine` depends on no other workspace crate. Config *types* and their descriptors
//! live here; reading TOML and emitting JSON is `config`'s job, so that this crate can
//! keep the no-I/O invariant.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

pub mod consts;
pub mod events;
pub mod fixed;
pub mod input;
pub mod quad;

pub use consts::{
    BOARD_H, BOARD_W, ENGINE_VER, FULL_ROW, MAX_PREVIEW, SUBTICK, TICK_HZ, VISIBLE_H,
};
pub use events::{Events, Stats, TickResult};
pub use fixed::{ms_to_subticks, subticks_to_centiframes, subticks_to_ms};
pub use input::{Buttons, Dir};
pub use quad::{QuadKind, Rot};
