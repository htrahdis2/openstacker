//! Game modes, read from TOML.
//!
//! A mode is a file, not code. Adding one needs no recompile, which is what makes a mode
//! something a player can post in a chat rather than a pull request.
//!
//! Files are validated against the descriptor tables before being deserialized, so that
//! problems are reported in terms of the setting the author actually wrote rather than
//! as a serde type error. Out-of-range values here are an authoring mistake and are
//! rejected; player settings, which arrive from an untrusted client, are clamped
//! instead.

use crate::error::{ConfigError, SUPPORTED_SPEC_VERSION};
use engine::config::desc::{FieldKind, Tunable, nearest_key};
use engine::{AttackTable, MatchConfig};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What a player is trying to achieve. Deliberately not part of [`MatchConfig`]: the
/// engine never sees a goal, so re-simulating a replay needs no mode metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Goal {
    /// Clear a number of rows as fast as possible.
    Lines { count: u32 },
    /// Score as much as possible before time runs out.
    Time { ms: u32 },
    /// Reach a score.
    Score { target: u64 },
    /// Last as long as possible. Used by versus modes.
    Survival,
}

/// A mode: an identity, a goal, and the rules to play it under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModeSpec {
    pub spec_version: u16,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub goal: Goal,
    #[serde(default)]
    pub config: MatchConfig,
}

/// Keys allowed at the top level of a mode file.
const TOP_LEVEL_KEYS: &[&str] = &[
    "spec_version",
    "id",
    "name",
    "description",
    "goal",
    "config",
];

/// Parse and validate a mode from TOML text.
///
/// `path` is used only for error messages.
pub fn parse_mode(text: &str, path: &Path) -> Result<ModeSpec, ConfigError> {
    let value: toml::Value = toml::from_str(text).map_err(|source| ConfigError::Syntax {
        path: path.to_path_buf(),
        source,
    })?;

    let table = value.as_table().ok_or_else(|| ConfigError::MissingKey {
        path: path.to_path_buf(),
        key: "id".into(),
    })?;

    // Version is checked before anything else. A file from a newer build will have keys
    // this one has never heard of, and "unknown setting `x`" is a much worse message
    // than "this needs a newer version".
    match table.get("spec_version").and_then(|v| v.as_integer()) {
        None => {
            return Err(ConfigError::MissingSpecVersion {
                path: path.to_path_buf(),
            });
        }
        Some(v) if v != SUPPORTED_SPEC_VERSION as i64 => {
            return Err(ConfigError::UnsupportedSpecVersion {
                path: path.to_path_buf(),
                found: v.clamp(0, u16::MAX as i64) as u16,
            });
        }
        Some(_) => {}
    }

    for key in table.keys() {
        if !TOP_LEVEL_KEYS.contains(&key.as_str()) {
            return Err(ConfigError::UnknownKey {
                path: path.to_path_buf(),
                section: "".into(),
                key: key.clone(),
                suggestion: closest(TOP_LEVEL_KEYS, key),
            });
        }
    }

    for required in ["id", "name", "goal"] {
        if !table.contains_key(required) {
            return Err(ConfigError::MissingKey {
                path: path.to_path_buf(),
                key: required.into(),
            });
        }
    }

    if let Some(config) = table.get("config").and_then(|v| v.as_table()) {
        validate_table::<MatchConfig>(path, "config", config)?;

        if let Some(at) = config.get("attack_table").and_then(|v| v.as_table()) {
            validate_table::<AttackTable>(path, "config.attack_table", at)?;
        }
        if let Some(g) = config.get("gravity").and_then(|v| v.as_table()) {
            validate_gravity(path, g)?;
        }
    }

    let spec: ModeSpec = toml::from_str(text).map_err(|source| ConfigError::Shape {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(spec)
}

/// Read one mode file, additionally requiring that its id matches its filename.
pub fn load_mode_file(path: &Path) -> Result<ModeSpec, ConfigError> {
    let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let spec = parse_mode(&text, path)?;

    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if spec.id != stem {
        return Err(ConfigError::IdMismatch {
            path: path.to_path_buf(),
            id: spec.id,
            stem: stem.to_string(),
        });
    }
    Ok(spec)
}

/// Read every `*.toml` in a directory as a mode.
///
/// Returns every error rather than only the first, so that fixing a batch of files does
/// not turn into one round trip per mistake. Modes are sorted by id, so the order a
/// filesystem happens to return entries in cannot change what a menu looks like.
pub fn load_modes(dir: &Path) -> Result<Vec<ModeSpec>, Vec<ConfigError>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(source) => {
            return Err(vec![ConfigError::Io {
                path: dir.to_path_buf(),
                source,
            }]);
        }
    };

    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    paths.sort();

    let mut modes: Vec<ModeSpec> = Vec::new();
    let mut errors = Vec::new();

    for path in paths {
        match load_mode_file(&path) {
            Ok(spec) => {
                if let Some(prev) = modes.iter().position(|m| m.id == spec.id) {
                    errors.push(ConfigError::DuplicateId {
                        id: spec.id.clone(),
                        first: PathBuf::from(format!("{}.toml", modes[prev].id)),
                        second: path.clone(),
                    });
                }
                modes.push(spec);
            }
            Err(e) => errors.push(e),
        }
    }

    if errors.is_empty() {
        modes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(modes)
    } else {
        Err(errors)
    }
}

/// Every mode in a directory, as the JSON the client bundles.
///
/// Emitted from the same files the server reads, so a mode is defined once and the browser
/// needs no TOML parser. Modes are ordered by id, so the output does not depend on what
/// order a filesystem hands back entries.
pub fn modes_json(dir: &Path) -> Result<String, Vec<ConfigError>> {
    let modes = load_modes(dir)?;
    let doc = serde_json::json!({ "version": SUPPORTED_SPEC_VERSION, "modes": modes });
    let mut text = serde_json::to_string_pretty(&doc).expect("modes should serialize");
    text.push('\n');
    Ok(text)
}

/// Check every key in a table against a descriptor set: known key, right type, in range.
fn validate_table<T: Tunable>(
    path: &Path,
    section: &str,
    table: &toml::Table,
) -> Result<(), ConfigError> {
    for (key, value) in table {
        if T::NESTED.contains(&key.as_str()) {
            continue;
        }
        let Some(desc) = T::FIELDS.iter().find(|f| f.key == key) else {
            return Err(ConfigError::UnknownKey {
                path: path.to_path_buf(),
                section: section.to_string(),
                key: key.clone(),
                suggestion: nearest_key(T::FIELDS, key).map(str::to_string),
            });
        };

        match desc.kind {
            FieldKind::Int { min, max, .. } => {
                let Some(n) = value.as_integer() else {
                    return Err(ConfigError::OutOfRange {
                        path: path.to_path_buf(),
                        key: key.clone(),
                        value: 0,
                        min,
                        max,
                    });
                };
                if n < min || n > max {
                    return Err(ConfigError::OutOfRange {
                        path: path.to_path_buf(),
                        key: key.clone(),
                        value: n,
                        min,
                        max,
                    });
                }
            }
            FieldKind::Enum { variants, .. } if !variants.is_empty() => {
                let got = value.as_str().unwrap_or_default();
                if !variants.iter().any(|v| v.value == got) {
                    return Err(ConfigError::UnknownVariant {
                        path: path.to_path_buf(),
                        key: key.clone(),
                        value: got.to_string(),
                        allowed: variants
                            .iter()
                            .map(|v| v.value)
                            .collect::<Vec<_>>()
                            .join(", "),
                    });
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Gravity is a tagged enum rather than a flat table, so its allowed keys depend on the
/// variant and it cannot go through [`validate_table`].
fn validate_gravity(path: &Path, table: &toml::Table) -> Result<(), ConfigError> {
    const KINDS: &[&str] = &["fixed", "staged"];
    let kind = table.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if !KINDS.contains(&kind) {
        return Err(ConfigError::UnknownVariant {
            path: path.to_path_buf(),
            key: "gravity.type".into(),
            value: kind.to_string(),
            allowed: KINDS.join(", "),
        });
    }
    let allowed: &[&str] = match kind {
        "fixed" => &["type", "ms_per_row"],
        _ => &["type", "stages"],
    };
    for key in table.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(ConfigError::UnknownKey {
                path: path.to_path_buf(),
                section: "config.gravity".into(),
                key: key.clone(),
                suggestion: closest(allowed, key),
            });
        }
    }
    Ok(())
}

/// Nearest match among plain string keys, for sections that have no descriptor table.
fn closest(keys: &[&'static str], key: &str) -> Option<String> {
    keys.iter()
        .map(|k| (levenshtein(k, key), *k))
        .min_by_key(|(d, _)| *d)
        .filter(|(d, _)| *d <= 3)
        .map(|(_, k)| k.to_string())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let sub = prev[j - 1] + usize::from(a[i - 1] != b[j - 1]);
            cur[j] = sub.min(prev[j] + 1).min(cur[j - 1] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}
