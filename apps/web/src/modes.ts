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

export interface Mode {
  id: string;
  name: string;
  description: string;
  goal: Goal;
  config: Record<string, unknown>;
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
  return goal.type === "lines" || goal.type === "time";
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
      return `${Math.max(goal.count - at.lines, 0)} left`;
    case "time":
      return `${Math.max(ticksFor(goal.ms) - at.tick, 0) / 60} s left`;
    case "score":
    case "survival":
      return null;
  }
}
