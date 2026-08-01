# M1 → M2 handoff

What the client gives you, and the things that will bite you. Read [SPEC.md](SPEC.md) for
the design and [M1-SPEC.md](M1-SPEC.md) for the client's; this covers only what M1 decided
that neither says.

## What exists

- `crates/engine` — unchanged in M1. All the rules, no I/O. **`ENGINE_VER` is still 1.**
- `crates/config` — mode files, player settings, the settings schema. File loading is
  behind a default-on `files` feature so the browser build carries no TOML parser.
- `crates/replay` — the replay format, moved out of `replay-cli` so the browser can
  produce it too.
- `crates/client-wasm` — the browser's view of the simulation.
- `apps/web` — the client. Sprint 40 and Blitz are playable, with settings, recordings and
  local bests.

M2 is versus rules made visible: attack, combo, B2B, spins and garbage. **All of it already
exists in the engine.** M0 built the scoring path and M1 built a HUD with the hooks in
place. Your job is mostly to show what is already being computed, and to tune numbers.

## What M2 does not have to build

The temptation is to start by adding engine features. Check first — most are there:

| Thing | Where it already is |
|---|---|
| Attack per clear, spins, combo, B2B | `scoring.rs`, all configurable in `MatchConfig` |
| `TickResult::attack` | Set on every locking tick, after local cancellation |
| Garbage queue and application | `garbage.rs`, `schedule_garbage`, applied at an absolute tick |
| `SPIN` / `MINI_SPIN` / `B2B_*` / `PERFECT_CLEAR` events | In the bitset since M0, populated, unconsumed |
| Pending rows and time to arrival | In the frame block, drawn as an empty bar |
| Attack table as data | `modes/versus.toml`, and in the settings schema |

What is genuinely missing is **routing** — deciding who receives what — and that is M3's
server, not M2. M2 is the local half: showing attack, showing incoming garbage, and making
the numbers feel right.

## Six things that will bite you

**1. The garbage bar is fed but never filled.** `pendingRows` and `nextGarbageIn` are in
the frame block and the bar renders from them. Nothing calls `schedule_garbage` in the
client, so it is always zero. To see it work, call it — there is no plumbing to add, only a
caller.

**2. Versus is listed but not selectable.** `modes/versus.toml` loads and is bundled, but
its goal is `survival`, and `isPlayable()` in [modes.ts](apps/web/src/modes.ts) returns
false for goals this build cannot decide the end of. A survival run ends only by topping
out; decide what that means before enabling it.

**3. Tuning the attack table moves no checksum, but tuning handling does.** Attack values
live in `MatchConfig` and travel with each replay, so changing `modes/versus.toml` costs
nothing. Changing what a *spin* is, or any handling semantics, changes the rules: golden
checksums move and `ENGINE_VER` must go up with them. Regenerate the goldens deliberately
in the same commit, rather than re-pinning numbers until CI is quiet.

**4. The golden replays do not clear a single line.** All five are 2–6 pieces and 19–40
ticks; `quad.replay` sets a quad up and ends before it lands. The engine's clear, spin,
combo and attack logic is covered by unit tests, but the *golden suite* — the guard that
makes a stranger's rules refactor safe to merge — does not exercise any of it. **Fix this
before you touch scoring**, or you will change attack values and watch every checksum hold
still. `replay-cli compile` and the script DSL in `testdata/scripts/` are what you need.

**5. Colors decide nothing, and garbage is not a color.** `colors()` marks garbage rows
with `8`, but that is a render channel and it is excluded from the checksum. A client that
asks a color whether a row is garbage has recreated the coupling the exclusion exists to
prevent. Ask the occupancy grid, or the engine.

**6. Time is ticks, everywhere.** Sprint times, PPS, goal evaluation and garbage arrival
are all tick counts. Nothing reads a wall clock except the code deciding how many whole
ticks are owed. Keep it that way: it is what makes a result reproducible by re-simulating
the recording, and it is what M3's stall guard will compare against real elapsed time.

## The client, in one paragraph

The client renders a game and reports which buttons were held. It decides nothing else. Not
shapes, not rotation, not timing, not scoring. Every value that would tempt a rule into
TypeScript is served by the wasm module for exactly that reason — `pieceShapes()`,
`buttonBits()`, `frameLayout()`, `normalizeSettings()`, `centiframes()`. The table in
[apps/web/README.md](apps/web/README.md) says which call to reach for. This matters more at
M2 than it did at M1: attack and garbage are the first rules a client author is tempted to
"just compute locally", and at M3 that is two peers disagreeing about a live match.

## How to add to the HUD

The frame block is a fixed 64 bytes with the offsets defined once, in
`crates/client-wasm/src/frame.rs`, and read by the client at startup via `frameLayout()`.
To surface something new:

1. Add the field to `Frame`, its offset to `offset`, and a line to `write`.
2. Add it to `frame_layout()` so the client can find it.
3. Add it to the `Frame` interface and `readFrame` in [sim/frame.ts](apps/web/src/sim/frame.ts).

There is room: the block is 64 bytes and about 59 are used. If you need more than a handful
of new fields, grow `FRAME_BYTES` rather than packing — it is one constant, and the test
that reserved bytes stay zero will tell you if something is stale.

## Adding a setting is still a Rust-only change

A field plus its descriptor in `crates/engine/src/config/`, then
`cargo run -p config --bin emit-schema > config-schema.json`. The control appears with its
bounds, step, unit and help text. This was demonstrated during M1 by adding a skin variant:
one entry in the descriptor table, no client change. CI fails if the schema drifts.

Every group the schema marks `affectsSimulation` is frozen while a game is running and
shown read-only when it belongs to the mode. If M2 adds player-tunable versus settings,
they need to be genuinely per-player or genuinely per-mode; the settings screen already
distinguishes the two and will lock the wrong one if you put it in the wrong place.

## Do not break determinism

Four checks, all in CI, in increasing order of what they cover:

- `./scripts/engine-purity.sh` — no floats, `HashMap`, clocks or I/O in the engine.
- `./scripts/wasm-parity.sh` — the engine crate agrees with itself on native and wasm.
- `./scripts/client-parity.sh` — the **built client module** reproduces every golden
  replay's pinned checksum. This is the artifact players actually load.
- `./scripts/capture-roundtrip.sh` — a game captured in the client is one `replay-cli`
  accepts, and replaying a golden through the client reproduces it.

Two traps found the hard way in M1, both still live:

- **`JSON.parse` rounds a `u64`.** Seeds and checksums are 64-bit; anything past 53 bits
  goes through a double and comes back wrong. Client seeds are drawn from 48 bits for this
  reason, and checksums are read from the text as `BigInt`. The first run of the parity
  harness reported all five goldens as mismatched, and the bug was in the harness.
- **`Engine::new` leaves the counter at 0; the first `tick()` is tick 1.** Recorded input
  index `i` is tick `i + 1`. `apply_at_tick` is absolute, so an off-by-one here lands
  garbage a frame early or late — which is precisely M2's territory.

## What M1 did not build that you may want

- **A garbage sender.** Nothing calls `schedule_garbage` outside tests. For a local versus
  feel test, a fake opponent that sends on a timer is a few lines and needs no server.
- **Goldens that clear lines.** See trap 4. This is the highest-value thing to do first.
- **Score goals.** `Goal::Score` parses and is listed; nothing evaluates it, because there
  is no scoring model beyond attack.
- **An `M1-SPEC.md` entry in git.** The document exists in the working tree and was written
  before the milestone; it is accurate but was deliberately left untracked.
- **The manual playtest pass.** [M1-SPEC.md](M1-SPEC.md) §13.4 lists twelve checks that no
  test can make — DAS/ARR at three settings, DCD, IRS/IHS, misdrop protection, input
  latency. The handling has been played and reported to feel good, but the list has not
  been walked item by item.
