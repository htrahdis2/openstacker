//! The random number generator.
//!
//! SplitMix64, written out here rather than pulled from a crate. That is a deliberate
//! trade: a dependency bump that changed a single constant would silently invalidate
//! every replay ever recorded, and nothing would fail loudly enough to notice. Fifteen
//! lines of arithmetic pinned by a known-answer test is the cheaper side of that trade.
//!
//! The algorithm is Steele, Lea and Flood's SplitMix64. It is not cryptographic and does
//! not need to be. What it needs is to produce the same stream on every platform, which
//! it does, because it is nothing but wrapping integer arithmetic.

/// A deterministic 64-bit generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}

const GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;
const MIX1: u64 = 0xBF58_476D_1CE4_E5B9;
const MIX2: u64 = 0x94D0_49BB_1331_11EB;

impl SplitMix64 {
    pub const fn new(seed: u64) -> Self {
        SplitMix64 { state: seed }
    }

    /// The internal state, which the engine checksum includes so that two peers
    /// disagreeing about the piece stream is caught immediately rather than several
    /// pieces later when the boards visibly diverge.
    #[inline]
    pub const fn state(&self) -> u64 {
        self.state
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GAMMA);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(MIX1);
        z = (z ^ (z >> 27)).wrapping_mul(MIX2);
        z ^ (z >> 31)
    }

    /// A value in `0..n`.
    ///
    /// Uses a plain remainder. The bias that introduces is real but vanishingly small at
    /// the sizes used here, and it is *deterministic*, which is the property that
    /// actually matters: a rejection-sampling loop would consume a variable number of
    /// values per call, making the generator's position depend on its own output.
    #[inline]
    pub fn below(&mut self, n: u64) -> u64 {
        debug_assert!(n > 0, "below(0) has no valid result");
        if n == 0 { 0 } else { self.next_u64() % n }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_published_reference_stream() {
        // Checked against the reference SplitMix64 rather than against our own output.
        // Testing an implementation against itself would happily pin a typo forever,
        // and every replay ever recorded depends on this stream being exactly right.
        let mut r = SplitMix64::new(0);
        assert_eq!(r.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(r.next_u64(), 0x6E78_9E6A_A1B9_65F4);
        assert_eq!(r.next_u64(), 0x06C4_5D18_8009_454F);
        assert_eq!(r.next_u64(), 0xF88B_B8A8_724C_81EC);
        assert_eq!(r.next_u64(), 0x1B39_896A_51A8_749B);
    }

    #[test]
    fn the_same_seed_always_replays_the_same_stream() {
        let mut a = SplitMix64::new(0x1234_5678_9ABC_DEF0);
        let mut b = SplitMix64::new(0x1234_5678_9ABC_DEF0);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        assert_eq!(a.state(), b.state());
    }

    #[test]
    fn different_seeds_diverge_immediately() {
        let mut a = SplitMix64::new(1);
        let mut b = SplitMix64::new(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn state_advances_by_a_fixed_step() {
        // The state is a plain counter; all the mixing happens on the way out. This is
        // what makes the stream reproducible from a seed alone.
        let mut r = SplitMix64::new(0);
        r.next_u64();
        assert_eq!(r.state(), GAMMA);
        r.next_u64();
        assert_eq!(r.state(), GAMMA.wrapping_mul(2));
    }

    #[test]
    fn seeding_at_a_state_resumes_the_same_stream() {
        // Needed to restore a generator mid-game, which replay verification does when
        // it checks a checksum partway through a run.
        let mut a = SplitMix64::new(99);
        for _ in 0..10 {
            a.next_u64();
        }
        let mut b = SplitMix64::new(a.state());
        for _ in 0..10 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn below_stays_in_range() {
        let mut r = SplitMix64::new(7);
        for n in 1..=16u64 {
            for _ in 0..200 {
                assert!(r.below(n) < n, "below({n}) escaped its range");
            }
        }
    }

    #[test]
    fn below_one_is_always_zero() {
        let mut r = SplitMix64::new(7);
        for _ in 0..100 {
            assert_eq!(r.below(1), 0);
        }
    }

    #[test]
    fn below_covers_its_whole_range() {
        // Not a randomness test. It only catches a generator stuck on one value, which
        // would make every bag identical and take a while to notice by eye.
        let mut r = SplitMix64::new(42);
        let mut seen = [false; 7];
        for _ in 0..500 {
            seen[r.below(7) as usize] = true;
        }
        assert!(seen.iter().all(|&s| s), "some outcomes never came up");
    }

    #[test]
    fn wrapping_arithmetic_never_panics_at_the_extremes() {
        // Release builds wrap and debug builds trap, so an accidental plain add here
        // would be a crash that only ever reproduces in development.
        let mut r = SplitMix64::new(u64::MAX);
        for _ in 0..100 {
            r.next_u64();
        }
    }
}
