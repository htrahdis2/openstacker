//! End-to-end behaviour of the simulation.
//!
//! These drive the engine the way a player or a replay does: buttons in, one tick at a
//! time. Nothing reaches inside, so anything asserted here is something a client or a
//! server could also rely on.

use engine::config::desc::Tunable;
use engine::config::handling::IrsMode;
use engine::config::match_config::{GravityCurve, LockResetMode};
use engine::{
    BOARD_H, BOARD_W, Buttons, Engine, Events, Handling, MatchConfig, PendingGarbage, Phase,
    QuadKind, VISIBLE_H,
};

/// Gravity slow enough that it never interferes with what a test is measuring.
fn calm() -> MatchConfig {
    MatchConfig {
        gravity: GravityCurve::Fixed {
            ms_per_row: 1_000_000,
        },
        lock_delay_ms: 500,
        ..Default::default()
    }
}

/// Handling with no delays, so one held tick is one movement.
fn snappy() -> Handling {
    Handling {
        das_ms: 0,
        arr_ms: 0,
        sdf_ms_per_row: 0,
        irs: IrsMode::Off,
        ihs: false,
        ..Default::default()
    }
}

fn engine() -> Engine {
    Engine::new(1, &calm(), &snappy())
}

fn run(e: &mut Engine, buttons: Buttons, ticks: u32) {
    for _ in 0..ticks {
        e.tick(buttons);
    }
}

fn idle(e: &mut Engine, ticks: u32) {
    run(e, Buttons::empty(), ticks);
}

/// Press a button for one tick, then release, so an edge-triggered action fires once.
fn tap(e: &mut Engine, b: Buttons) -> engine::TickResult {
    let r = e.tick(b);
    e.tick(Buttons::empty());
    r
}

// ---- starting state --------------------------------------------------------

#[test]
fn a_new_engine_starts_with_a_piece_and_a_preview() {
    let e = engine();
    assert_eq!(e.stats().tick, 0);
    assert!(!e.is_over());
    assert_eq!(e.preview().len(), e.config().preview_len as usize);
    assert!(e.hold().is_none());
    assert!(e.board().is_empty(), "the board starts clear");
}

#[test]
fn the_same_seed_and_inputs_produce_the_same_game() {
    // The property everything else rests on. Replays, verification and desync detection
    // are all just this, observed from different places.
    let (mut a, mut b) = (engine(), engine());
    let script = [
        Buttons::LEFT,
        Buttons::LEFT,
        Buttons::CW,
        Buttons::empty(),
        Buttons::HARD_DROP,
        Buttons::empty(),
        Buttons::RIGHT,
        Buttons::SOFT_DROP,
    ];
    for i in 0..2000 {
        let input = script[i % script.len()];
        assert_eq!(a.tick(input), b.tick(input), "diverged at tick {i}");
        assert_eq!(a.checksum(), b.checksum(), "checksum diverged at tick {i}");
    }
}

#[test]
fn different_seeds_produce_different_games() {
    let mut a = Engine::new(1, &calm(), &snappy());
    let mut b = Engine::new(2, &calm(), &snappy());
    for _ in 0..200 {
        a.tick(Buttons::HARD_DROP);
        a.tick(Buttons::empty());
        b.tick(Buttons::HARD_DROP);
        b.tick(Buttons::empty());
    }
    assert_ne!(a.checksum(), b.checksum());
}

// ---- movement --------------------------------------------------------------

#[test]
fn a_held_direction_carries_the_piece_to_the_wall() {
    let mut e = engine();
    run(&mut e, Buttons::LEFT, 30);
    let cells = e.active().unwrap().cells();
    assert_eq!(
        cells.iter().map(|c| c.0).min().unwrap(),
        0,
        "should reach the wall"
    );
}

#[test]
fn a_piece_never_leaves_the_board_however_long_a_direction_is_held() {
    let mut e = engine();
    for dir in [Buttons::LEFT, Buttons::RIGHT] {
        run(&mut e, dir, 200);
        for (x, _) in e.active().unwrap().cells() {
            assert!((0..BOARD_W as i32).contains(&x), "escaped at x={x}");
        }
    }
}

#[test]
fn das_delays_the_repeat_but_not_the_first_step() {
    // The first tap must be immediate or the controls feel broken; everything after it
    // waits out the delay.
    let handling = Handling {
        das_ms: 200,
        arr_ms: 0,
        ..snappy()
    };
    let mut e = Engine::new(1, &calm(), &handling);
    let start = e.active().unwrap().x;

    e.tick(Buttons::LEFT);
    assert_eq!(
        e.active().unwrap().x,
        start - 1,
        "the first step is immediate"
    );

    // Still within the delay: no further movement.
    run(&mut e, Buttons::LEFT, 5);
    assert_eq!(
        e.active().unwrap().x,
        start - 1,
        "repeat started before DAS elapsed"
    );

    // Past the delay: the piece runs to the wall.
    run(&mut e, Buttons::LEFT, 20);
    assert!(e.active().unwrap().x < start - 1, "repeat never started");
}

#[test]
fn releasing_a_direction_recharges_the_delay() {
    let handling = Handling {
        das_ms: 200,
        arr_ms: 0,
        ..snappy()
    };
    let mut e = Engine::new(1, &calm(), &handling);
    run(&mut e, Buttons::LEFT, 30);
    let at_wall = e.active().unwrap().x;

    idle(&mut e, 1);
    e.tick(Buttons::RIGHT);
    assert_eq!(
        e.active().unwrap().x,
        at_wall + 1,
        "one step, then the delay again"
    );
    run(&mut e, Buttons::RIGHT, 5);
    assert_eq!(
        e.active().unwrap().x,
        at_wall + 1,
        "repeat resumed without recharging"
    );
}

// ---- rotation --------------------------------------------------------------

#[test]
fn rotation_is_edge_triggered() {
    // Holding the button must not spin the piece every frame.
    let mut e = engine();
    let start = e.active().unwrap().rot;
    run(&mut e, Buttons::CW, 10);
    assert_eq!(
        e.active().unwrap().rot,
        start.cw(),
        "held rotation fired more than once"
    );
}

#[test]
fn each_rotation_button_turns_the_expected_way() {
    let mut e = engine();
    let start = e.active().unwrap().rot;
    tap(&mut e, Buttons::CW);
    assert_eq!(e.active().unwrap().rot, start.cw());
    tap(&mut e, Buttons::CCW);
    assert_eq!(e.active().unwrap().rot, start);
    tap(&mut e, Buttons::FLIP);
    assert_eq!(e.active().unwrap().rot, start.flip());
}

// ---- dropping and locking --------------------------------------------------

#[test]
fn hard_drop_lands_the_piece_and_locks_it() {
    let mut e = engine();
    let r = e.tick(Buttons::HARD_DROP);
    assert!(r.contains(Events::HARD_DROPPED));
    assert!(r.contains(Events::PIECE_LOCKED));
    assert_eq!(e.stats().pieces, 2, "the next piece should already be out");
    assert!(!e.board().is_empty(), "cells should have been written");
}

#[test]
fn hard_drop_is_edge_triggered() {
    let mut e = engine();
    run(&mut e, Buttons::HARD_DROP, 5);
    assert_eq!(
        e.stats().pieces,
        2,
        "held hard drop locked more than one piece"
    );
}

#[test]
fn a_resting_piece_locks_once_the_delay_runs_out() {
    let mut e = engine();
    run(&mut e, Buttons::SOFT_DROP, 2);
    assert_eq!(e.phase(), Phase::Locking);
    idle(&mut e, 40);
    assert!(e.stats().pieces >= 2, "the piece never locked");
}

#[test]
fn moving_while_resting_postpones_the_lock_but_only_so_many_times() {
    // Without a cap on resets a player could hold a piece in place forever, which is
    // both a stalling tactic and, in a networked game, a way to freeze an opponent.
    let config = MatchConfig {
        lock_reset_mode: LockResetMode::Extended,
        lock_reset_cap: 3,
        ..calm()
    };
    let mut e = Engine::new(1, &config, &snappy());
    run(&mut e, Buttons::SOFT_DROP, 2);
    assert_eq!(e.phase(), Phase::Locking);

    for _ in 0..200 {
        e.tick(Buttons::LEFT);
        e.tick(Buttons::RIGHT);
    }
    assert!(e.stats().pieces >= 2, "resets were never exhausted");
}

#[test]
fn infinite_reset_lets_a_piece_be_held_indefinitely() {
    let config = MatchConfig {
        lock_reset_mode: LockResetMode::Infinite,
        ..calm()
    };
    let mut e = Engine::new(1, &config, &snappy());
    run(&mut e, Buttons::SOFT_DROP, 2);
    for _ in 0..300 {
        e.tick(Buttons::LEFT);
        e.tick(Buttons::RIGHT);
    }
    assert_eq!(e.stats().pieces, 1, "the piece should never have locked");
}

// ---- hold ------------------------------------------------------------------

#[test]
fn hold_stores_a_piece_and_swaps_it_back() {
    let mut e = engine();
    let first = e.active().unwrap().kind;
    tap(&mut e, Buttons::HOLD);
    assert_eq!(e.hold(), Some(first));
    let second = e.active().unwrap().kind;
    assert_ne!(second, first, "hold should have produced a different piece");

    // Hold is spent until a piece locks.
    tap(&mut e, Buttons::HOLD);
    assert_eq!(
        e.active().unwrap().kind,
        second,
        "hold should not work twice in a row"
    );

    e.tick(Buttons::HARD_DROP);
    e.tick(Buttons::empty());
    tap(&mut e, Buttons::HOLD);
    assert_eq!(
        e.active().unwrap().kind,
        first,
        "the stored piece should come back"
    );
}

#[test]
fn hold_can_be_switched_off_by_a_mode() {
    let config = MatchConfig {
        hold_enabled: false,
        ..calm()
    };
    let mut e = Engine::new(1, &config, &snappy());
    let first = e.active().unwrap().kind;
    tap(&mut e, Buttons::HOLD);
    assert!(e.hold().is_none());
    assert_eq!(e.active().unwrap().kind, first);
}

// ---- clearing --------------------------------------------------------------

/// A seed whose first piece is an I, so a test can set up an exact clear.
///
/// The piece sequence comes from the seed, so a test cannot ask for the piece it wants.
/// Choosing a seed that opens with the piece needed is the honest way to get a precise
/// scenario without reaching inside the engine to plant one.
const I_FIRST_SEED: u64 = 4;

/// Build a well: garbage rows across the bottom with one column left open, then stand
/// the opening I piece vertically in that column. Dropping it completes every garbage
/// row at once.
fn set_up_a_quad(rows: u8, hole_col: u8) -> Engine {
    let config = MatchConfig {
        garbage_cap: 20,
        ..calm()
    };
    let mut e = Engine::new(I_FIRST_SEED, &config, &snappy());
    assert_eq!(
        e.active().unwrap().kind,
        QuadKind::I,
        "the seed no longer opens with an I"
    );

    e.schedule_garbage(PendingGarbage {
        apply_at_tick: 1,
        amount: rows,
        hole_col,
    });
    e.tick(Buttons::empty());

    // Stand the I on end, then walk it to the open column.
    tap(&mut e, Buttons::CW);
    run(&mut e, Buttons::LEFT, 12);
    for _ in 0..hole_col {
        e.tick(Buttons::RIGHT);
        e.tick(Buttons::empty());
    }
    let occupied: Vec<i32> = e.active().unwrap().cells().iter().map(|c| c.0).collect();
    assert!(
        occupied.iter().all(|&x| x == hole_col as i32),
        "the I should be standing in column {hole_col}, got {occupied:?}"
    );
    e
}

#[test]
fn filling_a_row_clears_it() {
    let mut e = set_up_a_quad(1, 0);
    let r = e.tick(Buttons::HARD_DROP);
    assert_eq!(r.lines_cleared, 1);
    assert!(r.contains(Events::LINES_CLEARED));
    assert_eq!(e.stats().lines, 1);
    // The I stands four tall, so only its lowest cell was part of the completed row.
    // The other three survive and settle to the floor.
    assert_eq!(e.board().cell_count(), 3);
    for y in (BOARD_H - 3)..BOARD_H {
        assert!(e.board().occupied(0, y));
    }
}

#[test]
fn completing_four_rows_at_once_clears_all_of_them() {
    let mut e = set_up_a_quad(4, 3);
    let r = e.tick(Buttons::HARD_DROP);
    assert_eq!(r.lines_cleared, 4);
    assert!(e.board().is_empty());
    assert!(r.attack > 0, "a quad should send rows");
}

#[test]
fn a_quad_sends_more_than_a_single() {
    let single = {
        let mut e = set_up_a_quad(1, 0);
        e.tick(Buttons::HARD_DROP).attack
    };
    let quad = {
        let mut e = set_up_a_quad(4, 0);
        e.tick(Buttons::HARD_DROP).attack
    };
    assert!(quad > single, "quad {quad} should beat single {single}");
}

#[test]
fn clearing_the_whole_board_is_a_perfect_clear() {
    let mut e = set_up_a_quad(4, 3);
    let r = e.tick(Buttons::HARD_DROP);
    assert!(r.contains(Events::PERFECT_CLEAR));
    assert!(e.board().is_empty());
}

#[test]
fn a_clear_that_leaves_rows_behind_only_removes_the_full_ones() {
    // Five garbage rows, four of them completed by the I. The fifth must survive and
    // settle to the floor.
    let mut e = set_up_a_quad(5, 0);
    let r = e.tick(Buttons::HARD_DROP);
    assert_eq!(r.lines_cleared, 4);
    assert!(!e.board().is_empty(), "the fifth row should still be there");
    assert_eq!(e.board().row(BOARD_H - 1).count_ones(), BOARD_W as u32 - 1);
}

// ---- garbage ---------------------------------------------------------------

#[test]
fn garbage_lands_on_the_tick_it_was_scheduled_for() {
    // The timing this depends on is the whole reason the game can be played over a
    // network without rollback.
    let mut e = engine();
    e.schedule_garbage(PendingGarbage {
        apply_at_tick: 10,
        amount: 2,
        hole_col: 4,
    });

    idle(&mut e, 9);
    assert!(e.board().is_empty(), "garbage arrived early");

    let r = e.tick(Buttons::empty());
    assert!(r.contains(Events::GARBAGE_APPLIED));
    assert_eq!(e.board().row(BOARD_H - 1).count_ones(), BOARD_W as u32 - 1);
    assert!(
        !e.board().occupied(4, BOARD_H - 1),
        "the hole must stay open"
    );
    assert_eq!(e.stats().garbage_received, 2);
}

#[test]
fn garbage_is_capped_by_the_mode() {
    let config = MatchConfig {
        garbage_cap: 3,
        ..calm()
    };
    let mut e = Engine::new(1, &config, &snappy());
    e.schedule_garbage(PendingGarbage {
        apply_at_tick: 1,
        amount: 30,
        hole_col: 0,
    });
    e.tick(Buttons::empty());
    assert_eq!(e.stats().garbage_received, 3);
}

#[test]
fn a_clear_cancels_incoming_garbage_before_sending_any_on() {
    // The defensive half of the exchange. Without it, answering an attack would still
    // leave you buried.
    let mut e = set_up_a_quad(4, 0);
    e.schedule_garbage(PendingGarbage {
        apply_at_tick: 1_000_000,
        amount: 8,
        hole_col: 2,
    });
    let pending_before = e.pending_garbage().total();

    let r = e.tick(Buttons::HARD_DROP);
    assert_eq!(r.lines_cleared, 4);
    assert_eq!(pending_before, 8);
    assert_eq!(
        e.pending_garbage().total(),
        0,
        "the incoming rows should have been answered in full"
    );

    // A quad that also empties the board is worth more than the eight rows it cancelled,
    // so the surplus carries on to the opponent rather than being discarded.
    let cfg = e.config();
    let raw = cfg.attack_table.quad as u32 + cfg.attack_table.perfect_clear as u32;
    assert_eq!(r.attack as u32, raw - pending_before);
}

#[test]
fn garbage_that_overflows_the_board_ends_the_game() {
    let config = MatchConfig {
        garbage_cap: 40,
        ..calm()
    };
    let mut e = Engine::new(1, &config, &snappy());
    for i in 0..12 {
        e.schedule_garbage(PendingGarbage {
            apply_at_tick: (i + 1) * 2,
            amount: 20,
            hole_col: 0,
        });
    }
    for _ in 0..100 {
        if e.tick(Buttons::empty()).contains(Events::TOPPED_OUT) {
            assert!(e.is_over());
            return;
        }
    }
    panic!("the board never overflowed");
}

// ---- ending ----------------------------------------------------------------

#[test]
fn a_dead_engine_ignores_further_input() {
    let config = MatchConfig {
        garbage_cap: 40,
        ..calm()
    };
    let mut e = Engine::new(1, &config, &snappy());
    for i in 0..12 {
        e.schedule_garbage(PendingGarbage {
            apply_at_tick: (i + 1) * 2,
            amount: 20,
            hole_col: 0,
        });
    }
    for _ in 0..200 {
        e.tick(Buttons::empty());
        if e.is_over() {
            break;
        }
    }
    assert!(e.is_over(), "setup failed to end the game");

    let before = e.checksum();
    for _ in 0..100 {
        assert!(e.tick(Buttons::all()).is_empty());
    }
    assert_eq!(e.checksum(), before, "a finished game must not change");
}

// ---- checksum --------------------------------------------------------------

#[test]
fn the_checksum_reflects_the_board() {
    let mut e = engine();
    let before = e.checksum();
    e.tick(Buttons::HARD_DROP);
    assert_ne!(
        e.checksum(),
        before,
        "locking a piece must change the checksum"
    );
}

#[test]
fn the_checksum_ignores_colors() {
    // Two engines that agree on play but not on presentation are not desynced. This is
    // what stops a skin change from reading as cheating.
    let mut a = engine();
    let mut b = engine();
    for _ in 0..50 {
        a.tick(Buttons::HARD_DROP);
        a.tick(Buttons::empty());
        b.tick(Buttons::HARD_DROP);
        b.tick(Buttons::empty());
    }
    assert_eq!(a.checksum(), b.checksum());
    assert!(a.board().sim_eq(b.board()));
}

#[test]
fn the_checksum_is_pinned_for_a_known_run() {
    // A regression guard on the whole simulation at once. Any rules change moves this,
    // which is exactly when stored replays stop being verifiable and the engine version
    // needs to be bumped.
    let mut e = Engine::new(0x5EED, &calm(), &snappy());
    let script = [
        Buttons::LEFT,
        Buttons::CW,
        Buttons::empty(),
        Buttons::RIGHT,
        Buttons::HARD_DROP,
        Buttons::empty(),
    ];
    for i in 0..600 {
        e.tick(script[i % script.len()]);
    }
    assert_eq!(e.checksum(), PINNED_CHECKSUM, "the simulation changed");
}

const PINNED_CHECKSUM: u64 = 5_176_672_727_642_401_077;

// ---- robustness ------------------------------------------------------------

#[test]
fn no_button_combination_can_crash_the_engine() {
    // Buttons arrive from a remote peer, so every one of the 256 combinations has to be
    // survivable, including ones no keyboard can produce.
    for bits in 0..=255u8 {
        let mut e = engine();
        let input = Buttons::from_bits_retain(bits);
        for _ in 0..300 {
            e.tick(input);
        }
    }
}

#[test]
fn the_piece_never_overlaps_the_stack() {
    // If this ever fails, locking would overwrite cells and the board would be corrupt.
    let mut e = engine();
    let script = [
        Buttons::LEFT,
        Buttons::RIGHT,
        Buttons::CW,
        Buttons::CCW,
        Buttons::FLIP,
        Buttons::SOFT_DROP,
        Buttons::HARD_DROP,
        Buttons::HOLD,
        Buttons::empty(),
    ];
    for i in 0..5000 {
        e.tick(script[(i * 7) % script.len()]);
        // Once the game ends the active piece is the one that was just locked, so it
        // sits on top of its own cells. The invariant is about live play.
        if e.is_over() {
            break;
        }
        for (x, y) in e.active().unwrap().cells() {
            assert!(
                (0..BOARD_W as i32).contains(&x),
                "piece off the board at tick {i}"
            );
            if y >= 0 {
                assert!(
                    !e.board().occupied(x as usize, y as usize),
                    "piece overlaps the stack at tick {i}"
                );
            }
        }
    }
}

#[test]
fn a_row_is_never_left_complete_on_the_board() {
    let mut e = engine();
    for i in 0..3000 {
        if e.is_over() {
            break;
        }
        e.tick(if i % 3 == 0 {
            Buttons::HARD_DROP
        } else {
            Buttons::empty()
        });
        if e.phase() == Phase::ClearDelay {
            continue;
        }
        for y in 0..BOARD_H {
            assert!(!e.board().is_row_full(y), "row {y} survived at tick {i}");
        }
    }
}

#[test]
fn stacking_to_the_ceiling_ends_the_game_rather_than_hanging() {
    let mut e = engine();
    let mut ended = false;
    for _ in 0..4000 {
        e.tick(Buttons::HARD_DROP);
        e.tick(Buttons::empty());
        if e.is_over() {
            ended = true;
            break;
        }
    }
    assert!(
        ended,
        "dropping pieces in one column should eventually top out"
    );
    assert!(e.stats().pieces > 5);
}

#[test]
fn every_config_default_produces_a_playable_engine() {
    // A mode file that sets nothing must still work.
    let mut e = Engine::new(7, &MatchConfig::default(), &Handling::default());
    for _ in 0..600 {
        e.tick(Buttons::empty());
    }
    assert!(e.stats().pieces >= 1);
}

#[test]
fn clamped_handling_is_what_the_engine_actually_uses() {
    // A peer that clamped a player's settings must simulate with the clamped values, or
    // it and the client diverge on the first held direction.
    let wild = Handling {
        das_ms: u16::MAX,
        arr_ms: u16::MAX,
        ..Default::default()
    };
    let mut clamped = wild;
    clamped.clamp();

    let mut a = Engine::new(3, &calm(), &wild);
    let mut b = Engine::new(3, &calm(), &clamped);
    for _ in 0..500 {
        a.tick(Buttons::LEFT);
        b.tick(Buttons::LEFT);
    }
    assert_eq!(a.checksum(), b.checksum());
}

#[test]
fn a_locked_piece_above_the_visible_field_ends_the_game() {
    let config = MatchConfig {
        garbage_cap: 40,
        ..calm()
    };
    let mut e = Engine::new(1, &config, &snappy());
    // Bury the board almost to the ceiling, then keep locking pieces on top.
    e.schedule_garbage(PendingGarbage {
        apply_at_tick: 1,
        amount: (BOARD_H - VISIBLE_H) as u8 + 18,
        hole_col: 0,
    });
    e.tick(Buttons::empty());

    for _ in 0..50 {
        e.tick(Buttons::HARD_DROP);
        e.tick(Buttons::empty());
        if e.is_over() {
            return;
        }
    }
    panic!("locking above the visible field should have ended the game");
}
