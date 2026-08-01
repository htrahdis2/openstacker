import { describe, expect, it } from "vitest";
import { Input, keymap } from "./input";

/** The engine's action-to-button mapping, as the wasm module reports it. */
const BITS: Record<string, number> = {
  move_left: 1 << 0,
  move_right: 1 << 1,
  rotate_cw: 1 << 2,
  rotate_ccw: 1 << 3,
  rotate_flip: 1 << 4,
  hold: 1 << 5,
  soft_drop: 1 << 6,
  hard_drop: 1 << 7,
};

const DEFAULTS = {
  move_left: "ArrowLeft",
  move_right: "ArrowRight",
  rotate_cw: "ArrowUp",
  rotate_ccw: "KeyZ",
  rotate_flip: "KeyA",
  hold: "ShiftLeft",
  soft_drop: "ArrowDown",
  hard_drop: "Space",
};

const bits = (action: string): number => BITS[action] ?? 0;

function input(binds: Record<string, string> = DEFAULTS): Input {
  return new Input(keymap(binds, bits));
}

describe("keymap", () => {
  it("resolves every bound action to the engine's button", () => {
    const map = keymap(DEFAULTS, bits);
    expect(map.codes.get("ArrowLeft")).toBe(BITS.move_left);
    expect(map.codes.get("Space")).toBe(BITS.hard_drop);
    expect(map.codes.size).toBe(8);
  });

  it("ignores an action the engine does not know", () => {
    const map = keymap({ ...DEFAULTS, not_an_action: "KeyQ" }, bits);
    expect(map.codes.has("KeyQ")).toBe(false);
  });

  it("lets one key drive two actions", () => {
    // Not recommended, but a player may bind it and the result should be both buttons.
    const map = keymap({ move_left: "KeyX", hold: "KeyX" }, bits);
    expect(map.codes.get("KeyX")).toBe(BITS.move_left! | BITS.hold!);
  });
});

describe("Input", () => {
  it("reports a held key on every tick it is down for", () => {
    const i = input();
    i.press("ArrowLeft");
    expect(i.consume()).toBe(BITS.move_left);
    expect(i.consume()).toBe(BITS.move_left);
    i.release("ArrowLeft");
    expect(i.consume()).toBe(0);
  });

  it("delivers a tap that began and ended between two ticks", () => {
    // The case the latch exists for. Without it a fast rotation is simply lost.
    const i = input();
    i.press("KeyZ");
    i.release("KeyZ");
    expect(i.consume()).toBe(BITS.rotate_ccw);
  });

  it("delivers such a tap for exactly one tick", () => {
    // Held for longer, the engine would see no press edge on the second tick anyway, but
    // reporting it twice would be a lie about the keyboard.
    const i = input();
    i.press("Space");
    i.release("Space");
    i.consume();
    expect(i.consume()).toBe(0);
  });

  it("gives a catch-up run of ticks the press only once", () => {
    const i = input();
    i.press("Space");
    i.release("Space");
    const ticks = [i.consume(), i.consume(), i.consume()];
    expect(ticks).toEqual([BITS.hard_drop, 0, 0]);
  });

  it("combines everything held at once", () => {
    const i = input();
    i.press("ArrowLeft");
    i.press("ArrowDown");
    expect(i.consume()).toBe(BITS.move_left! | BITS.soft_drop!);
  });

  it("ignores keys that are not bound", () => {
    const i = input();
    i.press("KeyQ");
    expect(i.consume()).toBe(0);
    expect(i.binds("KeyQ")).toBe(false);
    expect(i.binds("ArrowLeft")).toBe(true);
  });

  it("forgets everything held when focus is lost", () => {
    // A key released while the page is unfocused is never seen, and the piece would slide
    // into the wall until the player clicked back and pressed it again.
    const i = input();
    i.press("ArrowRight");
    i.clear();
    expect(i.consume()).toBe(0);
    expect(i.current).toBe(0);
  });

  it("keeps held keys across a rebind", () => {
    const i = input();
    i.press("ArrowLeft");
    i.rebind(keymap({ ...DEFAULTS, move_left: "KeyH" }, bits));
    expect(i.consume()).toBe(BITS.move_left);
    // The old key no longer releases it, so the new binding governs from here.
    i.release("KeyH");
    expect(i.consume()).toBe(0);
  });

  it("survives a release with no matching press", () => {
    const i = input();
    i.release("ArrowLeft");
    expect(i.consume()).toBe(0);
  });
});
