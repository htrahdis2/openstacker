/**
 * Drawing the playfield.
 *
 * Rows 20..39 are the visible field and rows 0..19 are spawn buffer. A band of the buffer
 * is always drawn, so a stack that tops out is visible instead of ending for no apparent
 * reason, and so the board never changes size: pieces spawn in the buffer, and a viewport
 * that opened and closed around them would resize on every piece.
 */

import type { Frame } from "../sim/frame";
import type { Skin } from "./palette";

export interface BoardGeometry {
  cell: number;
  width: number;
  height: number;
  /** First row drawn. */
  topRow: number;
}

const BOARD_W = 10;
const BOARD_H = 40;
const VISIBLE_H = 20;

/**
 * Buffer rows kept on screen above the field.
 *
 * Pieces spawn across rows 18 and 19, so this has to cover at least two for a spawning
 * piece to be visible at all. The rest is headroom for seeing what topped a stack out.
 */
export const BUFFER_ROWS = 4;

/** First row drawn. Constant, so the board does not move while a game is running. */
export const TOP_ROW = VISIBLE_H - BUFFER_ROWS;

export function geometry(cell: number): BoardGeometry {
  return {
    cell,
    topRow: TOP_ROW,
    width: BOARD_W * cell,
    height: (BOARD_H - TOP_ROW) * cell,
  };
}

export interface DrawOptions {
  skin: Skin;
  ghostOpacity: number;
  showGrid: boolean;
}

export function drawBoard(
  ctx: CanvasRenderingContext2D,
  frame: Frame,
  occupancy: Uint16Array,
  colors: Uint8Array,
  geo: BoardGeometry,
  options: DrawOptions,
): void {
  const { cell, topRow: top } = geo;
  const { skin } = options;

  ctx.clearRect(0, 0, geo.width, geo.height);

  ctx.fillStyle = skin.background;
  ctx.fillRect(0, 0, geo.width, geo.height);

  // The buffer band is shaded and ruled off, so what is above the field reads as above it.
  const fieldTop = (VISIBLE_H - top) * cell;
  ctx.fillStyle = skin.grid;
  ctx.globalAlpha = 0.35;
  ctx.fillRect(0, 0, geo.width, fieldTop);
  ctx.globalAlpha = 1;

  if (options.showGrid) {
    ctx.strokeStyle = skin.grid;
    ctx.lineWidth = 1;
    ctx.beginPath();
    for (let x = 1; x < BOARD_W; x++) {
      ctx.moveTo(x * cell + 0.5, 0);
      ctx.lineTo(x * cell + 0.5, geo.height);
    }
    for (let y = 1; y < BOARD_H - top; y++) {
      ctx.moveTo(0, y * cell + 0.5);
      ctx.lineTo(geo.width, y * cell + 0.5);
    }
    ctx.stroke();
  }

  ctx.strokeStyle = skin.dim;
  ctx.globalAlpha = 0.5;
  ctx.beginPath();
  ctx.moveTo(0, fieldTop + 0.5);
  ctx.lineTo(geo.width, fieldTop + 0.5);
  ctx.stroke();
  ctx.globalAlpha = 1;

  for (let y = top; y < BOARD_H; y++) {
    const row = occupancy[y] ?? 0;
    if (row === 0) continue;
    for (let x = 0; x < BOARD_W; x++) {
      if ((row & (1 << x)) === 0) continue;
      fillCell(ctx, x, y - top, cell, skin.cells[colors[y * BOARD_W + x] ?? 0] ?? skin.dim);
    }
  }

  if (frame.ghost && options.ghostOpacity > 0) {
    ctx.globalAlpha = options.ghostOpacity / 100;
    for (const c of frame.ghost) {
      fillCell(ctx, c.x, c.y - top, cell, skin.cells[frame.activeKind] ?? skin.dim);
    }
    ctx.globalAlpha = 1;
  }

  if (frame.active) {
    for (const c of frame.active) {
      fillCell(ctx, c.x, c.y - top, cell, skin.cells[frame.activeKind] ?? skin.dim);
    }
  }
}

function fillCell(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  cell: number,
  color: string,
): void {
  if (y < 0) return;
  ctx.fillStyle = color;
  ctx.fillRect(x * cell + 1, y * cell + 1, cell - 2, cell - 2);
}
