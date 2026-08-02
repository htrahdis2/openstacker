//! Everything the renderer reads in one tick, in one fixed-size block.
//!
//! The block is written into wasm memory once per tick and read through a single view, so
//! drawing a frame costs one crossing rather than one per value. Nothing here is derived
//! from anything else in it: a client that recomputes a field is a client that can
//! disagree with the simulation.

use engine::{BOARD_H, Engine, Events, MAX_PREVIEW, Phase, Piece};

/// Size of the block. Fixed, so the view built at startup stays valid for the whole game.
///
/// Grown from 64 when the versus HUD needed the live combo and chain and the incoming
/// batches. The block is written once per tick and read through one view, so its size
/// costs nothing per frame; packing new fields into the spare bytes to avoid changing one
/// constant would have been a false economy.
pub const FRAME_BYTES: usize = 128;

/// How many incoming batches the block describes. The queue holds more; a bar that shows
/// more than this is a bar nobody can read.
pub const MAX_INCOMING: usize = 8;

/// A falling piece has been placed and is being drawn.
pub const FLAG_ACTIVE: u8 = 1 << 0;
/// The landing preview is available.
pub const FLAG_GHOST: u8 = 1 << 1;
/// Hold is occupied.
pub const FLAG_HOLD: u8 = 1 << 2;
/// The game is over.
pub const FLAG_OVER: u8 = 1 << 3;

/// The four cells of a piece, in board coordinates.
pub type Cells = [(i8, i8); 4];

/// Byte offsets within the block.
///
/// The client reads these from the module at startup instead of keeping its own copy, so
/// the layout is defined once.
pub mod offset {
    pub const TICK: usize = 0;
    pub const LINES: usize = 4;
    pub const PIECES: usize = 8;
    pub const ATTACK_SENT: usize = 12;
    pub const GARBAGE_RECEIVED: usize = 16;
    pub const EVENTS: usize = 20;
    pub const ATTACK: usize = 22;
    pub const LINES_CLEARED: usize = 23;
    pub const PHASE: usize = 24;
    pub const FLAGS: usize = 25;
    pub const ACTIVE_KIND: usize = 26;
    pub const HOLD_KIND: usize = 27;
    pub const PREVIEW_LEN: usize = 28;
    pub const MAX_COMBO: usize = 29;
    pub const MAX_B2B: usize = 30;
    pub const PENDING_BATCHES: usize = 31;
    pub const PENDING_ROWS: usize = 32;
    pub const NEXT_GARBAGE_IN: usize = 34;
    pub const ACTIVE_CELLS: usize = 36;
    pub const GHOST_CELLS: usize = 44;
    pub const PREVIEW: usize = 52;
    pub const COMBO: usize = 59;
    pub const B2B: usize = 60;
    pub const INCOMING_LEN: usize = 61;
    /// Three bytes per batch: rows, then ticks until it lands as a `u16`.
    pub const INCOMING: usize = 64;
}

/// Bytes per entry in the incoming list.
pub const INCOMING_STRIDE: usize = 3;

/// One tick's worth of state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Frame {
    pub tick: u32,
    pub lines: u32,
    pub pieces: u32,
    pub attack_sent: u32,
    pub garbage_received: u32,

    pub events: Events,
    /// Rows this tick's clear sends onward, after local cancellation.
    pub attack: u8,
    pub lines_cleared: u8,

    pub phase: u8,
    pub over: bool,

    /// `None` during a spawn delay, a clear delay, and after a topout.
    pub active: Option<Cells>,
    pub active_kind: u8,
    pub ghost: Option<Cells>,
    pub hold_kind: Option<u8>,

    pub preview: [u8; MAX_PREVIEW],
    pub preview_len: u8,

    pub max_combo: u8,
    pub max_b2b: u8,
    /// The runs in progress, as opposed to the high-water marks beside them.
    pub combo: u8,
    pub b2b: u8,

    /// Rows waiting to be pushed up, across every scheduled batch.
    pub pending_rows: u16,
    pub pending_batches: u8,
    /// Ticks until the next batch lands, or 0 when nothing is scheduled.
    pub next_garbage_in: u16,
    /// Each batch on its way, soonest first: rows, and ticks until it lands.
    ///
    /// The total alone cannot be drawn as separate batches, and four rows arriving now
    /// is not the same thing to a player as four rows arriving in two seconds.
    pub incoming: [(u8, u16); MAX_INCOMING],
    pub incoming_len: u8,
}

impl Frame {
    /// Take a reading of the simulation.
    pub fn read(engine: &Engine, last: engine::TickResult) -> Frame {
        let stats = engine.stats();
        let queue = engine.pending_garbage();

        let next_garbage_in = queue
            .iter()
            .map(|g| g.apply_at_tick.saturating_sub(stats.tick))
            .min()
            .unwrap_or(0);

        let mut incoming = [(0u8, 0u16); MAX_INCOMING];
        let mut batches: Vec<&engine::PendingGarbage> = queue.iter().collect();
        batches.sort_by_key(|g| g.apply_at_tick);
        for (slot, g) in incoming.iter_mut().zip(&batches) {
            *slot = (
                g.amount,
                g.apply_at_tick
                    .saturating_sub(stats.tick)
                    .min(u16::MAX as u32) as u16,
            );
        }

        let mut preview = [0u8; MAX_PREVIEW];
        let kinds = engine.preview();
        for (slot, kind) in preview.iter_mut().zip(kinds) {
            *slot = kind.color();
        }

        Frame {
            tick: stats.tick,
            lines: stats.lines,
            pieces: stats.pieces,
            attack_sent: stats.attack_sent,
            garbage_received: stats.garbage_received,

            events: last.events,
            attack: last.attack,
            lines_cleared: last.lines_cleared,

            phase: phase_code(engine.phase()),
            over: engine.is_over(),

            active: engine.active().map(cells),
            active_kind: engine.active().map_or(0, |p| p.kind.color()),
            ghost: engine.ghost().map(cells),
            hold_kind: engine.hold().map(|k| k.color()),

            preview,
            preview_len: kinds.len() as u8,

            max_combo: stats.max_combo,
            max_b2b: stats.max_b2b,
            combo: engine.combo(),
            b2b: engine.b2b(),

            pending_rows: queue.total().min(u16::MAX as u32) as u16,
            pending_batches: queue.len() as u8,
            next_garbage_in: next_garbage_in.min(u16::MAX as u32) as u16,
            incoming,
            incoming_len: batches.len().min(MAX_INCOMING) as u8,
        }
    }

    /// Serialize into the block, little-endian.
    pub fn write(&self, out: &mut [u8; FRAME_BYTES]) {
        use offset as o;
        out.fill(0);
        put(out, o::TICK, &self.tick.to_le_bytes());
        put(out, o::LINES, &self.lines.to_le_bytes());
        put(out, o::PIECES, &self.pieces.to_le_bytes());
        put(out, o::ATTACK_SENT, &self.attack_sent.to_le_bytes());
        put(
            out,
            o::GARBAGE_RECEIVED,
            &self.garbage_received.to_le_bytes(),
        );
        put(out, o::EVENTS, &self.events.bits().to_le_bytes());
        out[o::ATTACK] = self.attack;
        out[o::LINES_CLEARED] = self.lines_cleared;
        out[o::PHASE] = self.phase;
        out[o::FLAGS] = self.flags();
        out[o::ACTIVE_KIND] = self.active_kind;
        out[o::HOLD_KIND] = self.hold_kind.unwrap_or(0);
        out[o::PREVIEW_LEN] = self.preview_len;
        out[o::MAX_COMBO] = self.max_combo;
        out[o::MAX_B2B] = self.max_b2b;
        out[o::PENDING_BATCHES] = self.pending_batches;
        put(out, o::PENDING_ROWS, &self.pending_rows.to_le_bytes());
        put(out, o::NEXT_GARBAGE_IN, &self.next_garbage_in.to_le_bytes());
        write_cells(&mut out[o::ACTIVE_CELLS..o::ACTIVE_CELLS + 8], self.active);
        write_cells(&mut out[o::GHOST_CELLS..o::GHOST_CELLS + 8], self.ghost);
        put(out, o::PREVIEW, &self.preview);
        out[o::COMBO] = self.combo;
        out[o::B2B] = self.b2b;
        out[o::INCOMING_LEN] = self.incoming_len;
        for (i, (rows, ticks)) in self.incoming.iter().enumerate() {
            let at = o::INCOMING + i * INCOMING_STRIDE;
            out[at] = *rows;
            put(out, at + 1, &ticks.to_le_bytes());
        }
    }

    fn flags(&self) -> u8 {
        let mut f = 0;
        if self.active.is_some() {
            f |= FLAG_ACTIVE;
        }
        if self.ghost.is_some() {
            f |= FLAG_GHOST;
        }
        if self.hold_kind.is_some() {
            f |= FLAG_HOLD;
        }
        if self.over {
            f |= FLAG_OVER;
        }
        f
    }
}

fn put(out: &mut [u8; FRAME_BYTES], at: usize, bytes: &[u8]) {
    out[at..at + bytes.len()].copy_from_slice(bytes);
}

fn write_cells(out: &mut [u8], cells: Option<Cells>) {
    let Some(cells) = cells else { return };
    for (i, (x, y)) in cells.iter().enumerate() {
        out[i * 2] = *x as u8;
        out[i * 2 + 1] = *y as u8;
    }
}

/// Board coordinates of a piece's four cells.
///
/// A piece in play is always on the board, so every coordinate fits a byte.
fn cells(p: Piece) -> Cells {
    let mut out = [(0i8, 0i8); 4];
    for (slot, (x, y)) in out.iter_mut().zip(p.cells()) {
        debug_assert!(
            (0..BOARD_H as i32).contains(&y),
            "cell above or below the board"
        );
        *slot = (x as i8, y as i8);
    }
    out
}

const fn phase_code(phase: Phase) -> u8 {
    match phase {
        Phase::Spawning => 0,
        Phase::Falling => 1,
        Phase::Locking => 2,
        Phase::ClearDelay => 3,
        Phase::Dead => 4,
    }
}
