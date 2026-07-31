# M0 — Engine + Replay CLI · Detailed Spec v0.1

Companion to [SPEC.md](SPEC.md). Expands milestone **M0** into an implementable
document. Where this doc and `SPEC.md` disagree, this doc wins for M0 scope and the
disagreement is called out explicitly under [§13 Amendments](#13-amendments-to-specmd).

> **Deliverable in one sentence:** a dependency-light, deterministic, fully-tunable
> falling-block simulation crate plus a native harness that can run, verify, and
> visualise recorded input streams — with no browser, no server, and no human input path.

---

## 1. Scope

### 1.1 In scope

| Area | Detail |
|---|---|
| **Board & pieces** | 10×40 bitboard, 7 quad kinds, spawn/collision/movement |
| **Rotation** | SRS+ including a 180° kick table |
| **Randomiser** | 7-bag over hand-rolled SplitMix64 |
| **Gravity** | Fixed-point subtick accumulator, staged curves, 20G |
| **Lock delay** | Classic / extended / infinite reset modes, reset cap |
| **Handling** | DAS, ARR, SDF, DCD, DAS-cut, IRS, IHS, misdrop protection, soft-drop lock |
| **Clears** | Row detection, collapse, clear delay, perfect-clear detection |
| **Scoring** | Spin detection, attack table, combo table, B2B chain |
| **Garbage** | `schedule_garbage` → deterministic application at a future tick |
| **Topout** | Block-out, lock-out, garbage push-out |
| **Determinism** | `checksum()`, native↔wasm parity, golden replays |
| **Configuration** | `MatchConfig` / `Handling` / `ModeSpec`, descriptor tables, TOML loading, layered resolution, JSON schema emission |
| **Tooling** | `replay-cli`: run, verify, render (ASCII), schema, modes |

### 1.2 Explicitly out of scope for M0

- Keyboard, gamepad, or any human input path. **M0 is not playable.**
- `client-wasm`, `apps/web`, Canvas rendering, WASM bindings. *(M1)*
- `server`, `protocol`, WebSockets, rooms, garbage **routing** and **cancellation**. *(M3)*
- Accounts, database, persistence of any kind. *(M4)*
- Audio, skins, particle effects.
- Configurable board dimensions or tick rate — see [§4.3](#43-explicit-non-tunables).

### 1.3 The input question, answered

There is no player in M0, but the engine's **input interface is complete**, because
`tick(&mut self, input: Buttons)` is the engine's only mutator (`SPEC.md` §5.4) and
because DAS/ARR/SDF live *inside* the engine (`SPEC.md` §5.3).

The engine's job is to turn *"LEFT was held for these 30 consecutive ticks"* into the
correct sequence of column movements. In M1, TypeScript's only added responsibility is
mapping a physical key to the `LEFT` bit. That mapping is trivial; the state machine
underneath it is the hard part, and it is M0 work.

So the "player" in M0 is a `Vec<Buttons>` originating from:

1. hand-written input scripts in a small text DSL ([§10.2](#102-input-script-dsl)),
2. golden replay files decoded by `replay-cli`,
3. property-test generators emitting random button streams.

This is precisely what makes the golden-replay suite possible: inputs are data, so a
test *is* a recorded player.

---

## 2. Decisions resolved for M0

These were open in `SPEC.md`; they are now closed for M0.

| # | Question | Decision |
|---|---|---|
| 1 | Versus-rule scope in M0 | **Full scoring in M0.** Spin detection, B2B, combo, attack computation and the garbage *application* path all ship in M0. M2 adds only server-side routing/cancellation and the UI. Rationale: pure engine code, no client dependency, and golden replays then cover the scoring paths from day one instead of needing regeneration. |
| 2 | Rotation system (`SPEC.md` §13.2) | **SRS+ with a 180° kick table.** The `FLIP` button in the `Buttons` bitflags already presumes it. Kick tables are compile-time `const` data, not TOML. |
| 3 | Mode goal representation (`SPEC.md` §13.10) | **`ModeSpec` wraps `MatchConfig`.** The engine never sees a goal and stays goal-agnostic; goal evaluation lives in the shell. Preserves invariant #6 — a replay is `(seed, MatchConfig, Handling, inputs)` with no mode metadata required to re-simulate. |
| 4 | Handling timer precision | **Fixed-point subticks internally; milliseconds in config.** `1 tick = 256 subticks`. Config stores `u16` milliseconds, converted once at `Engine::new` with integer math. Gives sub-frame DAS without floats, and keeps stored settings meaningful if `TICK_HZ` ever changes. |
| 5 | Handling field set | **Full tetr.io-parity set** (9 fields, [§5.4](#54-handling)). `Handling` is frozen into every replay, so sim-affecting additions later force an `engine_ver` bump that marks existing golden replays unverifiable. Settling the set now avoids that. |
| 6 | Debug visibility | **ASCII board renderer in `replay-cli`.** With no client, the alternative view into a bug is a checksum diff and a hex dump. ~40 lines, zero deps. |

Still deferred: project name (`SPEC.md` §13.1), attack-table *values* (§13.3 — the
*mechanism* ships in M0, the numbers are playtest-tuned in M2), garbage delay (§13.4),
room size (§13.5), auth (§13.6), wire format (§13.7), snapshot cadence (§13.8).

---

## 3. Crate layout for M0

```
/
├── crates/
│   ├── engine/          pure sim. no I/O, no time, no async, no floats.
│   ├── config/          TOML + JSON I/O, layered resolution, schema emission.   [NEW — §13]
│   └── replay-cli/      native harness: run / verify / render / schema / modes.
├── modes/
│   ├── sprint40.toml
│   ├── blitz.toml
│   └── versus.toml
├── testdata/
│   ├── scripts/         *.script — human-authored input DSL
│   └── replays/         *.replay + expected checksums (insta snapshots)
├── LICENSE-MIT
├── LICENSE-APACHE
└── justfile
```

Dependency direction, unchanged in spirit from `SPEC.md` §3:

```
engine ← config ← replay-cli
```

`engine` depends on no other workspace crate. `config` may reference engine types;
never the reverse.

### 3.1 Dependencies

**`engine`** — kept minimal so it stays `no_std`-friendly and cheap to depend on
(`SPEC.md` §2.2 surface #1).

| Crate | Feature-gated? | Why |
|---|---|---|
| `bitflags` | no | `Buttons`, `Events` |
| `arrayvec` | no | bounded queue / garbage, no allocation |
| `serde` (derive) | **yes**, `serde` feature, default-on | Config types only. Off → `no_std` minimal build still compiles. |
| `ts-rs` | **yes**, `ts` feature, default-off | Emits `.ts` for M1. Dev-only path. |
| `proptest` | dev-only | Kick tables, lock-delay machine |
| `insta` | dev-only | Golden checksums |

**Banned in `engine/`** (CI-enforced, `SPEC.md` §11): `f32`, `f64`, `HashMap`,
`SystemTime`, `Instant`, `std::fs`, `async`, any per-tick allocation.

**`config`** — `toml`, `serde`, `serde_json`, `thiserror`.
All file I/O lives here so `engine` can keep invariant #1.

**`replay-cli`** — `clap`, `anyhow`, plus the two above.

---

## 4. Constants and fixed-point arithmetic

### 4.1 Constants

```rust
pub const BOARD_W:     usize = 10;
pub const BOARD_H:     usize = 40;   // 20 visible + 20 buffer
pub const VISIBLE_H:   usize = 20;
pub const TICK_HZ:     u32   = 60;
pub const FULL_ROW:    u16   = 0b11_1111_1111;
pub const MAX_PREVIEW: usize = 7;

/// Subdivisions of one tick. Every timer counts these — handling, lock, delays,
/// AND gravity. See §4.2 for why gravity does not get its own sub-cell scale.
pub const SUBTICK: u32 = 256;

/// Bumped on ANY change to simulation-affecting rules. See §11.4.
pub const ENGINE_VER: u32 = 1;
```

### 4.2 Milliseconds → subticks

Conversion happens **once**, in `Engine::new`, in integer arithmetic only:

```rust
/// subticks_per_ms = TICK_HZ * SUBTICK / 1000 = 60 * 256 / 1000 = 15.36
/// As an exact integer ratio: 384 / 25.
pub const fn ms_to_subticks(ms: u16) -> u32 {
    (ms as u32 * 384) / 25
}
```

Bounds check: `65_535 * 384 = 25_165_440`, comfortably inside `u32`.

The reduction `384 / 25` is computed from `SUBTICK` and `TICK_HZ` by a `const fn gcd`
rather than hardcoded, so that changing either constant cannot leave a stale ratio behind.

**Resolution.** One subtick ≈ 0.0651 ms ≈ 0.0039 frames. A DAS of 8.5 frames
(141.67 ms) stores as `142 ms` → 2181 subticks → 8.52 frames.

**Display helpers round to nearest**, unlike `ms_to_subticks` which truncates. This makes
`ms → subticks → ms` the exact identity across the entire `u16` range — verified
exhaustively by test. Truncating on the way back instead would make a settings slider
lose 1 ms every time it was saved and reloaded, which reads as a bug even when the stored
subtick count is correct.

**One scale, not two.** Gravity does *not* get a separate sub-cell accumulator. "One row
per `ms_per_row` milliseconds" is a duration like any other, so gravity is a subtick
threshold and the piece descends whenever the accumulator crosses it. One conversion
function, one scale, one class of rounding error — and `1000 ms/row` comes out to exactly
60 ticks, which a sub-cell rate of `SUBCELL * TICK_MS / ms_per_row` would not (it
truncates to 4 subcells/tick against a true 4.267, a 6% error).

**Documented characteristic, not a bug:** because milliseconds are the stored unit,
frame-denominated values quantise to roughly ±0.03 F. The settings UI must therefore
treat **ms as canonical** and display frames as a derived read-out (`142 ms ≈ 8.5 F`),
exactly as tetr.io's ms/frame toggle does — never let the user type a frame value and
expect an exact round-trip. If exact frame round-tripping is ever wanted, the change is
one line (store subticks directly in `Handling` and convert for display), but it costs
the tick-rate independence that motivated ms in the first place.

### 4.3 Explicit non-tunables

`BOARD_W`, `BOARD_H`, and `TICK_HZ` are `const` and **stay** `const` in v1.

Board width is baked into the `u16` row bitmasks and every collision, clear, and
checksum routine derives from it. Making it dynamic infects the entire engine for no v1
benefit. Stated here so that nobody half-implements it and leaves the codebase in a
worse state than either endpoint.

---

## 5. Configuration architecture

This is the load-bearing section. Config shape is the single hardest thing to retrofit,
and "expose these in the UI later" is a requirement that has to be designed for now.

### 5.1 The four config structs

Split by **owner** and by **does it touch the simulation** — refining `SPEC.md` §8.1:

| Struct | Authored in | Affects sim | Lifetime | Enters engine |
|---|---|---|---|---|
| `MatchConfig` | `modes/*.toml` | **yes** | frozen at match start | **yes** |
| `Handling` | player settings | **yes** | frozen at match start | **yes** |
| `Keybinds` | player settings | no | live | never — resolved to `Buttons` in the shell |
| `Cosmetic` | player settings | no | live | never — stops at the render layer |

M0 defines and validates all four. Only the first two are consumed by `engine`;
`Keybinds` and `Cosmetic` exist in M0 as **types plus descriptors only**, so that M1 can
generate their UI from the same machinery without inventing a second system.

### 5.2 Descriptors are the single source of truth

Every tunable field carries a machine-readable descriptor. Plain `const` data — no
proc-macros, no dependencies, `no_std`-clean — so it lives in `engine` without violating
the zero-dep goal.

```rust
pub struct FieldDesc {
    /// Must match the serde field name exactly. Enforced by test (§10.5).
    pub key:   &'static str,
    pub label: &'static str,          // "DAS"
    pub help:  &'static str,          // "Delay before a held direction starts repeating."
    pub group: &'static str,          // "handling.movement"
    pub kind:  FieldKind,
}

pub enum FieldKind {
    Int  { min: i64, max: i64, default: i64, step: i64, unit: Unit },
    Bool { default: bool },
    Enum { variants: &'static [(&'static str, &'static str)], default: &'static str },
}

pub enum Unit { Millis, Ticks, Cells, Rows, Count, Percent, None }

impl Handling    { pub const FIELDS: &'static [FieldDesc] = &[ /* ... */ ]; }
impl MatchConfig { pub const FIELDS: &'static [FieldDesc] = &[ /* ... */ ]; }
impl Keybinds    { pub const FIELDS: &'static [FieldDesc] = &[ /* ... */ ]; }
impl Cosmetic    { pub const FIELDS: &'static [FieldDesc] = &[ /* ... */ ]; }
```

One definition then buys four things:

1. **Validation.** `Handling::clamp(&mut self)` is a loop over `FIELDS`, not nine
   hand-written bounds checks that silently drift from their doc comments.
2. **A generated settings UI in M1.** Adding a handling option becomes a one-line Rust
   change with **zero** TypeScript edits. This is the actual mechanism behind
   "expose these in the UI later."
3. **`config-schema.json`**, emitted by `replay-cli schema`, consumed by the client and
   by the docs site. CI diffs it, so it cannot go stale.
4. **Mode-file errors that help non-programmers.** A typo'd `das_tikcs` produces
   *"unknown key `das_tikcs` — did you mean `das_ms`?"* rather than silently defaulting,
   which matters when modes are meant to be contributed by people who don't write Rust.

Drift guard is a single test: every serde field must appear in `FIELDS`, and every
`FIELDS` key must round-trip through serde. See [§10.5](#105-config-tests).

### 5.3 `MatchConfig`

Shared by all players in a match. Server-chosen. Every field is `Option` in TOML and
falls back to the descriptor default, so a minimal mode file is three lines.

```rust
pub struct MatchConfig {
    // --- timing ---
    pub gravity:            GravityCurve,
    pub lock_delay_ms:      u16,          // 0..=5000,  default 500
    pub lock_reset_mode:    LockResetMode,// Classic | Extended | Infinite
    pub lock_reset_cap:     u8,           // 0..=255,   default 15
    pub clear_delay_ms:     u16,          // 0..=1000,  default 0
    pub spawn_delay_ms:     u16,          // 0..=1000,  default 0  (ARE)

    // --- board / queue ---
    pub preview_len:        u8,           // 0..=7,     default 5
    pub hold_enabled:       bool,         // default true

    // --- scoring ---
    pub spin_detection:     SpinRule,     // None | ThreeCorner | Immobile | AllSpin
    pub attack_table:       AttackTable,
    pub combo_table:        ArrayVec<u8, 21>,
    pub b2b_bonus:          u8,           // default 1

    // --- garbage ---
    pub garbage_delay_ms:   u16,          // default 1000
    pub garbage_cap:        u8,           // default 8
    pub garbage_hole_repeat: bool,        // hole column repeats within a batch
}

pub enum GravityCurve {
    /// Constant speed. 0 = 20G (instant to floor).
    Fixed  { ms_per_row: u32 },
    /// Piecewise, keyed on elapsed ticks. Last stage holds forever.
    Staged { stages: ArrayVec<GravityStage, 16> },
}

pub struct GravityStage { pub from_tick: u32, pub ms_per_row: u32 }

pub enum LockResetMode {
    /// Reset only when the piece descends a row.
    Classic,
    /// Reset on any successful move or rotation, up to `lock_reset_cap`.
    Extended,
    /// Reset on any successful move or rotation, no cap.
    Infinite,
}

pub enum SpinRule { None, ThreeCorner, Immobile, AllSpin }
```

`AttackTable` is a flat struct of `u8`s mirroring `SPEC.md` §5.5, so it deserialises
from a TOML sub-table directly. The *mechanism* is M0; the *numbers* are placeholder
until M2 playtesting.

### 5.4 `Handling`

Per-player. Client-proposed, server-validated, frozen for the match. Nine fields,
tetr.io parity.

```rust
pub struct Handling {
    /// Delay before a held direction begins auto-repeating.
    pub das_ms:              u16,  // 0..=500,  default 133
    /// Interval between auto-repeat steps. 0 = instant (snap to wall).
    pub arr_ms:              u16,  // 0..=200,  default 0
    /// Time per row while soft-dropping. 0 = instant (snap to floor).
    pub sdf_ms_per_row:      u16,  // 0..=500,  default 0
    /// On direction change, the new direction's DAS wait becomes this. 0 = full DAS.
    pub dcd_ms:              u16,  // 0..=200,  default 0
    /// On spawn with a direction still held and DAS charged, suppress ARR this long.
    pub das_cut_delay_ms:    u16,  // 0..=200,  default 0
    /// Rotation held across spawn applies on the spawn tick.
    pub irs:                 IrsMode, // Off | Hold | Tap, default Hold
    /// Hold held across spawn applies on the spawn tick.
    pub ihs:                 bool, // default true
    /// Hard drop is ignored for this long after spawn. 0 = off.
    pub prevent_misdrop_ms:  u16,  // 0..=200,  default 0
    /// Soft-dropping into the floor locks immediately, bypassing lock delay.
    pub soft_drop_lock:      bool, // default false
}
```

**Validation is bounds-only** (`SPEC.md` §8.2). `das_ms = 0` / `arr_ms = 0` is
legitimate and competitively normal. The server clamps to the descriptor ranges and
otherwise holds no opinions.

### 5.5 `ModeSpec` — modes are data

```rust
pub struct ModeSpec {
    pub spec_version: u16,        // format version; see §5.8
    pub id:           String,     // "sprint40" — filename stem, must match
    pub name:         String,     // "Sprint 40"
    pub description:  String,
    pub goal:         Goal,
    pub config:       MatchConfig,
}

pub enum Goal {
    Lines    { count: u32 },
    Time     { ms: u32 },
    Score    { target: u64 },
    Survival,                     // versus / endless
}
```

`Goal` lives on `ModeSpec`, **not** on `MatchConfig`. The engine never sees it and stays
goal-agnostic; goal evaluation is the shell's job. This preserves invariant #6 —
re-simulating a replay needs `(seed, MatchConfig, Handling, inputs)` and nothing else.

```toml
# modes/sprint40.toml
spec_version = 1
id           = "sprint40"
name         = "Sprint 40"
description  = "Clear 40 rows as fast as possible."

[goal]
type  = "lines"
count = 40

[config]
lock_delay_ms   = 500
lock_reset_cap  = 15
preview_len     = 5
spin_detection  = "three_corner"

[config.gravity]
type       = "fixed"
ms_per_row = 1000

[config.attack_table]
single = 0
double = 1
triple = 2
quad   = 4
```

### 5.6 Layered resolution

```
engine defaults   ←   mode file   ←   server policy   ←   player settings
  (const, §5.2)     (MatchConfig)    (caps & locks)      (Handling only)
```

Each resolved field records **which layer set it**:

```rust
pub struct Resolved<T> { pub value: T, pub layer: Layer }
pub enum Layer { Default, Mode, ServerPolicy, Player }
```

Cheap now — one enum on a struct the resolver already builds — and it's what lets the
M1 UI grey out a slider with *"locked by mode: Sprint 40"* instead of accepting an edit
that silently does nothing. Server policy is an empty layer in M0; it exists so M3 can
populate it without changing the resolver's shape.

Precedence rule: a later layer overrides an earlier one **unless** the earlier layer
marked the field locked. Locking is `ServerPolicy`-only in practice.

### 5.7 Where config is read

`engine` performs **no I/O** (invariant #1). It defines the types, the descriptors, the
defaults, and `clamp()`. Everything else lives in `crates/config`:

| Function | Responsibility |
|---|---|
| `load_modes(dir) -> Result<Vec<ModeSpec>>` | Read `modes/*.toml`, parse, validate, check `id` matches filename stem |
| `resolve(layers) -> (MatchConfig, Handling, Provenance)` | Apply §5.6 precedence |
| `emit_schema() -> String` | Walk `FIELDS`, produce `config-schema.json` |
| `emit_ts() -> String` | *(feature `ts`)* emit `config.generated.ts` for M1 |

### 5.8 Compatibility rules for community-authored files

- **`#[serde(deny_unknown_fields)]` everywhere.** A typo must be a loud error with a
  Levenshtein "did you mean" suggestion. Silent defaulting is the failure mode that
  wastes a contributor's evening.
- **Every config field optional**, falling back to the descriptor default. Minimal mode
  files stay short.
- **`spec_version` is mandatory.** An older binary meeting a newer file must say
  *"this mode requires format v2, this build supports v1"* — not fail on an unknown key.
- **Descriptor ranges are the contract.** Out-of-range values in a mode file are an
  error; out-of-range values in *player* settings are silently clamped, because those
  arrive from an untrusted client.

---

## 6. Engine

### 6.1 State

```rust
pub struct Engine {
    // --- board ---
    occupancy:     [u16; BOARD_H],           // bit per column; FULL_ROW == complete row
    colors:        [u8; BOARD_W * BOARD_H],  // RENDER ONLY. Game logic never reads this.

    // --- piece ---
    active:        Piece,                    // { kind, rot, x: i8, y: i8 }
    grav_sub:      u32,                      // gravity accumulator, in subticks
    hold:          Option<QuadKind>,
    can_hold:      bool,

    // --- randomiser ---
    queue:         ArrayVec<QuadKind, 14>,   // two bags
    rng:           SplitMix64,

    // --- garbage ---
    garbage:       ArrayVec<PendingGarbage, 32>,

    // --- timing (all in SUBTICKS) ---
    tick:          u32,
    das_sub:       u32,
    arr_sub:       u32,
    sdf_sub:       u32,
    lock_sub:      u32,
    delay_sub:     u32,                      // clear delay / spawn delay
    misdrop_sub:   u32,
    lock_resets:   u8,
    dir:           Option<Dir>,              // currently DAS-charging direction

    // --- scoring ---
    combo:         u8,
    b2b:           u8,
    last_kick_idx: u8,                       // drives mini/full spin classification
    last_was_rot:  bool,

    // --- control ---
    phase:         Phase,
    prev_buttons:  Buttons,
    pending_irs:   Option<Rot>,
    pending_ihs:   bool,

    // --- frozen config (subtick-converted at construction) ---
    config:        MatchConfig,
    handling:      HandlingSub,              // ms already converted to subticks
}

pub enum Phase { Spawning, Falling, Locking, ClearDelay, Dead }
```

`colors` is a strict render channel. If any rule ever reads it, determinism is at risk
(invariant #4) — enforced by review and by the checksum excluding it entirely.

### 6.2 Public API

```rust
impl Engine {
    pub fn new(seed: u64, config: &MatchConfig, handling: &Handling) -> Self;

    /// The only mutator. Fully deterministic on (state, input).
    pub fn tick(&mut self, input: Buttons) -> TickResult;

    /// Server-authoritative. `apply_at_tick` must be strictly in the future.
    pub fn schedule_garbage(&mut self, g: PendingGarbage);

    pub fn checksum(&self) -> u64;
    pub fn phase(&self) -> Phase;
    pub fn stats(&self) -> Stats;            // lines, pieces, attack sent, tick

    // read-only accessors for the render layer (M1)
    pub fn occupancy_ptr(&self) -> *const u16;
    pub fn colors_ptr(&self) -> *const u8;
}

pub struct TickResult {
    pub events:        Events,
    pub attack:        u8,
    pub lines_cleared: u8,
}
```

`Events` is the **full bitset from `SPEC.md` §5.4, defined and populated in M0** — all
flags, including the ones nothing consumes until M2. Per `SPEC.md` §9's ordering note,
this means the M1 renderer never needs reworking when versus rules land.

### 6.3 Tick pipeline

Strict, fixed order. Every branch is integer-deterministic.

```
1.  tick += 1
2.  pressed  = input & !prev_buttons          (edge-triggered set)
    released = !input & prev_buttons
3.  apply_due_garbage()                        // any g where g.apply_at_tick == tick
4.  match phase {
        Spawning   => spawn_step(input, pressed),
        Falling    => falling_step(input, pressed),
        Locking    => locking_step(input, pressed),
        ClearDelay => delay_step(),
        Dead       => {}
    }
5.  prev_buttons = input
6.  return TickResult
```

Garbage is applied at step 3 **regardless of phase**, so both client and server apply it
on the identical tick with identical state (invariant #5). Rows shift up; the active
piece's `y` shifts up by the same amount; if that pushes it past the buffer, top out.

### 6.4 Randomiser

SplitMix64, hand-rolled, ~15 lines — deliberately not `rand`, because a version bump
silently invalidating every stored replay is a bad afternoon (`SPEC.md` §4).

```rust
impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}
```

7-bag: fill `[I,O,T,S,Z,J,L]`, Fisher-Yates shuffle backwards using
`next_u64() % (i as u64 + 1)`, push to `queue`. Refill whenever
`queue.len() <= PREVIEW_LEN`. Modulo bias is negligible at n≤7 and, more importantly,
is *deterministic* — which is the property that actually matters here.

### 6.5 Piece geometry and SRS+

- `QuadKind`: `I, O, T, S, Z, J, L` (descriptive of geometry, not protected terminology —
  `SPEC.md` §1 constrains the *product* name, the piece-collective noun, and the palette).
- Geometry: `const SHAPES: [[[(i8, i8); 4]; 4]; 7]` — kind × rotation × four cells.
- Spawn: rows 19–20 (top of the visible field), horizontally centred, rotation 0.
- Kick tables, all compile-time `const`:

| Table | Transitions | Offsets each |
|---|---|---|
| `KICKS_JLSTZ` | 8 (0↔1, 1↔2, 2↔3, 3↔0) | 5 |
| `KICKS_I` | 8 | 5 |
| `KICKS_180` | 4 (0↔2, 1↔3) | 6 |

`O` never kicks. `last_kick_idx` is recorded on every successful rotation and drives the
mini/full spin distinction ([§6.10](#610-spin-detection)).

Kick values are transcribed from the public SRS tables and verified by an exhaustive
test ([§10.3](#103-rotation-tests)) rather than trusted by eye.

### 6.6 Gravity

Gravity is a subtick threshold, on the same scale as every other timer (§4.2):

```
threshold = ms_to_subticks_nonzero(config.gravity.ms_per_row_at(tick))
grav_sub += SUBTICK
while grav_sub >= threshold && can_move_down() { move_down(); grav_sub -= threshold; }
if !can_move_down() { grav_sub = 0; phase = Locking }
```

`ms_per_row == 0` means 20G: move down until blocked, in one tick.

Soft drop replaces the rate with `handling.sdf_ms_per_row` when `SOFT_DROP` is held,
taking whichever of the two is faster. `sdf_ms_per_row == 0` snaps to the floor.
If `soft_drop_lock` is set, reaching the floor under soft drop locks immediately rather
than entering `Locking`.

### 6.7 Handling state machine

The part most worth specifying precisely, because it is the part players feel.

```
LEFT and RIGHT both held  → the more recently pressed direction wins.
On direction change:
    dir     = new direction
    das_sub = if dcd_ms > 0 { dcd_sub } else { das_sub_full }
    arr_sub = 0
    move one cell immediately                         // the initial tap
On direction press (from none):
    dir     = direction
    das_sub = das_sub_full
    move one cell immediately
While direction held:
    if das_sub > 0 { das_sub = das_sub.saturating_sub(SUBTICK); }
    else {
        if arr_sub_full == 0 { move until blocked }   // instant ARR
        else {
            arr_sub += SUBTICK;
            while arr_sub >= arr_sub_full { move_one(); arr_sub -= arr_sub_full; }
        }
    }
On direction release:
    if the other direction is still held → treat as direction change
    else dir = None, das_sub = das_sub_full, arr_sub = 0
On spawn with a direction still held and DAS already expired:
    arr_sub = -das_cut_delay_sub                      // suppresses ARR for that long
```

Rotation (`CW`, `CCW`, `FLIP`), `HOLD`, and `HARD_DROP` are **edge-triggered** — they
fire on `pressed`, never on held state. `prev_buttons` is engine-internal, so this is
invisible to the caller.

> These definitions are the M0 contract. DAS/ARR feel is fundamentally a *human
> playtesting* problem (`SPEC.md` §9 ordering note) and cannot be validated headless.
> Expect DCD and DAS-cut semantics specifically to be re-tuned during M1 against
> reference feel. Any change to them is a rules change and bumps `ENGINE_VER` (§11.4).

### 6.8 IRS / IHS

Evaluated in `spawn_step`, before the block-out check:

- **IHS** — if `HOLD` is held across spawn (`ihs == true`) and `can_hold`, perform the
  hold on the spawn tick, swapping before the piece is placed.
- **IRS** — if a rotation button is held across spawn, the piece spawns already rotated.
  `IrsMode::Hold` accepts a button merely held; `IrsMode::Tap` requires it to have been
  pressed during the preceding delay; `Off` disables it.
- If the IRS-rotated spawn position collides, fall back to rotation 0; if *that*
  collides, it is a block-out.

### 6.9 Lock delay

```
On entering Locking:      lock_sub = lock_delay_sub; lock_resets = 0
Each Locking tick:        lock_sub = lock_sub.saturating_sub(SUBTICK)
On successful move/rot in Locking, per LockResetMode:
    Classic  → reset only if the piece descended a row
    Extended → reset if lock_resets < lock_reset_cap, then lock_resets += 1
    Infinite → always reset
If can_move_down() becomes true again → phase = Falling  (lock_resets is retained)
When lock_sub == 0, or HARD_DROP is pressed → lock_piece()
```

`lock_resets` retained across a re-entry into `Falling` is deliberate: it is what stops
infinite stalling under `Extended`.

### 6.10 Spin detection

Governed by `config.spin_detection`:

| Rule | Behaviour |
|---|---|
| `None` | Never sets `SPIN` / `MINI_SPIN` |
| `ThreeCorner` | T only. ≥3 of the 4 corners around the T's centre occupied, and the last successful action was a rotation |
| `Immobile` | T only. Piece cannot move up, left, or right after locking |
| `AllSpin` | `Immobile`, applied to every kind |

Mini vs. full: `last_kick_idx == 4` (the final, largest kick) promotes a mini to a full
spin; the two "front" corners being occupied likewise promotes it. Both conditions are
recorded at rotation time, not recomputed at lock time.

### 6.11 Clears and scoring

```
lock_piece()
  → write cells into occupancy + colors
  → detect full rows (row == FULL_ROW)
  → classify: lines_cleared, spin flags, perfect clear (all rows empty after collapse)
  → attack = attack_table.lookup(lines, spin_kind)
           + combo_table[min(combo, len-1)]
           + if b2b_chain { b2b_bonus } else { 0 }
           + if perfect_clear { attack_table.perfect_clear } else { 0 }
  → update combo (increment on clear, reset to 0 on a clear-less lock)
  → update b2b   (continue on quad or spin clear, break on a plain single/double/triple)
  → collapse rows
  → cancel against pending garbage (attack reduces the pending queue first)
  → phase = if lines_cleared > 0 && clear_delay_ms > 0 { ClearDelay } else { Spawning }
```

Cancellation **inside** the engine reduces this player's own pending queue. Cross-player
cancellation is resolved server-side (`SPEC.md` §5.6) and is M3 work — but the local
half must exist in M0 or the garbage path can't be tested at all.

### 6.12 Garbage

```rust
pub struct PendingGarbage {
    pub apply_at_tick: u32,
    pub amount:        u8,
    pub hole_col:      u8,
}
```

- `schedule_garbage` **debug-asserts** `apply_at_tick > self.tick` (invariant #5).
- Applied at pipeline step 3: shift `occupancy` up by `amount`, fill the new bottom rows
  with `FULL_ROW & !(1 << hole_col)`, shift `active.y` up by `amount`.
- `garbage_hole_repeat` controls whether one batch reuses a single hole column.
- The hole column is **always supplied by the caller**, never derived from the engine's
  RNG stream. In M3 the server supplies it from the server's own stream.
- Capped at `config.garbage_cap` rows per application.

### 6.13 Topout

| Condition | Trigger |
|---|---|
| **Block out** | Spawn position collides with occupied cells |
| **Lock out** | A locked piece lies entirely at or above row `VISIBLE_H` |
| **Push out** | Garbage shifts occupied cells past row 0 |

Any of these sets `phase = Dead` and raises `TOPPED_OUT`. `tick()` on a `Dead` engine is
a no-op returning empty events — it never panics.

### 6.14 Checksum

FNV-1a 64, over a byte stream in **exactly** this order. Byte order is explicitly
little-endian at every step so native and wasm agree.

```
occupancy[0..40]                as LE u16
active.kind, active.rot         as u8, u8
active.x, active.y              as i8, i8
grav_sub                        as LE u32
hold (0xFF if None), can_hold   as u8, u8
queue.len, then each kind       as u8
rng.state                       as LE u64
garbage.len, then each entry    as (LE u32, u8, u8)
tick                            as LE u32
combo, b2b, lock_resets, phase  as u8 ×4
das_sub, arr_sub, sdf_sub,
  lock_sub, delay_sub           as LE u32 ×5
dir (0 none / 1 left / 2 right) as u8
```

**`colors` is excluded entirely** — it is render-only, and including it would let a
cosmetic change trigger a desync (invariant #4).

---

## 7. Events

```rust
bitflags! {
    pub struct Events: u16 {
        const PIECE_LOCKED   = 1 << 0;
        const LINES_CLEARED  = 1 << 1;
        const SPIN           = 1 << 2;
        const MINI_SPIN      = 1 << 3;
        const B2B_CONTINUED  = 1 << 4;
        const B2B_BROKEN     = 1 << 5;
        const PERFECT_CLEAR  = 1 << 6;
        const GARBAGE_APPLIED= 1 << 7;
        const TOPPED_OUT     = 1 << 8;
        const HELD           = 1 << 9;
        const ROTATED        = 1 << 10;
        const MOVED          = 1 << 11;
        const HARD_DROPPED   = 1 << 12;
        const SOFT_DROPPED   = 1 << 13;
        const SPAWNED        = 1 << 14;
    }
}
```

Five flags beyond `SPEC.md` §5.4 (`ROTATED`, `MOVED`, `HARD_DROPPED`, `SOFT_DROPPED`,
`SPAWNED`) — the M1 audio layer needs one-shot triggers for each, and deriving them by
diffing engine state would violate the "shell reacts to `Events`, never to introspecting
state" rule.

---

## 8. Replay format

```rust
pub struct Replay {
    pub version:        u16,
    pub engine_ver:     u32,
    pub seed:           u64,
    pub match_config:   MatchConfig,
    pub handling:       Handling,
    pub inputs:         Vec<(u8, u8)>,   // RLE: (buttons, run_length)
    pub claimed_result: Outcome,
}

pub struct Outcome {
    pub final_tick:  u32,
    pub lines:       u32,
    pub pieces:      u32,
    pub attack:      u32,
    pub checksum:    u64,
    pub topped_out:  bool,
}
```

Exactly `(seed, MatchConfig, Handling, inputs)` plus a claimed result — invariant #6.
No mode id, no goal, no player name: everything needed to re-simulate and nothing more.

Serialisation in M0 is JSON, for hand-editability while the format is still moving.
Postcard lands with `protocol` in M3 (`SPEC.md` §13.7).

---

## 9. `replay-cli`

```
replay-cli run <replay.json> [--render] [--render-every N] [--render-on lock|clear]
replay-cli verify <replay.json>            # re-simulate, diff against claimed_result
replay-cli compile <script.script> -o <replay.json> --mode <m.toml> --seed <n>
replay-cli checksum <replay.json> [--at-tick N]
replay-cli schema [-o config-schema.json]
replay-cli modes [--dir modes/] [--validate]
```

### 9.1 ASCII renderer

With no client, the alternative view into a kick-table bug is a `u16` hex dump. This is
~40 lines and zero dependencies:

```
  tick 342   piece T@(4,18) r1   combo 2   b2b 1   hold I
  ┌──────────┐
18│    ██    │  next: S Z L O I
19│   ███    │
  │          │
36│██████  ██│  attack sent: 6
37│█████  ███│  lines: 12
38│███████ ██│  pending garbage: 4 @ tick 402
39│██████ ███│
  └──────────┘
```

Only the 20 visible rows plus any occupied buffer rows are drawn. `--render-on lock`
prints one frame per locked piece, which is the right granularity for eyeballing a
whole run.

---

## 10. Test strategy

The determinism suite is, per `SPEC.md` §2.2, *"the single biggest thing separating a
repo that accumulates contributors from one that accumulates stale forks."* It is a
first-class M0 deliverable, not an afterthought.

### 10.1 Golden replays

`testdata/replays/*.replay` + `insta` snapshots of their checksums. Minimum set:

| Replay | Exercises |
|---|---|
| `bag_1000.replay` | 1000 pieces, hard drop only — randomiser + stacking |
| `tspin_suite.replay` | Every T-spin variant: single, double, triple, mini, neo |
| `kicks_all.replay` | Every piece × every transition against a wall and a floor |
| `das_edge.replay` | DAS=0/ARR=0, DAS=max, DCD, DAS-cut boundaries |
| `lock_reset.replay` | All three reset modes, cap exhaustion |
| `garbage.replay` | Scheduled garbage, cancellation, push-out topout |
| `perfect_clear.replay` | PC detection and bonus |
| `sprint40.replay` | A full realistic 40-row run |
| `topout.replay` | All three topout conditions |

### 10.2 Input script DSL

Golden replays must be authorable **without a client**. A tiny text format:

```
# testdata/scripts/tspin_double.script
mode:     versus
seed:     0x1234ABCD
handling: default

LEFT*12
CW
.*4              # '.' = no buttons held
HARD_DROP
.*2
```

`replay-cli compile` turns a script into a `.replay`. Scripts are the human-editable
source; replays are the committed artefact.

### 10.3 Rotation tests

Exhaustive, not sampled: for every `(kind, from_rot, to_rot)` and every board
configuration in a curated fixture set, assert the resulting position matches the
reference SRS table. 7 × 12 transitions is small enough to test completely.

### 10.4 Property tests (`proptest`)

Invariants that must hold after **any** random button stream of any length:

1. The active piece never overlaps an occupied cell.
2. The active piece is always within `0..BOARD_W` horizontally.
3. Every 7 consecutive dequeued pieces contain each kind exactly once.
4. `occupancy` never contains a `FULL_ROW` outside `ClearDelay`.
5. Cell count only ever changes by `+4` (lock), `−10×n` (clear), or `+10×n−1` (garbage).
6. `lock_resets <= lock_reset_cap` under `Extended`.
7. `tick()` never panics, in any phase, including `Dead`.
8. Running the same `(seed, config, handling, inputs)` twice yields identical checksums.

### 10.5 Config tests

1. Every serde field of every config struct appears in that struct's `FIELDS`.
2. Every `FIELDS` key round-trips through serde.
3. Every descriptor default is inside its own declared range.
4. Every `modes/*.toml` parses, validates, and has `id` matching its filename stem.
5. `config-schema.json` matches the committed copy (CI diff).
6. An unknown key produces an error naming the nearest valid key.

### 10.6 Native ↔ wasm parity

`SPEC.md` §11 lists this as CI, and it belongs in M0 even though `client-wasm` doesn't
exist yet: build `engine` for `wasm32-wasip1`, run every golden replay under `wasmtime`,
assert byte-identical checksums against the native run.

This is the check that catches determinism drift *the day it's introduced* rather than
the day a ranked match desyncs — and it is far cheaper to stand up now, against nine
replays, than later against a hundred.

---

## 11. CI for M0

| Job | Gate |
|---|---|
| `cargo test --workspace` | Unit + proptest + insta |
| `cargo clippy --all-targets -- -D warnings` | — |
| `cargo fmt --check` | — |
| **grep guard on `engine/`** | Fail on `f32`, `f64`, `HashMap`, `SystemTime`, `Instant`, `std::fs`, `async` |
| **native↔wasm checksum parity** | Every golden replay, both targets, identical |
| `cargo build -p engine --no-default-features` | Proves the `no_std`-friendly path still compiles |
| **schema freshness** | `replay-cli schema` output matches the committed `config-schema.json` |

### 11.4 `ENGINE_VER` policy

`ENGINE_VER` is bumped on **any** change to simulation-affecting behaviour: kick tables,
handling semantics, scoring, RNG, spawn positions, garbage application, or the checksum
byte order.

Bumping it invalidates existing golden replays for *verification*. The rule
(`SPEC.md` §8.6): keep them playable, mark them unverifiable, never silently re-verify
them under new rules. A bump therefore requires regenerating the snapshots in the same
commit, which makes the blast radius visible in review — the point of the policy.

---

## 12. Acceptance criteria

M0 is done when all of the following are true:

1. `cargo test --workspace` is green, including all nine golden replays.
2. Every golden replay produces identical checksums on native and `wasm32-wasip1`.
3. `cargo build -p engine --no-default-features` succeeds.
4. `replay-cli run testdata/replays/sprint40.replay --render-on lock` prints a legible
   run and a final checksum.
5. `replay-cli modes --validate` accepts all three shipped modes and rejects a
   deliberately typo'd fixture with a "did you mean" message.
6. `replay-cli schema` emits a `config-schema.json` covering **every** field of all four
   config structs, with bounds, units, defaults, and help text.
7. Adding a new tunable handling field requires editing exactly one Rust file and no
   TypeScript. *(Demonstrated by doing it once and reverting.)*
8. The grep guard passes: no floats, no `HashMap`, no time, no I/O in `engine/`.
9. All nine design invariants in `SPEC.md` §14 hold, spot-checked in review.

### 12.1 Suggested build order

Each step ends somewhere testable.

| # | Step | Ends with |
|---|---|---|
| 1 | Constants, fixed-point, `Buttons`, `Events`, `QuadKind`, board type | Unit tests on row masks |
| 2 | `FieldDesc` + descriptors + `clamp()` for all four config structs | §10.5 tests 1–3 green |
| 3 | `crates/config`: TOML load, resolution, schema emission | §10.5 tests 4–6 green |
| 4 | SplitMix64 + 7-bag | Property test 3 |
| 5 | Piece geometry, collision, spawn, basic movement | `replay-cli` renders a stationary board |
| 6 | SRS+ kick tables + rotation | §10.3 exhaustive rotation tests |
| 7 | Gravity, lock delay, reset modes, topout | Property tests 1, 2, 4–7 |
| 8 | Handling state machine, IRS/IHS, misdrop protection | `das_edge.replay` |
| 9 | Clears, collapse, clear delay, perfect clear | `perfect_clear.replay` |
| 10 | Spin detection, attack table, combo, B2B | `tspin_suite.replay` |
| 11 | Garbage scheduling, application, local cancellation | `garbage.replay` |
| 12 | `checksum()` | Property test 8 |
| 13 | `replay-cli` full command set + ASCII renderer | Acceptance criteria 4–6 |
| 14 | CI wiring, wasm parity job | Acceptance criteria 1–3, 8 |

Steps 2–3 come early on purpose. Retrofitting the descriptor layer means touching every
config field and every construction site, and it is the difference between "a new
handling option is a one-line change" and "a new handling option is a change in three
languages."

---

## 13. Amendments to `SPEC.md`

Deviations from the parent spec, with rationale. Worth folding back into `SPEC.md` once
M0 lands.

| § | Change | Rationale |
|---|---|---|
| §3 | **New crate `crates/config`** | `engine` must do no I/O (invariant #1), but TOML loading, layered resolution, and schema emission are needed by both `replay-cli` (M0) and `server` (M3). A shared crate is the right seam; the alternative is duplicating it or breaking the invariant. |
| §5.1 | `PREVIEW_LEN` const → `MAX_PREVIEW` const + `preview_len` config field | Preview length is a per-mode rule, not a build constant. |
| §5.2 | Added `SUBTICK` fixed-point (one scale for time *and* gravity); timers widened `u8` → `u32` | Sub-frame DAS without floats (decision #4). |
| §5.4 | `Events` gains 5 flags | M1 audio needs one-shot triggers; deriving them by diffing state breaks the reactive-shell rule. |
| §8.2 | `Handling` 5 fields → 9; `*_ticks: u8` → `*_ms: u16` | tetr.io parity (decision #5) and ms storage (decision #4). |
| §8.2 | `MatchConfig` gains `lock_reset_mode`, `clear_delay_ms`, `spawn_delay_ms`, `hold_enabled`, `spin_detection`, `b2b_bonus`, `garbage_hole_repeat`; `*_ticks` → `*_ms` | Customisability is a stated goal; these are the knobs every comparable game exposes. |
| §5.7 | Mode TOML gains `spec_version`, `id`, `description`; config nested under `[config]`; `goal` on `ModeSpec` | Decision #3 plus forward-compat for community files. |
| §13.10 | **Resolved:** `ModeSpec` wraps `MatchConfig` | Decision #3. |
| §13.2 | **Resolved:** SRS+ with 180° kicks | Decision #2. |
| §9 | M0 absorbs the scoring work `SPEC.md` assigns to M2 | Decision #1. M2 becomes server routing + UI only. |

---

## 14. Deferred to M1+

Recorded so they aren't rediscovered as surprises.

| Item | Milestone | Note |
|---|---|---|
| Attack table **values** | M2 | Mechanism is M0; numbers need playtesting |
| DAS/ARR feel validation | M1 | Cannot be done headless — the whole reason for client-before-versus ordering |
| Garbage delay tuning | M3 | Must exceed RTT + batch interval + jitter (`SPEC.md` §6.3) |
| Cross-player cancellation | M3 | Server-side; the local half ships in M0 |
| Postcard wire format | M3 | JSON replays until then |
| `ts-rs` type emission | M1 | Feature exists in M0, wired up in M1 |
| Settings UI generation | M1 | Reads `config-schema.json` from M0 |
| Kick tables as data | M4+ | Would enable ARS/Nullpo as community contributions; costs the `const` tables |
| Project name | — | Blocks nothing in M0; crate names are already generic |
