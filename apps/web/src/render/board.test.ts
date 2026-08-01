import { describe, expect, it } from "vitest";
import { BUFFER_ROWS, TOP_ROW, geometry } from "./board";

describe("geometry", () => {
  it("does not depend on anything that moves", () => {
    // A viewport that opened around a spawning piece resized the board on every piece,
    // because pieces spawn in the buffer. Geometry is a function of the cell size alone.
    const a = geometry(30);
    const b = geometry(30);
    expect(a).toEqual(b);
  });

  it("shows the whole visible field plus a band of buffer", () => {
    const geo = geometry(30);
    expect(geo.topRow).toBe(TOP_ROW);
    expect(geo.height).toBe((40 - TOP_ROW) * 30);
    expect(geo.width).toBe(10 * 30);
  });

  it("keeps enough buffer on screen for a spawning piece", () => {
    // Pieces spawn across rows 18 and 19. Fewer than two buffer rows and a new piece is
    // invisible until it falls into the field.
    expect(BUFFER_ROWS).toBeGreaterThanOrEqual(2);
    expect(TOP_ROW).toBeLessThanOrEqual(18);
  });

  it("scales with the cell size and nothing else", () => {
    expect(geometry(20).height * 1.5).toBe(geometry(30).height);
    expect(geometry(20).topRow).toBe(geometry(30).topRow);
  });
});
