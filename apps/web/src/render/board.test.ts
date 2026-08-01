import { describe, expect, it } from "vitest";
import { topRow } from "./board";

/** A board with the given rows occupied. */
function occupancy(rows: number[]): Uint16Array {
  const board = new Uint16Array(40);
  for (const y of rows) board[y] = 0b11_1111_1111;
  return board;
}

describe("topRow", () => {
  it("shows the visible field when the buffer is empty", () => {
    expect(topRow(occupancy([30, 35, 39]), null)).toBe(20);
  });

  it("opens the buffer when the stack reaches into it", () => {
    // Otherwise a stack that tops out looks like it ended for no reason.
    expect(topRow(occupancy([17, 25, 39]), null)).toBe(17);
  });

  it("opens the buffer for a piece spawning above the field", () => {
    expect(topRow(occupancy([]), [{ x: 4, y: 18 }, { x: 5, y: 18 }, { x: 4, y: 19 }, { x: 5, y: 19 }])).toBe(18);
  });

  it("never scrolls past the top of the buffer", () => {
    expect(topRow(occupancy([0]), null)).toBe(0);
  });

  it("never hides part of the visible field", () => {
    expect(topRow(occupancy([]), [{ x: 0, y: 39 }, { x: 1, y: 39 }, { x: 2, y: 39 }, { x: 3, y: 39 }])).toBe(20);
  });
});
