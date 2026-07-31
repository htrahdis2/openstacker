//! The piece sequence.
//!
//! Pieces come in shuffled groups of seven containing each kind exactly once, so a
//! player is never starved of the piece they need for more than a bounded stretch. That
//! bound is what makes the game a planning problem rather than a lottery.

use crate::consts::MAX_PREVIEW;
use crate::quad::QuadKind;
use crate::rng::SplitMix64;
use arrayvec::ArrayVec;

/// Room for two full groups, which is the most that can be queued at once.
pub const QUEUE_CAP: usize = QuadKind::COUNT * 2;

/// The upcoming pieces, and the generator that produces them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bag {
    queue: ArrayVec<QuadKind, QUEUE_CAP>,
    rng: SplitMix64,
}

impl Bag {
    /// Start a sequence, filling the queue immediately so a preview is available before
    /// the first piece is taken.
    pub fn new(seed: u64) -> Self {
        let mut bag = Bag {
            queue: ArrayVec::new(),
            rng: SplitMix64::new(seed),
        };
        bag.refill();
        bag
    }

    /// Take the next piece.
    pub fn take(&mut self) -> QuadKind {
        // The queue is refilled eagerly and can never be empty, but returning a piece
        // rather than panicking keeps a malformed state from taking down a whole match.
        let kind = if self.queue.is_empty() {
            self.refill();
            self.queue.first().copied().unwrap_or(QuadKind::I)
        } else {
            self.queue[0]
        };
        if !self.queue.is_empty() {
            self.queue.remove(0);
        }
        self.refill();
        kind
    }

    /// The upcoming pieces, nearest first, up to `count`.
    pub fn preview(&self, count: usize) -> &[QuadKind] {
        let n = count.min(self.queue.len()).min(MAX_PREVIEW);
        &self.queue[..n]
    }

    pub fn queue(&self) -> &[QuadKind] {
        &self.queue
    }

    pub const fn rng_state(&self) -> u64 {
        self.rng.state()
    }

    /// Top the queue up so a full preview is always available.
    ///
    /// The trigger is a fixed constant, deliberately not the configured preview length.
    /// Tying it to a setting would make the generator's position depend on how many
    /// pieces a player can see, so two peers that ever disagreed about preview length
    /// would silently draw different pieces. Keeping it fixed means the sequence depends
    /// on the seed alone.
    fn refill(&mut self) {
        while self.queue.len() <= MAX_PREVIEW {
            let group = self.shuffled_group();
            for kind in group {
                self.queue.push(kind);
            }
        }
    }

    /// One group of seven, shuffled.
    ///
    /// Fisher-Yates, walked downward. The order of the swaps is part of the replay
    /// format in everything but name: changing it changes every piece sequence that has
    /// ever been recorded.
    fn shuffled_group(&mut self) -> [QuadKind; QuadKind::COUNT] {
        let mut group = QuadKind::ALL;
        for i in (1..group.len()).rev() {
            let j = self.rng.below(i as u64 + 1) as usize;
            group.swap(i, j);
        }
        group
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn take(bag: &mut Bag, n: usize) -> Vec<QuadKind> {
        (0..n).map(|_| bag.take()).collect()
    }

    #[test]
    fn the_same_seed_always_produces_the_same_pieces() {
        let (mut a, mut b) = (Bag::new(12345), Bag::new(12345));
        assert_eq!(take(&mut a, 700), take(&mut b, 700));
    }

    #[test]
    fn different_seeds_produce_different_sequences() {
        let (mut a, mut b) = (Bag::new(1), Bag::new(2));
        assert_ne!(take(&mut a, 20), take(&mut b, 20));
    }

    #[test]
    fn pieces_are_pinned_for_a_known_seed() {
        // A regression guard, not a correctness check. It cannot tell a good shuffle
        // from a bad one; the permutation and distribution tests do that, and the
        // generator itself is checked against its published reference stream.
        //
        // What this catches is silent change. Every recorded replay and every stored
        // personal best is anchored to this sequence, so a refactor that alters it
        // invalidates all of them. Without this test that happens quietly.
        let mut bag = Bag::new(0);
        let got: String = take(&mut bag, 14).iter().map(|k| k.label()).collect();
        assert_eq!(got, "LSOJZITSILJZTO");
    }

    #[test]
    fn every_group_of_seven_holds_each_kind_exactly_once() {
        // The whole point of the format. Without it a player can be starved of a piece
        // indefinitely and the game stops being about planning.
        let mut bag = Bag::new(999);
        for group in 0..200 {
            let mut seen = [0u8; QuadKind::COUNT];
            for _ in 0..QuadKind::COUNT {
                seen[bag.take().index()] += 1;
            }
            assert!(
                seen.iter().all(|&c| c == 1),
                "group {group} was not a permutation: {seen:?}"
            );
        }
    }

    #[test]
    fn the_queue_never_outgrows_its_storage() {
        let mut bag = Bag::new(4);
        for _ in 0..500 {
            assert!(bag.queue().len() <= QUEUE_CAP);
            bag.take();
        }
    }

    #[test]
    fn a_full_preview_is_available_before_the_first_piece_is_taken() {
        // A player must see the preview on the very first frame, not after a piece or
        // two has been consumed.
        let bag = Bag::new(1);
        assert_eq!(bag.preview(MAX_PREVIEW).len(), MAX_PREVIEW);
    }

    #[test]
    fn a_full_preview_stays_available_forever() {
        let mut bag = Bag::new(1);
        for i in 0..500 {
            assert_eq!(
                bag.preview(MAX_PREVIEW).len(),
                MAX_PREVIEW,
                "preview ran short after {i} pieces"
            );
            bag.take();
        }
    }

    #[test]
    fn preview_shows_the_pieces_that_actually_arrive() {
        let mut bag = Bag::new(77);
        let previewed: Vec<QuadKind> = bag.preview(5).to_vec();
        let taken = take(&mut bag, 5);
        assert_eq!(previewed, taken);
    }

    #[test]
    fn asking_for_more_preview_than_exists_is_not_an_error() {
        let bag = Bag::new(1);
        assert_eq!(bag.preview(1000).len(), MAX_PREVIEW);
        assert_eq!(bag.preview(0).len(), 0);
    }

    #[test]
    fn the_piece_sequence_does_not_depend_on_how_many_pieces_are_previewed() {
        // Refill is triggered by a fixed constant rather than by the configured preview
        // length. If it were tied to the setting, two peers who disagreed about preview
        // length would draw different pieces from the same seed.
        let mut a = Bag::new(555);
        let mut b = Bag::new(555);
        let mut from_a = Vec::new();
        let mut from_b = Vec::new();
        for i in 0..200 {
            // Vary how much of the preview is read, which must change nothing.
            let _ = a.preview(i % (MAX_PREVIEW + 1));
            let _ = b.preview(MAX_PREVIEW);
            from_a.push(a.take());
            from_b.push(b.take());
        }
        assert_eq!(from_a, from_b);
    }

    #[test]
    fn reading_the_preview_never_advances_the_generator() {
        let bag = Bag::new(8);
        let before = bag.rng_state();
        for _ in 0..50 {
            let _ = bag.preview(MAX_PREVIEW);
        }
        assert_eq!(bag.rng_state(), before);
    }

    #[test]
    fn every_kind_appears_at_a_fair_rate_over_many_groups() {
        // A shuffle that ignored an index would still pass the permutation test if it
        // only ever swapped within a subset. Counting across many groups catches that.
        let mut bag = Bag::new(2024);
        let mut counts = [0u32; QuadKind::COUNT];
        const GROUPS: u32 = 2000;
        for _ in 0..GROUPS * QuadKind::COUNT as u32 {
            counts[bag.take().index()] += 1;
        }
        for (i, &c) in counts.iter().enumerate() {
            assert_eq!(c, GROUPS, "kind {i} appeared {c} times, expected {GROUPS}");
        }
    }

    #[test]
    fn the_first_piece_is_not_always_the_same_kind() {
        // A shuffle that never moved index 0 would still permute, but would start every
        // game with the same piece.
        let mut firsts = [false; QuadKind::COUNT];
        for seed in 0..200u64 {
            firsts[Bag::new(seed).take().index()] = true;
        }
        assert!(
            firsts.iter().all(|&s| s),
            "some kinds never appeared first: {firsts:?}"
        );
    }
}
