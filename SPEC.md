# [PROJECT NAME] — Technical Spec v0.1

An open-source competitive falling-block stacker. Rust engine, TypeScript client, self-hosted, no ads.

> **Status:** draft for iteration. Sections marked **[DECIDE]** are open questions, collected at the end.

---

## 0. Goals & non-goals

**Goals**

- Competitive-grade feel: SRS+ rotation, configurable DAS/ARR/SDF, sub-frame-accurate input handling.
- Server-authoritative multiplayer that is cheap to run (single VPS, hundreds of concurrent players).
- Deterministic engine → free replays, free server verification, free desync detection.
- Genuinely forkable: one binary + static assets, `docker compose up` for a private lobby.
- No ads, no telemetry, no accounts required to play.
- **No open core.** Everything ships in the repo; the hosted instance is a config, not a better build (§2).

**Non-goals for v1**

- Mobile / touch controls.
- Ranked ladder, ELO, seasons.
- Mod or skin loading (design shouldn't preclude it; don't build it).
- Rollback netcode — architecturally unnecessary, see §6.3.
- Rendering beyond Canvas2D.

---

## 1. Legal constraints (binding on design)

Falling-block *mechanics* are not copyrightable; the *expression* in Tetris is (*Tetris Holding v. Xio*, D.N.J. 2012 — Xio wrote all its own code and still lost). Rules that follow:

| Constraint | Rationale |
|---|---|
| Name contains no `tris` / `tetr` / `tetri` | Trademark, aggressively enforced |
| Pieces are called **quads**; a line clear is a **clear**; four at once is a **quad clear** | Avoid protected terminology |
| Own color palette — do **not** use cyan-I / yellow-O / purple-T / green-S / red-Z / blue-J / orange-L | The single most identifiable protected element |
| Own board proportions, own UI layout, own effects | Cumulative "look and feel" is what sank Xio |
| SRS kick tables, 7-bag, DAS/ARR **are** implemented faithfully | Functional, publicly documented, required for competitive parity |

Prior art that has survived years doing exactly this: **Techmino**, **NullpoMino**.

---

## 2. Distribution & contribution model

### 2.1 One codebase, three tiers — no open core

Everything ships in the repo. Tiers are **configuration**, not feature gates. Nothing is withheld from self-hosters.

| Tier | Enable with | What you get | Persistent state |
|---|---|---|---|
| **0 — default** | `./server` | Anonymous play, room codes, all game modes, full handling config, replays saved in-browser | **none** |
| **1** | `DATABASE_URL=...` | Accounts, persistent leaderboards, server-side replays, cross-device settings | SQLite file |
| **2 — public instance** | config file | Rate limiting, moderation tools, ranked ladder, account recovery | SQLite + object store |

The hosted instance is Tier 2 with a deployment config and nothing more. A self-hoster who wants accounts sets one env var.

What this *does* buy as a maintainer is scope discipline: "core doesn't take PRs for cosmetic shops or season passes" is a legitimate boundary, and it's a completely different thing from "self-hosters get a worse game."

### 2.2 Contribution surfaces, by leverage

1. **`engine` published standalone to crates.io.** Zero deps, no I/O, `no_std`-friendly. Enables TUI clients, bots, solvers, opener trainers, analysis tools — contributions that never touch the server and cost nothing to maintain. Largest surface by far.
2. **Game modes as data** (§5.7). TOML files, not code. Non-programmers can contribute; modes become shareable artifacts rather than PRs.
3. **Skins as data.** JSON manifest + image, M4+.
4. **The determinism test suite.** Golden replays plus native/wasm checksum parity means you can merge a stranger's kick-table refactor with confidence. Most game projects cannot safely accept gameplay PRs. This is the single biggest thing separating a repo that accumulates contributors from one that accumulates stale forks.

### 2.3 Licensing — the split is load-bearing

| Path | License | Why |
|---|---|---|
| `crates/engine`, `crates/protocol` | **MIT OR Apache-2.0** | Rust ecosystem convention. An AGPL crate on crates.io is one nobody depends on — that would kill surface #1 outright. |
| `crates/server`, `apps/web` | **MIT** *(recommended)* | **[DECIDE]** |

AGPL on the server would not prevent forking — it prevents *closed* forking of a hosted service. That's worth something only if a closed ad-supported rehost would bother you. Stated goal is community contribution and a fun game, so MIT throughout is the coherent choice and the simpler one. Revisit only if that changes.

### 2.4 Zero-config is the adoption metric

Nobody forks a project because it does less. They fork it because it ran on the first try. Concrete targets, treated as bugs when they regress:

- `cargo run -p server` → playable at `localhost:3000`. No config file, no DB, no external service.
- `docker compose up` → the same, with TLS.
- `git clone` → a friend joining your room: **under 5 minutes.**

---

## 3. Repository layout

```
/
├── crates/
│   ├── engine/          pure sim. no I/O, no time, no async, no floats.
│   ├── protocol/        wire types. serde + postcard + ts-rs.
│   ├── server/          axum + room actors.
│   ├── client-wasm/     wasm-bindgen shim over engine.
│   └── replay-cli/      native harness: run replay → print checksum.
├── apps/
│   └── web/             vite + ts. canvas, input, menus, transport.
├── modes/               *.toml — game modes as data. Server reads at startup.
├── testdata/
│   └── replays/         golden replays + expected checksums.
├── LICENSE-MIT
├── LICENSE-APACHE       (engine + protocol are dual-licensed, §2.3)
└── justfile
```

Cargo workspace at root; pnpm workspace under `apps/`. `just dev` runs `wasm-pack --watch`, `vite dev`, and `cargo watch -x run -p server` together.

**Dependency direction is strictly one-way:**

```
engine ← protocol ← server
       ← client-wasm ← web
```

`engine` depends on nothing in the workspace. `protocol` may reference engine types but not vice versa.

---

## 4. Library choices

The ask was "use existing libraries, especially networking." Concretely:

### Server

| Concern | Choice | Why |
|---|---|---|
| HTTP + WS | **`axum`** with `axum::extract::ws` | Revision from earlier discussion: axum's built-in WS extractor handles the upgrade for you. Reaching for `tokio-tungstenite` directly is unnecessary — axum wraps it. |
| Runtime | **`tokio`** (`rt-multi-thread`, `macros`, `sync`, `time`) | — |
| Static assets | **`tower-http`** `ServeDir` + `CompressionLayer` | Single binary serves the client. No nginx needed for v1. |
| Logging | **`tracing`** + `tracing-subscriber` | Structured spans per room are genuinely useful for netcode debugging |
| IDs | **`uuid`** v4 | — |
| Room lookup | `tokio::sync::RwLock<HashMap<RoomId, mpsc::Sender<RoomMsg>>>` | No crate needed. `dashmap` only if lock contention shows up, which it won't. |
| Persistence | **deferred to M4** — then `sqlx` + SQLite | Rooms are in-memory. Nothing to persist in v1. |

### Protocol

| Concern | Choice | Why |
|---|---|---|
| Serialization | **`serde`** + **`postcard`** | Compact, no-std, zero-config. `bitcode` is smaller but immature. |
| TS type generation | **`ts-rs`** | Derive `TS` on protocol structs, `cargo test` emits `.ts`. Prevents client/server drift. |
| v0.1 escape hatch | `serde_json` behind a feature flag | Debug in the browser console; flip to postcard when the protocol stabilizes. |

### Engine

| Concern | Choice | Why |
|---|---|---|
| Input flags | **`bitflags`** | — |
| Fixed collections | **`arrayvec`** | Bounded, no allocation, no-std |
| PRNG | **hand-rolled SplitMix64** (~15 lines) | **Deliberately not `rand`.** A version bump silently invalidating every stored replay is a bad afternoon. |
| Property tests | **`proptest`** | Kick tables and lock-delay state machine |
| Snapshot tests | **`insta`** | Golden replay checksums |

**Banned in `engine/`:** floats, `HashMap` (iteration order isn't build-stable — use arrays or `BTreeMap`), `std::time`, anything allocating per tick.

### Client

| Concern | Choice | Why |
|---|---|---|
| Bundler | **Vite** | No SSR value for a game canvas. Fast HMR. |
| WASM glue | **`wasm-pack --target web`** + `vite-plugin-wasm` + `vite-plugin-top-level-await` | Standard path; the two plugins save real pain |
| Rendering | **Canvas2D** | A 10×20 grid does not need WebGL. Revisit only if profiling says so. |
| UI framework | **none in v0.1** — plain TS | Menus are a few screens. Add Preact at M4 if it hurts. |
| Audio | **deferred** — then plain WebAudio | Howler is overkill for ~10 one-shots |
| Transport | native `WebSocket` | — |

**No Next.js.** If you want a marketing page / leaderboards / SEO later, run it as a *separate* app alongside the game server.

---

## 5. Engine

### 5.1 Constants

```rust
pub const BOARD_W: usize = 10;
pub const BOARD_H: usize = 40;        // 20 visible + 20 buffer
pub const TICK_HZ: u32 = 60;
pub const PREVIEW_LEN: usize = 5;
pub const FULL_ROW: u16 = 0b11_1111_1111;
```

### 5.2 State

```rust
pub struct Engine {
    occupancy: [u16; BOARD_H],   // bit per column; row == FULL_ROW means clear
    colors:    [u8; BOARD_W * BOARD_H],  // render-only. game logic NEVER reads this.
    active:    Piece,
    hold:      Option<QuadKind>,
    can_hold:  bool,
    queue:     ArrayVec<QuadKind, 14>,   // two bags
    rng:       SplitMix64,
    garbage:   ArrayVec<PendingGarbage, 32>,
    tick:      u32,
    das_timer: u8,
    arr_timer: u8,
    lock_timer: u8,
    lock_resets: u8,
    combo:     u8,
    b2b:       u8,
    state:     Phase,   // Spawning | Falling | Locking | ClearDelay | Dead
    config:    MatchConfig,
    handling:  Handling,
}
```

`colors` is a strict render channel. If a rule ever reads it, determinism is at risk.

### 5.3 Input

Input is **buttons held this tick**, never actions:

```rust
bitflags! {
    pub struct Buttons: u8 {
        const LEFT      = 1 << 0;
        const RIGHT     = 1 << 1;
        const CW        = 1 << 2;
        const CCW       = 1 << 3;
        const FLIP      = 1 << 4;   // 180
        const HOLD      = 1 << 5;
        const SOFT_DROP = 1 << 6;
        const HARD_DROP = 1 << 7;
    }
}
```

DAS / ARR / SDF live **inside** the engine and are derived from held state. The client physically cannot express "I moved left 10 times this frame" — movement isn't in the protocol. A cheater's only lever is emitting button patterns a human couldn't produce, which is a statistics problem, not a correctness one.

Rotation and hard drop are edge-triggered (fire on press, not hold); the engine tracks `prev_buttons` internally.

### 5.4 Public API

```rust
impl Engine {
    pub fn new(seed: u64, config: &MatchConfig, handling: &Handling) -> Self;  // see §8.2

    /// The only mutator. Fully deterministic on (state, input).
    pub fn tick(&mut self, input: Buttons) -> Events;

    /// Server-authoritative. apply_at_tick is always in the future.
    pub fn schedule_garbage(&mut self, g: PendingGarbage);

    pub fn checksum(&self) -> u64;

    // read-only accessors for the render layer
    pub fn occupancy_ptr(&self) -> *const u16;
    pub fn colors_ptr(&self) -> *const u8;
}

bitflags! {
    pub struct Events: u16 {
        const PIECE_LOCKED = 1 << 0;
        const LINES_CLEARED = 1 << 1;
        const SPIN = 1 << 2;
        const MINI_SPIN = 1 << 3;
        const B2B_CONTINUED = 1 << 4;
        const B2B_BROKEN = 1 << 5;
        const PERFECT_CLEAR = 1 << 6;
        const GARBAGE_APPLIED = 1 << 7;
        const TOPPED_OUT = 1 << 8;
        const HELD = 1 << 9;
    }
}
```

Returning a bitset (plus an `attack: u8` field on a small struct wrapping it) keeps the shell reactive — audio and network react to `Events`, never to introspecting state.

### 5.5 Attack table **[DECIDE]**

Starting proposal, all tunable in `MatchConfig`:

| Clear | Attack |
|---|---|
| Single | 0 |
| Double | 1 |
| Triple | 2 |
| Quad | 4 |
| Mini spin single | 0 |
| Mini spin double | 1 |
| Spin single | 2 |
| Spin double | 4 |
| Spin triple | 6 |
| Perfect clear | +10 |
| Back-to-back | +1 |

Combo bonus: `[0,0,1,1,1,2,2,3,3,4,4,4,5]`, saturating.

Spin detection: 3-corner rule, with the mini/full distinction determined by which kick index was used.

### 5.6 Garbage

```rust
pub struct PendingGarbage {
    pub apply_at_tick: u32,
    pub amount: u8,
    pub hole_col: u8,
}
```

- `GARBAGE_DELAY_TICKS = 60` (1000ms) **[DECIDE]**. Hard constraint: must exceed `RTT + batch_interval + jitter margin` with room to spare.
- Hole column repeats within a garbage batch; new column per batch (server-chosen, from the *server's* RNG stream, sent explicitly — never derived client-side).
- Cancellation is resolved **server-side** against the receiving player's pending queue before a `GarbageIncoming` is emitted.

---

### 5.7 Game modes are data, not code

`MatchConfig` is `Deserialize`. A mode is a TOML file.

```toml
# modes/sprint40.toml
name          = "Sprint 40"
goal          = { type = "lines", count = 40 }
gravity       = { type = "fixed", ticks_per_row = 60 }
lock_delay_ticks = 30
lock_reset_cap   = 15
preview_len      = 5
garbage_delay_ticks = 60

[attack_table]
single = 0
double = 1
triple = 2
quad   = 4
```

Loading:

- **Server** reads `modes/*.toml` from a directory next to the binary at startup. A self-hoster adds a mode without recompiling — that's the forkability story made concrete.
- **Client** gets `modes.generated.json`, emitted by a build step from the same TOML files, bundled by Vite. Avoids a TOML parser in the browser.
- **`replay-cli`** takes `--mode path/to.toml`.

**Land this in M0.** Retrofitting it means changing how every `Engine` gets constructed, and it's the difference between "new mode = a PR to Rust source" and "new mode = a file someone can post in Discord."

Custom modes must be part of the replay payload (`MatchConfig` already is, §8.5), or replays of community modes become unverifiable.

---

## 6. Networking

### 6.1 The split

The state machine runs in **both** places. This is the core of the design.

```
CLIENT A                  SERVER                   CLIENT B
engine(A) ──inputs──────► engine(A)
  ▲                       engine(B) ◄───inputs──── engine(B)
  │                          │                        ▲
  └──garbage/snapshots───────┴───garbage/snapshots────┘
```

Client-side sim exists **only** so input feels instant. The server's copy is the truth. Clients never report outcomes — the server derives them.

### 6.2 Messages

```rust
// client → server
enum ClientMsg {
    JoinRoom { room: RoomId, name: String, handling: Handling },
    Ready,
    InputBatch { start_tick: u32, buttons: Vec<u8> },  // ~every 50ms (3 ticks)
    Sync { tick: u32, checksum: u64 },                 // ~every 1s
}

// server → client
enum ServerMsg {
    MatchStart { seed: u64, config: MatchConfig, start_epoch_ms: u64,
                 players: Vec<PlayerInfo> },  // PlayerInfo carries that player's Handling
    GarbageIncoming { apply_at_tick: u32, amount: u8, hole_col: u8 },
    OpponentBoard { player: u8, tick: u32, occupancy: [u16; 40] },  // ~10Hz
    PlayerOut { player: u8, place: u8 },
    MatchEnd { winner: u8 },
    Desync { at_tick: u32 },
    Kick { reason: KickReason },
}
```

Opponent boards are **snapshots, not simulations** — you never run an opponent's engine locally. 80 bytes at 10Hz renders a mini board fine and keeps client cost flat in player count.

### 6.3 Why no rollback

The server runs *behind* the client by roughly one RTT. It confirms; it never predicts. The client is therefore never wrong about its own board and never corrects.

This works because the only external input to your board is garbage, and garbage is scheduled at a future tick. `apply_at_tick` sits ~1000ms ahead against ~50ms RTT, so the message always lands before the tick arrives. Both sides apply it on the same tick, deterministically. That gap is the entire reason this game class is tractable without rollback.

### 6.4 Anti-cheat

| Vector | Mitigation |
|---|---|
| Fake line clears | Server re-simulates from inputs; client outcomes are ignored entirely |
| Impossible movement | Not expressible — protocol carries held buttons, not actions |
| Modified handling | `Handling` bounds-validated at join, frozen at `MatchStart`, server sims with the same values |
| Tampered engine | `Sync` checksum every ~1s; mismatch → log + kick |
| **Stalling** | Server can't advance a board without inputs. Wall-clock guard: assert each player's tick tracks real elapsed time within tolerance; forfeit on excessive drift. **Build this in M3, not later.** |
| Input-pattern bots | Out of scope for v1. Statistical detection is an M4+ problem. |

### 6.5 Room actor

One `tokio` task per room, owning all state. No shared mutable state between rooms.

```rust
enum RoomMsg {
    Join { player: PlayerId, tx: mpsc::Sender<ServerMsg> },
    Leave { player: PlayerId },
    Input { player: PlayerId, batch: InputBatch },
    Sync { player: PlayerId, tick: u32, checksum: u64 },
}

async fn room_task(mut rx: mpsc::Receiver<RoomMsg>) {
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    loop {
        tokio::select! {
            Some(msg) = rx.recv() => handle(msg),
            _ = interval.tick() => {
                broadcast_snapshots();
                check_stall_guard();
            }
        }
    }
}
```

Engines advance **on input arrival**, not on a server timer. The 100ms interval is only for snapshot broadcast and liveness checks.

---

## 7. WASM boundary

Cross once per frame, not per cell.

- `wasm-pack build --target web`, `vite-plugin-wasm` in the client.
- Expose `occupancy_ptr()` / `colors_ptr()`; build `Uint8Array` / `Uint16Array` views over `wasm.memory.buffer` **once** at init and let the renderer read directly.
- **Pre-allocate and never grow wasm memory** — growth invalidates existing views. Easy here since the engine is fixed-size.
- Per frame: read input → `engine.tick(buttons)` → read `Events` → draw from the memory views.

Decouple the 60Hz sim tick from `requestAnimationFrame` on day one — accumulator pattern, catch up in whole ticks. Retrofitting this is a rewrite.

---

## 8. Player state & persistence

### 8.1 Taxonomy

Split by owner and by whether it touches the simulation:

| Category | Examples | Affects sim? | Owner | Storage |
|---|---|---|---|---|
| **Handling** | DAS, ARR, SDF, DCD | **Yes** | Client-proposed, server-validated | localStorage → sent at join |
| **Keybinds** | key → button mapping | No — resolved before the engine | Client only | localStorage |
| **Cosmetic** | skin, palette, board scale, ghost alpha, volume | No | Client only | localStorage |
| **Identity** | device id, display name, later user id | No | Client, then server | localStorage → DB |
| **Records** | sprint PBs, games played | No | Server once accounts exist | IndexedDB → DB |
| **Replays** | `(seed, config, handling, inputs)` | — | Either | IndexedDB → object store |

The line that matters: **only handling crosses into the engine.** Keybinds resolve to `Buttons` in TS before the WASM call. Cosmetics never leave the render layer.

### 8.2 Config split — correction to §5

A single `GameConfig` was wrong. It's two structs, because handling is per-player but the rules are shared:

```rust
/// Shared by all players in a match. Server-chosen.
pub struct MatchConfig {
    pub gravity: GravityCurve,
    pub lock_delay_ticks: u8,
    pub lock_reset_cap: u8,
    pub attack_table: AttackTable,
    pub combo_table: [u8; 13],
    pub garbage_delay_ticks: u16,
    pub garbage_cap: u8,
    pub preview_len: u8,
}

/// Per-player. Client-proposed, server-validated, then frozen for the match.
pub struct Handling {
    pub das_ticks: u8,   // 0..=20
    pub arr_ticks: u8,   // 0..=10, 0 = instant
    pub sdf_ticks: u8,   // 0 = instant soft drop
    pub dcd_ticks: u8,   // direction-change cancels DAS
    pub prevent_misdrop: bool,
}
```

`Engine::new(seed, &MatchConfig, &Handling)`. The server stores each player's `Handling` at `MatchStart`, sims with it, and echoes **all** players' handling to **all** clients — spectating and replay playback both need it.

Validation is bounds-only. 0 DAS / 0 ARR is legitimate and competitively normal; the server clamps to sane ranges and otherwise doesn't have opinions.

### 8.3 Identity — anonymous-first

```
device_id   UUID v4, first load, localStorage.
            Restores YOUR OWN local state. Not a credential. Never authorizes anything.
session_id  server-assigned per connection, ephemeral
user_id     M4 only. Real identity.
```

Anonymous play needs no server-side row at all. Display names are client-chosen, non-unique, unreserved until accounts exist — show a `(guest)` marker so nobody assumes a name means anything.

Account creation at M4 is a one-time localStorage → server migration keyed by `device_id`.

### 8.4 Client storage

`localStorage`, one key, versioned:

```ts
// key: "<project>.settings"
type Settings = {
  version: 3;               // bump + migrate on every schema change
  deviceId: string;
  handling: Handling;
  keybinds: Record<string, ButtonName>;
  cosmetic: { skin: string; palette: string; ghostAlpha: number };
};
```

Write the `migrate(raw): Settings` chain on day one. Settings schemas always change, and wiping someone's keybinds on an update is exactly the kind of thing that costs you a contributor.

`IndexedDB` via **`idb`** (Jake Archibald's ~1KB wrapper) for replays and local PBs — two stores, `replays` and `records`. Don't put replays in localStorage; it's synchronous and capped around 5MB.

### 8.5 Replays are tiny — keep all of them

A 40L sprint is ~2400 ticks, so 2400 bytes raw. Buttons are held across long runs, so RLE `(buttons: u8, run: u8)` pairs cut that to roughly 200–400 bytes.

```rust
pub struct Replay {
    pub version: u16,
    pub engine_ver: u32,
    pub seed: u64,
    pub match_config: MatchConfig,
    pub handling: Handling,
    pub inputs: Vec<(u8, u8)>,   // RLE
    pub claimed_result: Outcome,
}
```

10,000 replays ≈ 25MB. "Save every game by default" is affordable here, which most games can't say — and it's what makes the M4 validator worth building.

### 8.6 Server schema (M4)

SQLite + `sqlx`. Rooms stay in memory and are never persisted.

```sql
CREATE TABLE users (
  id            BLOB PRIMARY KEY,
  handle        TEXT UNIQUE NOT NULL,
  email         TEXT UNIQUE,
  password_hash TEXT,                    -- argon2
  created_at    INTEGER NOT NULL
);

CREATE TABLE settings (                  -- cross-device sync only
  user_id    BLOB PRIMARY KEY REFERENCES users(id),
  payload    TEXT NOT NULL,              -- same JSON blob as localStorage
  updated_at INTEGER NOT NULL            -- last-write-wins
);

CREATE TABLE records (
  id         BLOB PRIMARY KEY,
  user_id    BLOB REFERENCES users(id),
  mode       TEXT NOT NULL,              -- 'sprint40' | 'blitz' | ...
  result_ms  INTEGER NOT NULL,
  replay_id  BLOB REFERENCES replays(id),
  verified   INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL
);

CREATE TABLE replays (
  id         BLOB PRIMARY KEY,
  user_id    BLOB REFERENCES users(id),
  engine_ver INTEGER NOT NULL,
  payload    BLOB NOT NULL,              -- postcard-encoded Replay
  created_at INTEGER NOT NULL
);
```

`engine_ver` earns its column: a rules change invalidates old replays for *verification*. Keep them playable, mark them unverifiable, don't silently re-verify them under new rules.

### 8.7 Build order

| Milestone | Persistence work |
|---|---|
| M1 | localStorage settings + `device_id` + migration chain. No server, no accounts. |
| M2 | Nothing new. |
| M3 | Server holds each connection's `Handling` in memory. Still no DB. |
| M4 | Users, accounts, the schema above, one-time localStorage → server migration. |

Skins in v1 are 2–3 built-in JSON themes bundled with the client, selection stored in `cosmetic.skin`. User-uploaded skins (manifest + hash-addressed assets) is M4+, and it's mostly a moderation problem rather than a storage one.

---

## 9. Milestones

| # | Deliverable | Est. |
|---|---|---|
| **M0** | `engine` + `replay-cli`. Board, 7-bag, SRS+ kicks, lock delay, DAS/ARR/SDF, line clears, checksum. **TOML mode loading (§5.7).** Golden replay tests green. | 1–2 wknd |
| **M1** | `client-wasm` + `apps/web`. Canvas renderer, keybind + handling config, localStorage settings, local replay capture. **40L sprint playable in the browser.** | 1–2 wknd |
| **M2** | Versus rules: attack table, combo, B2B, spin detection, garbage queue + cancellation. Now visualized in the client you already have. | 1 wknd |
| **M3** | `server` + `protocol`. Rooms, 1v1, garbage routing, checksum desync detection, stall guard, spectating. **This is Tier 0 complete — the whole self-hostable game.** | 2–4 wknd |
| **M4** | **Tier 1/2**, all optional and all in-repo: accounts, leaderboards, ranked, moderation, skin loading, mobile. Rust replay validator (§10). | open |
| **M5** | Publish `engine` to crates.io. Docs + examples for bot/TUI/solver authors. | 1 wknd |

Realistic: solid single-player in a month of evenings. Playable 1v1 in 2–3 months. Community-adoptable in 6+.

**Ordering note.** Client-before-versus is the right call because DAS/ARR tuning is a *human playtesting* problem — you cannot do it headless, and it's the single hardest thing to get right for competitive players. Two consequences to plan for:

- Define the **full `Events` bitset in M0**, including the flags nothing consumes until M2 (`SPIN`, `B2B_*`, `PERFECT_CLEAR`, `GARBAGE_APPLIED`). The renderer then never needs reworking when versus rules land.
- The HUD gets partially rebuilt at M2 — sprint shows time/lines/PPS, versus adds a garbage bar, attack counter, and opponent boards. Cheap, and the layout would have changed anyway.

---

## 10. Replay validation service (M4)

Ranked submissions land in a queue; a Rust worker re-simulates each replay from `(seed, config, inputs)`, confirms the claimed result, writes a verdict. Batch, not live — zero latency path, and throughput actually matters when re-simulating thousands of runs.

Bonus property: if you ever build a second independent implementation of the rules, a divergence on the same replay is a bug you'd never otherwise catch.

---

## 11. CI

| Job | Purpose |
|---|---|
| `cargo test --workspace` | Unit + proptest + insta snapshots |
| **native vs wasm checksum parity** | Run every golden replay through both builds, assert identical checksums. **Catches determinism drift the day it's introduced, not the day a ranked match desyncs.** |
| `cargo clippy -- -D warnings` | — |
| grep guard on `engine/` | Fail on `f32`, `f64`, `HashMap`, `SystemTime`, `Instant` |
| `pnpm typecheck` + `ts-rs` diff | Fail if generated TS types are stale |

---

## 12. Deployment

Single static binary + `dist/`. `tower-http::ServeDir` serves the client; `/ws` is the game socket. Caddy in front for TLS. Docker image + `docker-compose.yml` in the repo so a private lobby is one command.

$10–20/mo VPS handles hundreds of concurrent players — a few hundred bytes/sec per player. Ad-free is cheap when the payload is this small.

Tier is inferred, never a build flag:

```
(no env)              → Tier 0. In-memory rooms, anonymous only.
DATABASE_URL=...      → Tier 1. Accounts + leaderboards light up.
CONFIG=./server.toml  → Tier 2. Rate limits, moderation, ranked.
```

One binary covers all three. `cargo build --release` produces the same artifact the hosted instance runs.

---

## 13. Open questions **[DECIDE]**

1. **Name.** Must clear §1. Suggest checking USPTO TESS + npm + a domain before committing.
2. **Rotation system.** SRS+ (with 180° kicks) is the modern competitive default. Alternative: ship SRS strictly, add 180s behind config.
3. **Attack table.** Clone tetr.io's feel for adoption, or diverge for identity? Adoption argues for near-parity in v1.
4. **Garbage delay.** 60 ticks proposed. Lower = twitchier, but tightens the netcode margin in §6.3.
5. **Room size.** 1v1 only in v1, or FFA up to N? FFA changes garbage *targeting* (random / badges / attackers), which is real scope.
6. **Auth.** Anonymous-first with optional accounts? Anonymous-only keeps M3 much smaller.
7. **Wire format at M3.** Ship JSON for debuggability, or go straight to postcard?
8. **Snapshot cadence.** Fixed 10Hz, or event-driven (on lock / on clear)? Event-driven is less bandwidth, more jitter.
9. ~~**License.**~~ **Resolved (§2.3):** engine + protocol dual MIT/Apache-2.0; server + web MIT. Revisit only if a closed ad-supported rehost would actually bother you.
10. **Mode goal types.** `lines`, `time`, `survival`, `score` cover the obvious ones — is `goal` an enum in `MatchConfig`, or a separate `ModeSpec` wrapping it? Affects §5.7's file format.

---

## 14. Design invariants

Rules that, if violated, mean something has gone wrong:

1. `engine/` contains no floats, no I/O, no time, no async.
2. TypeScript never implements a game rule.
3. Clients report **inputs**, never outcomes.
4. `colors` is never read by game logic.
5. Garbage `apply_at_tick` is always in the future for every recipient.
6. A replay is exactly `(seed, MatchConfig, Handling, Vec<Buttons>)` — nothing else.
7. Only `Handling` crosses from settings into the engine. Keybinds and cosmetics stop at the render layer.
8. No feature exists in the hosted instance that a self-hoster cannot enable. Tiers are config; there is no private build.
9. `./server` with no environment, no config file, and no database is always a playable game.
