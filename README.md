# openstacker

An open-source competitive falling-block stacker. Rust simulation, self-hosted, no ads,
no accounts required to play.

**Status: early.** Playable in a browser, against a training opponent. There is no server
yet, so there is nobody *real* to play against. See [SPEC.md](SPEC.md) for the overall
design, [M0-SPEC.md](M0-SPEC.md) for the simulation, [M1-SPEC.md](M1-SPEC.md) for the
client and [M2-SPEC.md](M2-SPEC.md) for the versus rules.

## Playing it

```bash
pnpm install
pnpm --dir apps/web run dev
```

Sprint 40, Blitz and Versus, with configurable handling, replays of every game, and local
bests. Needs [wasm-pack](https://rustwasm.github.io/wasm-pack/) and the
`wasm32-unknown-unknown` target.

Versus sends you rows on a timer and you last as long as you can. Attack, combo,
back-to-back and spins all count; a well-timed clear cancels what is coming instead of
trading blows.

## What is where

```
crates/       the simulation and its tools — Rust
apps/web/     the client — TypeScript                    <- the front end
modes/        game modes, as files
```

```bash
cargo test --workspace          # the simulation
pnpm --dir apps/web test        # the client
```

- **`crates/engine`** — the whole of the game's rules and none of its I/O. No filesystem,
  no clock, no async, no floating point. Board, piece geometry, SRS rotation with wall
  kicks, 7-bag randomization, gravity, lock delay, clears, spin detection, scoring,
  garbage, and a state checksum.
- **`crates/config`** — reads game modes from TOML, layers config from several sources,
  and emits the settings schema.
- **`crates/replay`** — the replay format, shared by the client and the tools.
- **`crates/client-wasm`** — the browser's view of the simulation: one call to advance it,
  one block of memory to read it.
- **`crates/replay-cli`** — runs, verifies and renders recorded games.
- **`apps/web`** — the client: canvas, input, menus, settings, replays. This is the front
  end, and the only TypeScript in the project. See [its README](apps/web/README.md) for
  the layout and what is meant to be tuned.

The client renders the game and reports which buttons are held. It decides nothing about
the game itself — no shapes, no rotation, no timing, no scoring. That boundary is what
lets the same simulation run on a server later without the two disagreeing.

## Determinism

Given the same seed, config, handling and inputs, the simulation produces the same
result on every platform and every build. Replays, server-side verification and desync
detection all fall out of that one property, so it is enforced rather than hoped for:

- `scripts/engine-purity.sh` fails the build on floats, `HashMap`, clocks or I/O in the
  engine, all of which quietly break reproducibility.
- `scripts/wasm-parity.sh` runs every golden replay through native and wasm builds and
  compares checksums. A client will run this in a browser while a server runs it
  natively, so a difference between them is two peers disagreeing about a live game.
- Golden replays are pinned to their checksums. If one moves, the rules moved, and every
  recorded game stops being verifiable.
- Tests run in both debug and release, because overflow and shift behaviour differ
  between the two and a rule that only holds in one profile is a desync waiting to
  happen.

## Trying it

```bash
cargo run -p replay-cli --bin replay -- run testdata/replays/quad.replay
```

```
tick 19  piece SR0  phase Falling
  +----------+
18|,,,,@@,,,,|  next OJTZL
19|,,,@@,,,,,|  hold -
20|..........|  lines 0
...
36|#.........|
37|#.........|
38|#...::....|
39|#..::.....|
  +----------+
```

`@` is the falling piece, `:` where it will land, `#` the stack, and `,` marks the buffer
above the playfield so a stack pushing out of view is visible.

Other commands:

```bash
cargo run -p replay-cli --bin replay -- help
```

## The numbers are meant to be moved

Everything the attack model does is data: what each clear sends, what a combo adds, what
a chain is worth, how long rows wait before landing. None of it is a constant in Rust.

Three ways to change it, in order of how permanent you want to be:

1. **The settings screen.** Match rules are the mode's until you press *tune these*, which
   copies them and makes them yours. Play, adjust, play again.
2. **Copy as TOML.** The tuner hands back a `[config]` block containing only what you
   moved. Paste it into a mode file and it is a mode anyone can play.
3. **A mode file.** `modes/versus.toml` is the default; `modes/versus_classic.toml` is the
   same game with a flat back-to-back bonus, kept as something to tune against.

None of this moves a checksum or the engine version: the rules a game was played under
travel inside its recording, so a tuned run stays verifiable and can be handed to anyone.

## Game modes are files

A mode is TOML, not code, so adding one needs no recompile:

```toml
spec_version = 1
id           = "sprint40"
name         = "Sprint 40"

[goal]
type  = "lines"
count = 40

[config]
lock_delay_ms = 500
preview_len   = 5
```

Everything not stated takes its default. Mistakes are reported against the setting you
actually wrote, since these are meant to be written by people who do not write Rust:

```
modes/blitz.toml: unknown setting `lock_delay_ticks` in [config], did you mean `lock_delay_ms`?
modes/blitz.toml: `garbage_cap` is 200, but must be between 1 and 40
modes/blitz.toml: needs mode format v4, but this build understands v1. Update the game,
                  or edit the file to match the older format
```

## Settings describe themselves

Every tunable setting carries its own bounds, default, unit, and help text. A test diffs
those descriptions against the real serde fields, so a new setting cannot ship without
them. `config-schema.json` is generated from the same tables and checked by CI, which is
what will let a settings screen be generated rather than hand-maintained in a second
language.

```bash
cargo run -p config --bin emit-schema
```

## Contributing

The determinism suite is what makes gameplay changes safe to accept from strangers: if
the golden checksums hold and native still matches wasm, a rules refactor did not change
the rules. If a golden does move, that is not automatically a failure — it means the
simulation changed, and the question is whether that was intended and whether the engine
version needs to go up with it.

Input for tests is written as scripts rather than by hand-editing button bitmasks:

```
seed: 4
mode: sprint40

CW           # stand the I on end
.
LEFT*12      # walk it to the wall
HARD_DROP
```

```bash
cargo run -p replay-cli --bin replay -- compile my.script -o my.replay
cargo run -p replay-cli --bin replay -- verify my.replay
```

## License

`crates/engine` is MIT OR Apache-2.0, so anything in the Rust ecosystem can depend on it.
Everything else is MIT.
