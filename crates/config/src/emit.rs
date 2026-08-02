//! Writing a set of rules back out as the TOML a mode file is written in.
//!
//! Tuning that cannot leave the browser is tuning that gets lost. This is the other half
//! of the loop: play with the numbers, then take the block this produces and paste it
//! into a mode file, where it becomes something that can be committed, reviewed and
//! posted to somebody else.
//!
//! Kept out of the `files` half of the crate — it writes text and reads nothing — so the
//! browser can produce a mode file without carrying a TOML parser.

use engine::MatchConfig;
use serde_json::Value;

/// A `[config]` block for a mode file, with the values a mode already sets left out.
///
/// `against` is the rules being played, usually the mode's own. Only what was actually
/// changed is printed, because a block that repeats every default is one nobody can read
/// a decision out of.
pub fn config_toml(config: &MatchConfig, against: Option<&MatchConfig>) -> String {
    let mine = to_value(config);
    let theirs = against.map(to_value);
    let same = |key: &str, value: &Value| {
        theirs
            .as_ref()
            .and_then(|t| t.get(key))
            .is_some_and(|v| v == value)
    };

    let mut scalars = String::new();
    let mut tables = String::new();

    for (key, value) in mine.as_object().expect("a config is an object") {
        if same(key, value) {
            continue;
        }
        match value {
            // A sub-table is diffed entry by entry too. One changed attack value should
            // print as one line, not as the whole table restated.
            Value::Object(_) => {
                let against = theirs.as_ref().and_then(|t| t.get(key));
                let body = table(value, against);
                if !body.is_empty() {
                    tables.push_str(&format!("\n[config.{key}]\n{body}"));
                }
            }
            Value::Array(_) => scalars.push_str(&format!("{key} = {}\n", array(value))),
            _ => scalars.push_str(&format!("{key} = {}\n", scalar(value))),
        }
    }

    if scalars.is_empty() && tables.is_empty() {
        return String::new();
    }
    let mut out = String::from("[config]\n");
    out.push_str(&scalars);
    out.push_str(&tables);
    out
}

fn to_value(config: &MatchConfig) -> Value {
    serde_json::to_value(config).expect("a config should serialize")
}

fn table(value: &Value, against: Option<&Value>) -> String {
    let mut out = String::new();
    for (key, v) in value.as_object().expect("a table") {
        if against.and_then(|a| a.get(key)).is_some_and(|a| a == v) {
            continue;
        }
        out.push_str(&format!("{key} = {}\n", value_of(v)));
    }
    out
}

fn array(value: &Value) -> String {
    let items: Vec<String> = value
        .as_array()
        .expect("an array")
        .iter()
        .map(value_of)
        .collect();
    format!("[{}]", items.join(", "))
}

/// A value in TOML. Tables inside an array are written inline, which is the only form
/// TOML has for them.
fn value_of(value: &Value) -> String {
    match value {
        Value::String(s) => format!("\"{s}\""),
        Value::Array(_) => array(value),
        Value::Object(fields) => {
            let items: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("{k} = {}", value_of(v)))
                .collect();
            format!("{{ {} }}", items.join(", "))
        }
        other => other.to_string(),
    }
}

fn scalar(value: &Value) -> String {
    value_of(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::AttackTable;

    #[test]
    fn nothing_changed_produces_nothing_to_paste() {
        let c = MatchConfig::default();
        assert!(config_toml(&c, Some(&c)).is_empty());
    }

    #[test]
    fn only_what_moved_is_printed() {
        let base = MatchConfig::default();
        let mut tuned = base.clone();
        tuned.garbage_delay_ms = 700;
        let out = config_toml(&tuned, Some(&base));
        assert!(out.starts_with("[config]\n"), "{out}");
        assert!(out.contains("garbage_delay_ms = 700"), "{out}");
        assert!(!out.contains("lock_delay_ms"), "{out}");
    }

    #[test]
    fn a_changed_attack_table_comes_out_as_its_own_block() {
        let base = MatchConfig::default();
        let tuned = MatchConfig {
            attack_table: AttackTable {
                quad: 6,
                ..base.attack_table
            },
            ..base.clone()
        };
        let out = config_toml(&tuned, Some(&base));
        assert!(out.contains("[config.attack_table]"), "{out}");
        assert!(out.contains("quad = 6"), "{out}");
        // One value moved, so one line: a block that restates the whole table is a block
        // nobody can read a decision out of.
        assert!(!out.contains("triple"), "{out}");
    }

    #[test]
    fn a_staged_gravity_curve_comes_out_as_something_toml_can_read() {
        let base = MatchConfig::default();
        let tuned = MatchConfig {
            gravity: engine::GravityCurve::Staged {
                stages: [
                    engine::config::GravityStage {
                        from_tick: 0,
                        ms_per_row: 1000,
                    },
                    engine::config::GravityStage {
                        from_tick: 1800,
                        ms_per_row: 500,
                    },
                ]
                .into_iter()
                .collect(),
            },
            ..base.clone()
        };
        let out = config_toml(&tuned, Some(&base));
        assert!(
            out.contains("{ from_tick = 0, ms_per_row = 1000 }"),
            "{out}"
        );
    }

    #[test]
    fn a_table_of_numbers_comes_out_as_a_list() {
        let base = MatchConfig::default();
        let mut tuned = base.clone();
        tuned.b2b_table.clear();
        for n in [0u8, 2, 4] {
            tuned.b2b_table.push(n);
        }
        let out = config_toml(&tuned, Some(&base));
        assert!(out.contains("b2b_table = [0, 2, 4]"), "{out}");
    }

    #[test]
    fn strings_are_quoted_so_the_block_parses() {
        let base = MatchConfig::default();
        let tuned = MatchConfig {
            spin_detection: engine::SpinRule::AllSpin,
            ..base.clone()
        };
        let out = config_toml(&tuned, Some(&base));
        assert!(out.contains("spin_detection = \"all_spin\""), "{out}");
    }

    #[cfg(feature = "files")]
    #[test]
    fn what_it_prints_is_a_mode_file_that_loads() {
        // The whole point: the block goes into a file and that file is playable.
        let base = MatchConfig::default();
        let tuned = MatchConfig {
            garbage_delay_ms: 700,
            attack_table: AttackTable {
                quad: 6,
                ..base.attack_table
            },
            ..base.clone()
        };
        let text = format!(
            "spec_version = 1\nid = \"tuned\"\nname = \"Tuned\"\n[goal]\ntype = \"survival\"\n{}",
            config_toml(&tuned, Some(&base))
        );
        let spec = crate::parse_mode(&text, std::path::Path::new("modes/tuned.toml"))
            .unwrap_or_else(|e| panic!("{e}\n{text}"));
        assert_eq!(spec.config.garbage_delay_ms, 700);
        assert_eq!(spec.config.attack_table.quad, 6);
        assert_eq!(spec.config.lock_delay_ms, base.lock_delay_ms);
    }

    #[cfg(feature = "files")]
    #[test]
    fn a_whole_config_round_trips_through_a_mode_file() {
        let mut tuned = MatchConfig {
            garbage_delay_ms: 450,
            garbage_cap: 12,
            lock_delay_ms: 300,
            spin_detection: engine::SpinRule::AllSpin,
            ..Default::default()
        };
        tuned.attack_table.spin_quad = 12;
        let text = format!(
            "spec_version = 1\nid = \"tuned\"\nname = \"Tuned\"\n[goal]\ntype = \"survival\"\n{}",
            config_toml(&tuned, None)
        );
        let spec = crate::parse_mode(&text, std::path::Path::new("modes/tuned.toml"))
            .unwrap_or_else(|e| panic!("{e}\n{text}"));
        assert_eq!(spec.config, tuned);
    }
}
