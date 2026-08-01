//! Reads game modes and player settings, and emits the settings schema.
//!
//! This crate exists so the engine can do no I/O. Config *types* and their descriptor
//! tables live in the engine, because they are part of the rules; parsing files, layering
//! overrides, and writing JSON live here.
//!
//! Mode files are behind the `files` feature. Settings, resolution and schema emission are
//! always available and read nothing from disk, so a client can share this crate's
//! validation rules without also carrying a TOML parser.

pub mod resolve;
pub mod schema;
pub mod settings;

#[cfg(feature = "files")]
pub mod error;
#[cfg(feature = "files")]
pub mod mode;

pub use resolve::{HostPolicy, Layer, Resolved, resolve};
pub use schema::{schema, schema_json};
pub use settings::{Note, SETTINGS_VERSION, Settings};

#[cfg(feature = "files")]
pub use error::{ConfigError, SUPPORTED_SPEC_VERSION};
#[cfg(feature = "files")]
pub use mode::{Goal, ModeSpec, load_mode_file, load_modes, modes_json, parse_mode};
