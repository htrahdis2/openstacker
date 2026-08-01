/**
 * Entry point: load the simulation, run it on the clock, draw it every frame.
 *
 * Ticks come from the clock and rendering from requestAnimationFrame. Tying the two
 * together would make the game speed depend on the display.
 */

import { Clock } from "./clock";
import { Input, attach, keymap } from "./input";
import { drawBoard, geometry } from "./render/board";
import { drawHold, drawNext, formatPps, formatTime, garbageFill, sizeBoxes } from "./render/hud";
import { skin } from "./render/palette";
import type { Shapes } from "./render/piece";
import { readFrame } from "./sim/frame";
import {
  Game,
  GameViews,
  buttonBits,
  defaultMatchConfig,
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

await initSim();

const settings = JSON.parse(defaultSettings());
const theme = skin(settings.cosmetic.skin);
const shapes = JSON.parse(pieceShapes()) as Shapes;
const game = new Game(newSeed(), defaultMatchConfig(), JSON.stringify(settings.handling));
const views = new GameViews(game);
const clock = new Clock();
const input = new Input(keymap(settings.keybinds, buttonBits));
attach(window, input);

const ctx = canvas.getContext("2d");
if (!ctx) throw new Error("this browser has no 2d canvas");

document.addEventListener("visibilitychange", () => {
  if (document.hidden) clock.pause();
  else clock.resume();
});

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

function step(ticks: number): void {
  for (let i = 0; i < ticks; i++) {
    // Sampled per tick, not per frame: a catch-up run must not replay one press.
    game.tick(input.consume());
  }
  draw();
}

function frame(now: number): void {
  const advance = clock.advance(now);
  if (advance.stalled) {
    status.textContent = "skipped a stalled gap";
  }
  step(advance.ticks);
  requestAnimationFrame(frame);
}

function draw(): void {
  views.refresh();
  const reading = readFrame(views.frame, simLayout());
  const geo = resize();

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

  if (reading.over) status.textContent = "topped out";
}

// A handle for driving the game without a display, which is how it is tested.
if (import.meta.env.DEV) {
  Object.assign(window, { openstacker: { step, draw, game, clock, input } });
}

requestAnimationFrame(frame);
draw();
