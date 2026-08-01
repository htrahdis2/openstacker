/**
 * Entry point: pick a mode, run it on the clock, draw it every frame.
 *
 * Ticks come from the clock and rendering from requestAnimationFrame. Tying the two
 * together would make the game speed depend on the display.
 */

import { Clock } from "./clock";
import { Input, attach, keymap } from "./input";
import { MODES, type Mode, goalReached, isPlayable, remaining } from "./modes";
import { drawBoard, geometry } from "./render/board";
import { drawHold, drawNext, formatPps, formatTime, garbageFill, sizeBoxes } from "./render/hud";
import { skin } from "./render/palette";
import type { Shapes } from "./render/piece";
import { type Frame, readFrame } from "./sim/frame";
import {
  Game,
  GameViews,
  buttonBits,
  defaultSettings,
  initSim,
  newSeed,
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

await initSim();

const settings = JSON.parse(defaultSettings());
const theme = skin(settings.cosmetic.skin);
const shapes = JSON.parse(pieceShapes()) as Shapes;
const clock = new Clock();
const input = new Input(keymap(settings.keybinds, buttonBits));
attach(window, input);

const ctx = canvas.getContext("2d");
if (!ctx) throw new Error("this browser has no 2d canvas");

/** What the client is doing. A game only advances while it is running. */
type Phase = "menu" | "running" | "done";

let phase: Phase = "menu";
let current: Mode | null = null;
let game: Game | null = null;
let views: GameViews | null = null;

document.addEventListener("visibilitychange", () => {
  if (document.hidden) clock.pause();
  else clock.resume();
});

function start(mode: Mode): void {
  current = mode;
  game = new Game(newSeed(), JSON.stringify(mode.config), JSON.stringify(settings.handling));
  views = new GameViews(game);
  clock.reset();
  input.clear();
  phase = "running";
  status.textContent = "";
  showOverlay(false);
  draw();
}

function finish(frame: Frame): void {
  phase = "done";
  const met = current ? goalReached(current.goal, frame) : false;
  showResult(frame, met);
}

function step(ticks: number): void {
  if (phase === "running" && game) {
    for (let i = 0; i < ticks; i++) {
      // Sampled per tick, not per frame: a catch-up run must not replay one press.
      game.tick(input.consume());
      const frame = read();
      if (frame && (frame.over || (current && goalReached(current.goal, frame)))) {
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
    ghostOpacity: settings.cosmetic.ghost_opacity,
    showGrid: settings.cosmetic.show_grid,
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
  resultList.hidden = true;
  modeList.hidden = false;
  againButton.hidden = true;
  backButton.hidden = true;
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
  for (const mode of MODES) {
    const li = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    button.innerHTML = `<strong>${mode.name}</strong><span>${mode.description}</span>`;
    if (isPlayable(mode.goal)) {
      button.addEventListener("click", () => start(mode));
    } else {
      button.disabled = true;
      button.title = "needs versus rules";
    }
    li.append(button);
    modeList.append(li);
  }
}

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
      state: () => ({ phase, mode: current?.id ?? null }),
    },
  });
}

requestAnimationFrame(frame);
