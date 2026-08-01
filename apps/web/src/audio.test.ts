import { describe, expect, it } from "vitest";
import { cuesFor, volume } from "./audio";
import { EVENT } from "./sim/frame";

const ids = (events: number, lines = 0): string[] => cuesFor(events, lines).map((c) => c.id);

describe("cuesFor", () => {
  it("says nothing about an uneventful tick", () => {
    expect(cuesFor(0, 0)).toEqual([]);
  });

  it("stays silent for movement", () => {
    // MOVED fires on every auto-repeat step. At a competitive ARR that is every frame.
    expect(ids(EVENT.MOVED)).toEqual([]);
    expect(ids(EVENT.SOFT_DROPPED)).toEqual([]);
  });

  it("marks a lock that cleared nothing", () => {
    expect(ids(EVENT.PIECE_LOCKED)).toEqual(["lock"]);
  });

  it("lets the clear speak for a lock that cleared rows", () => {
    // Both at once is two thuds on the same frame, and the clear is the interesting half.
    const both = ids(EVENT.PIECE_LOCKED | EVENT.LINES_CLEARED, 2);
    expect(both).not.toContain("lock");
    expect(both).toContain("clear2");
  });

  it("pitches a clear by how many rows it took", () => {
    const single = cuesFor(EVENT.LINES_CLEARED, 1)[0]!;
    const quad = cuesFor(EVENT.LINES_CLEARED, 4)[0]!;
    expect(quad.freq).toBeGreaterThan(single.freq);
    expect(quad.id).toBe("clear4");
  });

  it("survives a clear count outside what the rules can produce", () => {
    expect(cuesFor(EVENT.LINES_CLEARED, 0)[0]!.id).toBe("clear1");
    expect(cuesFor(EVENT.LINES_CLEARED, 99)[0]!.id).toBe("clear4");
  });

  it("has a distinct cue for a spin and a perfect clear", () => {
    expect(ids(EVENT.SPIN)).toEqual(["spin"]);
    expect(ids(EVENT.MINI_SPIN)).toEqual(["spin"]);
    expect(ids(EVENT.PERFECT_CLEAR)).toContain("perfect");
  });

  it("plays a whole eventful tick without dropping any of it", () => {
    const quad = EVENT.PIECE_LOCKED | EVENT.LINES_CLEARED | EVENT.SPIN | EVENT.PERFECT_CLEAR;
    expect(ids(quad, 4)).toEqual(["clear4", "spin", "perfect"]);
  });

  it("has a cue for topping out", () => {
    expect(ids(EVENT.TOPPED_OUT)).toEqual(["topout"]);
  });

  it("gives every cue a positive duration and gain", () => {
    const everything = Object.values(EVENT).reduce((a, b) => a | b, 0);
    for (const cue of cuesFor(everything, 4)) {
      expect(cue.duration, cue.id).toBeGreaterThan(0);
      expect(cue.gain, cue.id).toBeGreaterThan(0);
      expect(cue.freq, cue.id).toBeGreaterThan(0);
    }
  });
});

describe("volume", () => {
  it("runs from silent to full", () => {
    expect(volume(0)).toBe(0);
    expect(volume(100)).toBe(1);
    expect(volume(70)).toBeCloseTo(0.7);
  });

  it("clamps a value from outside the range", () => {
    expect(volume(-20)).toBe(0);
    expect(volume(400)).toBe(1);
  });
});
