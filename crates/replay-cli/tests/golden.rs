//! Golden replays.
//!
//! Each of these is a hand-written script compiled into a replay and pinned to the
//! checksum it produced. Together they are the guard that lets a stranger's change to
//! the rules be merged with confidence: if any of these move, the rules moved, and every
//! recorded game stops being verifiable.
//!
//! A failure here is not necessarily a bug. It means the simulation changed, and the
//! question to answer is whether that change was intended and whether the engine version
//! needs to go up with it.

use engine::{Buttons, ENGINE_VER};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}

fn replay_path(name: &str) -> PathBuf {
    repo_root().join("testdata/replays").join(name)
}

/// Every golden replay and the checksum it must produce.
const GOLDEN: &[(&str, u64)] = &[
    ("bag_walk.replay", 0x7d10_a95d_6692_06ca),
    ("das.replay", 0x62e3_b9d1_c15c_1ea8),
    ("hold.replay", 0x62c5_a8c7_5ea7_782a),
    ("quad.replay", 0xfb51_3ebc_a0ce_19b1),
    ("rotation.replay", 0x8954_6b99_a713_c38f),
];

fn load(name: &str) -> serde_json::Value {
    let path = replay_path(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
    serde_json::from_str(&text).expect("a golden replay should be valid JSON")
}

#[test]
fn every_golden_replay_exists_and_parses() {
    for (name, _) in GOLDEN {
        let v = load(name);
        assert!(v["seed"].is_number(), "{name} has no seed");
        assert!(v["inputs"].is_array(), "{name} has no inputs");
    }
}

#[test]
fn every_golden_replay_was_recorded_under_the_current_rules() {
    // If this fails, the engine version moved without the goldens being regenerated,
    // and they are no longer testing what they claim to test.
    for (name, _) in GOLDEN {
        let v = load(name);
        assert_eq!(
            v["engine_ver"].as_u64().unwrap() as u32,
            ENGINE_VER,
            "{name} was recorded under different rules; recompile it"
        );
    }
}

#[test]
fn every_golden_replay_reproduces_its_claimed_result() {
    // The core property. A replay that does not reproduce its own claim is either from
    // a different build or was edited.
    for (name, _) in GOLDEN {
        let path = replay_path(name);
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_replay"))
            .arg("verify")
            .arg(&path)
            .output()
            .expect("the replay binary should run");
        assert!(
            out.status.success(),
            "{name} failed to verify:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn replaying_the_same_file_twice_gives_the_same_checksum() {
    for (name, _) in GOLDEN {
        let path = replay_path(name);
        let run = || {
            let out = std::process::Command::new(env!("CARGO_BIN_EXE_replay"))
                .arg("checksum")
                .arg(&path)
                .output()
                .expect("the replay binary should run");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        assert_eq!(run(), run(), "{name} is not reproducible");
    }
}

#[test]
fn a_replay_with_a_tampered_result_is_rejected() {
    // Verification has to actually fail on a bad claim, or it is just an expensive way
    // of printing "verified".
    let mut v = load("bag_walk.replay");
    v["claimed"]["lines"] = serde_json::json!(999);

    let tampered = std::env::temp_dir().join("tampered.replay");
    std::fs::write(&tampered, serde_json::to_string(&v).unwrap()).unwrap();

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_replay"))
        .arg("verify")
        .arg(&tampered)
        .output()
        .expect("the replay binary should run");
    let _ = std::fs::remove_file(&tampered);

    assert!(!out.status.success(), "a tampered replay verified anyway");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("does NOT match"), "{stderr}");
    assert!(stderr.contains("lines: claimed 999"), "{stderr}");
}

#[test]
fn a_replay_from_older_rules_is_reported_as_unverifiable_rather_than_failing() {
    // Old recordings stay watchable. They just cannot have their result re-checked under
    // rules they were not played under.
    let mut v = load("bag_walk.replay");
    v["engine_ver"] = serde_json::json!(ENGINE_VER + 1);

    let old = std::env::temp_dir().join("old_rules.replay");
    std::fs::write(&old, serde_json::to_string(&v).unwrap()).unwrap();

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_replay"))
        .arg("verify")
        .arg(&old)
        .output()
        .expect("the replay binary should run");
    let _ = std::fs::remove_file(&old);

    assert!(out.status.success(), "an old replay should not be an error");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("cannot be re-checked"), "{stdout}");
}

#[test]
fn the_renderer_draws_a_board() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_replay"))
        .arg("run")
        .arg(replay_path("quad.replay"))
        .output()
        .expect("the replay binary should run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains('#'), "the stack should be drawn:\n{stdout}");
    assert!(stdout.contains('@'), "the active piece should be drawn");
    assert!(stdout.contains("checksum"), "a checksum should be reported");
}

#[test]
fn the_modes_command_accepts_the_shipped_modes() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_replay"))
        .arg("modes")
        .arg("--modes-dir")
        .arg(repo_root().join("modes"))
        .output()
        .expect("the replay binary should run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    for id in ["sprint40", "blitz", "versus"] {
        assert!(stdout.contains(id), "{id} missing from:\n{stdout}");
    }
}

#[test]
fn a_malformed_mode_directory_is_reported_rather_than_ignored() {
    let dir = std::env::temp_dir().join("bad_modes_test");
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(
        dir.join("broken.toml"),
        "spec_version = 1\nid = \"broken\"\nname = \"B\"\n[goal]\ntype = \"survival\"\n\
         [config]\nlock_delay_tiks = 30\n",
    )
    .unwrap();

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_replay"))
        .arg("modes")
        .arg("--modes-dir")
        .arg(&dir)
        .output()
        .expect("the replay binary should run");
    let _ = std::fs::remove_dir_all(&dir);

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("lock_delay_tiks"), "{stderr}");
    assert!(stderr.contains("did you mean"), "{stderr}");
}

#[test]
fn a_script_compiles_to_a_replay_that_verifies() {
    // The full path a contributor uses: write a script, compile it, check it holds.
    let script = std::env::temp_dir().join("roundtrip.script");
    std::fs::write(&script, "seed: 99\nLEFT*4\nCW\n.\nHARD_DROP\n.*3\n").unwrap();
    let out_file = std::env::temp_dir().join("roundtrip.replay");

    let compile = std::process::Command::new(env!("CARGO_BIN_EXE_replay"))
        .args(["compile"])
        .arg(&script)
        .arg("-o")
        .arg(&out_file)
        .output()
        .expect("the replay binary should run");
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let verify = std::process::Command::new(env!("CARGO_BIN_EXE_replay"))
        .arg("verify")
        .arg(&out_file)
        .output()
        .expect("the replay binary should run");
    assert!(
        verify.status.success(),
        "{}",
        String::from_utf8_lossy(&verify.stderr)
    );

    let _ = std::fs::remove_file(&script);
    let _ = std::fs::remove_file(&out_file);
}

#[test]
fn a_script_with_a_typo_is_rejected_with_a_suggestion() {
    let script = std::env::temp_dir().join("typo.script");
    std::fs::write(&script, "seed: 1\nHARDDROP\n").unwrap();
    let out_file = std::env::temp_dir().join("typo.replay");

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_replay"))
        .arg("compile")
        .arg(&script)
        .arg("-o")
        .arg(&out_file)
        .output()
        .expect("the replay binary should run");
    let _ = std::fs::remove_file(&script);

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("line 2"), "{stderr}");
    assert!(stderr.contains("HARD_DROP"), "{stderr}");
}

#[test]
fn golden_checksums_are_pinned() {
    // The regression guard proper. If one of these moves, the rules moved.
    for (name, expected) in GOLDEN {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_replay"))
            .arg("checksum")
            .arg(replay_path(name))
            .output()
            .expect("the replay binary should run");
        let text = String::from_utf8_lossy(&out.stdout);
        let got = u64::from_str_radix(text.trim().trim_start_matches("0x"), 16)
            .unwrap_or_else(|_| panic!("bad checksum output for {name}: {text}"));
        assert_eq!(got, *expected, "{name} checksum moved: the rules changed");
    }
}

#[test]
fn buttons_cover_every_bit_a_script_can_express() {
    // A sanity check that the script vocabulary and the engine's input type have not
    // drifted apart, which would leave part of the input surface untested.
    assert_eq!(Buttons::all().bits().count_ones(), 8);
}
