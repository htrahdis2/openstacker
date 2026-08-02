import { describe, expect, it } from "vitest";
import {
  URGENT_TICKS,
  apm,
  banner,
  formatApm,
  formatPps,
  formatTime,
  garbageFill,
  garbageSegments,
  pps,
  seconds,
} from "./hud";
import { EVENT } from "../sim/frame";
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

describe("the incoming bar", () => {
  const batches = [
    { rows: 4, inTicks: 20 },
    { rows: 2, inTicks: 200 },
  ];

  it("draws a segment per batch rather than one total", () => {
    // Four rows landing now and four landing in two seconds are not the same thing to
    // the player holding the board.
    const segments = garbageSegments(batches, 8);
    expect(segments).toHaveLength(2);
    expect(segments[0]!.rows).toBe(4);
    expect(segments[0]!.fraction).toBeCloseTo(0.5);
    expect(segments[1]!.fraction).toBeCloseTo(0.25);
  });

  it("marks the batch that is about to land", () => {
    const segments = garbageSegments(batches, 8);
    expect(segments[0]!.urgent).toBe(true);
    expect(segments[1]!.urgent).toBe(false);
    expect(garbageSegments([{ rows: 1, inTicks: URGENT_TICKS }], 8)[0]!.urgent).toBe(true);
  });

  it("scales to the rules being played, not to a number picked here", () => {
    expect(garbageSegments([{ rows: 4, inTicks: 5 }], 4)[0]!.fraction).toBe(1);
    expect(garbageSegments([{ rows: 4, inTicks: 5 }], 16)[0]!.fraction).toBeCloseTo(0.25);
  });

  it("never draws a batch taller than the bar", () => {
    expect(garbageSegments([{ rows: 40, inTicks: 5 }], 8)[0]!.fraction).toBe(1);
  });

  it("is empty when nothing is coming", () => {
    expect(garbageSegments([], 8)).toEqual([]);
    expect(garbageFill(0)).toBe(0);
  });
});

describe("attack per minute", () => {
  it("is zero before any time has passed", () => {
    expect(apm(0, 0)).toBe(0);
    expect(formatApm(4, 0)).toBe("0.0");
  });

  it("counts against game time, not the wall clock", () => {
    // One minute is 3600 ticks, whatever the display did in between.
    expect(apm(30, 3600)).toBeCloseTo(30);
    expect(apm(30, 1800)).toBeCloseTo(60);
  });
});

describe("what the last clear was", () => {
  const frame = (events: number, linesCleared: number): Parameters<typeof banner>[0] =>
    ({ events, linesCleared }) as Parameters<typeof banner>[0];

  it("names a spin by the rows it took", () => {
    expect(banner(frame(EVENT.LINES_CLEARED | EVENT.SPIN, 2))).toBe("spin double");
    expect(banner(frame(EVENT.LINES_CLEARED | EVENT.MINI_SPIN, 1))).toBe("mini spin single");
  });

  it("puts a perfect clear above everything else", () => {
    const everything = EVENT.LINES_CLEARED | EVENT.SPIN | EVENT.PERFECT_CLEAR;
    expect(banner(frame(everything, 4))).toBe("perfect clear");
  });

  it("says nothing about an ordinary clear", () => {
    expect(banner(frame(EVENT.LINES_CLEARED, 1))).toBeNull();
    expect(banner(frame(EVENT.LINES_CLEARED, 3))).toBeNull();
    expect(banner(frame(EVENT.PIECE_LOCKED, 0))).toBeNull();
  });

  it("calls a quad a quad", () => {
    expect(banner(frame(EVENT.LINES_CLEARED, 4))).toBe("quad");
  });
});
