/**
 * Where a player's settings live, and how they survive the game changing.
 *
 * Validation is not done here. Clamping, migration and defaulting are the engine's rules,
 * so they are applied by the simulation crate through the wasm boundary and this module
 * only moves text in and out of storage. A second implementation in TypeScript would be a
 * second opinion about what a player's DAS is.
 */

export const SETTINGS_KEY = "openstacker.settings";
export const DEVICE_KEY = "openstacker.device_id";

export interface Handling {
  das_ms: number;
  arr_ms: number;
  [key: string]: number | string | boolean;
}

/** Match rules, as the schema describes them. Opaque here on purpose. */
export type Rules = Record<string, unknown>;

export interface Settings {
  version: number;
  handling: Handling;
  keybinds: Record<string, string>;
  cosmetic: Record<string, number | string | boolean>;
  /**
   * Rules the player has pinned for their own games, overriding the mode's.
   *
   * Absent until they tune something. Applied through the same layer a server will use,
   * so a local game and a hosted one resolve their rules the same way.
   */
  house_rules?: Rules;
}

/** The engine-side settings rules, injected so this module can be tested without wasm. */
export interface Codec {
  load(stored: string): string;
  normalize(json: string): string;
  defaults(): string;
}

export interface Loaded {
  settings: Settings;
  /**
   * What had to be adjusted to produce usable settings.
   *
   * Written to be shown to the player. These are the only signal that something they
   * chose could not be carried forward, so a client that logs them tells nobody.
   */
  notes: string[];
}

/** Read settings from storage. Never fails; unreadable storage yields defaults. */
export function load(storage: Storage, codec: Codec): Loaded {
  let stored: string | null = null;
  try {
    stored = storage.getItem(SETTINGS_KEY);
  } catch {
    // Storage can be unavailable entirely, in a private window or with cookies blocked.
    stored = null;
  }
  const parsed = JSON.parse(codec.load(stored ?? "")) as {
    settings: Settings;
    notes: string[];
  };
  return { settings: parsed.settings, notes: stored === null ? [] : parsed.notes };
}

/**
 * Write settings back, in the form the engine would actually use.
 *
 * Normalising on the way out means what is stored is what is played with, so a value the
 * engine would clamp does not sit in storage looking like it took effect.
 */
export function save(storage: Storage, codec: Codec, settings: Settings): Settings {
  const normalized = codec.normalize(JSON.stringify(settings));
  try {
    storage.setItem(SETTINGS_KEY, normalized);
  } catch {
    // A full or unavailable store must not cost the player their game.
  }
  return JSON.parse(normalized) as Settings;
}

/**
 * This device's identifier, created on first use.
 *
 * Restores local state and authorises nothing. Deliberately outside the settings blob:
 * it never reaches the engine and has no descriptor, so it cannot be a field in a
 * structure whose every field is describable.
 */
export function deviceId(storage: Storage, uuid: () => string = () => crypto.randomUUID()): string {
  try {
    const existing = storage.getItem(DEVICE_KEY);
    if (existing) return existing;
    const created = uuid();
    storage.setItem(DEVICE_KEY, created);
    return created;
  } catch {
    return uuid();
  }
}
