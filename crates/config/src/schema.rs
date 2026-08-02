//! Emits the settings schema as JSON.
//!
//! This file is what lets a settings screen be generated instead of hand-written. The
//! client reads it and renders a control per setting, using the bounds, defaults, units
//! and help text that the engine already declares. Adding a setting becomes one entry in
//! a Rust table, with no matching change in the client.
//!
//! CI diffs the emitted output against the committed copy, so the schema cannot silently
//! fall behind the engine.

use engine::config::desc::{FieldDesc, FieldKind, Tunable};
use engine::{AttackTable, Cosmetic, Handling, Keybinds, MatchConfig};
use serde_json::{Map, Value, json};

/// Every settings group the client knows how to render.
pub fn schema() -> Value {
    json!({
        "version": 1,
        "groups": [
            group::<Handling>("handling", "Handling", true),
            group::<MatchConfig>("match", "Match rules", true),
            group::<AttackTable>("attack_table", "Attack table", true),
            group::<Keybinds>("keybinds", "Controls", false),
            group::<Cosmetic>("cosmetic", "Appearance", false),
        ],
    })
}

/// Serialize the schema as pretty JSON with a trailing newline.
pub fn schema_json() -> String {
    let mut s = serde_json::to_string_pretty(&schema()).expect("schema should serialize");
    s.push('\n');
    s
}

/// Describe one config struct.
///
/// `affects_simulation` matters to the client: those settings are frozen once a match
/// starts and have to be sent to peers, while the rest can be changed at any time and
/// never leave the machine.
fn group<T: Tunable>(id: &str, label: &str, affects_simulation: bool) -> Value {
    json!({
        "id": id,
        "label": label,
        "affectsSimulation": affects_simulation,
        "fields": T::FIELDS.iter().map(describe).collect::<Vec<_>>(),
        "nested": T::NESTED,
    })
}

fn describe(f: &FieldDesc) -> Value {
    let mut m = Map::new();
    m.insert("key".into(), json!(f.key));
    m.insert("label".into(), json!(f.label));
    m.insert("help".into(), json!(f.help));
    m.insert("group".into(), json!(f.group));

    match f.kind {
        FieldKind::Int {
            min,
            max,
            default,
            step,
            unit,
        } => {
            m.insert("type".into(), json!("int"));
            m.insert("min".into(), json!(min));
            m.insert("max".into(), json!(max));
            m.insert("default".into(), json!(default));
            m.insert("step".into(), json!(step));
            m.insert("unit".into(), json!(unit.suffix()));
        }
        FieldKind::Bool { default } => {
            m.insert("type".into(), json!("bool"));
            m.insert("default".into(), json!(default));
        }
        FieldKind::IntList {
            min,
            max,
            max_len,
            default,
            unit,
        } => {
            m.insert("type".into(), json!("intList"));
            m.insert("min".into(), json!(min));
            m.insert("max".into(), json!(max));
            m.insert("maxLen".into(), json!(max_len));
            m.insert("default".into(), json!(default));
            m.insert("unit".into(), json!(unit.suffix()));
        }
        FieldKind::Enum { variants, default } => {
            // A binding has no fixed set of values, since a key name is whatever the
            // platform calls it. The client renders those as a key-capture control.
            m.insert(
                "type".into(),
                json!(if variants.is_empty() {
                    "binding"
                } else {
                    "enum"
                }),
            );
            m.insert("default".into(), json!(default));
            m.insert(
                "variants".into(),
                json!(
                    variants
                        .iter()
                        .map(|v| json!({ "value": v.value, "label": v.label, "help": v.help }))
                        .collect::<Vec<_>>()
                ),
            );
        }
    }
    Value::Object(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn groups() -> Vec<Value> {
        schema()["groups"].as_array().unwrap().clone()
    }

    fn fields_of(id: &str) -> Vec<Value> {
        groups()
            .into_iter()
            .find(|g| g["id"] == id)
            .unwrap_or_else(|| panic!("no group {id}"))["fields"]
            .as_array()
            .unwrap()
            .clone()
    }

    #[test]
    fn every_config_group_is_present() {
        let gs = groups();
        let ids: Vec<&str> = gs.iter().map(|g| g["id"].as_str().unwrap()).collect();
        assert_eq!(
            ids,
            ["handling", "match", "attack_table", "keybinds", "cosmetic"]
        );
    }

    #[test]
    fn the_schema_describes_every_declared_field() {
        // The client renders one control per entry here, so a field missing from the
        // schema is a setting the player can never change.
        assert_eq!(fields_of("handling").len(), Handling::FIELDS.len());
        assert_eq!(fields_of("match").len(), MatchConfig::FIELDS.len());
        assert_eq!(fields_of("attack_table").len(), AttackTable::FIELDS.len());
        assert_eq!(fields_of("keybinds").len(), Keybinds::FIELDS.len());
        assert_eq!(fields_of("cosmetic").len(), Cosmetic::FIELDS.len());
    }

    #[test]
    fn integer_fields_carry_everything_a_slider_needs() {
        let das = fields_of("handling")
            .into_iter()
            .find(|f| f["key"] == "das_ms")
            .unwrap();
        assert_eq!(das["type"], "int");
        assert_eq!(das["min"], 0);
        assert_eq!(das["max"], 500);
        assert_eq!(das["default"], 133);
        assert_eq!(das["unit"], "ms");
        assert!(!das["help"].as_str().unwrap().is_empty(), "needs help text");
    }

    #[test]
    fn enum_fields_carry_their_variants() {
        let irs = fields_of("handling")
            .into_iter()
            .find(|f| f["key"] == "irs")
            .unwrap();
        assert_eq!(irs["type"], "enum");
        let values: Vec<&str> = irs["variants"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["value"].as_str().unwrap())
            .collect();
        assert_eq!(values, ["off", "hold", "tap"]);
    }

    #[test]
    fn keybinds_are_marked_as_bindings_rather_than_enums() {
        // A key name is whatever the platform calls it, so there is no list to pick
        // from and the client has to render a key-capture control instead of a dropdown.
        for f in fields_of("keybinds") {
            assert_eq!(f["type"], "binding", "{}", f["key"]);
        }
    }

    #[test]
    fn groups_declare_whether_they_reach_the_simulation() {
        // The client needs this to know which settings freeze at match start and get
        // sent to peers, and which stay local and can change at any time.
        let by_id = |id: &str| {
            groups().into_iter().find(|g| g["id"] == id).unwrap()["affectsSimulation"]
                .as_bool()
                .unwrap()
        };
        assert!(by_id("handling"));
        assert!(by_id("match"));
        assert!(by_id("attack_table"));
        assert!(!by_id("keybinds"));
        assert!(!by_id("cosmetic"));
    }

    #[test]
    fn nested_settings_are_named_so_the_client_can_skip_them() {
        let m = groups().into_iter().find(|g| g["id"] == "match").unwrap();
        let nested: Vec<&str> = m["nested"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        // The combo table used to be here. It is a described field now, so a client can
        // render it rather than skip it.
        assert_eq!(nested, ["gravity", "attack_table"]);
    }

    #[test]
    fn a_table_of_numbers_carries_its_bounds_and_its_length_cap() {
        let combo = fields_of("match")
            .into_iter()
            .find(|f| f["key"] == "combo_table")
            .expect("the combo table should be a described setting");
        assert_eq!(combo["type"], "intList");
        assert_eq!(combo["min"], 0);
        assert!(combo["max"].as_i64().unwrap() > 0);
        assert!(combo["maxLen"].as_u64().unwrap() >= 13);
        assert_eq!(combo["default"][2], 1);
        assert_eq!(combo["unit"], "rows");
    }

    #[test]
    fn every_field_has_a_label_and_a_ui_group() {
        for g in groups() {
            for f in g["fields"].as_array().unwrap() {
                assert!(!f["label"].as_str().unwrap().is_empty(), "{}", f["key"]);
                assert!(!f["group"].as_str().unwrap().is_empty(), "{}", f["key"]);
            }
        }
    }

    #[test]
    fn emission_is_stable_across_runs() {
        // CI diffs this against a committed file, so unstable ordering would turn every
        // unrelated change into a spurious schema diff.
        assert_eq!(schema_json(), schema_json());
        assert!(schema_json().ends_with('\n'));
    }
}
