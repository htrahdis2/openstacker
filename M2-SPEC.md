# M2 — Versus rules, made visible · Detailed Spec v0.1

Companion to [SPEC.md](SPEC.md), following [M0-SPEC.md](M0-SPEC.md) and
[M1-SPEC.md](M1-SPEC.md), and picking up from [HANDOFF-M2.md](HANDOFF-M2.md). Expands
milestone **M2** into an implementable document. Where this doc and `SPEC.md` disagree, this
doc wins for M2 scope and the disagreement is listed under
[§16 Amendments](#16-amendments-to-specmd).

> **Deliverable in one sentence:** the versus rules M0 already implements, made visible and
> made tuneable — attack, combo, back-to-back, spins and garbage shown in the client that
> exists, every number of the attack model a described setting with bounds and a control, and
> a golden suite that actually clears lines.

---

## 1. Scope

### 1.1 In scope

| Area | Detail |
|---|---|
| **Goldens** | Recorded games that clear lines, spin, combo, chain B2B, perfect clear and take garbage |
| **`crates/engine`** | Attack becomes fully table-driven: `combo_table` and a new `b2b_table` described, `spin_quad` added. One deliberate `ENGINE_VER` bump |
| **`crates/engine/config`** | `FieldKind::IntList`, so a table of numbers is as describable as a slider |
| **`crates/config`** | Mode-file validation for list-valued settings; regenerated `config-schema.json` |
| **`crates/replay`** | Replays record the garbage they received. `REPLAY_VERSION` 2 |
| **`crates/client-wasm`** | A seeded sparring garbage source; a wider frame block carrying live combo, B2B and incoming batches |
| **`apps/web`** | Versus HUD: a garbage bar with real rows, attack and APM, combo and B2B, spin and perfect-clear cues |
| **`apps/web`** | The tuner: match rules and attack tables editable for local play, exportable as TOML |
| **`modes/`** | `versus.toml` tuned and playable; one or more preset variants shipped as files |
| **CI** | The new goldens, the schema and mode checks, parity under a bumped engine version |

### 1.2 Explicitly out of scope for M2

- `server`, `protocol`, WebSockets, rooms, routing garbage between two people. *(M3)*
- Opponent boards, spectating, desync detection, the stall guard. *(M3)*
- Accounts, database, server-side records or leaderboards. *(M4)*
- Score goals. There is still no scoring model beyond attack, and inventing one to fill in
  `Goal::Score` would be a rule nobody asked for. *(M4)*
- Rollback, prediction, or anything that assumes a peer.
- New skins, effects, particles or music.

### 1.3 The question M2 exists to answer

M0 built the whole attack model — spin detection, combo, back-to-back, perfect clears,
garbage scheduling and cancellation — and shipped it configurable. M1 built a client with the
hooks in place. **Nothing has ever run the two together.** Attack is computed on every locking
tick and discarded; `schedule_garbage` is called by no one outside a unit test; the garbage
bar renders from a value that is structurally always zero.

So the question M2 answers is not *"can the engine do versus?"* — it can — but:

1. **Do the numbers feel right?** That cannot be answered without seeing them, and it cannot
   be iterated on if changing one is a Rust edit and a rebuild.
2. **Would we notice if they changed?** Today, no. All five golden replays are 2–6 pieces
   long and none of them clears a line, so the guard that makes a stranger's rules refactor
   safe to merge does not cover clears, spins, combo, B2B or attack at all.

Both shape the milestone. (2) is why the goldens come first, before any scoring change. (1)
is why the attack model becomes data with bounds and controls rather than constants with a
TOML escape hatch.

---

## 2. Decisions resolved for M2

| # | Question | Decision |
|---|---|---|
| 1 | Where attack numbers live | **All of them in `MatchConfig`, all described.** `combo_table` gets a real descriptor, `b2b_bonus` becomes `b2b_table` indexed by chain length, and `attack_table` gains `spin_quad`. Nothing about the attack model stays a constant in Rust. |
| 2 | How a table of numbers is described | **A new `FieldKind::IntList`** in the descriptor tables, rendered by the generated settings screen like every other control. Adding a list-valued setting stays a Rust-only change. |
| 3 | Cost of decision 1 | **One deliberate `ENGINE_VER` 1 → 2 bump**, taken in the first scoring commit of the milestone, with every golden recompiled in the same commit. Not one bump per tweak — after this, moving a number is a data edit that moves nothing. |
| 4 | Whether golden checksums move with it | **They must not.** Attack never enters `checksum()`; it reaches state only through `GarbageQueue::cancel`, and every existing golden has an empty queue. The `GOLDEN` constants in [golden.rs](crates/replay-cli/tests/golden.rs) must survive the bump unedited. A moved checksum here is a bug, not a rules update. |
| 5 | How tuning is done | **Three surfaces, in order of leverage:** mode files (per-mode, committed), an in-app tuner for local play layered through the existing `HostPolicy`, and preset variants shipped as extra mode files. |
| 6 | How a playtest becomes a committed number | **The tuner exports TOML.** A tuned local game can be copied out as a `[config]` block and pasted into a mode file. Tuning that cannot leave the browser is tuning that gets lost. |
| 7 | What a replay is | **Buttons *and* received garbage.** Garbage is the second input channel into the simulation; a recording that omits it cannot reproduce a versus game. `REPLAY_VERSION` 1 → 2. Amends `SPEC.md` §14.6. |
| 8 | Where garbage comes from with no server | **A seeded sparring source in `client-wasm`.** Rust, driven by the tick counter, seeded from the match seed. Never TypeScript: a sender that decides amounts, timing or hole columns is deciding rules. |
| 9 | Who honours `garbage_hole_repeat` | **The sender, not the engine.** A batch with varied holes is several `PendingGarbage` entries sharing an `apply_at_tick`. The engine keeps applying one hole per batch and never derives a column, which is what `SPEC.md` §5.6 requires. |
| 10 | Whether versus is playable in M2 | **Yes, against the sparring source.** `survival` becomes a goal the client can decide the end of: the run ends on topout. Two humans is M3. |
| 11 | Frame block size | **Grows from 64 to 128 bytes.** Live combo, live B2B and per-batch incoming detail do not fit, and packing them into reserved bytes to avoid changing one constant is a false economy. |
| 12 | Starting numbers | **Close to today's table**, which already matches modern practice, plus escalating back-to-back. Stated in §13 as a starting point for playtest, not as settled. |
| 13 | Who pays the chain bonus | **Only a clear that carries the chain on.** Found while making §5.3's table: the flat bonus was being paid by the plain clear that *broke* a run, rewarding the thing the bonus exists to discourage. Fixed inside the same `ENGINE_VER` bump, which is why `b2b_chain.replay` claims 9 rather than 10. |

---

## 3. Layout

No new crate. M2 changes files in five places:

```
crates/
├── engine/          scoring becomes fully table-driven; ENGINE_VER 2
│   └── config/      FieldKind::IntList; combo_table, b2b_table, spin_quad descriptors
├── config/          list validation in mode files; schema regenerated
├── replay/          the garbage stream; REPLAY_VERSION 2
├── client-wasm/     sparring source; wider frame block
└── replay-cli/      compiles and renders the new goldens
apps/web/            versus HUD, the tuner, survival runs
modes/               versus.toml tuned; preset variants
testdata/
├── scripts/         the goldens that clear lines            [NEW]
└── replays/         compiled from them                      [NEW]
```

Dependency direction is unchanged:

```
engine ← config  ← replay-cli
       ← replay  ← client-wasm ← web
```

No new dependencies in any crate or in the client.

---

## 4. What M2 does not build

Most of the versus model is already implemented and tested. Check here before writing
anything:

| Thing | Where it already is |
|---|---|
| Attack per clear, spins, mini spins, combo, B2B, perfect clear | [`attack_for`](crates/engine/src/scoring.rs), reading `MatchConfig` for every value |
| Spin detection, three rules plus off | [`detect_spin`](crates/engine/src/scoring.rs) |
| `TickResult::attack`, after local cancellation | Set on every locking tick, [engine.rs:642](crates/engine/src/engine.rs:642) |
| Garbage queue, sorted, capped, cancellable | [garbage.rs](crates/engine/src/garbage.rs) |
| Garbage applied at an absolute tick, with topout on overflow | [engine.rs:681](crates/engine/src/engine.rs:681) |
| `SPIN` / `MINI_SPIN` / `B2B_*` / `PERFECT_CLEAR` / `GARBAGE_APPLIED` | In `Events` since M0, populated, unconsumed |
| Pending rows, batch count, ticks to arrival | Already in the frame block ([frame.rs](crates/client-wasm/src/frame.rs)) |
| An attack table with bounds, units and help text | Already a schema group; `config-schema.json` carries all ten fields |
| A layer above the mode for host-imposed rules | [`HostPolicy` / `resolve`](crates/config/src/resolve.rs), unused so far |
| Match rules travelling inside every replay | `Replay::config` — a tuned game is already self-describing |

---

## 5. Attack as data

### 5.1 The gap

`attack_table` is already tuneable in every sense that matters: descriptors, bounds, a schema
group, mode-file validation, and controls the client generates for free. Three parts of the
same model are not:

| Value | Today | Problem |
|---|---|---|
| `combo_table` | `ArrayVec<u8, 21>`, listed in `MatchConfig::NESTED` with no descriptor | No bounds, no control, no validation. A mode file can put anything in it |
| `b2b_bonus` | A flat `u8` added once, however long the chain | Cannot express escalation, which is how modern stackers reward chaining |
| Spin quad | Falls into `spin_triple` via `(Spin::Full, n) if n >= 3` | Invisible in `three_corner`, wrong under `all_spin` |

The point of fixing all three at once is §2 decision 3: the rules move exactly once.

### 5.2 `FieldKind::IntList`

A fourth variant beside `Int`, `Bool` and `Enum` in
[desc.rs](crates/engine/src/config/desc.rs):

```rust
IntList {
    /// Bound on every entry.
    min: i64,
    max: i64,
    /// Longest list accepted. Shorter is legal; the last entry saturates.
    max_len: usize,
    default: &'static [i64],
    unit: Unit,
}
```

Obligations that come with it, each mirroring what the three existing kinds already promise:

- `FieldKind::default_is_in_range` returns false if any default entry is outside `min..=max`
  or the default is longer than `max_len`. Tested, not assumed, like the others.
- `FieldDesc` gains `clamp_list`, clamping each entry and truncating to `max_len`. Empty
  input keeps the existing repair behaviour: `MatchConfig::clamp` already replaces an empty
  `combo_table` with the default rather than leaving `combo_bonus` to index into nothing.
- `schema.rs` emits `{"type": "intList", min, max, maxLen, default, unit}`.
- `validate_table` in [mode.rs](crates/config/src/mode.rs) grows an `IntList` arm: the value
  must be a TOML array of integers, each within bounds, no longer than `max_len`. Today a
  list-valued key is skipped entirely because it is in `NESTED`, so
  `combo_table = ["nonsense"]` passes the loader and fails deep inside serde.
- `combo_table` comes out of `MatchConfig::NESTED` and into `FIELDS`. `gravity` stays nested;
  it is a tagged enum, not a bounded list, and it has its own `validate_gravity`.

### 5.3 `b2b_table` replaces `b2b_bonus`

```rust
/// Rows added for a back-to-back chain, indexed by chain length. Saturates at the last
/// entry, exactly like `combo_table`.
pub b2b_table: ArrayVec<u8, B2B_TABLE_LEN>,
```

`attack_for` currently adds `config.b2b_bonus` when `b2b_active`. It instead adds
`config.b2b_bonus(chain)`, a lookup with the same saturating shape as `combo_bonus`
([match_config.rs:487](crates/engine/src/config/match_config.rs:487)) — one helper written
twice is better than a generic one used twice here, but either is fine as long as the
saturation is identical and tested.

The chain length passed in is the value of `self.b2b` *before* this clear extends it, which is
what `b2b_active` is derived from today. Index 0 is therefore "no chain" and must be 0 in any
sane table; nothing enforces that, because a mode is allowed to be strange.

**Migration.** `b2b_bonus` disappears, and mode files that set it are a hard error with a
suggestion, because `nearest_key` will offer `b2b_table` for it. [versus.toml](modes/versus.toml)
sets `b2b_bonus = 1` today and is updated in the same commit. This is the intended behaviour
of the loader and needs no special case.

### 5.4 `spin_quad`

One more entry in `AttackTable`, and one more arm in `attack_for`:

```rust
(Spin::Full, 3) => t.spin_triple,
(Spin::Full, n) if n >= 4 => t.spin_quad,
```

`(Spin::Mini, n) if n >= 2 => t.mini_spin_double` is deliberately left as it is. A mini triple
is not reachable under `three_corner` or `immobile`, and adding a table entry for a case no
rule produces is a setting that can only ever be wrong. Noted here so the asymmetry reads as a
decision.

### 5.5 The schema, regenerated

`cargo run -p config --bin emit-schema > config-schema.json`, in the same commit. CI already
fails on drift. The client needs one change to render the new kind — a numeric row per entry,
with the entry index as its label — and that is the only per-setting knowledge TypeScript is
allowed to have about it: bounds, length and help all come from the schema.

### 5.6 What stays out of the tables

Worth stating so the boundary is not re-litigated per pull request. These would be new
*rules*, not new numbers, and none is in M2:

- Garbage multipliers or surge mechanics.
- A blocking window in which incoming rows can be cancelled after they land.
- Cheese patterns — several holes per batch, or holes that move mid-batch. §7.4 covers what a
  sender can already express without them.
- Per-piece spin bonuses beyond the mini/full split.

---

## 6. Tuning surfaces

### 6.1 Mode files are the unit of tuning

A mode file already carries a full `MatchConfig`, so a variant of versus with different
numbers is a file, not a code change, and it can be posted in a chat. That is
`SPEC.md` §2.2's second contribution surface, and it is the surface M2 makes real:

```toml
# modes/versus.toml
[config.attack_table]
quad = 4
spin_double = 4
spin_quad = 8

[config]
combo_table = [0, 0, 1, 1, 1, 2, 2, 3, 3, 4, 4, 4, 5]
b2b_table   = [0, 1, 1, 1, 2, 2, 2, 3, 3, 4]
```

Per-mode is what makes per-difficulty possible without a new concept: a gentler variant is
another file with a smaller table and a longer `garbage_delay_ms`.

### 6.2 The tuner

Mode files require an editor, a rebuild of `modes.generated.json`, and a reload. That is fine
for committing a decision and far too slow for making one. So the settings screen gains an
editable path for the `match` and `attack_table` groups **for local play**.

It reuses what already exists rather than inventing a fourth notion of where a setting comes
from. [resolve.rs](crates/config/src/resolve.rs) already defines the precedence:

```
defaults  <-  mode file  <-  host policy  <-  player
```

`HostPolicy::match_config` is exactly "rules the host pins, overriding the mode", and in a
local game the player *is* the host. So:

- `Settings` gains an optional `house_rules: Option<MatchConfig>` (with `SETTINGS_VERSION`
  bumped to 2 and a migration step, which [settings.rs](crates/config/src/settings.rs)
  already has the chain for).
- Starting a local game resolves `resolve(Some(mode.config), &policy, Some(handling))`, where
  `policy.match_config` is the house rules when set.
- Opening the tuner with nothing set seeds it from the selected mode, so editing starts from
  what is actually being played rather than from defaults.
- A visible "back to the mode's rules" control clears it. Tuning you cannot get out of is a
  bug report.

This replaces the hard-coded `SECTION_OF[group] = null` in
[panel.ts:23](apps/web/src/settings/panel.ts:23), which currently renders both groups
read-only with the fixed caption "fixed by this mode". The caption instead comes from
`Layer::locked_reason()`, which already provides the four wordings.

### 6.3 What locks during a run

Unchanged in principle from M1 §8.5, and worth restating because M2 doubles the number of
simulation-affecting controls: every group the schema marks `affectsSimulation` is frozen
while a game is running, because `MatchConfig` is captured at `Engine::new`. A tuner change
applies to the next game. The control says so; it does not silently do nothing.

### 6.4 Presets ship as files

At least two versus variants ship, so the tuner has something to compare against and so the
preset mechanism is exercised by the build:

| File | What it is |
|---|---|
| `modes/versus.toml` | The tuned default (§13) |
| `modes/versus_classic.toml` | The M0 proposal — flat B2B, no spin quad entry above the triple — kept as a comparison point |

`load_mode_file` requires the id to match the filename, so ids follow the file names.

### 6.5 Export as TOML

The tuner offers the current resolved `MatchConfig` as a `[config]` block, formatted for a
mode file, with values identical to the mode's omitted. That is the whole path from "this
feels better" to a committed file, and it is a serializer plus a clipboard write.

### 6.6 A tuned run is still a replay

`Replay::config` already carries the exact `MatchConfig` the game was played under, so a run
under house rules verifies against those rules rather than the mode's, and hands to anyone
with the repo. No change needed — noted because it is the property that makes the tuner safe
to ship.

---

## 7. Garbage, locally

### 7.1 The sparring source

Nothing sends garbage yet. M2 adds a source in `client-wasm`, not in `engine` (a training
opponent is not a rule of the game) and not in TypeScript (what it decides *are* rules):

```rust
pub struct Sparring {
    rng: SplitMix64,      // seeded from the match seed
    next_at: u32,         // absolute tick
    // rate, batch size, and ramp come from the mode's config plus a small profile
}
```

Per tick, `Game::tick` asks the source whether anything is due, and forwards what it produces
to `Engine::schedule_garbage`. It is deterministic on `(seed, config, tick)`, so a sparring
run replays exactly — and, because §8 records what was scheduled, it replays exactly even if
the source's behaviour changes later.

Profiles are data, kept in the mode file where they belong to the mode rather than to the
player. Something on the order of: an opening quiet period, then a batch every *n* ticks with
the interval shrinking, so a survival run has an end.

### 7.2 Scheduling ticks

Two traps from M1, both live here:

- **`Engine::new` leaves the counter at 0; the first `tick()` is tick 1.** Recorded input
  index `i` is tick `i + 1`. `apply_at_tick` is absolute, so an off-by-one lands rows a frame
  early or late.
- **Scheduling is done before the tick that would apply it.** `apply_due_garbage` runs at the
  top of `tick()` and takes everything with `apply_at_tick <= tick`, so a batch scheduled for
  a tick already passed lands immediately. The source computes arrival as
  `now + garbage_delay_ticks`, using `MatchConfig::garbage_delay_ticks`
  ([match_config.rs:510](crates/engine/src/config/match_config.rs:510)) rather than
  converting milliseconds itself.

The pending queue is part of `checksum()`
([engine.rs:739](crates/engine/src/engine.rs:739)), so *when* a batch enters the queue is
simulation state, not bookkeeping. That is invisible in M2, where one machine schedules for
itself, and is the central problem of M3; §18 carries the note forward rather than solving it
here.

### 7.3 Cancellation

Already implemented and already correct: incoming rows are answered before anything is sent
on, soonest-arriving batch first ([engine.rs:642](crates/engine/src/engine.rs:642),
[garbage.rs:98](crates/engine/src/garbage.rs:98)). M2's job is to make it *legible* — a clear
that cancels must visibly consume the bar rather than silently reducing a number nobody sees
(§9.2).

### 7.4 Caps and hole columns

- `garbage_cap` is applied per batch at application time, in the engine. The sender does not
  need to know it.
- The hole column is chosen by the sender and carried explicitly. The engine never derives
  one. Under a server this is `SPEC.md` §5.6; locally it is the same code path, which is the
  point.
- `garbage_hole_repeat` is currently declared, clamped, schema'd, set in `versus.toml` — and
  read by nothing. M2 gives it its meaning **in the sender**: when false, a batch is emitted
  as several `PendingGarbage` entries sharing an `apply_at_tick` with different columns. The
  rejected alternative is having the engine vary holes within a batch, which would mean
  deriving a column from a generator, which is the one thing §5.6 forbids.

### 7.5 What must not move to TypeScript

The temptation at M2 is specifically attack and garbage — they are arithmetic, and the numbers
are right there in the frame block. The rule from [apps/web/README.md](apps/web/README.md)
stands: the client renders and reports buttons. It does not compute attack, decide what a
spin was worth, count a combo, or work out when rows land. At M3 every one of those is two
peers disagreeing about a live match.

---

## 8. Replays record the garbage they received

### 8.1 Why

`Replay` is `(seed, config, handling, inputs)`. Garbage arrives from outside and changes the
board, so a recording of a game that received any is not reproducible: `Replay::simulate`
re-runs it with an empty queue and produces a different board, a different checksum, and a
claimed result that no longer verifies. Concretely, this milestone cannot have a golden
covering garbage application or cancellation until this changes — which is trap 4 of the
handoff meeting the one part of the model it cannot reach.

The framing that resolves it: **a replay records the simulation's inputs, and buttons are only
one of the two input channels.** Garbage is the other.

### 8.2 The shape

```rust
pub struct Replay {
    pub version: u16,          // 2
    pub engine_ver: u32,
    pub seed: u64,
    pub config: MatchConfig,
    pub handling: Handling,
    pub inputs: Vec<(u8, u8)>,
    /// Garbage scheduled during the game, in the order it was scheduled, each with the
    /// tick it was scheduled *on* as well as the tick it lands on.
    pub garbage: Vec<ScheduledGarbage>,
    pub claimed: Outcome,
}

pub struct ScheduledGarbage {
    /// The tick during which `schedule_garbage` was called.
    pub at_tick: u32,
    pub garbage: PendingGarbage,   // apply_at_tick, amount, hole_col
}
```

`at_tick` is not redundant with `apply_at_tick`. The queue is part of the checksum, so a batch
that entered the queue at a different tick is a different game even if it lands on the same
one. Re-simulation therefore schedules on `at_tick`, not on receipt.

`Replay::simulate` gains that step: before each `engine.tick`, schedule everything with
`at_tick == next tick`. `Replay::record` grows a matching parameter. Both stay pure.

### 8.3 Version 2, and reading version 1

`REPLAY_VERSION` goes to 2. A v1 file has no `garbage` key; `#[serde(default)]` on that field
alone reads it as empty, which is exactly right — a v1 recording is a solo game and received
none. No migration code, and every M1 recording keeps playing.

The `engine_ver` field is separate and keeps its meaning: after §11's bump, a v1 recording is
still watchable and is marked unverifiable.

### 8.4 `replay-cli`

- `verify` schedules from the stream, so a versus recording verifies.
- `render` marks garbage rows from the occupancy grid, never from the color channel.
- The script DSL ([script.rs](crates/replay-cli/src/script.rs)) gains one directive so a
  golden can inject garbage without a client:

  ```
  garbage: at=120 apply=180 rows=4 hole=3
  ```

  Same shape as `seed:` and `mode:`, ignored-with-a-message by older builds in the same way
  `handling:` already is.

---

## 9. The frame block and the versus HUD

### 9.1 New fields

`FRAME_BYTES` grows from 64 to 128. The block is written once per tick and read through one
view; its size costs nothing per frame, and packing new fields into the five reserved bytes to
avoid touching a constant would be a false economy. Added:

| Field | Why the client cannot derive it |
|---|---|
| `combo` | Live combo, not the max. Counting locks in TypeScript is a rule |
| `b2b` | Live chain length, which now decides a table lookup |
| `incoming[8]` | Per batch: rows and ticks until arrival. Only the total exists today |
| `incomingLen` | How many of those slots are real |
| `attackThisTick` | Already present as `attack`; kept, now consumed |

Each addition is the three-step change the handoff describes: the field in `Frame`, its offset
in `offset`, a line in `write`, an entry in `frame_layout()`, and the matching read in
[sim/frame.ts](apps/web/src/sim/frame.ts). The existing test that reserved bytes stay zero
moves with the size and keeps its job.

### 9.2 The garbage bar

The bar exists and renders from `pendingRows` ([hud.ts:72](apps/web/src/render/hud.ts:72)).
M2 makes it mean something:

- **Segmented by batch**, from `incoming`, so four rows arriving now and four in two seconds
  do not read as eight rows of the same thing.
- **Urgency from `nextGarbageIn`**, not from a wall clock. The nearest segment is marked as it
  approaches its tick.
- **Cancellation is visible.** A clear that cancels removes the segment it consumed; that is
  the feedback that teaches a player that a well-timed clear is a defensive move.
- Capacity is `garbage_cap`-aware rather than the current fixed 12.

### 9.3 Attack and APM

Attack sent is already cumulative in `Stats::attack_sent`. The HUD shows it, plus attack per
minute derived from the tick counter the same way PPS already is
([hud.ts:26](apps/web/src/render/hud.ts:26)). Time is ticks, everywhere; nothing reads a
clock.

### 9.4 Combo, B2B, spins, perfect clears

All driven by the `Events` bits in the frame block, never by comparing this frame's state to
the last. That rule is why M0 defined the flags at all, and diffing state would reintroduce
the coupling the bitset removes:

| Cue | Trigger |
|---|---|
| Spin / mini spin banner | `SPIN` / `MINI_SPIN` |
| B2B chain indicator | `B2B_CONTINUED`, cleared on `B2B_BROKEN`, count from `b2b` |
| Combo counter | `LINES_CLEARED` with `combo > 0` |
| Perfect clear | `PERFECT_CLEAR` |
| Garbage landing | `GARBAGE_APPLIED` |

Audio already dispatches on the same bits ([audio.ts](apps/web/src/audio.ts)) and gains cues
for the flags nothing plays yet.

### 9.5 What the HUD must not compute

Attack for a clear, whether a lock was a spin, whether a chain continued, how many rows are
about to land, or when they land. Every one of those is in the block. A HUD that computes one
is a HUD that can disagree with the game it is describing.

---

## 10. Versus mode and survival runs

### 10.1 Selectable

`isPlayable` ([modes.ts:38](apps/web/src/modes.ts:38)) accepts `survival`, which today it
rejects for want of an end condition. The end condition is a topout, which the client already
detects and already handles for every other mode.

`goalReached` returns false for survival by construction — the run does not *meet* a goal, it
ends — and `remaining` keeps returning null, which the HUD already renders as "survive".

### 10.2 Results

The results screen gains the numbers a versus run is judged on: time survived, attack sent,
APM, max combo, max B2B, rows received. All from the frame block.

### 10.3 Bests point the other way

`recordBest` ([replays/store.ts:75](apps/web/src/replays/store.ts:75)) keeps the *lowest* tick
count and only for finished runs. Both are wrong for survival, where a run ends in a topout by
definition and longer is better. The store takes the goal's direction: `lines` and `time`
goals keep the current behaviour, `survival` keeps the longest run and counts topped-out runs.

---

## 11. Determinism obligations

1. **One `ENGINE_VER` bump, on purpose, once.** 1 → 2, in the commit that makes attack
   table-driven, with every golden recompiled in that same commit. A test already asserts each
   golden's `engine_ver` matches the build
   ([golden.rs](crates/replay-cli/tests/golden.rs)), so this cannot be forgotten.
2. **The golden checksums must not move with it.** Attack changes what is *sent*, and reaches
   state only through cancellation against a queue that every existing golden leaves empty.
   The `GOLDEN` constants are not edited. If one needs editing, something changed that was not
   meant to.
3. **After the bump, tuning moves nothing.** Editing `versus.toml` or the tuner changes no
   checksum and no version, because the values travel inside each replay. That is the property
   that makes fast iteration safe, and it is the reason for doing the rule changes in one
   deliberate step at the start rather than trickling them through the milestone.
4. **M1 recordings become unverifiable, and stay playable.** `is_verifiable()` already draws
   that distinction. Nothing silently re-verifies an old file under new rules.
5. **Colors still decide nothing.** Garbage rows are marked `8` in the render channel and the
   channel is excluded from the checksum. A client asking a color whether a row is garbage has
   recreated the coupling the exclusion prevents. Ask the occupancy grid.
6. **Time is still ticks.** Garbage arrival, attack rate, survival time and the sparring
   source all count ticks. Nothing reads a wall clock but the code deciding how many whole
   ticks are owed.
7. **TypeScript still implements no rule.** Restated because §7.5 is where this milestone will
   be tempted.

---

## 12. Test strategy

### 12.1 The goldens come first

Before any scoring change. This is the highest-value work in the milestone and the handoff
calls it out as trap 4: today's five goldens are 2–6 pieces and 19–40 ticks, none clears a
line, and `quad.replay` sets a quad up and ends before it lands. Changing attack values
against that suite means watching every checksum hold still and learning nothing.

Each is a script in `testdata/scripts/`, compiled with `replay-cli compile` and pinned:

| Script | Covers |
|---|---|
| `single.script` | One row clears; combo starts |
| `quad_clear.script` | Four rows clear; `quad.replay`'s setup, carried through the drop |
| `spin_double.script` | `SPIN`, the three-corner path, spin attack over plain |
| `mini_spin.script` | `MINI_SPIN` and the mini/full distinction |
| `combo_chain.script` | Several consecutive clears; the combo table indexed past its start |
| `b2b_chain.script` | Quad, quad, then a plain clear: `B2B_CONTINUED` then `B2B_BROKEN` |
| `perfect_clear.script` | An empty board after a clear |
| `garbage_land.script` | Injected garbage applied on its tick; the stack rises with the active piece |
| `garbage_cancel.script` | A clear cancelling incoming rows, and the leftover passing through |

The last two depend on §8's format change and on the script directive in §8.4. They are the
first goldens in the project's history that exercise the queue.

### 12.2 Rust unit

| Level | Covers |
|---|---|
| `engine::config::desc` | `IntList` bounds, `clamp_list` truncation, an out-of-range default detected |
| `engine::config::match_config` | `b2b_bonus(chain)` saturation past the table; empty tables repaired; the descriptor drift test extended to list fields |
| `engine::scoring` | `spin_quad` distinct from `spin_triple`; escalation strictly increasing for the shipped table; attack still saturating rather than wrapping |
| `config::mode` | A list with a bad entry, a too-long list, and a non-array value each rejected with the key named; `b2b_bonus` suggesting `b2b_table` |
| `replay` | v2 round trip; a v1 file reading as empty garbage; re-simulation scheduling on `at_tick`; a tampered garbage stream failing to verify |
| `client-wasm` | The sparring source deterministic on `(seed, config)`; the wider block's new fields against a known engine state; reserved bytes still zero |

### 12.3 TypeScript

| Level | Covers |
|---|---|
| Unit | The schema→control mapping for `intList`; survival goal evaluation; bests direction per goal; garbage bar segmentation and capacity from `garbage_cap` |
| Unit | House rules resolving over a mode, and clearing back to it |
| Storage | Survival bests against `fake-indexeddb`, including that a topped-out run counts |

### 12.4 Integration

Both existing scripts keep working and both gain reach:

- `client-parity.sh` — every golden, including the new ones, reproduced through the built
  client module. The garbage goldens make this cover the queue for the first time.
- `capture-roundtrip.sh` — a captured sparring game, with its garbage stream, verifying under
  `replay-cli` unmodified.

### 12.5 What only a human can test

The milestone exists for these. None can be automated, and each needs a pass before M2 is
called done:

| Check | What good looks like |
|---|---|
| A quad feels worth building | The bar visibly answers it |
| Cancellation reads as defence | Clearing while rows are incoming visibly eats the segment about to land |
| The bar is readable under pressure | Four batches queued is still parseable at a glance, mid-game |
| Arrival is not a surprise | Rows landing is telegraphed early enough to react |
| B2B escalation is felt | A chain is worth continuing; breaking it is a real loss |
| Spin cues are distinguishable | Spin, mini and perfect clear are not one flash |
| Survival has an arc | The sparring ramp is beatable at the start and lethal eventually |
| Tuning round trip | Change a number, play, export the TOML, paste into a mode file, get the same game |
| The M1 checklist | [M1-SPEC.md](M1-SPEC.md) §13.4's twelve handling checks, which were never walked item by item |

---

## 13. The numbers

The starting point. Today's table already matches modern practice closely, so most values do
not move; what changes is that all of them are now data.

| Clear | Rows |
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
| **Spin quad** | **8** |
| Perfect clear | +10 |

```toml
combo_table = [0, 0, 1, 1, 1, 2, 2, 3, 3, 4, 4, 4, 5]   # unchanged
b2b_table   = [0, 1, 1, 1, 2, 2, 2, 3, 3, 4]            # was a flat +1
garbage_delay_ms = 1000
garbage_cap      = 8
```

How to iterate, in order of how often it will be needed:

1. **Too much pressure early** → raise `garbage_delay_ms` or soften the sparring ramp before
   touching the table. Delay changes how a number *feels* without changing what it *is*.
2. **Quads not worth it** → the gap between `quad` and `triple`, then `b2b_table`.
3. **Spins overpowered** → `spin_double` first; it is the one most games are decided on.
4. **Combos dominate** → flatten the middle of `combo_table` rather than its tail.

Every one of those is a mode-file edit or a tuner drag. None moves a checksum.

---

## 14. CI

Added to [ci.yml](.github/workflows/ci.yml):

| Job | Purpose |
|---|---|
| Golden suite | Already runs; now covers clears, spins, combo, B2B, perfect clears and garbage |
| `emit-schema --check` | Already runs; now covers `intList` fields |
| `emit-modes --check` | Already runs; now covers the versus variants |
| `client-parity.sh` | Already runs; now includes the garbage goldens |
| `capture-roundtrip.sh` | Extended to a capture that received garbage |

No new jobs. The existing purity guard, wasm parity, release-profile test run and doc build
stay exactly as they are.

---

## 15. Acceptance criteria

M2 is done when:

1. `cargo test --workspace` (debug and release) and `pnpm test` are green.
2. The golden suite contains recordings that clear single, double, triple and quad, spin and
   mini-spin, chain and break B2B, perfect clear, and both receive and cancel garbage.
3. `ENGINE_VER` is 2, every golden was recompiled under it, and **not one pinned checksum
   changed**.
4. `combo_table`, `b2b_table` and the full attack table are described settings: bounded,
   validated in mode files with a message naming the key, present in `config-schema.json`, and
   rendered as controls with no per-setting TypeScript.
5. Adding a list-valued setting in Rust makes a working control appear with no client change.
6. Versus is selectable, playable, and ends in a topout, with a results screen showing time
   survived, attack, APM, max combo, max B2B and rows received.
7. Garbage visibly arrives, visibly lands, and is visibly cancelled by a well-timed clear.
8. A sparring game captured in the browser verifies under `replay-cli` unmodified, including
   its garbage stream, and plays back in the browser to the same result.
9. A replay recorded in M1 still plays and is reported as unverifiable rather than failing.
10. Match rules and attack tables can be edited between games, exported as a `[config]` block,
    pasted into a mode file, and produce the same game.
11. A tuned run's replay verifies under the rules it was played with, not the mode's.
12. `garbage_hole_repeat` does something, or it does not exist.
13. The manual checklist in §12.5 has been walked once, on a real machine, by a human.

---

## 16. Amendments to `SPEC.md`

| § | Change | Rationale |
|---|---|---|
| §5.5 | The attack table's `[DECIDE]` is resolved, and `spin_quad` is added to it | §13. Values are data and travel in the replay, so this is a default rather than a commitment |
| §5.5 | Back-to-back is a table indexed by chain length, not a flat bonus | Escalation is how the mechanic is expected to behave, and it is a number rather than a rule once it is a table |
| §8.5 / §14.6 | A replay is `(seed, MatchConfig, Handling, buttons, received garbage)` | Garbage is an input to the simulation. A versus recording without it cannot be re-simulated, which would make replay verification useless for exactly the games it matters most for |
| §5.6 | `garbage_hole_repeat` is honoured by the sender, not the engine | The engine deriving hole columns contradicts the same section's rule that they are sent explicitly |
| §9 | M2 also ships a local sparring opponent | Versus rules cannot be tuned against nothing, and the alternative is tuning them for the first time during M3 with a network in the way |
| §9 | M2 also ships an in-app tuner | "Tune the numbers" is the milestone; a tuning loop that requires an editor and a rebuild is not one |

---

## 17. Build order

Each step ends somewhere testable, and each is a separate reviewable change.

| # | Step | Ends with |
|---|---|---|
| 1 | Scripts and goldens for clears, spins, combo, B2B and perfect clear | The suite covers scoring, with no rule changed and no checksum moved |
| 2 | `FieldKind::IntList`, `clamp_list`, schema emission, mode validation | A list setting is describable; schema regenerated |
| 3 | `combo_table` described, `b2b_table` replaces `b2b_bonus`, `spin_quad` added, `ENGINE_VER` 2, goldens recompiled | Acceptance criteria 3 and 4 |
| 4 | The client renders `intList` controls | Acceptance criterion 5 |
| 5 | `Replay` v2 with the garbage stream; `replay-cli` scheduling and its script directive | Garbage goldens are expressible |
| 6 | Garbage goldens | Acceptance criterion 2 complete |
| 7 | The sparring source in `client-wasm`, recorded into the replay | A local game receives garbage; capture round trip passes |
| 8 | Frame block widened; new fields exposed | The client can see combo, B2B and per-batch incoming |
| 9 | Versus HUD: bar, attack, APM, combo, B2B, spin and PC cues, audio | Acceptance criterion 7 |
| 10 | Survival goals, results screen, bests direction | Acceptance criteria 6 |
| 11 | House rules via `HostPolicy`, the tuner, TOML export | Acceptance criteria 10 and 11 |
| 12 | `versus.toml` tuned, preset variants, `garbage_hole_repeat` honoured | Acceptance criterion 12 |
| 13 | Docs, README, CI wiring, the manual pass | Acceptance criteria 1 and 13 |

Steps 1–6 are verifiable without a browser and without a rendered pixel. Everything after
rests on an attack model already proven to produce the right numbers, so a disagreement found
later is a client bug rather than an open question about which side is right.

---

## 18. Deferred to M3+

| Item | Milestone | Note |
|---|---|---|
| Routing garbage between two people | M3 | M2 is the local half; nothing about the engine changes for it |
| **When a batch enters the queue** | M3 | The queue is part of `checksum()`, so a server and a client that schedule the same batch on different ticks disagree — and cancellation, which reads the queue, makes that disagreement real rather than cosmetic. M2 should not paper over it; M3 must design for it |
| Opponent boards, spectating | M3 | Snapshots, not simulation |
| Desync detection, stall guard | M3 | `Sync` checksums and a wall-clock guard |
| `protocol` crate, `ts-rs`, postcard | M3 | Nothing crosses a wire yet |
| Score goals | M4 | Still no scoring model beyond attack; `Goal::Score` parses and stays unevaluated |
| Garbage multipliers, surge, cheese patterns, blocking windows | M4+ | New rules, not new numbers (§5.6) |
| Server-side records and leaderboards | M4 | Local bests only |
