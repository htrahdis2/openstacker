/**
 * Turning a described setting into a control.
 *
 * Which control a setting gets is decided by the schema's type, never by its name. That
 * is what makes adding a setting a change in Rust with no matching change here.
 */

import type { Field } from "./schema";

export type ControlKind = "slider" | "toggle" | "choice" | "capture";

/** The control a described field is rendered as. */
export function controlFor(field: Field): ControlKind {
  switch (field.type) {
    case "int":
      return "slider";
    case "bool":
      return "toggle";
    case "enum":
      return "choice";
    case "binding":
      return "capture";
  }
}

/** A value brought inside the range the schema declares. */
export function clampToField(field: Field, value: number): number {
  if (field.type !== "int") return value;
  return Math.min(Math.max(value, field.min), field.max);
}

/** A value snapped to the field's step, so a slider cannot land between stops. */
export function snapToStep(field: Field, value: number): number {
  if (field.type !== "int" || field.step <= 0) return value;
  const snapped = Math.round((value - field.min) / field.step) * field.step + field.min;
  return clampToField(field, snapped);
}

/**
 * How a value reads beside its control.
 *
 * Durations show frames as well, because a competitive player thinks in frames. The
 * milliseconds are what is stored: frames quantise to about a hundredth, so a player who
 * typed a frame value and got it handed back would see it move.
 */
export function readout(field: Field, value: unknown, centiframes: (ms: number) => number): string {
  if (field.type !== "int") return String(value);
  const n = Number(value);
  if (field.unit === "ms") {
    return `${n} ms · ${(centiframes(n) / 100).toFixed(2)} F`;
  }
  return field.unit ? `${n} ${field.unit}` : String(n);
}

/** Actions bound to the same key, which is legal but almost never meant. */
export function duplicateBindings(keybinds: Record<string, string>): string[][] {
  const byCode = new Map<string, string[]>();
  for (const [action, code] of Object.entries(keybinds)) {
    if (!code) continue;
    if (!byCode.has(code)) byCode.set(code, []);
    byCode.get(code)!.push(action);
  }
  return [...byCode.values()].filter((actions) => actions.length > 1);
}
