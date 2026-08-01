/**
 * Game time, in whole ticks.
 *
 * The simulation advances on elapsed time, never on frame callbacks: one tick is one
 * sixtieth of a second of game time whatever the display is doing. A dropped or doubled
 * tick is a different game, not a dropped frame.
 */

export const TICK_HZ = 60;

/** Most ticks run in one frame. Beyond this the rest stay owed for the next one. */
export const MAX_CATCHUP_TICKS = 8;

/** A gap this long is a stall — the tab was hidden or the machine hitched. */
export const STALL_MS = 1000;

/**
 * Ticks owed at a point in game time.
 *
 * Derived from the total rather than accumulated per frame, so rounding cannot drift.
 */
export function ticksDue(elapsedMs: number, ticksRun: number): number {
  return Math.max(0, Math.floor((elapsedMs * TICK_HZ) / 1000) - ticksRun);
}

export interface ClockStep {
  /** Ticks to run now. */
  ticks: number;
  /** Whether the gap since the last call was discarded as a stall. */
  stalled: boolean;
}

/** Accumulates real time into game time and hands out whole ticks. */
export class Clock {
  private elapsedMs = 0;
  private ticksRun = 0;
  private lastMs: number | null = null;
  private paused = false;

  get ticks(): number {
    return this.ticksRun;
  }

  get elapsed(): number {
    return this.elapsedMs;
  }

  get isPaused(): boolean {
    return this.paused;
  }

  pause(): void {
    this.paused = true;
    this.lastMs = null;
  }

  resume(): void {
    this.paused = false;
    this.lastMs = null;
  }

  reset(): void {
    this.elapsedMs = 0;
    this.ticksRun = 0;
    this.lastMs = null;
    this.paused = false;
  }

  /**
   * Take the time since the last call and report the ticks it bought.
   *
   * A stalled gap is discarded rather than played: the player could not have been at the
   * keyboard for it, and game time that nobody played is not game time.
   */
  advance(nowMs: number): ClockStep {
    if (this.paused) {
      this.lastMs = nowMs;
      return { ticks: 0, stalled: false };
    }

    const last = this.lastMs;
    this.lastMs = nowMs;
    if (last === null) {
      return { ticks: 0, stalled: false };
    }

    const delta = nowMs - last;
    if (delta >= STALL_MS) {
      return { ticks: 0, stalled: true };
    }
    if (delta > 0) {
      this.elapsedMs += delta;
    }

    const owed = ticksDue(this.elapsedMs, this.ticksRun);
    const ticks = Math.min(owed, MAX_CATCHUP_TICKS);
    this.ticksRun += ticks;
    return { ticks, stalled: false };
  }
}
