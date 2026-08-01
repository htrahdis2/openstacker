/**
 * Loading the simulation, and reading it without copying.
 *
 * The board is read in place through views over wasm memory. Views are rebuilt if the
 * buffer is ever replaced, because growing wasm memory detaches every existing view and
 * the symptom is a board that silently stops updating.
 */

import init, {
  Game,
  buttonBits,
  centiframes,
  defaultMatchConfig,
  defaultSettings,
  engineVer,
  frameLayout,
  loadSettings,
  normalizeSettings,
} from "../wasm/client.js";
import type { FrameLayout } from "./frame";

export { Game, buttonBits, centiframes, defaultMatchConfig, defaultSettings, engineVer, loadSettings, normalizeSettings };

let memory: WebAssembly.Memory | null = null;
let layout: FrameLayout | null = null;

/** Load the module. Safe to call more than once. */
export async function initSim(): Promise<void> {
  if (memory) return;
  const exports = await init();
  memory = exports.memory;
  layout = JSON.parse(frameLayout()) as FrameLayout;
}

export function simLayout(): FrameLayout {
  if (!layout) throw new Error("the simulation has not been loaded yet");
  return layout;
}

/** The board and frame block of one game, read directly out of wasm memory. */
export class GameViews {
  private buffer: ArrayBufferLike;
  occupancy!: Uint16Array;
  colors!: Uint8Array;
  frame!: DataView;

  constructor(private game: Game) {
    this.buffer = wasmMemory().buffer;
    this.rebuild();
  }

  /** Point the views at current memory if it has been replaced since the last frame. */
  refresh(): void {
    const current = wasmMemory().buffer;
    if (current !== this.buffer) {
      this.buffer = current;
      this.rebuild();
    }
  }

  private rebuild(): void {
    const { board, bytes } = simLayout();
    this.occupancy = new Uint16Array(this.buffer, this.game.occupancyPtr(), board.height);
    this.colors = new Uint8Array(this.buffer, this.game.colorsPtr(), board.width * board.height);
    this.frame = new DataView(this.buffer, this.game.framePtr(), bytes);
  }
}

function wasmMemory(): WebAssembly.Memory {
  if (!memory) throw new Error("the simulation has not been loaded yet");
  return memory;
}

/**
 * A seed for a new game.
 *
 * Drawn from 48 bits rather than 64: a replay is JSON, and JSON.parse rounds anything
 * past 53 bits through a double, which would make a recording play a different game.
 */
export function newSeed(): bigint {
  const bytes = new Uint8Array(6);
  crypto.getRandomValues(bytes);
  let seed = 0n;
  for (const b of bytes) seed = (seed << 8n) | BigInt(b);
  return seed;
}
