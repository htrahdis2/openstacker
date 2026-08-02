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

use engine::{Buttons, ENGINE_VER, Events};
use replay::Replay;
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
///
/// The first five cover movement, rotation, hold and the bag. The rest clear rows, which
/// is the half that used to be missing: without them the attack table could be rewritten
/// from top to bottom without a single checksum moving.
const GOLDEN: &[(&str, u64)] = &[
    ("bag_walk.replay", 0x7d10_a95d_6692_06ca),
    ("das.replay", 0x62e3_b9d1_c15c_1ea8),
    ("hold.replay", 0x62c5_a8c7_5ea7_782a),
    ("quad.replay", 0xfb51_3ebc_a0ce_19b1),
    ("rotation.replay", 0x8954_6b99_a713_c38f),
    ("single.replay", 0xa447_af0e_7407_f65f),
    ("double.replay", 0x4a08_b35a_73ef_e4ce),
    ("triple.replay", 0xc61b_809a_4da6_cc50),
    ("quad_clear.replay", 0x0a6c_1862_422f_cb45),
    ("combo_chain.replay", 0xb475_a949_37c1_914f),
    ("b2b_chain.replay", 0x29c9_dbd3_50b0_271b),
    ("perfect_clear.replay", 0x94dc_fb2d_d5f2_3959),
    ("spin_double.replay", 0xc8dc_c659_b30f_eb67),
    ("mini_spin.replay", 0x8314_5d04_bea6_6fe5),
    ("garbage_land.replay", 0x19ed_e7ff_5319_a3b9),
    ("garbage_cancel.replay", 0x0a6c_1862_422f_cb45),
];

fn load(name: &str) -> serde_json::Value {
    let path = replay_path(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
    serde_json::from_str(&text).expect("a golden replay should be valid JSON")
}

/// What the scoring goldens are for.
///
/// `verify` already re-checks every claimed number, which is what catches a change to the
/// attack table. This says out loud what each recording is meant to exercise, so a
/// reviewer can see the coverage without replaying anything, and so a change shows up as
/// a named expectation rather than a moved number.
const SCORING: &[Expect] = &[
    Expect {
        file: "single.replay",
        lines: 1,
        attack: 0,
        events: Events::LINES_CLEARED,
        absent: Events::SPIN,
    },
    Expect {
        file: "double.replay",
        lines: 2,
        attack: 1,
        events: Events::LINES_CLEARED,
        absent: Events::SPIN,
    },
    Expect {
        file: "triple.replay",
        lines: 3,
        attack: 2,
        events: Events::LINES_CLEARED,
        absent: Events::SPIN,
    },
    Expect {
        file: "quad_clear.replay",
        lines: 4,
        attack: 4,
        events: Events::LINES_CLEARED,
        absent: Events::B2B_BROKEN,
    },
    Expect {
        file: "combo_chain.replay",
        lines: 4,
        attack: 2,
        events: Events::LINES_CLEARED,
        absent: Events::SPIN,
    },
    Expect {
        file: "b2b_chain.replay",
        lines: 9,
        attack: 9,
        events: Events::B2B_CONTINUED.union(Events::B2B_BROKEN),
        absent: Events::PERFECT_CLEAR,
    },
    Expect {
        file: "perfect_clear.replay",
        lines: 4,
        attack: 12,
        events: Events::PERFECT_CLEAR,
        absent: Events::TOPPED_OUT,
    },
    Expect {
        file: "spin_double.replay",
        lines: 2,
        attack: 4,
        events: Events::SPIN,
        absent: Events::MINI_SPIN,
    },
    Expect {
        file: "mini_spin.replay",
        lines: 1,
        attack: 0,
        events: Events::MINI_SPIN,
        absent: Events::SPIN,
    },
    Expect {
        file: "garbage_land.replay",
        lines: 0,
        attack: 0,
        events: Events::GARBAGE_APPLIED,
        absent: Events::LINES_CLEARED,
    },
    Expect {
        file: "garbage_cancel.replay",
        lines: 4,
        attack: 0,
        // Nothing lands: the quad answered the rows before they were due.
        events: Events::LINES_CLEARED,
        absent: Events::GARBAGE_APPLIED,
    },
];

struct Expect {
    file: &'static str,
    lines: u32,
    attack: u32,
    /// Flags that must appear at some point during the recording.
    events: Events,
    /// Flags that must never appear, so a golden cannot quietly become a test of
    /// something else.
    absent: Events,
}

fn read_replay(name: &str) -> Replay {
    let text = std::fs::read_to_string(replay_path(name)).expect("golden should be readable");
    serde_json::from_str(&text).expect("golden should be a replay")
}

/// Every flag raised over the whole recording.
///
/// Garbage is scheduled the way `replay::run` does it, since a recording that receives
/// rows raises flags that a button-only replay never would.
fn events_of(r: &Replay) -> Events {
    let mut engine = engine::Engine::new(r.seed, &r.config, &r.handling);
    let mut all = Events::empty();
    for (i, b) in r.buttons().iter().enumerate() {
        let tick = i as u32 + 1;
        for g in r.garbage.iter().filter(|g| g.at_tick == tick) {
            engine.schedule_garbage(g.garbage);
        }
        all |= engine.tick(*b).events;
    }
    all
}

#[test]
fn the_scoring_goldens_exercise_what_they_claim_to() {
    // Before these existed, every golden was a handful of pieces that cleared nothing:
    // the attack table could be rewritten from top to bottom without moving a checksum.
    for e in SCORING {
        let r = read_replay(e.file);
        let (_, outcome) = r.simulate();
        assert_eq!(outcome.lines, e.lines, "{}: rows cleared", e.file);
        assert_eq!(outcome.attack, e.attack, "{}: rows sent", e.file);

        let raised = events_of(&r);
        assert!(
            raised.contains(e.events),
            "{}: expected {:?}, got {raised:?}",
            e.file,
            e.events
        );
        assert!(
            !raised.intersects(e.absent),
            "{}: {:?} should not happen here",
            e.file,
            e.absent
        );
    }
}

#[test]
fn a_spin_is_worth_more_than_the_plain_clear_of_the_same_size() {
    // The reason to learn spins, pinned against real recordings rather than against the
    // scoring function's own unit tests.
    let plain = read_replay("double.replay").simulate().1.attack;
    let spun = read_replay("spin_double.replay").simulate().1.attack;
    assert!(spun > plain, "spin double {spun} vs plain double {plain}");
}

#[test]
fn a_well_timed_clear_cancels_what_was_coming_instead_of_trading_blows() {
    // Two recordings of the same game, one of which has four rows on the way. The board
    // ends identical — the rows never land — and the attack that would have been sent is
    // spent answering them instead.
    let quiet = read_replay("quad_clear.replay").simulate();
    let under_fire = read_replay("garbage_cancel.replay").simulate();

    assert_eq!(quiet.1.attack, 4, "the same game with nothing incoming");
    assert_eq!(under_fire.1.attack, 0, "all four rows answered");
    assert_eq!(under_fire.0.stats().garbage_received, 0, "nothing landed");
    assert_eq!(
        quiet.1.checksum, under_fire.1.checksum,
        "cancelled rows leave no trace on the board"
    );
}

#[test]
fn rows_land_on_the_tick_they_were_scheduled_for() {
    let r = read_replay("garbage_land.replay");
    let g = r.garbage.first().expect("this golden schedules garbage");
    let mut engine = engine::Engine::new(r.seed, &r.config, &r.handling);
    let buttons = r.buttons();

    replay::run(
        &mut engine,
        &buttons[..g.garbage.apply_at_tick as usize - 1],
        &r.garbage,
    );
    assert_eq!(engine.stats().garbage_received, 0, "not landed yet");
    assert_eq!(engine.pending_garbage().total(), g.garbage.amount as u32);

    let (engine, _) = r.simulate();
    assert_eq!(engine.stats().garbage_received, g.garbage.amount as u32);
    assert!(engine.pending_garbage().is_empty());
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
