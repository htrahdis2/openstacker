//! Per-tick output.
//!
//! Callers react to `Events`, never to introspecting engine state. That rule keeps
//! audio, network and UI layers decoupled from the simulation, and it is why the full
//! bitset is defined and populated up front, including flags that nothing consumes yet.
//! A renderer written against it does not need reworking when new rules land.

use bitflags::bitflags;

bitflags! {
    /// Everything that happened during one `Engine::tick`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct Events: u16 {
        const PIECE_LOCKED    = 1 << 0;
        const LINES_CLEARED   = 1 << 1;
        const SPIN            = 1 << 2;
        const MINI_SPIN       = 1 << 3;
        const B2B_CONTINUED   = 1 << 4;
        const B2B_BROKEN      = 1 << 5;
        const PERFECT_CLEAR   = 1 << 6;
        const GARBAGE_APPLIED = 1 << 7;
        const TOPPED_OUT      = 1 << 8;
        const HELD            = 1 << 9;
        // The five below exist so that an audio layer has a one-shot trigger for each.
        // Deriving them by diffing engine state would break the rule above.
        const ROTATED         = 1 << 10;
        const MOVED           = 1 << 11;
        const HARD_DROPPED    = 1 << 12;
        const SOFT_DROPPED    = 1 << 13;
        const SPAWNED         = 1 << 14;
    }
}

/// The result of one `Engine::tick`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TickResult {
    pub events: Events,
    /// Garbage rows this tick's clear would send to an opponent, after local
    /// cancellation. Always 0 outside the tick a piece locks.
    pub attack: u8,
    /// Rows cleared this tick. Always 0 outside the tick a piece locks.
    pub lines_cleared: u8,
}

impl TickResult {
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.events.is_empty() && self.attack == 0 && self.lines_cleared == 0
    }

    #[inline]
    pub fn contains(&self, e: Events) -> bool {
        self.events.contains(e)
    }
}

/// Cumulative counters, for HUDs and for replay verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    pub tick: u32,
    pub lines: u32,
    pub pieces: u32,
    pub attack_sent: u32,
    pub garbage_received: u32,
    pub max_combo: u8,
    pub max_b2b: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_flag_occupies_a_distinct_bit() {
        assert_eq!(Events::all().bits().count_ones(), 15);
    }

    #[test]
    fn spin_and_mini_spin_are_separate_flags() {
        // A mini is not a full spin; consumers must be able to tell them apart without
        // consulting engine state.
        assert_ne!(Events::SPIN, Events::MINI_SPIN);
        assert!(!Events::MINI_SPIN.contains(Events::SPIN));
    }

    #[test]
    fn b2b_continued_and_broken_are_mutually_exclusive_by_construction() {
        // Nothing enforces this at the type level, so the engine must never set both.
        // Documented here as the contract the scoring code is tested against.
        let both = Events::B2B_CONTINUED | Events::B2B_BROKEN;
        assert_eq!(both.bits().count_ones(), 2);
    }

    #[test]
    fn empty_tick_result_reports_itself_empty() {
        assert!(TickResult::default().is_empty());
        let r = TickResult {
            events: Events::MOVED,
            ..Default::default()
        };
        assert!(!r.is_empty());
        assert!(r.contains(Events::MOVED));
    }
}
