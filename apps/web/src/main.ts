/**
 * Entry point: pick a mode, run it on the clock, draw it every frame.
 *
 * Ticks come from the clock and rendering from requestAnimationFrame. Tying the two
 * together would make the game speed depend on the display.
 */

import { Sound } from "./audio";
import { Clock } from "./clock";
import { Input, attach, keymap } from "./input";
import { MODES, type Mode, goalReached, isPlayable, mode, remaining } from "./modes";
import { drawBoard, geometry } from "./render/board";
import { drawHold, drawNext, formatPps, formatTime, garbageFill, sizeBoxes } from "./render/hud";
import { skin } from "./render/palette";
import type { Shapes } from "./render/piece";
import {
  type StoredReplay,
  best,
  decode,
  download,
  listReplays,
  open as openReplays,
  recordBest,
  saveReplay,
} from "./replays/store";
import { buildPanel } from "./settings/panel";
import { type Settings, deviceId, load, save } from "./settings/store";
import { type Frame, readFrame } from "./sim/frame";
import {
  Game,
  GameViews,
  buttonBits,
  centiframes,
  defaultSettings,
  initSim,
  loadSettings,
  newSeed,
  normalizeSettings,
  pieceShapes,
  simLayout,
} from "./sim/wasm";

const CELL = 30;

const el = <T extends HTMLElement>(id: string): T => {
  const found = document.getElementById(id);
  if (!found) throw new Error(`the page has no #${id}`);
  return found as T;
};

const canvas = el<HTMLCanvasElement>("board");
const holdCanvas = el<HTMLCanvasElement>("hold");
const nextCanvas = el<HTMLCanvasElement>("next");
const garbage = el("garbage");
const garbageTrack = el("garbage-track");
const status = el("status");
const statTime = el("stat-time");
const statLines = el("stat-lines");
const statPieces = el("stat-pieces");
const statPps = el("stat-pps");
const goalValue = el("goal-value");
const overlay = el("overlay");
const overlayTitle = el("overlay-title");
const overlayNote = el("overlay-note");
const modeList = el("mode-list");
const resultList = el<HTMLElement>("result");
const againButton = el<HTMLButtonElement>("again");
const backButton = el<HTMLButtonElement>("back");
const saveReplayButton = el<HTMLButtonElement>("save-replay");
const openReplaysButton = el<HTMLButtonElement>("open-replays");
const closeReplaysButton = el<HTMLButtonElement>("close-replays");
const replaysScreen = el("replays-screen");
const replayList = el("replay-list");
const replaysEmpty = el("replays-empty");
const overlayBest = el("overlay-best");
const openSettings = el<HTMLButtonElement>("open-settings");
const closeSettings = el<HTMLButtonElement>("close-settings");
const settingsScreen = el("settings-screen");
const settingsPanel = el("settings-panel");
const settingsNotes = el("settings-notes");

await initSim();

const codec = {
  load: loadSettings,
  normalize: normalizeSettings,
  defaults: defaultSettings,
};

const stored = load(window.localStorage, codec);
let settings: Settings = stored.settings;
deviceId(window.localStorage);

let theme = skin(String(settings.cosmetic.skin));
const shapes = JSON.parse(pieceShapes()) as Shapes;
const clock = new Clock();
const input = new Input(keymap(settings.keybinds, buttonBits));
attach(window, input);

const sound = new Sound();
sound.setVolume(Number(settings.cosmetic.effects_volume ?? 70));

// Browsers only allow audio to start from a real interaction.
for (const event of ["keydown", "pointerdown"]) {
  window.addEventListener(event, () => sound.resume(), { once: true });
}

showNotes(stored.notes);

const ctx = canvas.getContext("2d");
if (!ctx) throw new Error("this browser has no 2d canvas");

/** What the client is doing. A game only advances while it is running. */
type Phase = "menu" | "running" | "done";

/**
 * A recording being played back.
 *
 * Playback is the same loop with buttons read from the recording instead of the keyboard,
 * so what a viewer sees is re-derived by running the simulation rather than stored.
 */
interface Playback {
  buttons: number[];
  at: number;
  label: string;
}

let playback: Playback | null = null;

let phase: Phase = "menu";
let current: Mode | null = null;
let game: Game | null = null;
let views: GameViews | null = null;
let lastReplay: StoredReplay | null = null;

document.addEventListener("visibilitychange", () => {
  if (document.hidden) clock.pause();
  else clock.resume();
});

function start(mode: Mode): void {
  playback = null;
  current = mode;
  game = new Game(newSeed(), JSON.stringify(mode.config), JSON.stringify(settings.handling));
  views = new GameViews(game);
  clock.reset();
  input.clear();
  phase = "running";
  status.textContent = "";
  garbageTrack.classList.add("active");
  showOverlay(false);
  draw();
}

function finish(frame: Frame): void {
  phase = "done";
  const met = current ? goalReached(current.goal, frame) : false;
  showResult(frame, met);
  void keep(frame, met);
}

/**
 * Record the run.
 *
 * Every game is kept: the recording is the seed, the rules, the handling and the inputs,
 * so a whole game is a few hundred bytes and there is nothing to decide about which to
 * throw away.
 */
async function keep(frame: Frame, met: boolean): Promise<void> {
  // A recording played back is not a new game and must not be stored as one.
  if (!game || !current || playback) return;
  const stored: StoredReplay = {
    id: crypto.randomUUID(),
    mode: current.id,
    createdAt: Date.now(),
    ticks: frame.tick,
    lines: frame.lines,
    pieces: frame.pieces,
    finished: met,
    payload: game.finishReplay(),
  };
  lastReplay = stored;
  saveReplayButton.hidden = false;

  try {
    const db = await openReplays();
    await saveReplay(db, stored);
    const beaten = await recordBest(
      db,
      { mode: stored.mode, ticks: stored.ticks, replayId: stored.id, createdAt: stored.createdAt },
      met,
    );
    const record = await best(db, stored.mode);
    overlayBest.hidden = !record;
    if (record) {
      overlayBest.textContent = beaten
        ? `new best · ${formatTime(record.ticks)}`
        : `best · ${formatTime(record.ticks)}`;
    }
  } catch {
    // Storage can be unavailable. The run still happened and is still downloadable.
    status.textContent = "this run could not be saved";
  }
}

function step(ticks: number): void {
  if (phase === "running" && game) {
    for (let i = 0; i < ticks; i++) {
      if (playback) {
        if (playback.at >= playback.buttons.length) {
          finish(read()!);
          break;
        }
        game.tick(playback.buttons[playback.at++]!);
        draw();
        continue;
      }
      // Sampled per tick, not per frame: a catch-up run must not replay one press.
      game.tick(input.consume());
      const frame = read();
      if (!frame) continue;
      sound.play(frame.events, frame.linesCleared);
      if (frame.over || (current && goalReached(current.goal, frame))) {
        finish(frame);
        break;
      }
    }
  }
  draw();
}

function frame(now: number): void {
  const advance = clock.advance(now);
  if (advance.stalled && phase === "running") {
    status.textContent = "skipped a stalled gap";
  }
  step(advance.ticks);
  requestAnimationFrame(frame);
}

function read(): Frame | null {
  if (!views) return null;
  views.refresh();
  return readFrame(views.frame, simLayout());
}

function resize(): ReturnType<typeof geometry> {
  const dpr = window.devicePixelRatio || 1;
  const geo = geometry(CELL);
  if (canvas.width !== geo.width * dpr || canvas.height !== geo.height * dpr) {
    canvas.width = geo.width * dpr;
    canvas.height = geo.height * dpr;
    canvas.style.width = `${geo.width}px`;
    canvas.style.height = `${geo.height}px`;
  }
  ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);
  return geo;
}

function draw(): void {
  const geo = resize();
  const reading = read();
  if (!reading || !views) {
    ctx!.clearRect(0, 0, geo.width, geo.height);
    return;
  }

  drawBoard(ctx!, reading, views.occupancy, views.colors, geo, {
    skin: theme,
    ghostOpacity: Number(settings.cosmetic.ghost_opacity ?? 0),
    showGrid: Boolean(settings.cosmetic.show_grid),
  });

  sizeBoxes({ hold: holdCanvas, next: nextCanvas }, reading.preview.length);
  drawHold(holdCanvas, reading, shapes, theme);
  drawNext(nextCanvas, reading, shapes, theme);

  statTime.textContent = formatTime(reading.tick);
  statLines.textContent = String(reading.lines);
  statPieces.textContent = String(reading.pieces);
  statPps.textContent = formatPps(reading.pieces, reading.tick);
  garbage.style.height = `${garbageFill(reading.pendingRows) * 100}%`;
  goalValue.textContent = current ? (remaining(current.goal, reading) ?? "survive") : "—";
}

// ---- menu and results ------------------------------------------------------

function showOverlay(visible: boolean): void {
  overlay.classList.toggle("visible", visible);
}

function showMenu(): void {
  phase = "menu";
  current = null;
  game = null;
  views = null;
  overlayTitle.textContent = "openstacker";
  overlayNote.textContent = "pick a mode";
  garbageTrack.classList.remove("active");
  resultList.hidden = true;
  modeList.hidden = false;
  againButton.hidden = true;
  backButton.hidden = true;
  saveReplayButton.hidden = true;
  overlayBest.hidden = true;
  goalValue.textContent = "—";
  draw();
  showOverlay(true);
}

function showResult(frame: Frame, met: boolean): void {
  overlayTitle.textContent = met ? "finished" : "topped out";
  overlayNote.textContent = current?.name ?? "";
  modeList.hidden = true;
  resultList.hidden = false;
  resultList.innerHTML = "";
  const rows: [string, string][] = [
    ["time", formatTime(frame.tick)],
    ["lines", String(frame.lines)],
    ["pieces", String(frame.pieces)],
    ["pps", formatPps(frame.pieces, frame.tick)],
  ];
  for (const [label, value] of rows) {
    const pair = document.createElement("div");
    const dt = document.createElement("dt");
    dt.textContent = label;
    const dd = document.createElement("dd");
    dd.textContent = value;
    pair.append(dt, dd);
    resultList.append(pair);
  }
  againButton.hidden = false;
  backButton.hidden = false;
  showOverlay(true);
}

function buildMenu(): void {
  modeList.innerHTML = "";
  for (const entry of MODES) {
    const li = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    button.innerHTML = `<strong>${entry.name}</strong><span>${entry.description}</span>`;
    if (isPlayable(entry.goal)) {
      button.addEventListener("click", () => start(entry));
    } else {
      button.disabled = true;
      button.title = "needs versus rules";
    }
    li.append(button);
    modeList.append(li);
  }
}

// ---- replays ----------------------------------------------------------------

/** Play a recording back through the same loop that played it live. */
function watch(stored: StoredReplay): void {
  const { seed, config, handling, buttons } = decode(stored.payload);
  playback = { buttons, at: 0, label: stored.mode };
  current = mode(stored.mode) ?? null;
  game = new Game(seed, config, handling);
  views = new GameViews(game);
  clock.reset();
  input.clear();
  phase = "running";
  status.textContent = `watching a ${stored.mode} replay`;
  garbageTrack.classList.add("active");
  replaysScreen.hidden = true;
  showOverlay(false);
  draw();
}

async function openReplaysScreen(): Promise<void> {
  replayList.innerHTML = "";
  let saved: StoredReplay[] = [];
  try {
    saved = await listReplays(await openReplays());
  } catch {
    status.textContent = "saved games could not be read";
  }

  replaysEmpty.hidden = saved.length > 0;
  for (const stored of saved) {
    const li = document.createElement("li");

    const summary = document.createElement("div");
    summary.className = "replay-summary";
    summary.innerHTML =
      `<strong>${stored.mode}</strong>` +
      `<span>${formatTime(stored.ticks)} · ${stored.lines} lines · ${stored.pieces} pieces` +
      `${stored.finished ? "" : " · topped out"}</span>`;

    const play = document.createElement("button");
    play.type = "button";
    play.textContent = "watch";
    play.addEventListener("click", () => watch(stored));

    const file = document.createElement("button");
    file.type = "button";
    file.textContent = "save";
    file.addEventListener("click", () => {
      download(stored, (url, name) => {
        const link = document.createElement("a");
        link.href = url;
        link.download = name;
        link.click();
      });
    });

    li.append(summary, play, file);
    replayList.append(li);
  }
  replaysScreen.hidden = false;
}

openReplaysButton.addEventListener("click", () => void openReplaysScreen());
closeReplaysButton.addEventListener("click", () => {
  replaysScreen.hidden = true;
});

// ---- settings ---------------------------------------------------------------

/**
 * Notes are shown, never logged.
 *
 * They are the only signal that something a player chose could not be carried forward.
 */
function showNotes(notes: string[]): void {
  settingsNotes.hidden = notes.length === 0;
  settingsNotes.textContent = notes.join(" · ");
}

/** Apply settings that take effect without restarting. */
function applySettings(next: Settings): void {
  settings = save(window.localStorage, codec, next);
  theme = skin(String(settings.cosmetic.skin));
  input.rebind(keymap(settings.keybinds, buttonBits));
  sound.setVolume(Number(settings.cosmetic.effects_volume ?? 70));
  draw();
}

function openSettingsScreen(): void {
  buildPanel(settingsPanel, {
    settings,
    centiframes,
    onChange: applySettings,
    // Handling is frozen at construction, so a change lands on the next game.
    locked: () => phase === "running",
  });
  settingsScreen.hidden = false;
}

openSettings.addEventListener("click", openSettingsScreen);
closeSettings.addEventListener("click", () => {
  settingsScreen.hidden = true;
});

saveReplayButton.addEventListener("click", () => {
  if (!lastReplay) return;
  download(lastReplay, (url, name) => {
    const link = document.createElement("a");
    link.href = url;
    link.download = name;
    link.click();
  });
});

againButton.addEventListener("click", () => {
  if (current) start(current);
});
backButton.addEventListener("click", showMenu);
window.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && phase === "running") {
    finishEarly();
  }
});

function finishEarly(): void {
  const frame = read();
  if (frame) finish(frame);
}

buildMenu();
showMenu();

// A handle for driving the game without a display, which is how it is tested.
if (import.meta.env.DEV) {
  Object.assign(window, {
    openstacker: {
      step,
      draw,
      clock,
      input,
      start,
      showMenu,
      openSettingsScreen,
      watch,
      settings: () => settings,
      frame: read,
      state: () => ({ phase, mode: current?.id ?? null }),
    },
  });
}

requestAnimationFrame(frame);
