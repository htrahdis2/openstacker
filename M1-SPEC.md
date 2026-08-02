# M1 — Client · Detailed Spec v0.1

Companion to [SPEC.md](SPEC.md), following [M0-SPEC.md](M0-SPEC.md). Expands milestone
**M1** into an implementable document. Where this doc and `SPEC.md` disagree, this doc wins
for M1 scope and the disagreement is listed under [§17 Amendments](#17-amendments-to-specmd).

> **Deliverable in one sentence:** the simulation M0 built, made playable — a browser client
> with a canvas renderer, a real input path, a settings screen generated from the schema, and
> replay capture, with 40L sprint completable end to end.

---

## 1. Scope

### 1.1 In scope

| Area | Detail |
|---|---|
| **`crates/replay`** | `Replay` / `Outcome` moved out of `replay-cli` so the browser can produce the same format |
| **`crates/client-wasm`** | wasm-bindgen shim: construct, tick, expose state, capture replays |
| **`apps/web`** | Vite + TypeScript client. Menu, mode select, game, settings, results, replay list |
| **Loop** | Fixed 60Hz simulation decoupled from `requestAnimationFrame` |
| **Input** | Keyboard → `Buttons`, sub-frame press capture, rebinding |
| **Renderer** | Canvas2D board, ghost, hold, preview, spawn buffer, HUD |
| **Settings** | Generated from `config-schema.json`; validated in Rust; stored in `localStorage` |
| **Modes** | `modes.generated.json` emitted from the same TOML the server will read |
| **Replays** | Captured every game, stored in IndexedDB, played back, exported |
| **Audio** | Synthesized one-shots driven by the `Events` bitset |
| **CI** | Client build, TypeScript checks, unit tests, client↔native checksum parity |

### 1.2 Explicitly out of scope for M1

- `server`, `protocol`, WebSockets, rooms, anything networked. *(M3)*
- Garbage **routing**, cancellation between players, versus HUD. *(M2)*
- Accounts, database, server-side records. *(M4)*
- Touch and gamepad input, mobile layout.
- Skins loaded from files; M1 ships bundled themes only.
- WebGL, particle effects, music.
- `ts-rs` type generation — see decision 6.

### 1.3 The question M1 exists to answer

M0 built a complete handling state machine that no human has ever touched. DAS, ARR, DCD,
DAS-cut, IRS, IHS and misdrop protection are all implemented, tested, and unvalidated: a
property test proves the timers are consistent, not that the game feels right.

That is the point of client-before-versus in `SPEC.md` §9, and it shapes this milestone. The
client is not a demo of the engine; it is the instrument used to tune it. Two consequences
run through this document:

1. **Handling must be adjustable while playing**, from the same screen, without a rebuild.
2. **Nothing in the client may quietly alter timing.** A dropped or doubled tick invalidates
   a feel judgement as surely as a wrong DAS value, and it does it invisibly.

---

## 2. Decisions resolved for M1

| # | Question | Decision |
|---|---|---|
| 1 | Where `Replay` lives | **New `crates/replay`**, depending on `engine` + `serde` only. `replay-cli` and `client-wasm` both use it. The format does not change, so `REPLAY_VERSION` stays 1. |
| 2 | Who validates settings | **Rust.** `client-wasm` wraps `config::Settings::from_json`. TypeScript holds the stored text opaquely and renders controls from the schema. Clamping, migration and defaults have one implementation. |
| 3 | Pointer accessors (`SPEC.md` §5.4) | **In `client-wasm`, not `engine`.** `Board::rows()` and `Board::colors()` already return fixed arrays; the shim takes their addresses. The engine is not modified in M1. |
| 4 | Audio | **In scope, synthesized.** One short tone per event flag, no asset files. The five extra `Events` flags M0 added exist for this, and `Cosmetic` already carries the volume settings. |
| 5 | UI surface | Menu, mode select, game, settings, results with a local best, and a replay list with playback and export. |
| 6 | `ts-rs` | **Not used.** M0 §14 pencilled it in for M1, but the only types crossing the boundary in M1 are config types, and those are already described by `config-schema.json`. Revisit when `protocol` exists at M3. |
| 7 | Time | **Derived from the tick counter, never from the wall clock.** Sprint time, PPS and goal evaluation all come from `Stats::tick`, so a result is reproducible by re-simulating the replay. |
| 8 | `device_id` | Its own `localStorage` key, outside `Settings`. It never reaches the engine and has no descriptor, so it does not belong in a struct whose whole contract is that every field is describable. Amends `SPEC.md` §8.4. |
| 9 | UI framework | None, per `SPEC.md` §4. Plain TypeScript. |
| 10 | Seed source | `crypto.getRandomValues` at match start, recorded in the replay. Seeds are kept under 2^53 — see §10.5. |

---

## 3. Layout

```
/
├── crates/
│   ├── engine/          unchanged in M1
│   ├── config/          gains a `files` feature so it builds without I/O
│   ├── replay/          the replay format                            [NEW]
│   ├── client-wasm/     wasm-bindgen shim over engine + replay       [NEW]
│   └── replay-cli/      unchanged apart from importing `replay`
├── apps/
│   └── web/             vite + typescript client                     [NEW]
├── modes/               *.toml, unchanged
├── config-schema.json   unchanged, now consumed by the client
└── modes.generated.json emitted from modes/*.toml                    [NEW]
```

Dependency direction:

```
engine ← replay ← client-wasm ← web
engine ← config ← replay-cli
```

`config` is used by the client for settings and by the build for schema and mode emission.
Its file-reading half is feature-gated so the browser build carries no TOML parser.

### 3.1 Dependencies added

| Crate / package | Where | Why |
|---|---|---|
| `wasm-bindgen` | `client-wasm` | The only supported path to `--target web` |
| `serde_json` | `client-wasm` | Config and settings cross the boundary as JSON at init |
| `vite` | `apps/web` | Bundler (`SPEC.md` §4) |
| `vite-plugin-wasm`, `vite-plugin-top-level-await` | `apps/web` | Required by `wasm-pack --target web` output |
| `idb` | `apps/web` | ~1KB IndexedDB wrapper (`SPEC.md` §8.4) |
| `vitest` | `apps/web` | Unit tests |
| `fake-indexeddb` | `apps/web` | Storage tests without a browser |
| `playwright` | `apps/web` | One smoke test that the page loads and plays |

No UI framework, no CSS framework, no audio library.

---

## 4. The wasm boundary

### 4.1 Crossing budget

Two crossings per simulated frame: one call to advance the simulation, one read of the
resulting state. Everything the renderer needs is written into a single fixed-size block in
wasm memory, which JavaScript reads through a view created once. The board itself is read
directly from engine memory through two more views — `u16` occupancy and `u8` colors — never
copied and never crossed cell by cell.

Strings cross only at initialisation: match config, handling, and settings, all as JSON.
Nothing per-frame is a string, an object, or an allocation.

### 4.2 The frame block

The block carries exactly what a renderer and HUD need, and nothing that can be derived:

- tick, lines, pieces, attack sent, garbage received, best combo, best B2B
- this tick's `Events` bits, attack, and lines cleared
- phase, and whether the game is over
- the active piece's four cells, its kind, and **whether it exists at all**
- the ghost's four cells, and whether it exists
- hold kind and whether hold is occupied
- the preview, up to `preview_len` kinds
- pending garbage: total rows and how many ticks until the next batch lands

**Piece cells are supplied by the shim rather than derived in TypeScript.** Shape and
rotation tables are game rules (`SPEC.md` §14.2); a client that re-implements them has two
definitions of what an S piece is, and only one of them is tested.

**`active` and `ghost` are optional.** There is no falling piece during a spawn delay, a
clear delay, or after a topout, and the leftover value sits exactly on top of the cells it
just locked into. The block therefore carries explicit presence flags and the renderer
branches on them; drawing unconditionally paints a duplicate piece onto the stack.

### 4.3 Memory

Wasm memory is sized at initialisation and never grows. Growth detaches every existing
typed-array view, which surfaces as an empty board rather than an error. The engine is
fixed-size and the replay buffer is the only thing that grows during a game, so it is
reserved up front for the longest run the mode allows.

### 4.4 Exported surface

```rust
Game::new(seed, config_json, handling_json) -> Result<Game, JsError>
Game::tick(buttons: u8)          // advances one frame and refreshes the frame block
Game::frame_ptr() / occupancy_ptr() / colors_ptr()
Game::checksum() -> u64
Game::finish_replay() -> String  // the same JSON replay-cli reads

button_bits(action_key: &str) -> u8
load_settings(stored: &str) -> { settings: string, notes: string[] }
normalize_settings(json: &str) -> String
centiframes(ms: u32) -> u32
engine_ver() -> u32
```

`button_bits` exists so the mapping from an action name to a bit is fetched rather than
restated. `Action::key()` and `Action::button()` in
[client.rs](crates/engine/src/config/client.rs) are the source; the client asks for the eight
keys the schema gave it and builds its table at startup.

### 4.5 Board coordinates

Stated here because every renderer bug in this milestone will be one of these:

- The board is 40 rows. Rows 20–39 are visible; rows 0–19 are spawn buffer. Row 0 is the top
  of the buffer, not the top of the screen.
- Gravity increases `y`.
- Within a row's `u16`, bit 0 is the leftmost column.
- `colors` is indexed `y * 10 + x`. `0` is empty, `8` is garbage, `1..=7` are piece kinds.

---

## 5. The loop

### 5.1 One tick per 1/60s of game time

The simulation advances in whole ticks, driven by elapsed time, never by frame callbacks.
`requestAnimationFrame` schedules rendering; how many ticks run before that render is a
function of the clock alone.

The number of ticks owed is computed from total elapsed game time rather than accumulated
per-frame deltas, so it cannot drift:

```
ticksDue(elapsedMs, ticksRun) = floor(elapsedMs * 60 / 1000) - ticksRun
```

This is a pure function and is unit tested against a jittery clock: for any sequence of
frame times summing to the same elapsed total, the tick count must be identical.

### 5.2 The first tick is tick 1

`Engine::new` spawns the first piece with the counter at 0; the first `tick()` runs tick 1.
Recorded input index `i` is therefore tick `i + 1`. Garbage `apply_at_tick` is absolute, so
an off-by-one here lands rows a frame early or late once M2 arrives.

### 5.3 Stalls

A tab that was throttled or a machine that hitched returns owing hundreds of ticks. Running
them is technically correct — the simulation is deterministic and the inputs for those ticks
were empty — but it fast-forwards a run the player could not play.

Catch-up is therefore capped per frame, and a gap beyond about a second pauses the session
instead. Pausing costs nothing because time is tick-derived: a paused game has not advanced,
so its clock has not moved. `visibilitychange` pauses proactively.

### 5.4 Session states

`Menu → Countdown → Running ⇄ Paused → Finished`, plus `Replaying`, which is `Running` with
buttons read from a recording instead of the keyboard. Playback reuses the loop unchanged;
seeking re-simulates from tick 0, which costs under a millisecond for a sprint.

---

## 6. Input

### 6.1 Keys to buttons

A key event carries `event.code`, which is the same physical-key identifier `Keybinds`
stores. Resolution is a lookup from code to action to bit, built once at startup from the
settings and `button_bits`.

Keybinds never reach the engine; only the resulting `Buttons` do. That is what lets two
players with opposite layouts produce identical games.

### 6.2 Presses shorter than a frame

The client holds two masks: what is currently held, and what has been pressed since the last
tick consumed input. A tick is given the union, and the press mask clears afterwards.

Without this, a tap that begins and ends between two ticks is invisible, and the buttons that
suffer are the edge-triggered ones — rotation, hold, hard drop — where a lost press is a lost
placement. With it, a sub-frame tap produces exactly one press edge on the next tick, which
is what `Buttons::pressed_since` is built to detect.

Input is sampled immediately before each tick, not at the start of the frame.

### 6.3 Repeats, focus, and blur

- Auto-repeat `keydown` events are ignored. Repeat is DAS, and DAS is in the engine.
- Losing focus clears the held mask; a key released while the page is unfocused otherwise
  stays held forever and the piece slides into the wall.
- Keys bound to game actions have their default browser behaviour suppressed while a game is
  running, and only then.

### 6.4 Rebinding

The key-capture control writes whatever `event.code` it receives. Names are not validated —
`Keybinds::clamp` deliberately does nothing, because the next browser to spell a key
differently would otherwise become unplayable. A binding already used by another action is
flagged, not refused.

---

## 7. Renderer

### 7.1 Geometry

Canvas2D, sized for device pixel ratio, cell size derived from `Cosmetic::board_scale`. The
grid is drawn on a separate canvas that is redrawn only when size or settings change; the
playfield layer is cleared and redrawn each frame. A full redraw of 200 visible cells per
frame is cheap enough that dirty-rectangle tracking would cost more than it saves.

### 7.2 Draw order

Grid → locked cells from the color channel → ghost → active piece → overlays.

The ghost is skipped when there is no active piece or when `ghost_opacity` is 0.

### 7.3 The spawn buffer

The visible region is rows 20–39, extended upward whenever anything occupies the buffer.
Without that, a stack that tops out looks like it ended for no reason: the piece that killed
the player was never drawn.

### 7.4 Palette

`SPEC.md` §1 makes the palette a legal constraint, not a taste question. The bundled themes
must not reproduce the protected mapping — no cyan I, yellow O, purple T, green S, red Z,
blue J, orange L.

Skins are data: a map from color index (`0` empty, `1..=7` kinds in `QuadKind` order, `8`
garbage) to a color. Three ship, matching the variants `Cosmetic` already declares: `default`,
`mono`, `high_contrast`. Adding one is a JSON entry.

**Colors decide nothing.** The color channel is excluded from the checksum precisely so that
two peers with different skins are not desynced; a renderer that reads a color to decide
whether a cell is garbage has recreated the coupling that exclusion exists to prevent. Ask
the occupancy grid.

### 7.5 HUD

Time, lines, pieces and PPS, all derived from the tick counter. Hold, preview sized by
`preview_len`, and a pending-garbage bar fed from the frame block. The bar renders empty in
sprint; it is built now so that M2 adds numbers to an existing HUD rather than a HUD.

---

## 8. Settings

### 8.1 Storage

One `localStorage` key holds the settings JSON. A second holds `device_id`, generated on
first load. `device_id` restores local state and authorises nothing.

### 8.2 Loading

Stored text goes to `load_settings`, which returns the settings and a list of notes. Loading
never fails: unknown keys are ignored, missing ones default, out-of-range values clamp, and a
damaged section costs only that section.

**Notes are shown to the player, not logged.** They are the only signal that something a
player chose could not be carried forward, and they are already written as sentences a player
can read. A note in a console is a note nobody sees.

Every write goes back through `normalize_settings`, so what is stored is always what the
engine would actually use.

### 8.3 The screen is generated

Controls are rendered from `config-schema.json`, which carries bounds, defaults, step, unit,
label, help text and UI group for all 45 settings:

| Schema type | Control |
|---|---|
| `int` | Slider plus numeric entry, honouring min/max/step |
| `bool` | Toggle |
| `enum` | Segmented control, one option per variant, with variant help |
| `binding` | Key capture |

Fields are grouped by their declared UI group. Nothing about any individual setting is
hard-coded in TypeScript; adding a setting is a Rust-side change and the control appears.
CI already fails if the committed schema drifts from the engine.

### 8.4 Milliseconds are canonical

Handling is stored in milliseconds. Frames are a derived read-out via `centiframes`, and they
quantise to roughly ±0.03 F. A UI that lets a player type `8.5 F` and hands it back will read
`8.52 F` and look broken.

So: the editable control is milliseconds, with the frame equivalent shown beside it.

### 8.5 What locks during a run

Groups the schema marks `affectsSimulation` are frozen while a game is running; the rest can
change at any time and take effect immediately. In M1 the match rules come from the mode file
and are shown read-only, labelled with the reason wording
[resolve.rs](crates/config/src/resolve.rs) already provides. A control the player can drag
that silently does nothing is worse than a disabled one.

Handling changes take effect on the next game, not the current one, because handling is
frozen at construction.

---

## 9. Modes in the browser

A build step emits `modes.generated.json` from `modes/*.toml` — the same files the server
will read at M3 — carrying each mode's id, name, description, goal and config. The browser
gets no TOML parser, and there is no second definition of a mode.

The emitter mirrors `emit-schema`: it prints to stdout, takes `--check` to compare against
the committed file, and CI runs the check. A mode added by a self-hoster is a file plus a
build, not a code change.

### 9.1 Goals

The engine is goal-agnostic and never sees a goal — that is what keeps a replay
`(seed, MatchConfig, Handling, inputs)` and nothing more. The client evaluates the goal from
`Stats` after each tick:

| Goal | M1 |
|---|---|
| `lines { count }` | Finished when `stats.lines >= count`. This is sprint40, the milestone target. |
| `time { ms }` | Finished when the tick counter passes the equivalent. Playable; scoring is M2. |
| `score`, `survival` | Not evaluated in M1. Those modes are listed but not selectable. |

A game also ends when the engine reports a topout.

---

## 10. Replays

### 10.1 The crate move

`Replay`, `Outcome` and `REPLAY_VERSION` move from `replay-cli` into `crates/replay`
unchanged, with their tests. The format is untouched, so existing golden replays remain
valid and byte-identical.

### 10.2 Capture

Every game is recorded: one `Buttons` value per tick, run-length encoded on completion by
the same code the CLI uses. A recording is the seed, the config, the handling, and the
inputs — everything else a viewer sees is re-derived.

The claimed outcome is filled in from the engine at the end, which makes the capture path
self-checking: a browser replay that does not reproduce its own claim is a bug in the client,
and `replay-cli verify` will say so.

### 10.3 Storage

IndexedDB, two stores: recordings, and one local best per mode. A sprint replay is a few
hundred bytes, so nothing is evicted. `localStorage` is not used for these — it is
synchronous and capped near 5MB.

### 10.4 64-bit values in a language without them

Seeds and checksums are `u64`. `JSON.parse` puts them through a double and silently rounds
anything past 53 bits, which turns a valid recording into one that plays a different game.

Two rules follow, and both are load-bearing:

- Client-generated seeds are drawn from 48 bits, so a replay's seed survives a JSON round
  trip unchanged.
- Checksums are never parsed as numbers in TypeScript. They are read from the text as
  `BigInt` and compared as such.

This is not hypothetical: the first run of the parity harness reported all five golden
replays as mismatched, and the mismatch was in the harness reading the expected value.

### 10.5 Playback and export

Playback drives the same loop with recorded buttons. Export downloads the file, which is the
format `replay-cli` reads, so a player can hand a run to anyone with the repo and have it
verified.

---

## 11. Audio

One audio context, resumed on first input as browsers require, and a short synthesized tone
per event: lock, clear, spin, quad, hold, rotate, hard drop, topout. Volume comes from
`Cosmetic`.

Sound is triggered by the `Events` bits in the frame block and never by comparing this
frame's state to the last. That rule is why M0 defined `ROTATED`, `MOVED`, `HARD_DROPPED`,
`SOFT_DROPPED` and `SPAWNED` at all, and diffing state would reintroduce exactly the coupling
the bitset removes.

---

## 12. Determinism obligations

M1 adds a second place the simulation runs. The obligations that come with it:

1. **No engine changes.** M1 touches no rule, so every golden checksum must be unmoved and
   `ENGINE_VER` stays 1. A moved checksum in this milestone is a bug, not a rules update.
2. **If handling semantics do change**, because playtesting demands it, that is a deliberate
   `ENGINE_VER` bump with the goldens regenerated in the same commit — not a re-pin of
   whatever numbers make the tests pass.
3. **TypeScript implements no game rule**, including timing, geometry, and scoring. Its only
   timing responsibility is deciding how many whole ticks are owed.
4. **The client build is checked, not just the engine.** `wasm-parity.sh` proves the engine
   crate matches across targets; M1 adds a check that the artifact players actually run
   produces the pinned checksums too.

---

## 13. Test strategy

### 13.1 Rust

| Level | Covers |
|---|---|
| Unit, `crates/replay` | The moved tests, unchanged — encoding, round trip, verification, tamper detection |
| Unit, `crates/client-wasm` | Frame block contents against a known engine state, including the no-active-piece and buffer-occupied cases; `button_bits` matching `Action::button`; settings pass-through preserving notes |
| Build | `config` compiles with its file feature off; `client-wasm` builds for `wasm32-unknown-unknown` |

The frame block is built by a pure function over `&Engine`, so it is tested natively rather
than through a browser.

### 13.2 TypeScript

| Level | Covers |
|---|---|
| Unit | `ticksDue` under jittery and stalled clocks; held/pressed mask semantics including sub-frame taps and multi-tick catch-up; code→action→bit resolution; schema→control mapping for all four control types; goal evaluation |
| Storage | Replay and best-time stores against `fake-indexeddb` |
| Smoke | One Playwright test: page loads, a sprint starts, keys place pieces, the line counter moves |

### 13.3 Integration

Two tests that span the boundary, both runnable in CI:

1. **Client parity.** Every golden replay in `testdata/replays/` is run through the built
   client wasm module and its checksum compared with the pinned value. This is the check that
   the thing players run agrees with the thing the server will run.
2. **Capture round trip.** A scripted button stream is driven through the client, the
   captured replay is written out, and `replay-cli verify` is run on it. It must verify with
   no edits.

### 13.4 What only a human can test

These are the reason M1 exists, and none of them can be automated. Each needs a pass before
the milestone is called done:

| Check | What good looks like |
|---|---|
| DAS/ARR at 133/0 | Tap moves one column; hold slides to the wall with a distinct pause first |
| DAS/ARR at 0/0 | The piece is at the wall the instant a direction is held; single taps still place |
| DAS/ARR at 100/33 | Repeat is visibly stepped rather than instant, and even |
| Direction change with DCD set | Reversing does not re-charge a full DAS |
| DAS-cut delay | A direction held across a spawn does not fling the new piece into the wall |
| IRS and IHS | A rotation or hold held through the spawn delay applies on the first frame |
| Misdrop protection | A fast repeat on hard drop does not slam the next piece |
| Soft drop, instant and timed | Instant reaches the floor in one frame; timed descends evenly |
| Topout above the field | The buffer becomes visible and the killing piece is drawn |
| Input latency | Placements feel immediate at 60Hz; no perceptible lag versus keypress |
| Settings notes | A hand-corrupted settings file loads, keeps what survived, and says what changed |
| Audio | Every event has a distinct sound; the volume sliders do what they say |

---

## 14. Performance

| Budget | Target |
|---|---|
| Boundary crossings per simulated frame | 2 |
| Allocation in the loop | None after startup |
| Wasm memory growth after init | Never |
| Render time per frame | Comfortably inside the frame at 60Hz |
| Added input latency | At most one tick beyond what the browser imposes |

---

## 15. CI

Added to the existing jobs in [ci.yml](.github/workflows/ci.yml):

| Job | Purpose |
|---|---|
| `wasm-pack build` | The client shim compiles for the browser target |
| `pnpm typecheck` | No untyped drift in the client |
| `vitest run` | §13.2 |
| Client parity | §13.3.1 — the built client agrees with the pinned checksums |
| Capture round trip | §13.3.2 |
| `emit-modes --check` | The generated mode file matches `modes/*.toml` |

The existing engine purity guard, wasm parity script and schema check stay exactly as they
are.

---

## 16. Acceptance criteria

M1 is done when:

1. `cargo test --workspace` and `pnpm test` are green.
2. A fresh clone runs one documented command and reaches a playable sprint in the browser.
3. **40L sprint is completable**, with a results screen showing time, lines, pieces and PPS,
   and a local best that survives a reload.
4. Every golden replay produces its pinned checksum through the built client, not only
   through the engine crate.
5. A replay captured in the browser passes `replay-cli verify` unmodified.
6. A replay captured in the browser plays back in the browser and reaches the same result.
7. The settings screen renders all 45 settings from the schema with no per-setting client
   code, and adding a handling field in Rust makes a working control appear with no
   TypeScript change.
8. Handling is editable between games and a change is felt immediately on the next game.
9. A corrupted settings file loads, preserves the sections that survived, and shows the
   player what changed.
10. A stack topping out is visible in the spawn buffer rather than vanishing.
11. Golden checksums are unmoved and `ENGINE_VER` is still 1.
12. The manual checklist in §13.4 has been walked once, on a real machine, by a human.

---

## 17. Amendments to `SPEC.md`

| § | Change | Rationale |
|---|---|---|
| §3 | New crate `crates/replay` | The browser and the CLI produce the same format; it cannot live inside one of them. |
| §5.4 | `occupancy_ptr` / `colors_ptr` live in `client-wasm`, not `engine` | The engine already exposes fixed arrays. Adding raw pointers to a crate published for bots and solvers buys nothing and costs its clean API. |
| §7 | The renderer reads a frame block, not individual accessors | Same crossing budget, one read instead of a dozen, and it is where the optional-piece flags live. |
| §8.4 | `Settings` carries no `deviceId` | It never reaches the engine and cannot be described by a settings descriptor. Separate key. |
| §4 | Audio moves from "deferred" into M1, synthesized rather than sampled | The event flags exist, the volume settings exist, and a generated settings screen should not render controls that do nothing. |
| §4 | `ts-rs` not used in M1 | Nothing but config crosses the boundary yet, and the schema already describes it. Revisit at M3 with `protocol`. |
| §9 | M1 also includes replay playback and a replay list | Capture without playback cannot be verified by the person who recorded it. |

---

## 18. Build order

Each step ends somewhere testable, and each is a separate reviewable change.

| # | Step | Ends with |
|---|---|---|
| 1 | Extract `crates/replay`; `replay-cli` imports it | Workspace tests green, goldens unchanged |
| 2 | Feature-gate file I/O in `config` | `config` builds with the feature off |
| 3 | `crates/client-wasm`: `Game`, frame block, settings and button exports | Native unit tests on the frame block; wasm target builds |
| 4 | Client parity script + CI job | Every golden checksum reproduced through the built client |
| 5 | `apps/web` skeleton: Vite, wasm loading, canvas, the loop | A board renders and the clock advances |
| 6 | Input layer | A piece can be moved, rotated, and dropped by hand |
| 7 | Renderer completion: skins, ghost, buffer, HUD | Acceptance criterion 10 |
| 8 | `emit-modes`, mode select, goal evaluation | Acceptance criterion 3 without the results screen |
| 9 | Settings: load path, generated screen, notes | Acceptance criteria 7 and 9 |
| 10 | Replay capture, storage, export | Acceptance criterion 5 |
| 11 | Replay list and playback | Acceptance criterion 6 |
| 12 | Audio | The §13.4 audio row |
| 13 | Results screen and local best | Acceptance criterion 3 |
| 14 | Remaining CI wiring, docs | Acceptance criteria 1 and 2 |

Steps 1–4 come first because they are verifiable without a browser. Every later step rests on
a boundary already proven to produce the right numbers, so a divergence found afterwards is a
client bug and not an open question about which side is wrong.

---

## 19. Deferred to M2+

| Item | Milestone | Note |
|---|---|---|
| Garbage bar with real numbers | M2 | The HUD element ships in M1, empty |
| Versus HUD, opponent boards | M3 | Snapshot rendering, not simulation |
| Attack table values | M2 | Tuned by playtest; the mechanism is M0's |
| `ts-rs` types | M3 | With `protocol` |
| Postcard encoding | M3 | JSON replays until then |
| Skins loaded from files | M4+ | Bundled themes in M1; user uploads are a moderation problem |
| Touch and gamepad | M4+ | `SPEC.md` non-goal for v1 |
| Server-side records | M4 | Local bests only in M1 |
| Score goals | M2 | Needs a scoring model beyond attack |
