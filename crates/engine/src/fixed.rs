//! Fixed-point time.
//!
//! Every duration in the engine — DAS, ARR, soft drop, gravity, lock delay, clear and
//! spawn delays — is stored as a **subtick** count, where `1 tick = SUBTICK subticks`.
//! Config expresses those durations in milliseconds; conversion happens exactly once,
//! at construction, in integer arithmetic only.
//!
//! Subticks exist so that handling can be tuned below whole-frame granularity, which
//! competitive players expect and which floats cannot provide without giving up
//! cross-platform determinism.
//!
//! # One scale for both time and gravity
//!
//! Gravity does not get its own sub-*cell* accumulator. "One row per `ms_per_row`
//! milliseconds" is a duration like any other, so gravity is a subtick threshold and the
//! piece descends whenever the accumulator crosses it. One conversion function, one
//! scale, one class of rounding error.

use crate::consts::{SUBTICK, TICK_HZ};

const fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Exact integer ratio of subticks to milliseconds, reduced.
///
/// `SUBTICK * TICK_HZ / 1000` = `256 * 60 / 1000` = `15.36` = `384 / 25`.
///
/// Derived rather than hardcoded so that changing `SUBTICK` or `TICK_HZ` cannot leave a
/// stale reduction behind.
const RAW_NUM: u64 = SUBTICK as u64 * TICK_HZ as u64;
const RAW_DEN: u64 = 1000;
const SUBTICKS_PER_MS_NUM: u64 = RAW_NUM / gcd(RAW_NUM, RAW_DEN);
const SUBTICKS_PER_MS_DEN: u64 = RAW_DEN / gcd(RAW_NUM, RAW_DEN);

/// Convert a millisecond duration to subticks, truncating.
///
/// Resolution is one subtick ≈ 0.0651 ms ≈ 0.0039 frames. Because milliseconds are the
/// stored unit, frame-denominated values quantise to roughly ±0.03 F. A settings UI must
/// therefore treat ms as canonical and show frames as a derived read-out — never let a
/// user type a frame value and expect it back unchanged.
pub const fn ms_to_subticks(ms: u32) -> u32 {
    let sub = (ms as u64 * SUBTICKS_PER_MS_NUM) / SUBTICKS_PER_MS_DEN;
    if sub > u32::MAX as u64 {
        u32::MAX
    } else {
        sub as u32
    }
}

/// Convert a millisecond duration to subticks, clamped to at least 1.
///
/// Used for values that act as a **divisor or threshold** — gravity and soft drop —
/// where a zero threshold would spin forever. Callers must special-case `ms == 0` as
/// "instant" *before* calling this; the clamp is a guard against a config that rounds
/// down to zero, not a substitute for that check.
pub const fn ms_to_subticks_nonzero(ms: u32) -> u32 {
    let sub = ms_to_subticks(ms);
    if sub == 0 { 1 } else { sub }
}

/// Convert subticks back to milliseconds, rounding to nearest. **Display only.**
///
/// Rounds rather than truncates so that a value which came from [`ms_to_subticks`]
/// round-trips exactly — a settings slider that shows `132` for a stored `133` reads as
/// a bug even when the underlying subtick count is right.
pub const fn subticks_to_ms(sub: u32) -> u32 {
    let n = sub as u64 * SUBTICKS_PER_MS_DEN;
    ((n + SUBTICKS_PER_MS_NUM / 2) / SUBTICKS_PER_MS_NUM) as u32
}

/// Convert subticks to hundredths of a frame, for UI read-outs like `8.52 F`.
/// Rounds to nearest, for the same reason as [`subticks_to_ms`]. **Display only.**
pub const fn subticks_to_centiframes(sub: u32) -> u32 {
    let n = sub as u64 * 100;
    ((n + SUBTICK as u64 / 2) / SUBTICK as u64) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_constants_are_the_intended_ones() {
        assert_eq!(SUBTICKS_PER_MS_NUM, 384);
        assert_eq!(SUBTICKS_PER_MS_DEN, 25);
    }

    #[test]
    fn whole_frame_durations_are_exact() {
        // 1000 ms is exactly 60 ticks; 100 ms is exactly 6.
        assert_eq!(ms_to_subticks(1000), 60 * SUBTICK);
        assert_eq!(ms_to_subticks(100), 6 * SUBTICK);
        assert_eq!(ms_to_subticks(0), 0);
    }

    #[test]
    fn sub_frame_precision_is_available() {
        // The whole point of subticks: 8.5 frames must be distinguishable from 8 and 9.
        let eight = ms_to_subticks(133); // ~7.98 F
        let eight_half = ms_to_subticks(142); // ~8.52 F
        let nine = ms_to_subticks(150); // ~9.00 F
        assert!(eight < eight_half && eight_half < nine);
        assert_ne!(eight / SUBTICK, eight_half / SUBTICK);
    }

    #[test]
    fn frame_readout_matches_the_documented_quantisation() {
        // A user asking for "8.5 frames" stores 142 ms and reads back 8.52 F. Pinned
        // because the ms-is-canonical UI contract depends on this exact behaviour.
        assert_eq!(subticks_to_centiframes(ms_to_subticks(142)), 852);
    }

    #[test]
    fn round_trip_is_exact_for_every_representable_millisecond() {
        // Rounding on the way back makes ms -> subticks -> ms the identity across the
        // whole config range. If this ever regresses, settings sliders start drifting
        // by 1 ms every time they are saved and reloaded.
        for ms in 0..=u16::MAX as u32 {
            let back = subticks_to_ms(ms_to_subticks(ms));
            assert_eq!(back, ms, "{ms} ms did not round-trip");
        }
    }

    #[test]
    fn conversion_is_monotonic() {
        let mut prev = 0;
        for ms in 0..=2000u32 {
            let sub = ms_to_subticks(ms);
            assert!(sub >= prev, "not monotonic at {ms} ms");
            prev = sub;
        }
    }

    #[test]
    fn nonzero_variant_never_returns_a_zero_threshold() {
        for ms in 0..=64u32 {
            assert!(ms_to_subticks_nonzero(ms) >= 1);
        }
    }

    #[test]
    fn does_not_overflow_at_the_config_ceiling() {
        // u16::MAX ms is the widest any config field can express.
        let sub = ms_to_subticks(u16::MAX as u32);
        assert_eq!(sub, (65535u64 * 384 / 25) as u32);
        assert!(sub < u32::MAX);
    }
}
