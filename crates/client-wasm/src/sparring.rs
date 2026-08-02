//! An opponent that is not there.
//!
//! Versus needs someone sending rows, and at M2 there is no server and no second player.
//! This produces them on a timer instead, so the garbage bar, cancellation and a survival
//! run can be played and felt before any of it goes over a wire.
//!
//! It lives here rather than in the engine because a training opponent is not a rule of
//! the game, and it lives in Rust rather than in the client because everything it decides
//! — when rows arrive, how many, and which column is left open — would be a rule if the
//! client decided it.
//!
//! Its output is recorded in the replay as ordinary garbage, so a sparring run replays
//! exactly even if this file changes afterwards.

use config::Sparring;
use engine::{BOARD_W, MatchConfig, PendingGarbage, SplitMix64, ms_to_subticks};

/// Distinguishes this generator's stream from the bag's, so drawing a hole column cannot
/// change which piece comes next.
const STREAM: u64 = 0x5350_4152_5249_4E47;

pub struct Opponent {
    rng: SplitMix64,
    profile: Sparring,
    /// Absolute tick of the next batch.
    next_at: u32,
    /// Current gap between batches, in ticks. Shrinks towards the profile's floor.
    interval: u32,
}

fn ticks(ms: u32) -> u32 {
    ms_to_subticks(ms) / engine::SUBTICK
}

impl Opponent {
    pub fn new(seed: u64, profile: &Sparring) -> Opponent {
        let mut profile = *profile;
        profile.clamp();
        Opponent {
            rng: SplitMix64::new(seed ^ STREAM),
            next_at: ticks(profile.first_batch_ms).max(1),
            interval: ticks(profile.interval_ms).max(1),
            profile,
        }
    }

    /// Rows to hand the engine before `tick` runs, if a batch is due on it.
    ///
    /// A batch is one entry per hole column: with `garbage_hole_repeat` on that is a
    /// single entry, and with it off the rows arrive together with different columns.
    /// Either way the engine is only ever told which column is open, never asked to work
    /// one out — deriving it is what two peers would disagree about.
    pub fn due(&mut self, tick: u32, config: &MatchConfig) -> Vec<PendingGarbage> {
        if tick < self.next_at {
            return Vec::new();
        }

        let span = (self.profile.rows_max - self.profile.rows_min) as u64 + 1;
        let rows = self.profile.rows_min + self.rng.below(span) as u8;
        let apply_at_tick = tick + config.garbage_delay_ticks().max(1);

        let mut batch = Vec::new();
        if config.garbage_hole_repeat {
            batch.push(PendingGarbage {
                apply_at_tick,
                amount: rows,
                hole_col: self.rng.below(BOARD_W as u64) as u8,
            });
        } else {
            for _ in 0..rows {
                batch.push(PendingGarbage {
                    apply_at_tick,
                    amount: 1,
                    hole_col: self.rng.below(BOARD_W as u64) as u8,
                });
            }
        }

        let floor = ticks(self.profile.min_interval_ms).max(1);
        self.interval = self
            .interval
            .saturating_sub(ticks(self.profile.interval_step_ms))
            .max(floor);
        self.next_at = tick + self.interval;
        batch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> Sparring {
        Sparring::default()
    }

    /// Every batch a run of `n` ticks produces.
    fn run(seed: u64, n: u32, config: &MatchConfig) -> Vec<(u32, PendingGarbage)> {
        let mut o = Opponent::new(seed, &profile());
        let mut out = Vec::new();
        for tick in 1..=n {
            for g in o.due(tick, config) {
                out.push((tick, g));
            }
        }
        out
    }

    #[test]
    fn the_opening_is_quiet() {
        let config = MatchConfig::default();
        let first = run(1, 60 * 60, &config).first().copied().unwrap();
        assert!(
            first.0 >= ticks(profile().first_batch_ms),
            "first batch at tick {}",
            first.0
        );
    }

    #[test]
    fn the_same_seed_produces_the_same_opponent() {
        // A sparring run has to be reproducible like any other, or its replay is fiction.
        let config = MatchConfig::default();
        assert_eq!(run(7, 60 * 120, &config), run(7, 60 * 120, &config));
    }

    #[test]
    fn different_seeds_produce_different_opponents() {
        let config = MatchConfig::default();
        assert_ne!(run(1, 60 * 120, &config), run(2, 60 * 120, &config));
    }

    #[test]
    fn rows_stay_inside_the_profile() {
        let config = MatchConfig::default();
        let p = profile();
        for (_, g) in run(3, 60 * 300, &config) {
            assert!(g.amount >= p.rows_min && g.amount <= p.rows_max, "{g:?}");
            assert!((g.hole_col as usize) < BOARD_W);
        }
    }

    #[test]
    fn rows_are_always_scheduled_ahead_of_the_tick_they_were_asked_for() {
        // The invariant the whole garbage design rests on. A batch due on a tick that has
        // already run lands immediately, which is not what anyone scheduled.
        let config = MatchConfig::default();
        for (tick, g) in run(4, 60 * 300, &config) {
            assert!(g.apply_at_tick > tick, "{g:?} scheduled on {tick}");
        }
    }

    #[test]
    fn the_pressure_builds_and_then_holds() {
        // A survival run has to end, and an opponent that never speeds up is one nobody
        // loses to. It also has to stop somewhere, or it stops being playable rather than
        // hard.
        let config = MatchConfig::default();
        let batches = run(5, 60 * 600, &config);
        let gaps: Vec<u32> = batches.windows(2).map(|w| w[1].0 - w[0].0).collect();
        assert!(gaps.first() > gaps.last(), "{gaps:?}");
        let floor = ticks(profile().min_interval_ms);
        assert!(gaps.iter().all(|g| *g >= floor), "{gaps:?}");
    }

    #[test]
    fn a_batch_with_varied_holes_arrives_as_one_batch() {
        // Turning hole repeat off is a sender's decision, not the engine's: it splits the
        // batch into rows that share an arrival tick. The engine never derives a column.
        let config = MatchConfig {
            garbage_hole_repeat: false,
            ..Default::default()
        };
        let mut o = Opponent::new(9, &profile());
        let mut batch = Vec::new();
        for tick in 1..=(60 * 60) {
            batch = o.due(tick, &config);
            if !batch.is_empty() {
                break;
            }
        }
        assert!(batch.len() > 1, "expected a row per hole: {batch:?}");
        assert!(batch.iter().all(|g| g.amount == 1));
        let first = batch[0].apply_at_tick;
        assert!(batch.iter().all(|g| g.apply_at_tick == first));
    }
}
