//! Incoming garbage rows and when they arrive.
//!
//! Garbage is scheduled for a tick in the future rather than applied on arrival. That
//! delay is the whole reason this game can be played over a network without rollback:
//! the message describing it reaches every peer well before the tick it lands on, so
//! both sides apply it on the same tick from the same state and neither has to predict
//! or correct anything.

use arrayvec::ArrayVec;

/// How many separate batches can be in flight at once.
pub const MAX_PENDING: usize = 32;

/// Rows that will enter the board at a known future tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PendingGarbage {
    /// The tick these rows enter the board. Always in the future when scheduled.
    pub apply_at_tick: u32,
    pub amount: u8,
    /// Column left open. Chosen by whoever sent the garbage and carried explicitly, so
    /// it never has to be derived from a generator two peers might disagree about.
    pub hole_col: u8,
}

/// Garbage waiting to land.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GarbageQueue {
    pending: ArrayVec<PendingGarbage, MAX_PENDING>,
}

impl GarbageQueue {
    pub const fn new() -> Self {
        GarbageQueue {
            pending: ArrayVec::new_const(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &PendingGarbage> {
        self.pending.iter()
    }

    /// Total rows waiting, saturating.
    pub fn total(&self) -> u32 {
        self.pending.iter().map(|g| g.amount as u32).sum()
    }

    /// Queue a batch.
    ///
    /// Batches are kept sorted by arrival so that cancellation and application both
    /// work oldest-first without having to search. A dropped batch when the queue is
    /// full is deliberate: refusing new garbage is survivable, growing without bound
    /// is not, and the cap is far above anything reachable in play.
    pub fn schedule(&mut self, g: PendingGarbage) {
        if g.amount == 0 {
            return;
        }
        let at = self
            .pending
            .iter()
            .position(|p| p.apply_at_tick > g.apply_at_tick)
            .unwrap_or(self.pending.len());
        if self.pending.is_full() {
            return;
        }
        self.pending.insert(at, g);
    }

    /// Remove and return every batch due on this tick.
    pub fn take_due(&mut self, tick: u32) -> ArrayVec<PendingGarbage, MAX_PENDING> {
        let mut due = ArrayVec::new();
        self.pending.retain(|g| {
            if g.apply_at_tick <= tick {
                // `retain` visits in order, so due batches keep their arrival order.
                let _ = due.try_push(*g);
                false
            } else {
                true
            }
        });
        due
    }

    /// Offset incoming garbage with outgoing attack, soonest-arriving first.
    ///
    /// Returns whatever attack is left over once the queue is exhausted, which is what
    /// actually reaches the opponent. Cancelling the soonest batch first is what makes
    /// a well-timed clear feel like a defensive move: it removes the rows that were
    /// about to land, not ones several seconds away.
    pub fn cancel(&mut self, attack: u32) -> u32 {
        let mut remaining = attack;
        while remaining > 0 && !self.pending.is_empty() {
            let first = &mut self.pending[0];
            if first.amount as u32 > remaining {
                first.amount -= remaining as u8;
                return 0;
            }
            remaining -= first.amount as u32;
            self.pending.remove(0);
        }
        remaining
    }

    /// Drop everything. Used when a game ends.
    pub fn clear(&mut self) {
        self.pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(tick: u32, amount: u8) -> PendingGarbage {
        PendingGarbage {
            apply_at_tick: tick,
            amount,
            hole_col: 3,
        }
    }

    #[test]
    fn a_new_queue_is_empty() {
        let q = GarbageQueue::new();
        assert!(q.is_empty());
        assert_eq!(q.total(), 0);
    }

    #[test]
    fn nothing_is_due_before_its_tick() {
        let mut q = GarbageQueue::new();
        q.schedule(g(100, 4));
        assert!(q.take_due(99).is_empty());
        assert_eq!(q.total(), 4, "must still be waiting");
    }

    #[test]
    fn a_batch_lands_on_exactly_its_tick() {
        let mut q = GarbageQueue::new();
        q.schedule(g(100, 4));
        let due = q.take_due(100);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].amount, 4);
        assert!(q.is_empty());
    }

    #[test]
    fn a_batch_whose_tick_was_missed_still_lands() {
        // Both peers must apply the same rows even if one of them stalled, or their
        // boards diverge permanently.
        let mut q = GarbageQueue::new();
        q.schedule(g(100, 4));
        assert_eq!(q.take_due(500).len(), 1);
    }

    #[test]
    fn batches_arrive_in_scheduled_order_however_they_were_queued() {
        let mut q = GarbageQueue::new();
        q.schedule(g(300, 3));
        q.schedule(g(100, 1));
        q.schedule(g(200, 2));
        let due = q.take_due(1000);
        let amounts: Vec<u8> = due.iter().map(|d| d.amount).collect();
        assert_eq!(amounts, [1, 2, 3], "order must follow arrival tick");
    }

    #[test]
    fn a_zero_row_batch_is_ignored() {
        let mut q = GarbageQueue::new();
        q.schedule(g(100, 0));
        assert!(q.is_empty());
    }

    #[test]
    fn attack_cancels_the_soonest_batch_first() {
        // Cancelling the batch about to land is what makes a well-timed clear read as
        // a defensive move rather than an arbitrary bookkeeping change.
        let mut q = GarbageQueue::new();
        q.schedule(g(100, 4));
        q.schedule(g(200, 4));
        assert_eq!(q.cancel(4), 0);
        assert_eq!(q.len(), 1);
        assert_eq!(q.iter().next().unwrap().apply_at_tick, 200);
    }

    #[test]
    fn partial_cancellation_leaves_the_rest_of_a_batch() {
        let mut q = GarbageQueue::new();
        q.schedule(g(100, 5));
        assert_eq!(q.cancel(2), 0);
        assert_eq!(q.total(), 3);
    }

    #[test]
    fn attack_beyond_the_queue_passes_through_to_the_opponent() {
        let mut q = GarbageQueue::new();
        q.schedule(g(100, 2));
        assert_eq!(q.cancel(6), 4, "four rows should get through");
        assert!(q.is_empty());
    }

    #[test]
    fn cancelling_against_an_empty_queue_sends_everything() {
        let mut q = GarbageQueue::new();
        assert_eq!(q.cancel(7), 7);
    }

    #[test]
    fn cancelling_nothing_changes_nothing() {
        let mut q = GarbageQueue::new();
        q.schedule(g(100, 4));
        assert_eq!(q.cancel(0), 0);
        assert_eq!(q.total(), 4);
    }

    #[test]
    fn the_queue_refuses_to_grow_past_its_cap() {
        // Unbounded growth would be a way to exhaust memory from across a network. The
        // cap is far above anything reachable in play.
        let mut q = GarbageQueue::new();
        for i in 0..(MAX_PENDING as u32 * 3) {
            q.schedule(g(i, 1));
        }
        assert_eq!(q.len(), MAX_PENDING);
    }

    #[test]
    fn taking_due_batches_never_loses_rows() {
        let mut q = GarbageQueue::new();
        for i in 1..=10u32 {
            q.schedule(g(i * 10, 2));
        }
        let before = q.total();
        let mut landed: u32 = 0;
        for t in 0..=100 {
            landed += q.take_due(t).iter().map(|d| d.amount as u32).sum::<u32>();
        }
        assert_eq!(landed, before);
        assert!(q.is_empty());
    }

    #[test]
    fn clearing_drops_everything() {
        let mut q = GarbageQueue::new();
        q.schedule(g(100, 4));
        q.clear();
        assert!(q.is_empty());
    }
}
