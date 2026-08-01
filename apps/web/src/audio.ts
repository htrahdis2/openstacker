/**
 * Sound.
 *
 * Every cue is triggered by a bit in the tick's event set, never by comparing this frame's
 * state to the last. That is what the event bitset is for, and diffing state would put a
 * second, disagreeing idea of what happened into the client.
 *
 * Tones are synthesized rather than loaded, so there are no assets to ship, nothing to
 * wait for on first play, and a cue is a few numbers to tune rather than a file to
 * re-record.
 */

import { EVENT } from "./sim/frame";

export interface Cue {
  id: string;
  /** Hertz. */
  freq: number;
  /** Seconds. */
  duration: number;
  type: OscillatorType;
  /** Relative loudness, before the player's volume is applied. */
  gain: number;
}

/**
 * Which cues a tick's events call for.
 *
 * `MOVED` and `SOFT_DROPPED` are deliberately silent: both fire on every step of
 * auto-repeat, and at a competitive ARR that is a cue every frame.
 */
export function cuesFor(events: number, linesCleared: number): Cue[] {
  const cues: Cue[] = [];

  if (events & EVENT.ROTATED) {
    cues.push({ id: "rotate", freq: 320, duration: 0.03, type: "triangle", gain: 0.18 });
  }
  if (events & EVENT.HELD) {
    cues.push({ id: "hold", freq: 460, duration: 0.05, type: "triangle", gain: 0.25 });
  }
  if (events & EVENT.HARD_DROPPED) {
    cues.push({ id: "drop", freq: 120, duration: 0.06, type: "square", gain: 0.3 });
  }
  if (events & EVENT.PIECE_LOCKED && !(events & EVENT.LINES_CLEARED)) {
    cues.push({ id: "lock", freq: 180, duration: 0.05, type: "sine", gain: 0.28 });
  }

  if (events & EVENT.LINES_CLEARED) {
    // Higher for more rows, so a quad is audibly better than a single.
    const rows = Math.min(Math.max(linesCleared, 1), 4);
    cues.push({
      id: `clear${rows}`,
      freq: 440 + (rows - 1) * 110,
      duration: 0.14 + rows * 0.02,
      type: "sine",
      gain: 0.4,
    });
  }
  if (events & (EVENT.SPIN | EVENT.MINI_SPIN)) {
    cues.push({ id: "spin", freq: 700, duration: 0.1, type: "triangle", gain: 0.35 });
  }
  if (events & EVENT.PERFECT_CLEAR) {
    cues.push({ id: "perfect", freq: 880, duration: 0.22, type: "sine", gain: 0.45 });
  }
  if (events & EVENT.TOPPED_OUT) {
    cues.push({ id: "topout", freq: 90, duration: 0.5, type: "sawtooth", gain: 0.35 });
  }
  return cues;
}

/** A player's volume, as a multiplier. */
export function volume(percent: number): number {
  return Math.min(Math.max(percent, 0), 100) / 100;
}

/**
 * Plays cues through WebAudio.
 *
 * The context starts suspended until the player has interacted with the page, which every
 * browser requires, so it is resumed from the first input rather than at load.
 */
export class Sound {
  private ctx: AudioContext | null = null;
  private level = 0.7;

  setVolume(percent: number): void {
    this.level = volume(percent);
  }

  /** Called from a real input, which is the only place a context may start. */
  resume(): void {
    if (!this.ctx) {
      const Ctor = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
      if (!Ctor) return;
      this.ctx = new Ctor();
    }
    if (this.ctx.state === "suspended") void this.ctx.resume();
  }

  play(events: number, linesCleared: number): void {
    if (!this.ctx || this.level === 0) return;
    for (const cue of cuesFor(events, linesCleared)) {
      this.emit(cue);
    }
  }

  private emit(cue: Cue): void {
    const ctx = this.ctx;
    if (!ctx) return;

    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = cue.type;
    osc.frequency.setValueAtTime(cue.freq, ctx.currentTime);

    // A short decay rather than a hard stop, which clicks.
    const peak = cue.gain * this.level;
    gain.gain.setValueAtTime(peak, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + cue.duration);

    osc.connect(gain).connect(ctx.destination);
    osc.start();
    osc.stop(ctx.currentTime + cue.duration);
  }
}
