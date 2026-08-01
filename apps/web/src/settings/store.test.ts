import { beforeEach, describe, expect, it } from "vitest";
import { DEVICE_KEY, SETTINGS_KEY, type Codec, deviceId, load, save } from "./store";

/** A storage that behaves like localStorage, including failing. */
class FakeStorage implements Storage {
  private map = new Map<string, string>();
  failing = false;

  get length(): number {
    return this.map.size;
  }
  clear(): void {
    this.map.clear();
  }
  key(i: number): string | null {
    return [...this.map.keys()][i] ?? null;
  }
  getItem(key: string): string | null {
    if (this.failing) throw new Error("storage is unavailable");
    return this.map.get(key) ?? null;
  }
  setItem(key: string, value: string): void {
    if (this.failing) throw new Error("storage is full");
    this.map.set(key, value);
  }
  removeItem(key: string): void {
    this.map.delete(key);
  }
}

const DEFAULTS = {
  version: 1,
  handling: { das_ms: 133, arr_ms: 0 },
  keybinds: { move_left: "ArrowLeft" },
  cosmetic: { skin: "default" },
};

/**
 * Stands in for the engine's rules: clamps DAS to 500 and reports when it had to.
 * The real ones live in Rust; this only has to behave like them at the boundary.
 */
const codec: Codec = {
  load(stored) {
    if (!stored) return JSON.stringify({ settings: DEFAULTS, notes: [] });
    let parsed: Record<string, unknown>;
    try {
      parsed = JSON.parse(stored);
    } catch {
      return JSON.stringify({
        settings: DEFAULTS,
        notes: ["settings could not be read, using defaults: not valid JSON"],
      });
    }
    const merged = { ...DEFAULTS, ...parsed } as typeof DEFAULTS;
    const notes: string[] = [];
    if (merged.handling.das_ms > 500) {
      merged.handling = { ...merged.handling, das_ms: 500 };
      notes.push("some handling values were outside the allowed range and were adjusted");
    }
    return JSON.stringify({ settings: merged, notes });
  },
  normalize(json) {
    return JSON.stringify(JSON.parse(this.load(json)).settings);
  },
  defaults: () => JSON.stringify(DEFAULTS),
};

let storage: FakeStorage;

beforeEach(() => {
  storage = new FakeStorage();
});

describe("load", () => {
  it("gives defaults on a first run, quietly", () => {
    // Nothing was stored, so nothing was adjusted. A note here would alarm a new player.
    const { settings, notes } = load(storage, codec);
    expect(settings.handling.das_ms).toBe(133);
    expect(notes).toEqual([]);
  });

  it("returns what the player chose", () => {
    storage.setItem(SETTINGS_KEY, JSON.stringify({ ...DEFAULTS, handling: { das_ms: 83, arr_ms: 0 } }));
    expect(load(storage, codec).settings.handling.das_ms).toBe(83);
  });

  it("passes on the notes the engine produced", () => {
    storage.setItem(SETTINGS_KEY, JSON.stringify({ ...DEFAULTS, handling: { das_ms: 9999, arr_ms: 0 } }));
    const { settings, notes } = load(storage, codec);
    expect(settings.handling.das_ms).toBe(500);
    expect(notes.length).toBe(1);
    expect(notes[0]).toContain("adjusted");
  });

  it("still yields playable settings when storage is unreadable", () => {
    // A private window, or cookies blocked. The player still gets to play.
    storage.failing = true;
    const { settings, notes } = load(storage, codec);
    expect(settings.handling.das_ms).toBe(133);
    expect(notes).toEqual([]);
  });

  it("recovers from a corrupted blob and says so", () => {
    storage.setItem(SETTINGS_KEY, "{ not json");
    const { settings, notes } = load(storage, codec);
    expect(settings.handling.das_ms).toBe(133);
    expect(notes[0]).toContain("could not be read");
  });
});

describe("save", () => {
  it("stores what the engine would actually use", () => {
    // A value sitting in storage that the engine would clamp looks like it took effect.
    const written = save(storage, codec, {
      ...DEFAULTS,
      handling: { das_ms: 9999, arr_ms: 0 },
    });
    expect(written.handling.das_ms).toBe(500);
    expect(JSON.parse(storage.getItem(SETTINGS_KEY)!).handling.das_ms).toBe(500);
  });

  it("round trips through a reload", () => {
    save(storage, codec, { ...DEFAULTS, handling: { das_ms: 100, arr_ms: 16 } });
    const { settings, notes } = load(storage, codec);
    expect(settings.handling).toEqual({ das_ms: 100, arr_ms: 16 });
    expect(notes).toEqual([]);
  });

  it("does not throw when storage refuses the write", () => {
    storage.failing = true;
    expect(() => save(storage, codec, DEFAULTS)).not.toThrow();
  });
});

describe("deviceId", () => {
  it("is created once and kept", () => {
    let n = 0;
    const uuid = (): string => `id-${n++}`;
    const first = deviceId(storage, uuid);
    expect(deviceId(storage, uuid)).toBe(first);
    expect(storage.getItem(DEVICE_KEY)).toBe(first);
  });

  it("lives outside the settings blob", () => {
    // It never reaches the engine and has no descriptor, so it cannot be a settings field.
    deviceId(storage, () => "id-1");
    save(storage, codec, DEFAULTS);
    expect(JSON.parse(storage.getItem(SETTINGS_KEY)!).deviceId).toBeUndefined();
    expect(storage.getItem(DEVICE_KEY)).toBe("id-1");
  });

  it("still returns one when storage is unavailable", () => {
    storage.failing = true;
    expect(deviceId(storage, () => "id-1")).toBe("id-1");
  });
});
