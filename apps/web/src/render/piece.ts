/**
 * Drawing a piece outside the board, for the hold and next boxes.
 *
 * Shapes come from the simulation, so this only has to place them. Centring is done from
 * the shape's own extent rather than a per-kind table, which would be a second opinion
 * about what a piece looks like.
 */

import type { Skin } from "./palette";

export type Shape = [number, number][];
export type Shapes = Record<string, Shape>;

export interface Bounds {
  minX: number;
  minY: number;
  width: number;
  height: number;
}

export function bounds(shape: Shape): Bounds {
  const xs = shape.map(([x]) => x);
  const ys = shape.map(([, y]) => y);
  const minX = Math.min(...xs);
  const minY = Math.min(...ys);
  return {
    minX,
    minY,
    width: Math.max(...xs) - minX + 1,
    height: Math.max(...ys) - minY + 1,
  };
}

/** Top-left corner that centres a shape in a box, in cells. */
export function centreOffset(shape: Shape, boxW: number, boxH: number): [number, number] {
  const b = bounds(shape);
  return [(boxW - b.width) / 2 - b.minX, (boxH - b.height) / 2 - b.minY];
}

/** Draw one piece centred in a box of the given size, in pixels. */
export function drawPiece(
  ctx: CanvasRenderingContext2D,
  shape: Shape,
  kind: number,
  skin: Skin,
  box: { x: number; y: number; w: number; h: number },
  cell: number,
): void {
  const [ox, oy] = centreOffset(shape, box.w / cell, box.h / cell);
  ctx.fillStyle = skin.cells[kind] ?? skin.dim;
  for (const [x, y] of shape) {
    ctx.fillRect(
      box.x + (x + ox) * cell + 1,
      box.y + (y + oy) * cell + 1,
      cell - 2,
      cell - 2,
    );
  }
}
