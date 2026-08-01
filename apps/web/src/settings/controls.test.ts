import { describe, expect, it } from "vitest";
import { clampToField, controlFor, duplicateBindings, readout, snapToStep } from "./controls";
import { SCHEMA, group, sections, showsFrames, type Field } from "./schema";

const field = (groupId: string, key: string): Field => {
  const found = group(groupId)?.fields.find((f) => f.key === key);
  if (!found) throw new Error(`no ${groupId}.${key} in the schema`);
  return found;
};

/** Frame conversion as the engine does it, for read-out tests. */
const centiframes = (ms: number): number => Math.round((ms * 60 * 100) / 1000);

describe("the schema the client builds from", () => {
  it("describes every setting the engine has", () => {
    const total = SCHEMA.groups.reduce((n, g) => n + g.fields.length, 0);
    expect(total).toBe(45);
  });

  it("says which groups reach the simulation", () => {
    expect(group("handling")?.affectsSimulation).toBe(true);
    expect(group("match")?.affectsSimulation).toBe(true);
    expect(group("keybinds")?.affectsSimulation).toBe(false);
    expect(group("cosmetic")?.affectsSimulation).toBe(false);
  });

  it("gives every field a label and help a player can read", () => {
    for (const g of SCHEMA.groups) {
      for (const f of g.fields) {
        expect(f.label, `${g.id}.${f.key}`).toBeTruthy();
        expect(f.group, `${g.id}.${f.key}`).toBeTruthy();
      }
    }
  });

  it("splits a group into the sections its fields declare", () => {
    const handling = sections(group("handling")!);
    expect(handling.map((s) => s.id)).toEqual([
      "handling.movement",
      "handling.drop",
      "handling.spawn",
    ]);
    expect(handling[0]!.label).toBe("movement");
  });

  it("offers the skins the client can actually draw", () => {
    const skin = field("cosmetic", "skin");
    expect(skin.type).toBe("enum");
    if (skin.type !== "enum") return;
    expect(skin.variants.map((v) => v.value)).toEqual([
      "default",
      "muted",
      "mono",
      "high_contrast",
    ]);
  });
});

describe("controlFor", () => {
  it("picks a control from the type, not the name", () => {
    expect(controlFor(field("handling", "das_ms"))).toBe("slider");
    expect(controlFor(field("handling", "ihs"))).toBe("toggle");
    expect(controlFor(field("handling", "irs"))).toBe("choice");
    expect(controlFor(field("keybinds", "move_left"))).toBe("capture");
  });

  it("has a control for every field in the schema", () => {
    // A field with no control is a setting nobody can change.
    for (const g of SCHEMA.groups) {
      for (const f of g.fields) {
        expect(controlFor(f), `${g.id}.${f.key}`).toBeTruthy();
      }
    }
  });
});

describe("values", () => {
  it("clamps to the range the schema declares", () => {
    const das = field("handling", "das_ms");
    expect(clampToField(das, 9999)).toBe(500);
    expect(clampToField(das, -5)).toBe(0);
    expect(clampToField(das, 133)).toBe(133);
  });

  it("snaps to the declared step", () => {
    const lock = field("match", "lock_delay_ms");
    expect(lock.type).toBe("int");
    if (lock.type !== "int") return;
    expect(lock.step).toBe(10);
    expect(snapToStep(lock, 503)).toBe(500);
    expect(snapToStep(lock, 507)).toBe(510);
  });

  it("keeps zero reachable on a stepped field", () => {
    // Zero DAS and zero ARR are competitively normal, not an error to round away.
    const arr = field("handling", "arr_ms");
    expect(snapToStep(arr, 0)).toBe(0);
    expect(clampToField(arr, 0)).toBe(0);
  });
});

describe("readout", () => {
  it("shows a duration in milliseconds with its frame equivalent", () => {
    expect(readout(field("handling", "das_ms"), 133, centiframes)).toBe("133 ms · 7.98 F");
  });

  it("marks durations as the fields that get a frame read-out", () => {
    expect(showsFrames(field("handling", "das_ms"))).toBe(true);
    expect(showsFrames(field("match", "preview_len"))).toBe(false);
  });

  it("shows a plain count without inventing a unit", () => {
    expect(readout(field("match", "preview_len"), 5, centiframes)).toContain("5");
  });
});

describe("duplicateBindings", () => {
  it("finds nothing in the defaults", () => {
    const binds = Object.fromEntries(
      group("keybinds")!.fields.map((f, i) => [f.key, `Key${i}`]),
    );
    expect(duplicateBindings(binds)).toEqual([]);
  });

  it("reports two actions sharing a key", () => {
    const clash = duplicateBindings({ move_left: "KeyX", hold: "KeyX", hard_drop: "Space" });
    expect(clash).toHaveLength(1);
    expect(clash[0]!.sort()).toEqual(["hold", "move_left"]);
  });

  it("ignores actions with nothing bound", () => {
    expect(duplicateBindings({ a: "", b: "" })).toEqual([]);
  });
});
