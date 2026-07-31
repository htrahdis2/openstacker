//! The replay format.
//!
//! A replay is the seed, the rules, the player's handling, and the buttons they held on
//! every tick. Nothing else. Everything a viewer sees is re-derived by running the
//! simulation, which is why a replay of a long game is a few hundred bytes and why the
//! same file can be used to verify a claimed result rather than just play it back.

use engine::{Buttons, ENGINE_VER, Engine, Handling, MatchConfig};
use serde::{Deserialize, Serialize};

/// The replay file format version, bumped when the file layout changes.
pub const REPLAY_VERSION: u16 = 1;

/// What the recorder claims happened.
///
/// Checked rather than trusted. A replay that does not reproduce its own claimed result
/// is either from a different build or was edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Outcome {
    pub final_tick: u32,
    pub lines: u32,
    pub pieces: u32,
    pub attack: u32,
    pub checksum: u64,
    pub topped_out: bool,
}

/// A recorded game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replay {
    pub version: u16,
    /// The rules version this was recorded under. A mismatch does not stop playback, but
    /// it does mean the claimed result cannot be re-verified.
    pub engine_ver: u32,
    pub seed: u64,
    pub config: MatchConfig,
    pub handling: Handling,
    /// Buttons per tick, run-length encoded as `(bits, run)`.
    ///
    /// Players hold buttons across many frames, so the raw stream is long runs of
    /// identical values. Encoding those runs turns a minute of play into a few hundred
    /// bytes, which is what makes keeping every game affordable.
    pub inputs: Vec<(u8, u8)>,
    pub claimed: Outcome,
}

impl Replay {
    /// Expand the run-length encoded inputs back into one value per tick.
    pub fn buttons(&self) -> Vec<Buttons> {
        let mut out = Vec::with_capacity(self.tick_count());
        for &(bits, run) in &self.inputs {
            for _ in 0..run {
                out.push(Buttons::from_bits_retain(bits));
            }
        }
        out
    }

    /// How many ticks this replay covers.
    pub fn tick_count(&self) -> usize {
        self.inputs.iter().map(|(_, run)| *run as usize).sum()
    }

    /// Encode a tick-by-tick button stream.
    pub fn encode(buttons: &[Buttons]) -> Vec<(u8, u8)> {
        let mut out: Vec<(u8, u8)> = Vec::new();
        for b in buttons {
            let bits = b.bits();
            match out.last_mut() {
                // A run length is a u8, so a long hold becomes several runs rather than
                // wrapping around to nothing.
                Some((last, run)) if *last == bits && *run < u8::MAX => *run += 1,
                _ => out.push((bits, 1)),
            }
        }
        out
    }

    /// Build a replay by running a button stream through the simulation.
    pub fn record(
        seed: u64,
        config: &MatchConfig,
        handling: &Handling,
        buttons: &[Buttons],
    ) -> Replay {
        let mut engine = Engine::new(seed, config, handling);
        for b in buttons {
            engine.tick(*b);
        }
        let stats = engine.stats();
        Replay {
            version: REPLAY_VERSION,
            engine_ver: ENGINE_VER,
            seed,
            config: config.clone(),
            handling: *handling,
            inputs: Replay::encode(buttons),
            claimed: Outcome {
                final_tick: stats.tick,
                lines: stats.lines,
                pieces: stats.pieces,
                attack: stats.attack_sent,
                checksum: engine.checksum(),
                topped_out: engine.is_over(),
            },
        }
    }

    /// Re-run the replay and report what actually happened.
    pub fn simulate(&self) -> (Engine, Outcome) {
        let mut engine = Engine::new(self.seed, &self.config, &self.handling);
        for b in self.buttons() {
            engine.tick(b);
        }
        let stats = engine.stats();
        let outcome = Outcome {
            final_tick: stats.tick,
            lines: stats.lines,
            pieces: stats.pieces,
            attack: stats.attack_sent,
            checksum: engine.checksum(),
            topped_out: engine.is_over(),
        };
        (engine, outcome)
    }

    /// Whether the rules this was recorded under still match the current build.
    pub fn is_verifiable(&self) -> bool {
        self.engine_ver == ENGINE_VER
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buttons(seq: &[u8]) -> Vec<Buttons> {
        seq.iter().map(|b| Buttons::from_bits_retain(*b)).collect()
    }

    #[test]
    fn encoding_round_trips() {
        let input = buttons(&[1, 1, 1, 2, 2, 0, 0, 0, 0, 4]);
        let r = Replay {
            version: REPLAY_VERSION,
            engine_ver: ENGINE_VER,
            seed: 0,
            config: MatchConfig::default(),
            handling: Handling::default(),
            inputs: Replay::encode(&input),
            claimed: Outcome::default(),
        };
        assert_eq!(r.buttons(), input);
        assert_eq!(r.tick_count(), input.len());
    }

    #[test]
    fn encoding_collapses_held_buttons() {
        // The reason a long game fits in a few hundred bytes.
        let held = vec![Buttons::LEFT; 200];
        assert_eq!(Replay::encode(&held).len(), 1);
    }

    #[test]
    fn a_run_longer_than_a_byte_splits_rather_than_wrapping() {
        // 300 identical ticks cannot be one run. Wrapping would silently drop 44 of them.
        let held = vec![Buttons::LEFT; 300];
        let encoded = Replay::encode(&held);
        assert_eq!(encoded.len(), 2);
        let total: usize = encoded.iter().map(|(_, r)| *r as usize).sum();
        assert_eq!(total, 300);
    }

    #[test]
    fn an_empty_stream_encodes_to_nothing() {
        assert!(Replay::encode(&[]).is_empty());
    }

    #[test]
    fn a_recorded_replay_reproduces_its_own_claim() {
        // The property the whole format exists for.
        let input: Vec<Buttons> = (0..500)
            .map(|i| match i % 7 {
                0 => Buttons::LEFT,
                1 => Buttons::CW,
                2 => Buttons::HARD_DROP,
                3 => Buttons::RIGHT,
                4 => Buttons::SOFT_DROP,
                5 => Buttons::HOLD,
                _ => Buttons::empty(),
            })
            .collect();
        let r = Replay::record(42, &MatchConfig::default(), &Handling::default(), &input);
        let (_, actual) = r.simulate();
        assert_eq!(actual, r.claimed);
        assert!(r.is_verifiable());
    }

    #[test]
    fn simulating_twice_gives_the_same_answer() {
        let input = [Buttons::HARD_DROP, Buttons::empty()].repeat(100);
        let r = Replay::record(7, &MatchConfig::default(), &Handling::default(), &input);
        assert_eq!(r.simulate().1, r.simulate().1);
    }

    #[test]
    fn a_tampered_result_no_longer_matches() {
        // What verification is for: a claimed result that the inputs do not produce.
        let input = [Buttons::HARD_DROP, Buttons::empty()].repeat(50);
        let mut r = Replay::record(1, &MatchConfig::default(), &Handling::default(), &input);
        r.claimed.lines += 10;
        assert_ne!(r.simulate().1, r.claimed);
    }

    #[test]
    fn changing_the_inputs_changes_the_outcome() {
        let a = Replay::record(
            1,
            &MatchConfig::default(),
            &Handling::default(),
            &[Buttons::HARD_DROP, Buttons::empty()].repeat(50),
        );
        let b = Replay::record(
            1,
            &MatchConfig::default(),
            &Handling::default(),
            &[Buttons::LEFT, Buttons::HARD_DROP, Buttons::empty()].repeat(50),
        );
        assert_ne!(a.claimed.checksum, b.claimed.checksum);
    }

    #[test]
    fn a_replay_from_older_rules_is_marked_unverifiable_rather_than_rejected() {
        // Old replays stay watchable. They just cannot have their result re-checked
        // under rules they were not played under.
        let input = [Buttons::HARD_DROP, Buttons::empty()].repeat(10);
        let mut r = Replay::record(1, &MatchConfig::default(), &Handling::default(), &input);
        r.engine_ver = ENGINE_VER - 1;
        assert!(!r.is_verifiable());
        let _ = r.simulate();
    }

    #[test]
    fn a_replay_serializes_to_json_and_back() {
        let input = [Buttons::LEFT, Buttons::HARD_DROP, Buttons::empty()].repeat(20);
        let r = Replay::record(9, &MatchConfig::default(), &Handling::default(), &input);
        let text = serde_json::to_string(&r).unwrap();
        let back: Replay = serde_json::from_str(&text).unwrap();
        assert_eq!(back, r);
        assert_eq!(back.simulate().1, r.claimed);
    }

    #[test]
    fn a_replay_of_a_long_game_stays_small() {
        // Small enough that keeping every game a player ever finishes is affordable,
        // which is what makes result verification worth building at all.
        let input: Vec<Buttons> = (0..3600)
            .map(|i| {
                if i % 30 == 0 {
                    Buttons::HARD_DROP
                } else {
                    Buttons::empty()
                }
            })
            .collect();
        let r = Replay::record(3, &MatchConfig::default(), &Handling::default(), &input);
        let bytes = serde_json::to_vec(&r).unwrap().len();
        assert!(bytes < 20_000, "a minute of play took {bytes} bytes");
    }
}
