import { describe, expect, it } from "vitest";
import { Clock, MAX_CATCHUP_TICKS, STALL_MS, ticksDue } from "./clock";

/** Drive a clock over a sequence of frame durations. */
function run(clock: Clock, frames: number[], startMs = 1000): number {
  let now = startMs;
  let total = 0;
  clock.advance(now);
  for (const d of frames) {
    now += d;
    total += clock.advance(now).ticks;
  }
  return total;
}

/** The clock's contract: ticks run are the ticks its game time has paid for. */
function isSettled(clock: Clock): boolean {
  return clock.ticks === ticksDue(clock.elapsed, 0);
}

describe("ticksDue", () => {
  it("owes one tick per sixtieth of a second", () => {
    expect(ticksDue(0, 0)).toBe(0);
    expect(ticksDue(1000, 0)).toBe(60);
    expect(ticksDue(1000, 60)).toBe(0);
    expect(ticksDue(10_000, 0)).toBe(600);
  });

  it("never owes a partial tick", () => {
    expect(ticksDue(16, 0)).toBe(0);
    expect(ticksDue(16.6, 0)).toBe(0);
    expect(ticksDue(16.7, 0)).toBe(1);
    expect(ticksDue(33.3, 0)).toBe(1);
  });

  it("never owes a negative number when the count runs ahead", () => {
    expect(ticksDue(100, 999)).toBe(0);
  });
});

describe("Clock", () => {
  it("runs about sixty ticks in a second of steady frames", () => {
    const clock = new Clock();
    const ticks = run(clock, Array(60).fill(1000 / 60));
    expect(ticks).toBeGreaterThanOrEqual(59);
    expect(ticks).toBeLessThanOrEqual(60);
    expect(isSettled(clock)).toBe(true);
  });

  it("pays out the same ticks for jittery frames as for steady ones", () => {
    // The property that matters: the count follows elapsed time, not frame pacing.
    const steady = new Clock();
    const ragged = new Clock();
    const jitter = [8, 25, 12, 30, 4, 19, 22, 9, 41, 6, 14, 17];
    const frames: number[] = [];
    for (let i = 0; i < 600; i++) frames.push(jitter[i % jitter.length]!);
    const total = frames.reduce((a, b) => a + b, 0);

    run(steady, Array(Math.round(total / (1000 / 60))).fill(1000 / 60));
    run(ragged, frames);

    expect(Math.abs(ragged.ticks - steady.ticks)).toBeLessThanOrEqual(1);
    expect(isSettled(ragged)).toBe(true);
  });

  it("stays settled over ten thousand ragged frames", () => {
    // Drift would show here: per-frame rounding that accumulated would pull the tick
    // count away from the elapsed time it is supposed to track.
    const clock = new Clock();
    const frames: number[] = [];
    for (let i = 0; i < 10_000; i++) frames.push(12 + ((i * 7919) % 11));
    run(clock, frames);
    expect(isSettled(clock)).toBe(true);
  });

  it("caps how many ticks one frame may run", () => {
    const clock = new Clock();
    clock.advance(1000);
    const step = clock.advance(1500);
    expect(step.ticks).toBe(MAX_CATCHUP_TICKS);
    expect(step.stalled).toBe(false);
  });

  it("keeps owing the ticks the cap held back, and drains them", () => {
    const clock = new Clock();
    clock.advance(1000);
    clock.advance(1500); // 500ms is 30 ticks; 8 run now.
    expect(clock.ticks).toBe(MAX_CATCHUP_TICKS);

    // Later frames pay off the debt rather than losing it.
    let now = 1500;
    for (let i = 0; i < 10; i++) {
      now += 1000 / 60;
      clock.advance(now);
    }
    expect(isSettled(clock)).toBe(true);
  });

  it("discards a stalled gap instead of fast-forwarding through it", () => {
    // A hidden tab owes minutes of ticks. Running them advances a game nobody played.
    const clock = new Clock();
    clock.advance(1000);
    const step = clock.advance(1000 + STALL_MS + 5000);
    expect(step.stalled).toBe(true);
    expect(step.ticks).toBe(0);
    expect(clock.ticks).toBe(0);
    expect(clock.elapsed).toBe(0);
  });

  it("does not advance game time while paused", () => {
    const clock = new Clock();
    run(clock, Array(6).fill(1000 / 60));
    const before = clock.ticks;

    clock.pause();
    clock.advance(5000);
    clock.advance(9000);
    expect(clock.ticks).toBe(before);

    clock.resume();
    clock.advance(9000);
    clock.advance(9100);
    expect(clock.ticks).toBeGreaterThan(before);
    expect(isSettled(clock)).toBe(true);
  });

  it("owes nothing for the frame that starts it", () => {
    // The first call only establishes a reference point.
    const clock = new Clock();
    expect(clock.advance(12345).ticks).toBe(0);
    expect(clock.advance(12365).ticks).toBe(1);
  });

  it("ignores a clock that goes backwards", () => {
    const clock = new Clock();
    clock.advance(1000);
    expect(clock.advance(900).ticks).toBe(0);
    expect(clock.elapsed).toBe(0);
  });

  it("forgets its elapsed time on reset", () => {
    const clock = new Clock();
    run(clock, Array(60).fill(1000 / 60));
    clock.reset();
    expect(clock.ticks).toBe(0);
    expect(clock.elapsed).toBe(0);
    expect(clock.isPaused).toBe(false);
  });
});
