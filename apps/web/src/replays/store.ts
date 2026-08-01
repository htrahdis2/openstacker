/**
 * Where recordings and personal bests are kept.
 *
 * IndexedDB rather than localStorage, which is synchronous and capped near five
 * megabytes. A recording is a seed, the rules, the handling and the inputs — a few
 * hundred bytes for a whole game — so every game a player finishes is kept rather than
 * the last few.
 */

import { type IDBPDatabase, openDB } from "idb";

export const DB_NAME = "openstacker";
export const DB_VERSION = 1;

export interface StoredReplay {
  id: string;
  mode: string;
  createdAt: number;
  ticks: number;
  lines: number;
  pieces: number;
  /** Whether the run met its goal, as opposed to ending in a topout. */
  finished: boolean;
  /** The recording, in the form the tools read. */
  payload: string;
}

/** The best finished run for a mode. */
export interface Best {
  mode: string;
  ticks: number;
  replayId: string;
  createdAt: number;
}

export function open(factory?: IDBFactory): Promise<IDBPDatabase> {
  return openDB(DB_NAME, DB_VERSION, {
    upgrade(db) {
      if (!db.objectStoreNames.contains("replays")) {
        const replays = db.createObjectStore("replays", { keyPath: "id" });
        replays.createIndex("createdAt", "createdAt");
        replays.createIndex("mode", "mode");
      }
      if (!db.objectStoreNames.contains("bests")) {
        db.createObjectStore("bests", { keyPath: "mode" });
      }
    },
    ...(factory ? { indexedDB: factory } : {}),
  });
}

export async function saveReplay(db: IDBPDatabase, replay: StoredReplay): Promise<void> {
  await db.put("replays", replay);
}

/** Recordings, newest first. */
export async function listReplays(db: IDBPDatabase, limit = 50): Promise<StoredReplay[]> {
  const all = (await db.getAllFromIndex("replays", "createdAt")) as StoredReplay[];
  return all.reverse().slice(0, limit);
}

export async function getReplay(db: IDBPDatabase, id: string): Promise<StoredReplay | undefined> {
  return (await db.get("replays", id)) as StoredReplay | undefined;
}

export async function best(db: IDBPDatabase, mode: string): Promise<Best | undefined> {
  return (await db.get("bests", mode)) as Best | undefined;
}

/**
 * Record a run if it beats the mode's best, and say whether it did.
 *
 * Only finished runs count. A topout is not a time.
 */
export async function recordBest(
  db: IDBPDatabase,
  candidate: Best,
  finished: boolean,
): Promise<boolean> {
  if (!finished) return false;
  const current = await best(db, candidate.mode);
  if (current && current.ticks <= candidate.ticks) return false;
  await db.put("bests", candidate);
  return true;
}

/**
 * The parts of a recording needed to replay it.
 *
 * The seed is taken from the text rather than the parsed object: it is 64-bit, and
 * JSON.parse rounds anything past 53 bits through a double, which would play a different
 * game from the one that was recorded.
 */
export function decode(payload: string): {
  seed: bigint;
  config: string;
  handling: string;
  buttons: number[];
} {
  const parsed = JSON.parse(payload) as {
    config: unknown;
    handling: unknown;
    inputs: [number, number][];
  };
  const seed = /"seed"\s*:\s*(\d+)/.exec(payload);
  if (!seed) throw new Error("this recording has no seed");

  const buttons: number[] = [];
  for (const [bits, run] of parsed.inputs) {
    for (let i = 0; i < run; i++) buttons.push(bits);
  }

  return {
    seed: BigInt(seed[1]!),
    config: JSON.stringify(parsed.config),
    handling: JSON.stringify(parsed.handling),
    buttons,
  };
}

/** A recording as a file the tools accept. */
export function download(replay: StoredReplay, click: (url: string, name: string) => void): void {
  const blob = new Blob([replay.payload], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const stamp = new Date(replay.createdAt).toISOString().replace(/[:.]/g, "-");
  click(url, `${replay.mode}-${stamp}.replay`);
  URL.revokeObjectURL(url);
}
