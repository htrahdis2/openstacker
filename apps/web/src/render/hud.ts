/**
 * The panel beside the board: hold, the next queue, and the run's numbers.
 *
 * Everything shown here is read from the frame block. Nothing is counted independently,
 * so the HUD cannot disagree with the game it is describing.
 */

import type { Frame } from "../sim/frame";
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

/** Fill of the garbage bar, as a fraction. Empty until rows are incoming. */
export function garbageFill(pendingRows: number, capacity = 12): number {
  return Math.min(pendingRows / capacity, 1);
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
