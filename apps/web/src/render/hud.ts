/**
 * The panel beside the board: hold, the next queue, and the run's numbers.
 *
 * Everything shown here is read from the frame block. Nothing is counted independently,
 * so the HUD cannot disagree with the game it is describing.
 */

import { EVENT, type Frame, type Incoming } from "../sim/frame";
import type { Skin } from "./palette";
import { drawPiece, type Shapes } from "./piece";

/** Seconds of game time a tick count represents. */
export function seconds(tick: number): number {
  return tick / 60;
}

/** `m:ss.cc`, the format a sprint time is read in. */
export function formatTime(tick: number): string {
  const total = seconds(tick);
  const minutes = Math.floor(total / 60);
  const rest = total - minutes * 60;
  return `${minutes}:${rest.toFixed(2).padStart(5, "0")}`;
}

/** Pieces per second, or 0 before any time has passed. */
export function pps(pieces: number, tick: number): number {
  const elapsed = seconds(tick);
  return elapsed > 0 ? pieces / elapsed : 0;
}

export function formatPps(pieces: number, tick: number): string {
  return pps(pieces, tick).toFixed(2);
}

const BOX_CELL = 18;
const BOX_COLS = 4;
const BOX_ROWS = 3;

export interface Boxes {
  hold: HTMLCanvasElement;
  next: HTMLCanvasElement;
}

/** Size the hold and next canvases for the current preview length. */
export function sizeBoxes(boxes: Boxes, previewLen: number): void {
  fit(boxes.hold, BOX_COLS * BOX_CELL, BOX_ROWS * BOX_CELL);
  fit(boxes.next, BOX_COLS * BOX_CELL, BOX_ROWS * BOX_CELL * Math.max(previewLen, 1));
}

export function drawHold(canvas: HTMLCanvasElement, frame: Frame, shapes: Shapes, skin: Skin): void {
  const ctx = context(canvas);
  const w = BOX_COLS * BOX_CELL;
  const h = BOX_ROWS * BOX_CELL;
  ctx.clearRect(0, 0, w, h);
  if (frame.hold === null) return;
  const shape = shapes[String(frame.hold)];
  if (shape) drawPiece(ctx, shape, frame.hold, skin, { x: 0, y: 0, w, h }, BOX_CELL);
}

export function drawNext(canvas: HTMLCanvasElement, frame: Frame, shapes: Shapes, skin: Skin): void {
  const ctx = context(canvas);
  const w = BOX_COLS * BOX_CELL;
  const h = BOX_ROWS * BOX_CELL;
  ctx.clearRect(0, 0, w, h * Math.max(frame.preview.length, 1));
  frame.preview.forEach((kind, i) => {
    const shape = shapes[String(kind)];
    if (shape) drawPiece(ctx, shape, kind, skin, { x: 0, y: i * h, w, h }, BOX_CELL);
  });
}

/** Attack per minute, or 0 before any time has passed. */
export function apm(attackSent: number, tick: number): number {
  const minutes = seconds(tick) / 60;
  return minutes > 0 ? attackSent / minutes : 0;
}

export function formatApm(attackSent: number, tick: number): string {
  return apm(attackSent, tick).toFixed(1);
}

/** Fill of the garbage bar, as a fraction. Empty until rows are incoming. */
export function garbageFill(pendingRows: number, capacity = 12): number {
  return Math.min(pendingRows / capacity, 1);
}

/** How close a batch is to landing, when a player should be reacting to it. */
export const URGENT_TICKS = 60;

/** One batch as the bar draws it. */
export interface Segment {
  rows: number;
  /** Share of the bar's height. */
  fraction: number;
  inTicks: number;
  /** About to land: worth reacting to now rather than in a moment. */
  urgent: boolean;
}

/**
 * The incoming queue as bar segments, soonest first.
 *
 * Drawn per batch rather than as one total because four rows landing now and four rows
 * landing in two seconds are not the same thing to the player holding the board.
 */
export function garbageSegments(
  incoming: Incoming[],
  capacity = 12,
  urgentTicks = URGENT_TICKS,
): Segment[] {
  const scale = Math.max(capacity, 1);
  return incoming.map((batch) => ({
    rows: batch.rows,
    fraction: Math.min(batch.rows / scale, 1),
    inTicks: batch.inTicks,
    urgent: batch.inTicks <= urgentTicks,
  }));
}

/**
 * What just happened, in a word, or null when it was an ordinary clear.
 *
 * Read from the tick's events rather than by comparing this frame to the last, which is
 * what the bitset is for.
 */
export function banner(frame: Frame): string | null {
  const rows = ["", "single", "double", "triple", "quad"][Math.min(frame.linesCleared, 4)] ?? "";
  if (frame.events & EVENT.PERFECT_CLEAR) return "perfect clear";
  if (frame.events & EVENT.SPIN) return `spin ${rows}`.trim();
  if (frame.events & EVENT.MINI_SPIN) return `mini spin ${rows}`.trim();
  if (frame.events & EVENT.LINES_CLEARED && frame.linesCleared >= 4) return rows;
  return null;
}

function fit(canvas: HTMLCanvasElement, w: number, h: number): void {
  const dpr = window.devicePixelRatio || 1;
  if (canvas.width !== w * dpr || canvas.height !== h * dpr) {
    canvas.width = w * dpr;
    canvas.height = h * dpr;
    canvas.style.width = `${w}px`;
    canvas.style.height = `${h}px`;
  }
  context(canvas).setTransform(dpr, 0, 0, dpr, 0, 0);
}

function context(canvas: HTMLCanvasElement): CanvasRenderingContext2D {
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("this browser has no 2d canvas");
  return ctx;
}
