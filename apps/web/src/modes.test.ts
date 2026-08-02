import { describe, expect, it } from "vitest";
import {
  MODES,
  type Goal,
  bestIsLongest,
  goalReached,
  isPlayable,
  mode,
  progress,
  remaining,
  ticksFor,
} from "./modes";

describe("the shipped modes", () => {
  it("are the ones on disk", () => {
    expect(MODES.map((m) => m.id)).toEqual(["blitz", "sprint40", "versus"]);
  });

  it("carry the rules to play them under", () => {
    const sprint = mode("sprint40");
    expect(sprint?.goal).toEqual({ type: "lines", count: 40 });
    expect(sprint?.config.preview_len).toBe(5);
    expect(sprint?.name).toBeTruthy();
  });

  it("say which ones this build can decide the end of", () => {
    expect(isPlayable(mode("sprint40")!.goal)).toBe(true);
    expect(isPlayable(mode("blitz")!.goal)).toBe(true);
    // Versus ends by topping out, which needs something to top you out. It has one now.
    expect(isPlayable(mode("versus")!.goal)).toBe(true);
    expect(isPlayable({ type: "score", target: 1 })).toBe(false);
  });

  it("give versus an opponent and the others none", () => {
    expect(mode("versus")?.sparring?.rows_max).toBeGreaterThan(0);
    expect(mode("sprint40")?.sparring).toBeUndefined();
  });
});

describe("personal bests", () => {
  it("run shortest-first in a race and longest-first in a survival run", () => {
    // A survival run ends when the player loses, so there is no finishing it and the
    // longest one wins. Sorting those the other way would rank the worst run best.
    expect(bestIsLongest({ type: "lines", count: 40 })).toBe(false);
    expect(bestIsLongest({ type: "time", ms: 120_000 })).toBe(false);
    expect(bestIsLongest({ type: "survival" })).toBe(true);
  });
});

describe("ticksFor", () => {
  it("converts a duration to whole ticks", () => {
    expect(ticksFor(1000)).toBe(60);
    expect(ticksFor(120_000)).toBe(7200);
    expect(ticksFor(0)).toBe(0);
  });
});

describe("goalReached", () => {
  const sprint: Goal = { type: "lines", count: 40 };
  const blitz: Goal = { type: "time", ms: 120_000 };

  it("ends a line goal on the line that meets it", () => {
    expect(goalReached(sprint, { lines: 39, tick: 100 })).toBe(false);
    expect(goalReached(sprint, { lines: 40, tick: 100 })).toBe(true);
  });

  it("ends a line goal that overshoots on the last clear", () => {
    // A quad taking 38 to 42 still finishes the run.
    expect(goalReached(sprint, { lines: 42, tick: 100 })).toBe(true);
  });

  it("ends a time goal on tick count, not on a clock", () => {
    expect(goalReached(blitz, { lines: 0, tick: 7199 })).toBe(false);
    expect(goalReached(blitz, { lines: 0, tick: 7200 })).toBe(true);
  });

  it("never ends a goal this build cannot decide", () => {
    expect(goalReached({ type: "survival" }, { lines: 999, tick: 999_999 })).toBe(false);
    expect(goalReached({ type: "score", target: 1 }, { lines: 999, tick: 999 })).toBe(false);
  });
});

describe("progress", () => {
  it("runs from nothing to full", () => {
    const sprint: Goal = { type: "lines", count: 40 };
    expect(progress(sprint, { lines: 0, tick: 0 })).toBe(0);
    expect(progress(sprint, { lines: 20, tick: 0 })).toBe(0.5);
    expect(progress(sprint, { lines: 40, tick: 0 })).toBe(1);
  });

  it("does not exceed full when a clear overshoots", () => {
    expect(progress({ type: "lines", count: 40 }, { lines: 44, tick: 0 })).toBe(1);
  });

  it("survives a goal of zero rather than dividing by it", () => {
    expect(progress({ type: "lines", count: 0 }, { lines: 0, tick: 0 })).toBe(1);
    expect(progress({ type: "time", ms: 0 }, { lines: 0, tick: 0 })).toBe(0);
  });
});

describe("remaining", () => {
  it("counts rows left on a line goal", () => {
    expect(remaining({ type: "lines", count: 40 }, { lines: 12, tick: 0 })).toBe("28 rows");
  });

  it("never counts below zero when a clear overshoots", () => {
    expect(remaining({ type: "lines", count: 40 }, { lines: 44, tick: 0 })).toBe("0 rows");
  });

  it("reads a time goal as a clock, not a raw float", () => {
    const blitz: Goal = { type: "time", ms: 120_000 };
    expect(remaining(blitz, { lines: 0, tick: 0 })).toBe("2:00.0");
    expect(remaining(blitz, { lines: 0, tick: 60 })).toBe("1:59.0");
    expect(remaining(blitz, { lines: 0, tick: 7200 })).toBe("0:00.0");
  });

  it("keeps the seconds field two digits wide", () => {
    expect(remaining({ type: "time", ms: 65_000 }, { lines: 0, tick: 0 })).toBe("1:05.0");
  });

  it("has nothing to count for a goal it cannot decide", () => {
    expect(remaining({ type: "survival" }, { lines: 0, tick: 0 })).toBeNull();
  });
});
