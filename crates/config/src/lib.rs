//! Reads game modes and player settings from disk, and emits the settings schema.
//!
//! This crate exists so the engine can do no I/O. Config *types* and their descriptor
//! tables live in the engine, because they are part of the rules; parsing files, layering
//! overrides, and writing JSON live here.

pub mod error;
pub mod mode;

pub use error::{ConfigError, SUPPORTED_SPEC_VERSION};
pub use mode::{Goal, ModeSpec, load_mode_file, load_modes, parse_mode};
