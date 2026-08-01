import { describe, expect, it } from "vitest";
import { formatPps, formatTime, garbageFill, pps, seconds } from "./hud";
import { bounds, centreOffset, type Shape } from "./piece";

const O: Shape = [
  [0, 0],
  [1, 0],
  [0, 1],
  [1, 1],
];
const I: Shape = [
  [0, 1],
  [1, 1],
  [2, 1],
  [3, 1],
];

describe("time", () => {
  it("is derived from the tick counter, not a clock", () => {
    expect(seconds(60)).toBe(1);
    expect(seconds(0)).toBe(0);
    expect(seconds(90)).toBe(1.5);
  });

  it("reads as minutes, seconds and hundredths", () => {
    expect(formatTime(0)).toBe("0:00.00");
    expect(formatTime(60)).toBe("0:01.00");
    expect(formatTime(60 * 60)).toBe("1:00.00");
    expect(formatTime(60 * 95)).toBe("1:35.00");
  });

  it("keeps the seconds field two digits wide", () => {
    // Otherwise a sprint clock jumps between "1:5.00" and "1:05.00" as it counts.
    expect(formatTime(60 * 65)).toBe("1:05.00");
  });
});

describe("pps", () => {
  it("is zero before any time has passed", () => {
    expect(pps(0, 0)).toBe(0);
    expect(pps(5, 0)).toBe(0);
    expect(formatPps(0, 0)).toBe("0.00");
  });

  it("counts pieces against elapsed game time", () => {
    expect(pps(60, 60 * 60)).toBe(1);
    expect(pps(40, 60 * 20)).toBe(2);
    expect(formatPps(40, 60 * 20)).toBe("2.00");
  });
});

describe("garbageFill", () => {
  it("is empty with nothing incoming", () => {
    expect(garbageFill(0)).toBe(0);
  });

  it("fills with the rows waiting", () => {
    expect(garbageFill(6, 12)).toBe(0.5);
  });

  it("never overflows its bar", () => {
    expect(garbageFill(400, 12)).toBe(1);
  });
});

describe("piece boxes", () => {
  it("measures a shape's extent", () => {
    expect(bounds(O)).toEqual({ minX: 0, minY: 0, width: 2, height: 2 });
    expect(bounds(I)).toEqual({ minX: 0, minY: 1, width: 4, height: 1 });
  });

  it("centres a shape in its box", () => {
    expect(centreOffset(O, 4, 3)).toEqual([1, 0.5]);
  });

  it("centres a shape whose cells do not start at the origin", () => {
    // The I piece sits on row 1 of its box. Centring has to account for that, or it hangs
    // one row low in the next queue while every other piece sits straight.
    expect(centreOffset(I, 4, 3)).toEqual([0, 0]);
  });
});
