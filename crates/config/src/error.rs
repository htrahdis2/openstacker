//! Errors from reading config files.
//!
//! Mode files are meant to be written by people who do not write Rust. That makes the
//! quality of these messages a feature rather than polish: an unknown key that is
//! silently ignored costs someone an evening of wondering why their mode does nothing.

use std::path::PathBuf;
use thiserror::Error;

/// The mode file format this build understands.
pub const SUPPORTED_SPEC_VERSION: u16 = 1;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{path} is not valid TOML: {source}")]
    Syntax {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("{path}: {source}")]
    Shape {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("{path}: unknown setting `{key}` in [{section}]{}", suggestion.as_ref().map(|s| format!(", did you mean `{s}`?")).unwrap_or_default())]
    UnknownKey {
        path: PathBuf,
        section: String,
        key: String,
        suggestion: Option<String>,
    },

    #[error(
        "{path}: missing `spec_version`. Add `spec_version = {SUPPORTED_SPEC_VERSION}` at the top of the file"
    )]
    MissingSpecVersion { path: PathBuf },

    #[error(
        "{path}: needs mode format v{found}, but this build understands v{SUPPORTED_SPEC_VERSION}. Update the game, or edit the file to match the older format"
    )]
    UnsupportedSpecVersion { path: PathBuf, found: u16 },

    #[error("{path}: `{key}` is {value}, but must be between {min} and {max}")]
    OutOfRange {
        path: PathBuf,
        key: String,
        value: i64,
        min: i64,
        max: i64,
    },

    #[error("{path}: `{key}` is `{value}`, but must be one of: {allowed}")]
    UnknownVariant {
        path: PathBuf,
        key: String,
        value: String,
        allowed: String,
    },

    #[error("{path}: id is `{id}` but the file is named `{stem}.toml`. They have to match")]
    IdMismatch {
        path: PathBuf,
        id: String,
        stem: String,
    },

    #[error("{path}: `{key}` is required")]
    MissingKey { path: PathBuf, key: String },

    #[error("two modes share the id `{id}`: {first} and {second}")]
    DuplicateId {
        id: String,
        first: PathBuf,
        second: PathBuf,
    },
}

impl ConfigError {
    /// The file the problem is in, for callers that group errors by file.
    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            ConfigError::Io { path, .. }
            | ConfigError::Syntax { path, .. }
            | ConfigError::Shape { path, .. }
            | ConfigError::UnknownKey { path, .. }
            | ConfigError::MissingSpecVersion { path }
            | ConfigError::UnsupportedSpecVersion { path, .. }
            | ConfigError::OutOfRange { path, .. }
            | ConfigError::UnknownVariant { path, .. }
            | ConfigError::IdMismatch { path, .. }
            | ConfigError::MissingKey { path, .. } => Some(path),
            ConfigError::DuplicateId { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_key_error_offers_the_suggestion_when_there_is_one() {
        let e = ConfigError::UnknownKey {
            path: PathBuf::from("modes/sprint40.toml"),
            section: "config".into(),
            key: "das_ticks".into(),
            suggestion: Some("das_ms".into()),
        };
        let msg = e.to_string();
        assert!(msg.contains("das_ticks"), "{msg}");
        assert!(msg.contains("did you mean `das_ms`?"), "{msg}");
    }

    #[test]
    fn an_unknown_key_error_reads_cleanly_with_no_suggestion() {
        let e = ConfigError::UnknownKey {
            path: PathBuf::from("modes/x.toml"),
            section: "config".into(),
            key: "wat".into(),
            suggestion: None,
        };
        let msg = e.to_string();
        assert!(msg.ends_with("in [config]"), "{msg}");
    }

    #[test]
    fn a_version_mismatch_says_which_way_to_go() {
        let e = ConfigError::UnsupportedSpecVersion {
            path: PathBuf::from("modes/x.toml"),
            found: 9,
        };
        let msg = e.to_string();
        assert!(msg.contains("needs mode format v9"), "{msg}");
        assert!(msg.contains("Update the game"), "{msg}");
    }

    #[test]
    fn every_file_scoped_error_reports_its_path() {
        let p = PathBuf::from("modes/x.toml");
        let cases = [
            ConfigError::MissingSpecVersion { path: p.clone() },
            ConfigError::UnsupportedSpecVersion {
                path: p.clone(),
                found: 2,
            },
            ConfigError::OutOfRange {
                path: p.clone(),
                key: "k".into(),
                value: 1,
                min: 2,
                max: 3,
            },
            ConfigError::MissingKey {
                path: p.clone(),
                key: "id".into(),
            },
        ];
        for c in cases {
            assert_eq!(c.path(), Some(&p), "{c}");
        }
    }
}
