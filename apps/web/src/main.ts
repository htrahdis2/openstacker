/**
 * Entry point: load the simulation, run it on the clock, draw it every frame.
 *
 * Ticks come from the clock and rendering from requestAnimationFrame. Tying the two
 * together would make the game speed depend on the display.
 */

import { Clock } from "./clock";
import { drawBoard, geometry, topRow } from "./render/board";
import { skin } from "./render/palette";
import { readFrame } from "./sim/frame";
import { Game, GameViews, defaultMatchConfig, defaultSettings, initSim, newSeed, simLayout } from "./sim/wasm";

const CELL = 30;

const canvas = document.getElementById("board") as HTMLCanvasElement;
const status = document.getElementById("status") as HTMLElement;
const statTick = document.getElementById("stat-tick") as HTMLElement;
const statTime = document.getElementById("stat-time") as HTMLElement;
const statPieces = document.getElementById("stat-pieces") as HTMLElement;
const statLines = document.getElementById("stat-lines") as HTMLElement;

await initSim();

const settings = JSON.parse(defaultSettings());
const theme = skin(settings.cosmetic.skin);
const game = new Game(newSeed(), defaultMatchConfig(), JSON.stringify(settings.handling));
const views = new GameViews(game);
const clock = new Clock();

const ctx = canvas.getContext("2d");
if (!ctx) throw new Error("this browser has no 2d canvas");

document.addEventListener("visibilitychange", () => {
  if (document.hidden) clock.pause();
  else clock.resume();
});

function resize(top: number): ReturnType<typeof geometry> {
  const dpr = window.devicePixelRatio || 1;
  const geo = geometry(CELL, top);
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
    game.tick(0);
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
  const geo = resize(topRow(views.occupancy, reading.active));

  drawBoard(ctx!, reading, views.occupancy, views.colors, geo, {
    skin: theme,
    ghostOpacity: settings.cosmetic.ghost_opacity,
    showGrid: settings.cosmetic.show_grid,
  });

  statTick.textContent = String(reading.tick);
  statTime.textContent = (reading.tick / 60).toFixed(2);
  statPieces.textContent = String(reading.pieces);
  statLines.textContent = String(reading.lines);
  if (reading.over) status.textContent = "topped out";
}

// A handle for driving the game without a display, which is how it is tested.
if (import.meta.env.DEV) {
  Object.assign(window, { openstacker: { step, draw, game, clock } });
}

requestAnimationFrame(frame);
draw();
