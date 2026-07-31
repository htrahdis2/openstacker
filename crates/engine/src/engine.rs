//! The simulation.
//!
//! One struct, one mutator. `tick` takes the buttons held during a frame and advances
//! everything by exactly one step. Nothing else changes state, and nothing consults a
//! clock, a file, or a random source outside the seeded generator, which is what makes a
//! run reproducible from `(seed, config, handling, inputs)` alone.

use crate::bag::Bag;
use crate::board::Board;
use crate::config::desc::Tunable;
use crate::config::handling::{Handling, HandlingSub, IrsMode};
use crate::config::match_config::{LockResetMode, MatchConfig};
use crate::consts::{BOARD_H, BOARD_W, SUBTICK, VISIBLE_H};
use crate::events::{Events, Stats, TickResult};
use crate::garbage::{GarbageQueue, PendingGarbage};
use crate::input::{Buttons, Dir};
use crate::kick::rotate;
use crate::piece::Piece;
use crate::quad::{QuadKind, Rot};
use crate::scoring::{Spin, attack_for, continues_b2b, detect_spin};

/// What the simulation is currently doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Waiting out the delay before a piece appears.
    Spawning,
    /// A piece is falling freely.
    Falling,
    /// A piece is resting and its lock timer is running.
    Locking,
    /// Waiting out the pause after rows cleared.
    ClearDelay,
    /// The game is over. Ticking does nothing.
    Dead,
}

impl Phase {
    const fn code(self) -> u8 {
        match self {
            Phase::Spawning => 0,
            Phase::Falling => 1,
            Phase::Locking => 2,
            Phase::ClearDelay => 3,
            Phase::Dead => 4,
        }
    }
}

/// A single player's game.
#[derive(Debug, Clone)]
pub struct Engine {
    board: Board,
    active: Piece,
    bag: Bag,
    hold: Option<QuadKind>,
    can_hold: bool,

    garbage: GarbageQueue,

    tick: u32,
    grav_sub: u32,
    das_sub: u32,
    arr_sub: u32,
    lock_sub: u32,
    delay_sub: u32,
    misdrop_sub: u32,
    lock_resets: u8,
    dir: Option<Dir>,

    combo: u8,
    b2b: u8,
    last_kick_index: u8,
    last_action_was_rotation: bool,

    phase: Phase,
    prev_buttons: Buttons,
    pending_irs: Option<Rot>,
    pending_ihs: bool,

    stats: Stats,
    config: MatchConfig,
    handling: HandlingSub,
}

impl Engine {
    /// Start a game.
    ///
    /// Handling is converted to subticks here, once, after being clamped. Two peers
    /// given the same arguments produce identical engines.
    pub fn new(seed: u64, config: &MatchConfig, handling: &Handling) -> Self {
        let mut config = config.clone();
        config.clamp();

        let mut engine = Engine {
            board: Board::new(),
            active: Piece::new(QuadKind::I, Rot::R0, 0, 0),
            bag: Bag::new(seed),
            hold: None,
            can_hold: true,
            garbage: GarbageQueue::new(),
            tick: 0,
            grav_sub: 0,
            das_sub: 0,
            arr_sub: 0,
            lock_sub: 0,
            delay_sub: 0,
            misdrop_sub: 0,
            lock_resets: 0,
            dir: None,
            combo: 0,
            b2b: 0,
            last_kick_index: 0,
            last_action_was_rotation: false,
            phase: Phase::Spawning,
            prev_buttons: Buttons::empty(),
            pending_irs: None,
            pending_ihs: false,
            stats: Stats::default(),
            handling: handling.to_subticks(),
            config,
        };
        // Spawn through the normal path rather than placing a piece by hand, so there
        // is exactly one definition of what a spawn does. Placing it here and leaving
        // the phase at Spawning would make the first tick discard it and draw again.
        let mut discard = TickResult::default();
        engine.spawn_step(Buttons::empty(), Buttons::empty(), &mut discard);
        engine
    }

    // ---- accessors ---------------------------------------------------------

    pub fn board(&self) -> &Board {
        &self.board
    }
    pub fn active(&self) -> Piece {
        self.active
    }
    pub fn hold(&self) -> Option<QuadKind> {
        self.hold
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn stats(&self) -> Stats {
        self.stats
    }
    pub fn config(&self) -> &MatchConfig {
        &self.config
    }
    pub fn is_over(&self) -> bool {
        self.phase == Phase::Dead
    }
    pub fn preview(&self) -> &[QuadKind] {
        self.bag.preview(self.config.preview_len as usize)
    }
    pub fn pending_garbage(&self) -> &GarbageQueue {
        &self.garbage
    }
    /// Where the active piece would land. Render-only.
    pub fn ghost(&self) -> Piece {
        self.active.landed(&self.board)
    }

    /// Queue garbage for a future tick.
    ///
    /// The caller owns the timing. An arrival tick in the past would land immediately on
    /// whichever peer received it late and one tick later on the other, so callers must
    /// schedule far enough ahead to cover the worst latency they expect.
    pub fn schedule_garbage(&mut self, g: PendingGarbage) {
        self.garbage.schedule(g);
    }

    // ---- the tick ----------------------------------------------------------

    /// Advance one frame.
    ///
    /// The order of the stages below is part of the rules. Two peers that ran them in a
    /// different order would diverge even with identical inputs.
    pub fn tick(&mut self, input: Buttons) -> TickResult {
        if self.phase == Phase::Dead {
            return TickResult::default();
        }

        self.tick += 1;
        self.stats.tick = self.tick;
        let pressed = input.pressed_since(self.prev_buttons);

        let mut out = TickResult::default();
        self.apply_due_garbage(&mut out);

        if self.phase != Phase::Dead {
            match self.phase {
                Phase::Spawning => self.spawn_step(input, pressed, &mut out),
                Phase::ClearDelay => self.delay_step(&mut out),
                Phase::Falling | Phase::Locking => self.active_step(input, pressed, &mut out),
                Phase::Dead => {}
            }
        }

        self.prev_buttons = input;
        out
    }

    // ---- phases ------------------------------------------------------------

    fn spawn_step(&mut self, input: Buttons, pressed: Buttons, out: &mut TickResult) {
        if self.delay_sub > 0 {
            self.delay_sub = self.delay_sub.saturating_sub(SUBTICK);
            // Rotation and hold pressed during the pause are remembered so they can be
            // applied the instant the piece appears, which is what makes a fast player's
            // inputs feel like they were not thrown away.
            if self.handling.irs != IrsMode::Off {
                if pressed.contains(Buttons::CW) {
                    self.pending_irs = Some(Rot::R1);
                } else if pressed.contains(Buttons::CCW) {
                    self.pending_irs = Some(Rot::R3);
                } else if pressed.contains(Buttons::FLIP) {
                    self.pending_irs = Some(Rot::R2);
                }
            }
            if self.handling.ihs && pressed.contains(Buttons::HOLD) {
                self.pending_ihs = true;
            }
            return;
        }

        let mut kind = self.bag.take();

        // Initial hold: swap before the piece is placed, so a held button does not cost
        // the player a frame.
        let want_hold = self.pending_ihs
            || (self.handling.ihs
                && self.handling.irs != IrsMode::Off
                && input.contains(Buttons::HOLD));
        if want_hold && self.config.hold_enabled && self.can_hold {
            let swapped = self.hold.replace(kind);
            kind = match swapped {
                Some(k) => k,
                None => self.bag.take(),
            };
            self.can_hold = false;
            out.events |= Events::HELD;
        }
        self.pending_ihs = false;

        let mut piece = self.spawn_piece(kind);

        // Initial rotation: apply the remembered rotation, falling back to unrotated if
        // it does not fit rather than refusing to spawn.
        let irs = self.pending_irs.take().or_else(|| {
            if self.handling.irs == IrsMode::Hold {
                if input.contains(Buttons::CW) {
                    Some(Rot::R1)
                } else if input.contains(Buttons::CCW) {
                    Some(Rot::R3)
                } else if input.contains(Buttons::FLIP) {
                    Some(Rot::R2)
                } else {
                    None
                }
            } else {
                None
            }
        });
        if let Some(rot) = irs {
            let rotated = Piece { rot, ..piece };
            if rotated.fits(&self.board) {
                piece = rotated;
                out.events |= Events::ROTATED;
            }
        }

        if !piece.fits(&self.board) {
            // Block out: there is no room for the next piece.
            self.die(out);
            return;
        }

        self.active = piece;
        self.phase = Phase::Falling;
        self.grav_sub = 0;
        self.lock_sub = self.config.lock_delay_sub();
        self.lock_resets = 0;
        self.last_action_was_rotation = irs.is_some();
        self.last_kick_index = 0;
        self.misdrop_sub = self.handling.prevent_misdrop;
        self.stats.pieces += 1;
        out.events |= Events::SPAWNED;

        // A direction still held across a spawn keeps its charge, but auto-repeat is
        // suppressed briefly so the fresh piece does not immediately slide to the wall.
        if self.dir.is_some() && self.das_sub == 0 {
            self.arr_sub = 0;
            self.das_cut();
        }
    }

    fn das_cut(&mut self) {
        // Represented as a fresh DAS wait rather than a separate timer: the effect a
        // player feels is the same, and one timer cannot disagree with another.
        if self.handling.das_cut_delay > 0 {
            self.das_sub = self.handling.das_cut_delay;
        }
    }

    fn delay_step(&mut self, out: &mut TickResult) {
        self.delay_sub = self.delay_sub.saturating_sub(SUBTICK);
        if self.delay_sub == 0 {
            self.phase = Phase::Spawning;
            self.delay_sub = self.config.spawn_delay_sub();
            if self.delay_sub == 0 {
                // Spawn immediately rather than burning a frame on an empty pause.
                let pressed = Buttons::empty();
                self.spawn_step(self.prev_buttons, pressed, out);
            }
        }
    }

    fn active_step(&mut self, input: Buttons, pressed: Buttons, out: &mut TickResult) {
        self.horizontal(input, out);
        self.rotation(pressed, out);
        self.hold_swap(pressed, out);

        if self.phase == Phase::Dead || self.phase == Phase::Spawning {
            return;
        }

        if self.misdrop_sub > 0 {
            self.misdrop_sub = self.misdrop_sub.saturating_sub(SUBTICK);
        }

        if pressed.contains(Buttons::HARD_DROP) && self.misdrop_sub == 0 {
            let dropped = self.active.landed(&self.board);
            if dropped != self.active {
                self.active = dropped;
                self.last_action_was_rotation = false;
                out.events |= Events::HARD_DROPPED;
            }
            self.lock(out);
            return;
        }

        self.gravity(input, out);

        if self.active.is_grounded(&self.board) {
            if self.phase == Phase::Falling {
                self.phase = Phase::Locking;
                self.lock_sub = self.config.lock_delay_sub();
            }
            if self.handling.soft_drop_lock && input.contains(Buttons::SOFT_DROP) {
                self.lock(out);
                return;
            }
            self.lock_sub = self.lock_sub.saturating_sub(SUBTICK);
            if self.lock_sub == 0 {
                self.lock(out);
            }
        } else {
            self.phase = Phase::Falling;
        }
    }

    // ---- movement ----------------------------------------------------------

    /// Turn held directions into column movements.
    ///
    /// The direction most recently pressed wins, so a player rolling from one to the
    /// other never gets a frame of neither.
    fn horizontal(&mut self, input: Buttons, out: &mut TickResult) {
        let left = input.contains(Buttons::LEFT);
        let right = input.contains(Buttons::RIGHT);
        let prev_left = self.prev_buttons.contains(Buttons::LEFT);
        let prev_right = self.prev_buttons.contains(Buttons::RIGHT);

        let want = match (left, right) {
            (true, false) => Some(Dir::Left),
            (false, true) => Some(Dir::Right),
            (true, true) => {
                // Both held: whichever arrived this tick takes over, otherwise keep the
                // one already charging.
                if left && !prev_left {
                    Some(Dir::Left)
                } else if right && !prev_right {
                    Some(Dir::Right)
                } else {
                    self.dir
                }
            }
            (false, false) => None,
        };

        let Some(dir) = want else {
            self.dir = None;
            self.das_sub = 0;
            self.arr_sub = 0;
            return;
        };

        if self.dir != Some(dir) {
            // A new direction, whether from nothing or a reversal.
            let changing = self.dir.is_some();
            self.dir = Some(dir);
            self.das_sub = if changing && self.handling.dcd > 0 {
                self.handling.dcd
            } else {
                self.handling.das
            };
            self.arr_sub = 0;
            self.step(dir, out);
            return;
        }

        if self.das_sub > 0 {
            self.das_sub = self.das_sub.saturating_sub(SUBTICK);
            return;
        }

        if self.handling.arr == 0 {
            // Instant repeat: travel until something stops the piece.
            while self.step(dir, out) {}
            return;
        }

        self.arr_sub += SUBTICK;
        while self.arr_sub >= self.handling.arr {
            self.arr_sub -= self.handling.arr;
            if !self.step(dir, out) {
                self.arr_sub = 0;
                break;
            }
        }
    }

    /// Move one column. Returns whether it happened.
    fn step(&mut self, dir: Dir, out: &mut TickResult) -> bool {
        match self.active.moved(dir.dx(), 0, &self.board) {
            Some(moved) => {
                self.active = moved;
                self.last_action_was_rotation = false;
                out.events |= Events::MOVED;
                self.on_successful_move(false);
                true
            }
            None => false,
        }
    }

    fn rotation(&mut self, pressed: Buttons, out: &mut TickResult) {
        let to = if pressed.contains(Buttons::CW) {
            self.active.rot.cw()
        } else if pressed.contains(Buttons::CCW) {
            self.active.rot.ccw()
        } else if pressed.contains(Buttons::FLIP) {
            self.active.rot.flip()
        } else {
            return;
        };

        if let Some(kicked) = rotate(&self.active, to, &self.board) {
            self.active = kicked.piece;
            self.last_kick_index = kicked.kick_index;
            self.last_action_was_rotation = true;
            out.events |= Events::ROTATED;
            self.on_successful_move(false);
        }
    }

    fn hold_swap(&mut self, pressed: Buttons, out: &mut TickResult) {
        if !pressed.contains(Buttons::HOLD) || !self.config.hold_enabled || !self.can_hold {
            return;
        }
        let current = self.active.kind;
        let next = match self.hold.replace(current) {
            Some(k) => k,
            None => self.bag.take(),
        };
        self.can_hold = false;

        let piece = self.spawn_piece(next);
        if !piece.fits(&self.board) {
            self.die(out);
            return;
        }
        self.active = piece;
        self.phase = Phase::Falling;
        self.grav_sub = 0;
        self.lock_sub = self.config.lock_delay_sub();
        self.lock_resets = 0;
        self.last_action_was_rotation = false;
        out.events |= Events::HELD;
    }

    /// Restart lock delay if the configured reset rule allows it.
    fn on_successful_move(&mut self, descended: bool) {
        if self.phase != Phase::Locking {
            return;
        }
        match self.config.lock_reset_mode {
            LockResetMode::Classic => {
                if descended {
                    self.lock_sub = self.config.lock_delay_sub();
                }
            }
            LockResetMode::Extended => {
                if self.lock_resets < self.config.lock_reset_cap {
                    self.lock_resets += 1;
                    self.lock_sub = self.config.lock_delay_sub();
                }
            }
            LockResetMode::Infinite => {
                self.lock_sub = self.config.lock_delay_sub();
            }
        }
    }

    fn gravity(&mut self, input: Buttons, out: &mut TickResult) {
        let soft = input.contains(Buttons::SOFT_DROP);
        let natural = self.config.gravity.threshold_at(self.tick);
        let soft_threshold = if soft {
            match self.handling.sdf_per_row {
                0 => None,
                n => Some(n),
            }
        } else {
            Some(u32::MAX)
        };

        // Whichever is faster wins. `None` means instant on either side.
        let threshold = match (natural, soft, soft_threshold) {
            (None, _, _) => None,
            (_, true, None) => None,
            (Some(g), true, Some(s)) => Some(g.min(s)),
            (Some(g), _, _) => Some(g),
        };

        let mut fell = 0;
        match threshold {
            None => {
                while self.descend() {
                    fell += 1;
                }
            }
            Some(t) => {
                self.grav_sub = self.grav_sub.saturating_add(SUBTICK);
                while self.grav_sub >= t {
                    self.grav_sub -= t;
                    if !self.descend() {
                        self.grav_sub = 0;
                        break;
                    }
                    fell += 1;
                }
            }
        }

        if fell > 0 {
            self.last_action_was_rotation = false;
            if soft {
                out.events |= Events::SOFT_DROPPED;
            }
        }
    }

    fn descend(&mut self) -> bool {
        match self.active.moved(0, 1, &self.board) {
            Some(moved) => {
                self.active = moved;
                self.on_successful_move(true);
                true
            }
            None => false,
        }
    }

    // ---- locking -----------------------------------------------------------

    fn lock(&mut self, out: &mut TickResult) {
        let spin = detect_spin(
            &self.active,
            &self.board,
            self.config.spin_detection,
            self.last_action_was_rotation,
            self.last_kick_index,
        );

        let mut above_visible = true;
        for (x, y) in self.active.cells() {
            if y >= 0 && (y as usize) < BOARD_H && (0..BOARD_W as i32).contains(&x) {
                self.board
                    .set(x as usize, y as usize, self.active.kind.color());
                if (y as usize) >= VISIBLE_H {
                    above_visible = false;
                }
            }
        }
        out.events |= Events::PIECE_LOCKED;

        let lines = self.board.clear_full_rows();
        out.lines_cleared = lines;
        self.stats.lines += lines as u32;

        if lines > 0 {
            out.events |= Events::LINES_CLEARED;
            match spin {
                Spin::Full => out.events |= Events::SPIN,
                Spin::Mini => out.events |= Events::MINI_SPIN,
                Spin::None => {}
            }

            let perfect = self.board.is_empty();
            if perfect {
                out.events |= Events::PERFECT_CLEAR;
            }

            let b2b_active = self.b2b > 0;
            if continues_b2b(lines, spin) {
                self.b2b = self.b2b.saturating_add(1);
                if b2b_active {
                    out.events |= Events::B2B_CONTINUED;
                }
            } else {
                if b2b_active {
                    out.events |= Events::B2B_BROKEN;
                }
                self.b2b = 0;
            }

            let raw = attack_for(lines, spin, perfect, b2b_active, self.combo, &self.config);
            self.combo = self.combo.saturating_add(1);
            self.stats.max_combo = self.stats.max_combo.max(self.combo);
            self.stats.max_b2b = self.stats.max_b2b.max(self.b2b);

            // Incoming rows are answered before anything is sent on, so a clear that
            // exactly matches what is coming cancels it rather than trading blows.
            let sent = self.garbage.cancel(raw as u32);
            out.attack = sent.min(u8::MAX as u32) as u8;
            self.stats.attack_sent += sent;
        } else {
            self.combo = 0;
            if above_visible {
                // Lock out: the whole piece came to rest above the visible field.
                self.die(out);
                return;
            }
        }

        self.can_hold = true;
        self.last_action_was_rotation = false;
        self.last_kick_index = 0;

        self.delay_sub = if lines > 0 {
            self.config.clear_delay_sub()
        } else {
            0
        };
        if self.delay_sub > 0 {
            self.phase = Phase::ClearDelay;
        } else {
            self.phase = Phase::Spawning;
            self.delay_sub = self.config.spawn_delay_sub();
            if self.delay_sub == 0 {
                self.spawn_step(self.prev_buttons, Buttons::empty(), out);
            }
        }
    }

    fn die(&mut self, out: &mut TickResult) {
        self.phase = Phase::Dead;
        out.events |= Events::TOPPED_OUT;
    }

    // ---- garbage -----------------------------------------------------------

    fn apply_due_garbage(&mut self, out: &mut TickResult) {
        let due = self.garbage.take_due(self.tick);
        if due.is_empty() {
            return;
        }
        for g in due {
            let amount = (g.amount as usize).min(self.config.garbage_cap as usize);
            if amount == 0 {
                continue;
            }
            let overflow = self.board.push_garbage(amount, g.hole_col as usize);
            self.stats.garbage_received += amount as u32;
            out.events |= Events::GARBAGE_APPLIED;

            // The stack rose, so the active piece rises with it and keeps its position
            // relative to the board rather than being swallowed.
            self.active.y -= amount as i8;

            if overflow || !self.active.fits(&self.board) {
                self.die(out);
                return;
            }
        }
    }

    // ---- helpers -----------------------------------------------------------

    /// Place a kind at its spawn position: centred, just above the visible field.
    fn spawn_piece(&self, kind: QuadKind) -> Piece {
        Piece::new(kind, Rot::R0, 3, VISIBLE_H as i8 - 2)
    }

    // ---- checksum ----------------------------------------------------------

    /// A fingerprint of everything the rules depend on.
    ///
    /// Two peers comparing this catch a disagreement the moment it appears, rather than
    /// several seconds later when the boards visibly differ and the cause is gone.
    ///
    /// The render channel is excluded on purpose: colors cannot affect play, and
    /// including them would let a cosmetic difference read as a desync.
    pub fn checksum(&self) -> u64 {
        let mut h = Fnv::new();
        for row in self.board.rows() {
            h.write_u16(*row);
        }
        h.write_u8(self.active.kind as u8);
        h.write_u8(self.active.rot as u8);
        h.write_i8(self.active.x);
        h.write_i8(self.active.y);
        h.write_u32(self.grav_sub);
        h.write_u8(self.hold.map_or(0xFF, |k| k as u8));
        h.write_u8(self.can_hold as u8);
        h.write_u8(self.bag.queue().len() as u8);
        for k in self.bag.queue() {
            h.write_u8(*k as u8);
        }
        h.write_u64(self.bag.rng_state());
        h.write_u8(self.garbage.len() as u8);
        for g in self.garbage.iter() {
            h.write_u32(g.apply_at_tick);
            h.write_u8(g.amount);
            h.write_u8(g.hole_col);
        }
        h.write_u32(self.tick);
        h.write_u8(self.combo);
        h.write_u8(self.b2b);
        h.write_u8(self.lock_resets);
        h.write_u8(self.phase.code());
        h.write_u32(self.das_sub);
        h.write_u32(self.arr_sub);
        h.write_u32(self.lock_sub);
        h.write_u32(self.delay_sub);
        h.write_u32(self.misdrop_sub);
        h.write_u8(Dir::checksum_code(self.dir));
        h.finish()
    }
}

/// FNV-1a, 64-bit.
///
/// Every value is fed in little-endian, explicitly, so that the result cannot depend on
/// the byte order or pointer width of whatever it is compiled for.
struct Fnv(u64);

impl Fnv {
    const fn new() -> Self {
        Fnv(0xCBF2_9CE4_8422_2325)
    }
    #[inline]
    fn write_u8(&mut self, v: u8) {
        self.0 ^= v as u64;
        self.0 = self.0.wrapping_mul(0x0000_0100_0000_01B3);
    }
    #[inline]
    fn write_i8(&mut self, v: i8) {
        self.write_u8(v as u8);
    }
    #[inline]
    fn write_u16(&mut self, v: u16) {
        for b in v.to_le_bytes() {
            self.write_u8(b);
        }
    }
    #[inline]
    fn write_u32(&mut self, v: u32) {
        for b in v.to_le_bytes() {
            self.write_u8(b);
        }
    }
    #[inline]
    fn write_u64(&mut self, v: u64) {
        for b in v.to_le_bytes() {
            self.write_u8(b);
        }
    }
    const fn finish(&self) -> u64 {
        self.0
    }
}
