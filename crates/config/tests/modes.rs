//! Mode loading, and the error messages it produces.
//!
//! The messages are tested as carefully as the parsing. Mode files are meant to be
//! written by people who do not write Rust, so a bad message is a real defect: it is the
//! difference between fixing a typo in ten seconds and giving up on contributing a mode.

#![cfg(feature = "files")]

use config::{ConfigError, Goal, load_modes, parse_mode};
use engine::{GravityCurve, LockResetMode, SpinRule};
use std::path::{Path, PathBuf};

fn p() -> PathBuf {
    PathBuf::from("modes/test.toml")
}

fn parse(text: &str) -> Result<config::ModeSpec, ConfigError> {
    parse_mode(text, &p())
}

const MINIMAL: &str = r#"
spec_version = 1
id = "test"
name = "Test"
[goal]
type = "survival"
"#;

// ---- the shipped modes -----------------------------------------------------

#[test]
fn every_shipped_mode_loads() {
    let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../modes"));
    let modes = match load_modes(dir) {
        Ok(m) => m,
        Err(errors) => {
            let joined: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            panic!("shipped modes failed to load:\n{}", joined.join("\n"));
        }
    };
    let ids: Vec<&str> = modes.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["blitz", "sprint40", "versus"], "sorted by id");
}

#[test]
fn shipped_modes_carry_the_goals_they_advertise() {
    let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../modes"));
    let modes = load_modes(dir).expect("shipped modes should load");
    let by_id = |id: &str| modes.iter().find(|m| m.id == id).unwrap().clone();

    assert_eq!(by_id("sprint40").goal, Goal::Lines { count: 40 });
    assert_eq!(by_id("blitz").goal, Goal::Time { ms: 120_000 });
    assert_eq!(by_id("versus").goal, Goal::Survival);
}

#[test]
fn blitz_gravity_actually_ramps() {
    let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../modes"));
    let modes = load_modes(dir).expect("shipped modes should load");
    let blitz = modes.iter().find(|m| m.id == "blitz").unwrap();

    let g = &blitz.config.gravity;
    assert_eq!(g.ms_per_row_at(0), 1000);
    assert_eq!(g.ms_per_row_at(1800), 500);
    assert_eq!(g.ms_per_row_at(5400), 100);
    assert_eq!(g.ms_per_row_at(u32::MAX), 100, "last stage holds");
}

// ---- defaults --------------------------------------------------------------

#[test]
fn a_minimal_mode_file_gets_every_default() {
    // Three lines plus a goal has to be enough. If a minimal file does not work, nobody
    // writes a mode without copying a long one first.
    let m = parse(MINIMAL).expect("minimal file should parse");
    let d = engine::MatchConfig::default();
    assert_eq!(m.config, d);
    assert_eq!(m.description, "");
}

#[test]
fn omitted_settings_keep_their_defaults_while_stated_ones_win() {
    let m = parse(
        r#"
spec_version = 1
id = "test"
name = "Test"
[goal]
type = "survival"
[config]
lock_delay_ms = 250
"#,
    )
    .unwrap();
    assert_eq!(m.config.lock_delay_ms, 250);
    assert_eq!(m.config.preview_len, 5, "untouched settings keep defaults");
    assert_eq!(m.config.lock_reset_mode, LockResetMode::Extended);
}

// ---- version handling ------------------------------------------------------

#[test]
fn a_file_with_no_version_says_exactly_what_to_add() {
    let err = parse(
        r#"
id = "test"
name = "Test"
[goal]
type = "survival"
"#,
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("spec_version = 1"), "{msg}");
}

#[test]
fn a_newer_file_is_reported_as_a_version_problem_not_a_key_problem() {
    // A file from a newer build will contain settings this one has never heard of.
    // Reporting the first unknown key would send the author chasing a typo that is not
    // there, so the version check has to come first.
    let err = parse(
        r#"
spec_version = 99
id = "test"
name = "Test"
[goal]
type = "survival"
[config]
some_setting_from_the_future = 3
"#,
    )
    .unwrap_err();
    assert!(
        matches!(err, ConfigError::UnsupportedSpecVersion { found: 99, .. }),
        "got: {err}"
    );
}

// ---- unknown keys ----------------------------------------------------------

#[test]
fn a_wrong_unit_suggests_the_right_setting() {
    // The mistake a player actually makes: thinking in frames when the setting is in
    // milliseconds.
    let err = parse(
        r#"
spec_version = 1
id = "test"
name = "Test"
[goal]
type = "survival"
[config]
lock_delay_ticks = 30
"#,
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("lock_delay_ticks"), "{msg}");
    assert!(msg.contains("did you mean `lock_delay_ms`?"), "{msg}");
}

#[test]
fn a_typo_in_a_nested_table_names_that_table() {
    let err = parse(
        r#"
spec_version = 1
id = "test"
name = "Test"
[goal]
type = "survival"
[config.attack_table]
qaud = 4
"#,
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("[config.attack_table]"), "{msg}");
    assert!(msg.contains("did you mean `quad`?"), "{msg}");
}

#[test]
fn an_unknown_top_level_key_is_caught() {
    let err = parse(
        r#"
spec_version = 1
id = "test"
name = "Test"
nmae = "typo"
[goal]
type = "survival"
"#,
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("nmae"), "{msg}");
    assert!(msg.contains("did you mean `name`?"), "{msg}");
}

// ---- values ----------------------------------------------------------------

#[test]
fn an_out_of_range_value_is_rejected_with_the_range_spelled_out() {
    // A file is authored, so this is a mistake worth reporting. Player settings, which
    // arrive from an untrusted client, are clamped instead.
    let err = parse(
        r#"
spec_version = 1
id = "test"
name = "Test"
[goal]
type = "survival"
[config]
preview_len = 99
"#,
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("preview_len"), "{msg}");
    assert!(msg.contains("99"), "{msg}");
    assert!(msg.contains("between 0 and 7"), "{msg}");
}

#[test]
fn an_unknown_enum_value_lists_the_ones_that_work() {
    let err = parse(
        r#"
spec_version = 1
id = "test"
name = "Test"
[goal]
type = "survival"
[config]
spin_detection = "tspin"
"#,
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("three_corner"), "{msg}");
    assert!(msg.contains("all_spin"), "{msg}");
}

#[test]
fn an_unknown_gravity_type_is_caught() {
    let err = parse(
        r#"
spec_version = 1
id = "test"
name = "Test"
[goal]
type = "survival"
[config.gravity]
type = "exponential"
"#,
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("gravity.type"), "{msg}");
    assert!(msg.contains("fixed, staged"), "{msg}");
}

#[test]
fn a_key_from_the_wrong_gravity_variant_is_caught() {
    let err = parse(
        r#"
spec_version = 1
id = "test"
name = "Test"
[goal]
type = "survival"
[config.gravity]
type = "fixed"
stages = []
"#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("stages"), "{err}");
}

#[test]
fn instant_gravity_is_accepted() {
    // Zero is meaningful, not a missing value: it means fall to the floor immediately.
    let m = parse(
        r#"
spec_version = 1
id = "test"
name = "Test"
[goal]
type = "survival"
[config.gravity]
type = "fixed"
ms_per_row = 0
"#,
    )
    .unwrap();
    assert_eq!(m.config.gravity, GravityCurve::Fixed { ms_per_row: 0 });
    assert_eq!(m.config.gravity.threshold_at(0), None);
}

// ---- structure -------------------------------------------------------------

#[test]
fn a_missing_required_field_names_it() {
    let err = parse(
        r#"
spec_version = 1
name = "Test"
[goal]
type = "survival"
"#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("`id` is required"), "{err}");
}

#[test]
fn broken_toml_is_reported_as_a_syntax_error() {
    let err = parse("spec_version = = 1").unwrap_err();
    assert!(matches!(err, ConfigError::Syntax { .. }), "got: {err}");
}

#[test]
fn every_error_message_names_the_file_it_is_about() {
    // With several modes on disk, an error that does not say which file it came from
    // makes the author check all of them.
    let bad = [
        "id = \"test\"\nname = \"T\"\n[goal]\ntype = \"survival\"\n",
        "spec_version = 1\nname = \"T\"\n[goal]\ntype = \"survival\"\n",
        "spec_version = 1\nid = \"test\"\nname = \"T\"\n[goal]\ntype = \"survival\"\n[config]\npreview_len = 99\n",
    ];
    for text in bad {
        let err = parse(text).unwrap_err();
        assert!(
            err.to_string().contains("modes/test.toml"),
            "message did not name the file: {err}"
        );
        assert_eq!(err.path(), Some(&p()));
    }
}

#[test]
fn spin_rule_and_lock_reset_round_trip_through_toml() {
    let m = parse(
        r#"
spec_version = 1
id = "test"
name = "Test"
[goal]
type = "survival"
[config]
spin_detection = "all_spin"
lock_reset_mode = "infinite"
"#,
    )
    .unwrap();
    assert_eq!(m.config.spin_detection, SpinRule::AllSpin);
    assert_eq!(m.config.lock_reset_mode, LockResetMode::Infinite);
}

#[test]
fn mode_files_stay_strict_even_though_the_config_struct_is_permissive() {
    // MatchConfig ignores unknown keys, because the same struct travels inside replays
    // and settings files where tolerating an unfamiliar key is what keeps an older build
    // usable. Mode files must not inherit that leniency: they are authored by hand, so a
    // typo is a mistake worth reporting rather than a key to skip.
    //
    // The strictness lives in the loader, which validates against the descriptor tables
    // before deserializing. If that check is ever removed, this catches it.
    let err = parse(
        r#"
spec_version = 1
id = "test"
name = "Test"
[goal]
type = "survival"
[config]
lock_delay_tiks = 30
"#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("lock_delay_tiks"), "{err}");
    assert!(err.to_string().contains("did you mean"), "{err}");

    // Serde alone would have accepted it, which is precisely why the loader checks.
    let permissive: Result<engine::MatchConfig, _> = toml::from_str("lock_delay_tiks = 30\n");
    assert!(
        permissive.is_ok(),
        "the struct is meant to be permissive; strictness belongs to the loader"
    );
}

#[test]
fn the_generated_json_carries_every_shipped_mode() {
    // The client bundles this instead of parsing TOML, so a mode missing here is a mode
    // nobody can play.
    let text = config::modes_json(Path::new("../../modes")).expect("shipped modes should load");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let modes = v["modes"].as_array().unwrap();

    let ids: Vec<&str> = modes.iter().map(|m| m["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["blitz", "sprint40", "versus"], "sorted by id");

    let sprint = modes.iter().find(|m| m["id"] == "sprint40").unwrap();
    assert_eq!(sprint["goal"]["type"], "lines");
    assert_eq!(sprint["goal"]["count"], 40);
    assert_eq!(sprint["config"]["preview_len"], 5);
    assert!(!sprint["name"].as_str().unwrap().is_empty());
}

#[test]
fn the_generated_json_is_stable_across_runs() {
    // CI diffs it against a committed file, so unstable ordering would turn every
    // unrelated change into a spurious diff.
    let a = config::modes_json(Path::new("../../modes")).unwrap();
    let b = config::modes_json(Path::new("../../modes")).unwrap();
    assert_eq!(a, b);
    assert!(a.ends_with('\n'));
}

#[test]
fn a_broken_mode_directory_produces_errors_rather_than_a_file() {
    let err = config::modes_json(Path::new("../../modes/does-not-exist")).unwrap_err();
    assert!(!err.is_empty());
}
