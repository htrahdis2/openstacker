/**
 * Game modes, and knowing when one is finished.
 *
 * Modes are read from a file generated from the same TOML the server reads, so a mode is
 * defined once. The simulation never sees a goal — that is what keeps a recording to a
 * seed, the rules, the handling and the inputs — so deciding when a run is over is the
 * client's job.
 */

import generated from "../../../modes.generated.json";

export type Goal =
  | { type: "lines"; count: number }
  | { type: "time"; ms: number }
  | { type: "score"; target: number }
  | { type: "survival" };

/** A training opponent, on modes that have something to survive. */
export interface Sparring {
  first_batch_ms: number;
  interval_ms: number;
  interval_step_ms: number;
  min_interval_ms: number;
  rows_min: number;
  rows_max: number;
}

export interface Mode {
  id: string;
  name: string;
  description: string;
  goal: Goal;
  config: Record<string, unknown>;
  sparring?: Sparring;
}

export const MODES: Mode[] = (generated as { modes: Mode[] }).modes;

export function mode(id: string): Mode | undefined {
  return MODES.find((m) => m.id === id);
}

/** Ticks a duration in milliseconds corresponds to. */
export function ticksFor(ms: number): number {
  return Math.floor((ms * 60) / 1000);
}

/** Whether a goal is one this build can decide the end of. */
export function isPlayable(goal: Goal): boolean {
  // Survival ends by topping out, which the client has always detected — it just had
  // nothing to survive until versus modes carried an opponent.
  return goal.type === "lines" || goal.type === "time" || goal.type === "survival";
}

/**
 * Which direction a personal best runs in.
 *
 * Sprint and blitz are races: the shortest run wins. A survival run ends when the player
 * loses, so the longest one wins, and every run counts — there is no such thing as
 * finishing one.
 */
export function bestIsLongest(goal: Goal): boolean {
  return goal.type === "survival";
}

export interface Progress {
  lines: number;
  tick: number;
}

/** Whether the run has met its goal. */
export function goalReached(goal: Goal, at: Progress): boolean {
  switch (goal.type) {
    case "lines":
      return at.lines >= goal.count;
    case "time":
      return at.tick >= ticksFor(goal.ms);
    // Survival ends by topping out, and scoring does not exist yet.
    case "score":
    case "survival":
      return false;
  }
}

/** How far through the goal a run is, from 0 to 1. */
export function progress(goal: Goal, at: Progress): number {
  switch (goal.type) {
    case "lines":
      return goal.count > 0 ? Math.min(at.lines / goal.count, 1) : 1;
    case "time":
      return Math.min(at.tick / Math.max(ticksFor(goal.ms), 1), 1);
    case "score":
    case "survival":
      return 0;
  }
}

/** What the HUD counts down, or null when a goal has nothing to count. */
export function remaining(goal: Goal, at: Progress): string | null {
  switch (goal.type) {
    case "lines":
      return `${Math.max(goal.count - at.lines, 0)} rows`;
    case "time": {
      const ticks = Math.max(ticksFor(goal.ms) - at.tick, 0);
      const seconds = ticks / 60;
      const minutes = Math.floor(seconds / 60);
      return `${minutes}:${(seconds - minutes * 60).toFixed(1).padStart(4, "0")}`;
    }
    case "score":
    case "survival":
      return null;
  }
}
