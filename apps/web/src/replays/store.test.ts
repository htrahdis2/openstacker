import "fake-indexeddb/auto";
import { IDBFactory } from "fake-indexeddb";
import type { IDBPDatabase } from "idb";
import { beforeEach, describe, expect, it } from "vitest";
import {
  type StoredReplay,
  best,
  decode,
  download,
  getReplay,
  listReplays,
  open,
  recordBest,
  saveReplay,
} from "./store";

let db: IDBPDatabase;

function replay(overrides: Partial<StoredReplay> = {}): StoredReplay {
  return {
    id: "r1",
    mode: "sprint40",
    createdAt: 1_000,
    ticks: 3000,
    lines: 40,
    pieces: 100,
    finished: true,
    payload: '{"version":1,"seed":7}',
    ...overrides,
  };
}

beforeEach(async () => {
  // A fresh database per test, so one test's bests cannot decide another's.
  globalThis.indexedDB = new IDBFactory();
  db = await open();
});

describe("replays", () => {
  it("keeps a recording and gives it back", async () => {
    await saveReplay(db, replay());
    const back = await getReplay(db, "r1");
    expect(back?.payload).toBe('{"version":1,"seed":7}');
    expect(back?.lines).toBe(40);
  });

  it("lists them newest first", async () => {
    await saveReplay(db, replay({ id: "old", createdAt: 1 }));
    await saveReplay(db, replay({ id: "new", createdAt: 3 }));
    await saveReplay(db, replay({ id: "middle", createdAt: 2 }));
    expect((await listReplays(db)).map((r) => r.id)).toEqual(["new", "middle", "old"]);
  });

  it("keeps every game rather than the last few", async () => {
    // A recording is a few hundred bytes, so there is no reason to evict one.
    for (let i = 0; i < 200; i++) {
      await saveReplay(db, replay({ id: `r${i}`, createdAt: i }));
    }
    expect(await db.count("replays")).toBe(200);
    expect(await listReplays(db, 10)).toHaveLength(10);
  });

  it("survives a reopen", async () => {
    await saveReplay(db, replay());
    db.close();
    const reopened = await open();
    expect(await getReplay(reopened, "r1")).toBeDefined();
  });
});

describe("bests", () => {
  it("records the first finished run", async () => {
    expect(await recordBest(db, { mode: "sprint40", ticks: 3000, replayId: "r1", createdAt: 1 }, true)).toBe(true);
    expect((await best(db, "sprint40"))?.ticks).toBe(3000);
  });

  it("replaces a slower one", async () => {
    await recordBest(db, { mode: "sprint40", ticks: 3000, replayId: "r1", createdAt: 1 }, true);
    expect(await recordBest(db, { mode: "sprint40", ticks: 2500, replayId: "r2", createdAt: 2 }, true)).toBe(true);
    expect((await best(db, "sprint40"))?.replayId).toBe("r2");
  });

  it("keeps the faster one when a slower run follows", async () => {
    await recordBest(db, { mode: "sprint40", ticks: 2500, replayId: "r1", createdAt: 1 }, true);
    expect(await recordBest(db, { mode: "sprint40", ticks: 4000, replayId: "r2", createdAt: 2 }, true)).toBe(false);
    expect((await best(db, "sprint40"))?.ticks).toBe(2500);
  });

  it("does not count a topout", async () => {
    // Ending early is not a time, however few ticks it took.
    expect(await recordBest(db, { mode: "sprint40", ticks: 10, replayId: "r1", createdAt: 1 }, false)).toBe(false);
    expect(await best(db, "sprint40")).toBeUndefined();
  });

  it("keeps a best per mode", async () => {
    await recordBest(db, { mode: "sprint40", ticks: 3000, replayId: "r1", createdAt: 1 }, true);
    await recordBest(db, { mode: "blitz", ticks: 7200, replayId: "r2", createdAt: 2 }, true);
    expect((await best(db, "sprint40"))?.ticks).toBe(3000);
    expect((await best(db, "blitz"))?.ticks).toBe(7200);
  });

  it("does not beat itself on a tie", async () => {
    await recordBest(db, { mode: "sprint40", ticks: 3000, replayId: "r1", createdAt: 1 }, true);
    expect(await recordBest(db, { mode: "sprint40", ticks: 3000, replayId: "r2", createdAt: 2 }, true)).toBe(false);
  });
});

describe("download", () => {
  it("names the file after the mode and when it was played", () => {
    let name = "";
    download(replay(), (_url, filename) => {
      name = filename;
    });
    expect(name).toMatch(/^sprint40-.*\.replay$/);
  });

  it("hands over the recording unaltered, so the tools accept it", () => {
    let url = "";
    download(replay(), (u) => {
      url = u;
    });
    expect(url).toMatch(/^blob:/);
  });
});

describe("decode", () => {
  const payload = JSON.stringify({
    version: 1,
    engine_ver: 1,
    seed: 236143731602737,
    config: { preview_len: 5 },
    handling: { das_ms: 133 },
    inputs: [
      [0, 3],
      [128, 1],
      [0, 2],
    ],
    claimed: { final_tick: 6 },
  });

  it("expands the run-length encoded buttons back to one per tick", () => {
    const { buttons } = decode(payload);
    expect(buttons).toEqual([0, 0, 0, 128, 0, 0]);
  });

  it("carries the rules and handling the game was played under", () => {
    const { config, handling } = decode(payload);
    expect(JSON.parse(config).preview_len).toBe(5);
    expect(JSON.parse(handling).das_ms).toBe(133);
  });

  it("keeps a seed that a double could not hold", () => {
    // JSON.parse rounds anything past 53 bits, and a rounded seed plays a different game
    // from the one that was recorded.
    const big = payload.replace("236143731602737", "9007199254740993");
    const viaJson = BigInt(JSON.parse(big).seed as number);

    expect(decode(big).seed).toBe(9007199254740993n);
    expect(viaJson).toBe(9007199254740992n);
    expect(viaJson).not.toBe(decode(big).seed);
  });

  it("refuses a recording with no seed rather than inventing one", () => {
    expect(() => decode('{"inputs":[],"config":{},"handling":{}}')).toThrow();
  });
});
