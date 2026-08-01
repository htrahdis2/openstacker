/**
 * Keyboard to buttons.
 *
 * The engine is told which buttons were held during a tick, never what to do. Auto-repeat,
 * rotation edges and drop speed are all decided inside the simulation, so this layer only
 * has to report the truth about the keyboard.
 */

export interface Keymap {
  /** Physical key name (`event.code`) to the button bit it resolves to. */
  codes: Map<string, number>;
}

/** Build a lookup from stored keybinds, using the engine's own action-to-button mapping. */
export function keymap(
  keybinds: Record<string, string>,
  bits: (action: string) => number,
): Keymap {
  const codes = new Map<string, number>();
  for (const [action, code] of Object.entries(keybinds)) {
    const bit = bits(action);
    if (bit === 0 || !code) continue;
    codes.set(code, (codes.get(code) ?? 0) | bit);
  }
  return { codes };
}

/**
 * What the keyboard is doing, sampled per tick.
 *
 * Holds two masks. `held` is what is down right now; `latched` is everything pressed since
 * the last tick took its reading. A tick is given the union, so a tap that starts and ends
 * between two ticks still produces one press edge instead of vanishing — which for the
 * edge-triggered buttons is the difference between a placement and a lost one.
 */
export class Input {
  private held = 0;
  private latched = 0;

  constructor(private map: Keymap) {}

  /** Swap in new bindings without losing what is currently held. */
  rebind(map: Keymap): void {
    this.map = map;
  }

  /** Whether a key is bound to anything, and so whether the page should keep it. */
  binds(code: string): boolean {
    return this.map.codes.has(code);
  }

  press(code: string): void {
    const bit = this.map.codes.get(code);
    if (bit === undefined) return;
    this.held |= bit;
    this.latched |= bit;
  }

  release(code: string): void {
    const bit = this.map.codes.get(code);
    if (bit === undefined) return;
    this.held &= ~bit;
  }

  /** Forget everything held. Used when the page loses focus. */
  clear(): void {
    this.held = 0;
    this.latched = 0;
  }

  /** The buttons for one tick. */
  consume(): number {
    const buttons = this.held | this.latched;
    this.latched = 0;
    return buttons;
  }

  /** What is held right now, without consuming anything. */
  get current(): number {
    return this.held;
  }
}

/** Route keyboard events on a window into an input state. */
export function attach(target: Window, input: Input): () => void {
  const down = (e: KeyboardEvent): void => {
    // Auto-repeat is the operating system's idea of DAS. The engine has its own.
    if (e.repeat) return;
    if (!input.binds(e.code)) return;
    e.preventDefault();
    input.press(e.code);
  };

  const up = (e: KeyboardEvent): void => {
    if (!input.binds(e.code)) return;
    e.preventDefault();
    input.release(e.code);
  };

  // A key released while the page is unfocused is never seen, and the piece would keep
  // sliding into the wall forever.
  const lost = (): void => input.clear();

  target.addEventListener("keydown", down);
  target.addEventListener("keyup", up);
  target.addEventListener("blur", lost);
  target.document.addEventListener("visibilitychange", lost);

  return () => {
    target.removeEventListener("keydown", down);
    target.removeEventListener("keyup", up);
    target.removeEventListener("blur", lost);
    target.document.removeEventListener("visibilitychange", lost);
  };
}
