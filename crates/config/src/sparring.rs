//! The training opponent a mode can carry.
//!
//! Kept out of the `files` half of this crate so the browser can read one without a TOML
//! parser, and kept out of `MatchConfig` because it is not a rule of the game: it decides
//! what a player has to survive, not how the game behaves. What it produces reaches the
//! simulation as ordinary garbage, recorded in the replay like any other.

use serde::{Deserialize, Serialize};

/// A training opponent: rows arriving on a timer, with no server and no second player.
///
/// Deliberately not part of [`MatchConfig`]. What it produces reaches the simulation —
/// as garbage, recorded in the replay like any other garbage — but the profile itself is
/// no more a rule of the game than a goal is. A real opponent replaces it without any of
/// these numbers meaning anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Sparring {
    /// Quiet opening, so a run does not begin under pressure.
    pub first_batch_ms: u32,
    /// Time between batches at the start.
    pub interval_ms: u32,
    /// How much shorter each gap is than the one before it.
    pub interval_step_ms: u32,
    /// The shortest gap it works down to. This is what decides whether a run ends.
    pub min_interval_ms: u32,
    pub rows_min: u8,
    pub rows_max: u8,
}

impl Default for Sparring {
    fn default() -> Self {
        Sparring {
            first_batch_ms: 8_000,
            interval_ms: 6_000,
            interval_step_ms: 250,
            min_interval_ms: 1_500,
            rows_min: 1,
            rows_max: 4,
        }
    }
}

impl Sparring {
    /// Bring a profile into a range that produces a playable opponent.
    ///
    /// Clamped rather than rejected for the same reason player settings are: a mode file
    /// that asks for something silly should still be playable.
    pub fn clamp(&mut self) {
        self.min_interval_ms = self.min_interval_ms.clamp(100, 60_000);
        self.interval_ms = self.interval_ms.clamp(self.min_interval_ms, 120_000);
        self.interval_step_ms = self.interval_step_ms.min(self.interval_ms);
        self.first_batch_ms = self.first_batch_ms.min(600_000);
        self.rows_max = self.rows_max.clamp(1, 20);
        self.rows_min = self.rows_min.clamp(1, self.rows_max);
    }
}
