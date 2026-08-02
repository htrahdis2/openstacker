# M2 → M3 handoff

What the versus half gives you, and the things that will bite you. Read [SPEC.md](SPEC.md)
for the design and [M2-SPEC.md](M2-SPEC.md) for this milestone's; this covers only what M2
decided that neither says.

## What exists

- `crates/engine` — attack is now entirely data. **`ENGINE_VER` is 2.**
- `crates/config` — mode files, player settings, the schema, and a rules-to-TOML writer.
- `crates/replay` — records the garbage a game received as well as its buttons.
  **`REPLAY_VERSION` is 2.**
- `crates/client-wasm` — the browser's view, plus a seeded training opponent.
- `apps/web` — versus is playable against that opponent, with the rules tunable in place.

M3 is the server: rooms, two real players, garbage routing, desync detection, the stall
guard and spectating. The rules it needs are all here and all exercised.

## What M3 does not have to build

| Thing | Where it already is |
|---|---|
| Attack, combo, back-to-back, spins, perfect clears | `scoring.rs`, every number in `MatchConfig` |
| Garbage scheduling, cancellation, arrival at an absolute tick | `garbage.rs`, `engine.rs` |
| A recording that survives receiving rows | `crates/replay`, `ScheduledGarbage` |
| Rules handed down from above the player | `HostPolicy` and `resolve`, now actually used |
| Goldens covering clears, spins, chains and the garbage queue | `testdata/scripts/` |
| A HUD for incoming rows, attack, combo and chains | `apps/web`, fed by the frame block |

## Six things that will bite you

**1. The pending queue is part of the checksum, and cancellation reads it.** This is the
central problem of M3 and the reason `ScheduledGarbage` records `at_tick` as well as
`apply_at_tick`. Two peers that put the same batch into the queue on *different ticks*
have different checksums until it lands — and worse, a clear in between cancels against
different queues, so they send different amounts and the boards genuinely diverge. A
server that schedules into its copy when it decides, while the client schedules on
receipt, is exactly that bug. Decide the tick a batch becomes visible, carry it in the
message, and have both sides hold the batch until they reach it. There is a test in
`crates/replay` that pins the difference.

**2. A versus replay needs both sides.** A recording carries the rows *it received*, which
makes one player's game reproducible. A whole match is two of those, and verifying that
what one player sent is what the other received is a comparison M3 has to define. The
format has room; nothing decides it yet.

**3. Three versions moved, for three different reasons.** `ENGINE_VER` 2 because scoring
changed; `REPLAY_VERSION` 2 because recordings gained a second input channel;
`SETTINGS_VERSION` 2 because players can now pin their own rules. They are independent on
purpose — a rules change does not invalidate stored settings, and a format change does not
invalidate a checksum.

**4. The sparring opponent must not be in a real match.** It lives in `client-wasm`, is
seeded from the match seed, and is off unless a mode carries a `[sparring]` profile. In a
room, the rows come from the other player and the opponent has to stay silent, or a
player is fighting two people. `Game::with_opponent(.., None)` is the whole switch.

**5. The attack table travels, so the server must send it.** Every number the scoring path
reads is in `MatchConfig`, and a mode file is free to change any of them. `MatchStart`
carries the config for this reason, and a peer that assumes the defaults will disagree
about what a quad was worth on the first exchange. The same applies to the tuner: a player
with house rules is playing a different game, which is fine locally and must be refused or
overridden in a room.

**6. Everything M1 warned about is still true.** `JSON.parse` rounds a `u64`, so seeds and
checksums are read from the text as `BigInt` and client seeds are drawn from 48 bits.
`Engine::new` leaves the counter at 0 and the first `tick()` is tick 1, so recorded input
index `i` is tick `i + 1` — and `apply_at_tick` is absolute, which makes an off-by-one here
land rows a frame out on one peer only.

## Tuning is a data edit now, and should stay one

The attack model is described settings all the way down: bounds, units, help text, mode
file validation and a generated control, for the attack table, the combo table and the
back-to-back table alike. Moving any of it changes no checksum and no version, because the
values travel inside each recording.

That means the answer to "this feels wrong" is a mode file, not a patch. It also means the
bar is higher for adding a *rule*: if a proposal cannot be expressed as a number in
`MatchConfig`, it is a rules change, it moves `ENGINE_VER`, and the goldens are
regenerated in the same commit.

## How to add to the HUD

Unchanged from M1 except the numbers: the frame block is a fixed **128** bytes with the
offsets defined once in `crates/client-wasm/src/frame.rs` and read by the client at
startup via `frameLayout()`. Add the field to `Frame`, its offset to `offset`, a line to
`write`, an entry in `frame_layout()`, and the matching read in
[sim/frame.ts](apps/web/src/sim/frame.ts). About 88 of the 128 bytes are used; the test
that reserved bytes stay zero will tell you if something is stale.

Opponent boards at M3 are the first thing that will not fit this shape: they are somebody
else's occupancy, not this game's state, and they belong in their own buffer rather than
in a block that describes one player.

## Do not break determinism

Four checks, all in CI, and two of them now cover the second input channel:

- `./scripts/engine-purity.sh` — no floats, `HashMap`, clocks or I/O in the engine.
- `./scripts/wasm-parity.sh` — the engine crate agrees with itself on native and wasm.
- `./scripts/client-parity.sh` — the built client module reproduces every golden,
  **including the rows it received**.
- `./scripts/capture-roundtrip.sh` — a game captured in the client verifies, and a
  sparring game's garbage stream survives the round trip intact.

Sixteen goldens now, covering each size of clear, spins and minis, combo chains,
back-to-back chains that break, perfect clears, rows landing and rows being cancelled. If
one moves, the rules moved.

## What M2 did not build that you may want

- **The manual playtest pass.** [M2-SPEC.md](M2-SPEC.md) §12.5 lists nine checks that no
  test can make — whether a quad feels worth building, whether cancellation reads as
  defence, whether the bar is parseable mid-game — and [M1-SPEC.md](M1-SPEC.md) §13.4's
  twelve handling checks are still unwalked. Neither list has been through a human.
- **Numbers anyone has argued about.** The starting table is the M0 proposal plus
  escalating back-to-back. It has been played against a timer, not against a person.
- **Score goals.** `Goal::Score` still parses and is still evaluated by nobody, because
  there is still no scoring model beyond attack.
- **Garbage that is more than rows.** No multipliers, no surge, no cheese patterns, no
  blocking window. All of those are rules rather than numbers, and none of them is
  necessary to route garbage between two people.
