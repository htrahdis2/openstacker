/**
 * Decoding the frame block.
 *
 * Offsets come from the module rather than being restated here, so the two sides cannot
 * drift. Everything is little-endian, as wasm memory is.
 */

export interface FrameLayout {
  bytes: number;
  maxPreview: number;
  offsets: Record<FrameField, number>;
  flags: { active: number; ghost: number; hold: number; over: number };
  board: { width: number; height: number; visible: number };
}

export type FrameField =
  | "tick"
  | "lines"
  | "pieces"
  | "attackSent"
  | "garbageReceived"
  | "events"
  | "attack"
  | "linesCleared"
  | "phase"
  | "flags"
  | "activeKind"
  | "holdKind"
  | "previewLen"
  | "maxCombo"
  | "maxB2b"
  | "pendingBatches"
  | "pendingRows"
  | "nextGarbageIn"
  | "activeCells"
  | "ghostCells"
  | "preview";

/** A cell in board coordinates: x from the left, y from the top of the buffer. */
export interface Cell {
  x: number;
  y: number;
}

/** One tick of the simulation, as the renderer sees it. */
export interface Frame {
  tick: number;
  lines: number;
  pieces: number;
  attackSent: number;
  garbageReceived: number;
  events: number;
  attack: number;
  linesCleared: number;
  phase: number;
  over: boolean;
  /** Null during spawn and clear delays, and after a topout. */
  active: Cell[] | null;
  activeKind: number;
  ghost: Cell[] | null;
  hold: number | null;
  preview: number[];
  maxCombo: number;
  maxB2b: number;
  pendingRows: number;
  pendingBatches: number;
  nextGarbageIn: number;
}

/** Events, matching the engine's bitset. */
export const EVENT = {
  PIECE_LOCKED: 1 << 0,
  LINES_CLEARED: 1 << 1,
  SPIN: 1 << 2,
  MINI_SPIN: 1 << 3,
  B2B_CONTINUED: 1 << 4,
  B2B_BROKEN: 1 << 5,
  PERFECT_CLEAR: 1 << 6,
  GARBAGE_APPLIED: 1 << 7,
  TOPPED_OUT: 1 << 8,
  HELD: 1 << 9,
  ROTATED: 1 << 10,
  MOVED: 1 << 11,
  HARD_DROPPED: 1 << 12,
  SOFT_DROPPED: 1 << 13,
  SPAWNED: 1 << 14,
} as const;

export function readFrame(view: DataView, layout: FrameLayout): Frame {
  const o = layout.offsets;
  const flags = view.getUint8(o.flags);
  const previewLen = view.getUint8(o.previewLen);

  const preview: number[] = [];
  for (let i = 0; i < Math.min(previewLen, layout.maxPreview); i++) {
    preview.push(view.getUint8(o.preview + i));
  }

  return {
    tick: view.getUint32(o.tick, true),
    lines: view.getUint32(o.lines, true),
    pieces: view.getUint32(o.pieces, true),
    attackSent: view.getUint32(o.attackSent, true),
    garbageReceived: view.getUint32(o.garbageReceived, true),
    events: view.getUint16(o.events, true),
    attack: view.getUint8(o.attack),
    linesCleared: view.getUint8(o.linesCleared),
    phase: view.getUint8(o.phase),
    over: (flags & layout.flags.over) !== 0,
    active: (flags & layout.flags.active) !== 0 ? readCells(view, o.activeCells) : null,
    activeKind: view.getUint8(o.activeKind),
    ghost: (flags & layout.flags.ghost) !== 0 ? readCells(view, o.ghostCells) : null,
    hold: (flags & layout.flags.hold) !== 0 ? view.getUint8(o.holdKind) : null,
    preview,
    maxCombo: view.getUint8(o.maxCombo),
    maxB2b: view.getUint8(o.maxB2b),
    pendingRows: view.getUint16(o.pendingRows, true),
    pendingBatches: view.getUint8(o.pendingBatches),
    nextGarbageIn: view.getUint16(o.nextGarbageIn, true),
  };
}

function readCells(view: DataView, at: number): Cell[] {
  const cells: Cell[] = [];
  for (let i = 0; i < 4; i++) {
    cells.push({ x: view.getInt8(at + i * 2), y: view.getInt8(at + i * 2 + 1) });
  }
  return cells;
}
