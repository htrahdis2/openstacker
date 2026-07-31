//! Invariants that must hold after any sequence of inputs.
//!
//! The example-based tests check that specific things work. These check that nothing
//! breaks, against inputs nobody thought to write down. That matters more than usual
//! here: buttons arrive from a remote peer, so the engine has to survive streams no
//! keyboard could produce.

use engine::config::match_config::{GravityCurve, LockResetMode, SpinRule};
use engine::{
    BOARD_H, BOARD_W, Buttons, Engine, Events, Handling, MatchConfig, PendingGarbage, Phase,
    QuadKind,
};
use proptest::prelude::*;

/// Any button combination, including ones no physical controller can produce.
fn any_buttons() -> impl Strategy<Value = Buttons> {
    any::<u8>().prop_map(Buttons::from_bits_retain)
}

fn any_input_stream(len: usize) -> impl Strategy<Value = Vec<Buttons>> {
    prop::collection::vec(any_buttons(), 0..len)
}

/// Configs across the whole legal range, not just the defaults.
fn any_config() -> impl Strategy<Value = MatchConfig> {
    (
        0u32..2000,
        0u16..1000,
        0u8..20,
        0u16..200,
        0u16..200,
        0u8..=7,
        any::<bool>(),
        0u8..40,
    )
        .prop_map(
            |(gravity, lock_delay, cap, clear_delay, spawn_delay, preview, hold, garbage_cap)| {
                MatchConfig {
                    gravity: GravityCurve::Fixed {
                        ms_per_row: gravity,
                    },
                    lock_delay_ms: lock_delay,
                    lock_reset_cap: cap,
                    clear_delay_ms: clear_delay,
                    spawn_delay_ms: spawn_delay,
                    preview_len: preview,
                    hold_enabled: hold,
                    garbage_cap: garbage_cap.max(1),
                    ..Default::default()
                }
            },
        )
}

fn any_handling() -> impl Strategy<Value = Handling> {
    (0u16..600, 0u16..300, 0u16..600, 0u16..300, any::<bool>()).prop_map(
        |(das, arr, sdf, dcd, sdl)| Handling {
            das_ms: das,
            arr_ms: arr,
            sdf_ms_per_row: sdf,
            dcd_ms: dcd,
            soft_drop_lock: sdl,
            ..Default::default()
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// The piece must never sit on top of the stack. If it does, locking would overwrite
    /// occupied cells and the board would quietly become wrong.
    #[test]
    fn the_active_piece_never_overlaps_the_stack(
        seed in any::<u64>(),
        config in any_config(),
        handling in any_handling(),
        inputs in any_input_stream(400),
    ) {
        let mut e = Engine::new(seed, &config, &handling);
        for (i, b) in inputs.iter().enumerate() {
            e.tick(*b);
            if e.is_over() {
                break;
            }
            // No piece exists during a spawn or clear delay.
            let Some(piece) = e.active() else { continue };
            for (x, y) in piece.cells() {
                prop_assert!(
                    (0..BOARD_W as i32).contains(&x),
                    "piece left the board at tick {i}"
                );
                prop_assert!(y < BOARD_H as i32, "piece fell through the floor at tick {i}");
                if y >= 0 {
                    prop_assert!(
                        !e.board().occupied(x as usize, y as usize),
                        "piece overlapped the stack at tick {i}"
                    );
                }
            }
        }
    }

    /// A completed row must never be left sitting on the board once play resumes.
    #[test]
    fn no_completed_row_survives(
        seed in any::<u64>(),
        inputs in any_input_stream(400),
    ) {
        let config = MatchConfig::default();
        let mut e = Engine::new(seed, &config, &Handling::default());
        for (i, b) in inputs.iter().enumerate() {
            e.tick(*b);
            if e.phase() == Phase::ClearDelay || e.is_over() {
                continue;
            }
            for y in 0..BOARD_H {
                prop_assert!(!e.board().is_row_full(y), "row {y} survived at tick {i}");
            }
        }
    }

    /// Cell count only ever moves in legal steps: four for a lock, whole rows for a
    /// clear, and a row minus its hole for garbage. Anything else means cells are being
    /// invented or lost.
    #[test]
    fn cell_count_only_changes_in_legal_amounts(
        seed in any::<u64>(),
        inputs in any_input_stream(300),
    ) {
        let mut e = Engine::new(seed, &MatchConfig::default(), &Handling::default());
        let mut before = e.board().cell_count();
        for (i, b) in inputs.iter().enumerate() {
            let r = e.tick(*b);
            let after = e.board().cell_count();
            let delta = after as i64 - before as i64;

            // On a tick where nothing cleared and no garbage arrived, the only thing
            // that can add cells is a piece locking, and a piece is exactly four cells.
            // Any other change means cells were invented or lost.
            if r.lines_cleared == 0 && !r.contains(Events::GARBAGE_APPLIED) {
                prop_assert!(
                    delta == 0 || delta == 4,
                    "cell count moved by {delta} at tick {i} with no clear and no garbage"
                );
            }
            prop_assert!(
                after <= (BOARD_W * BOARD_H) as u32,
                "more cells than the board holds at tick {i}"
            );
            before = after;
        }
    }

    /// Ticking is deterministic for any config, handling and input stream.
    #[test]
    fn identical_runs_stay_identical(
        seed in any::<u64>(),
        config in any_config(),
        handling in any_handling(),
        inputs in any_input_stream(300),
    ) {
        let mut a = Engine::new(seed, &config, &handling);
        let mut b = Engine::new(seed, &config, &handling);
        for (i, input) in inputs.iter().enumerate() {
            prop_assert_eq!(a.tick(*input), b.tick(*input), "diverged at tick {}", i);
            prop_assert_eq!(a.checksum(), b.checksum(), "checksum diverged at tick {}", i);
        }
    }

    /// The engine must never panic, whatever it is fed.
    #[test]
    fn ticking_never_panics(
        seed in any::<u64>(),
        config in any_config(),
        handling in any_handling(),
        inputs in any_input_stream(500),
    ) {
        let mut e = Engine::new(seed, &config, &handling);
        for b in inputs {
            e.tick(b);
        }
    }

    /// Garbage from a peer can name any row count and any column. None of it may crash
    /// the engine or corrupt the board.
    #[test]
    fn arbitrary_garbage_is_survivable(
        seed in any::<u64>(),
        batches in prop::collection::vec((any::<u32>(), any::<u8>(), any::<u8>()), 0..24),
        inputs in any_input_stream(200),
    ) {
        let mut e = Engine::new(seed, &MatchConfig::default(), &Handling::default());
        for (at, amount, hole) in batches {
            e.schedule_garbage(PendingGarbage {
                apply_at_tick: at,
                amount,
                hole_col: hole,
            });
        }
        for b in inputs {
            e.tick(b);
            prop_assert!(e.board().cell_count() <= (BOARD_W * BOARD_H) as u32);
        }
    }

    /// A finished game stays finished and stops changing.
    #[test]
    fn a_finished_game_is_frozen(
        seed in any::<u64>(),
        inputs in any_input_stream(3000),
    ) {
        let mut e = Engine::new(seed, &MatchConfig::default(), &Handling::default());
        for b in &inputs {
            e.tick(*b);
            if e.is_over() {
                break;
            }
        }
        if !e.is_over() {
            return Ok(());
        }
        let frozen = e.checksum();
        for b in &inputs {
            prop_assert!(e.tick(*b).is_empty());
        }
        prop_assert_eq!(e.checksum(), frozen);
    }

    /// Every seven pieces drawn contain each kind exactly once, whatever else happens.
    #[test]
    fn the_piece_sequence_stays_a_permutation(seed in any::<u64>()) {
        let mut bag = engine::Bag::new(seed);
        for _ in 0..300 {
            let mut seen = [0u8; QuadKind::COUNT];
            for _ in 0..QuadKind::COUNT {
                seen[bag.take().index()] += 1;
            }
            prop_assert!(seen.iter().all(|&c| c == 1));
        }
    }

    /// A lock delay reset cap actually bounds how long a piece can be kept alive.
    #[test]
    fn a_reset_cap_eventually_forces_a_lock(
        seed in any::<u64>(),
        cap in 0u8..8,
    ) {
        let config = MatchConfig {
            gravity: GravityCurve::Fixed { ms_per_row: 1_000_000 },
            lock_delay_ms: 100,
            lock_reset_mode: LockResetMode::Extended,
            lock_reset_cap: cap,
            spin_detection: SpinRule::ThreeCorner,
            ..Default::default()
        };
        let handling = Handling { das_ms: 0, arr_ms: 0, sdf_ms_per_row: 0, ..Default::default() };
        let mut e = Engine::new(seed, &config, &handling);

        // Drive the piece to the floor, then wiggle it forever.
        for _ in 0..4 {
            e.tick(Buttons::SOFT_DROP);
        }
        for _ in 0..600 {
            e.tick(Buttons::LEFT);
            e.tick(Buttons::RIGHT);
        }
        prop_assert!(e.stats().pieces >= 2, "the piece was never forced to lock");
    }

    /// Clamping is idempotent for any handling a client might send.
    #[test]
    fn clamping_handling_twice_changes_nothing(handling in any_handling()) {
        use engine::config::desc::Tunable;
        let mut once = handling;
        once.clamp();
        let mut twice = once;
        twice.clamp();
        prop_assert_eq!(once, twice);
    }
}
