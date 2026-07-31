# M0 → M1 handoff

What the simulation gives you, and the things that will bite you. Read
[SPEC.md](SPEC.md) for the design; this covers only what M0 decided that the spec does
not say.

## What exists

- `crates/engine` — all the rules, no I/O. Builds for `wasm32-unknown-unknown`.
- `crates/config` — mode files, player settings, the settings schema.
- `crates/replay-cli` — run / verify / render recorded games, in the terminal.

There is no client. That is your job.

## The API you will use

```rust
let mut engine = Engine::new(seed, &match_config, &handling);
let result: TickResult = engine.tick(buttons);   // the only mutator
```

Read state with `board()`, `active()`, `ghost()`, `hold()`, `preview()`, `stats()`,
`phase()`, `is_over()`, `pending_garbage()`, `checksum()`.

## Six things that will bite you

**1. `active()` and `ghost()` return `Option`.** There is no falling piece during a spawn
delay, a clear delay, or after topping out. The spec says they return a piece; they do
not. Drawing the leftover value would paint a duplicate piece on top of the stack, which
is the bug that made this an option in the first place.

**2. Row 0 is the top of the buffer, not the top of the screen.** The board is 40 rows;
rows 20–39 are visible, 0–19 are spawn buffer. Gravity increases `y`. In a row's `u16`,
bit 0 is the leftmost column. Draw the buffer when anything occupies it, or a stack that
tops out looks like it ended for no reason.

**3. Colors live on the board, and game logic never reads them.**
`board().colors()` is `[u8; 400]`, indexed `y * 10 + x`. `0` is empty, `8` is garbage,
`1..=7` are piece kinds. They are excluded from `checksum()` on purpose: two peers with
different skins are not desynced. Never let a color decide anything.

**4. Handling is stored in milliseconds and the UI must treat ms as canonical.**
Internally it converts to subticks once, at construction. Frames are a *derived* read-out
via `subticks_to_centiframes` and quantise to about ±0.03 F. If you let a player type
`8.5 F` and hand it back, it will read `8.52 F` and look broken. Show a ms slider with a
frame read-out beside it.

**5. The first `tick()` runs tick 1, not tick 0.** `Engine::new` leaves the counter at 0
and spawns the first piece without ticking. This matters the moment garbage arrives:
`apply_at_tick` is absolute, so an off-by-one here lands rows on the wrong frame.

**6. One `tick()` per 1/60s of game time, always.** Use an accumulator and catch up in
whole ticks; never tie ticks to `requestAnimationFrame`. A dropped or doubled tick is a
different game, not a dropped frame.

## Settings

```rust
let (settings, notes) = Settings::from_json(&stored);   // never fails
```

Unknown keys are ignored, missing ones default, out-of-range values clamp, and a damaged
section costs only that section. `notes` is a `Vec<Note>` whose `Display` is already
written for a player to read — surface them, do not log them. That is the only signal
that something they chose could not be carried forward.

Only `handling` reaches the engine. Keybinds resolve to `Buttons` in your code before any
engine call (`Action::button()` gives the mapping), and cosmetics stop at the renderer.
Keeping that line sharp is what lets two players with opposite key layouts stay
bit-identical.

## Build the settings UI from the schema, not by hand

`config-schema.json` describes all 45 settings: bounds, defaults, units, help text, UI
group, and whether the group reaches the simulation. CI fails if it drifts from the
engine. Render controls from it and adding a setting stays a one-line Rust change with no
client edit — which was the entire point of the config work.

```bash
cargo run -p config --bin emit-schema
```

## What M0 did not build that you will need

- **Pointer accessors for the wasm boundary.** `occupancy_ptr()` / `colors_ptr()` from
  the spec do not exist. Today you get `&[u16; 40]` and `&[u8; 400]`. Add the shim in
  `client-wasm`, build typed-array views over wasm memory once at init, and pre-allocate
  so memory never grows — growth invalidates existing views.
- **`modes.generated.json`.** The server reads `modes/*.toml`; the browser should not
  parse TOML. A build step emitting JSON from the same files does not exist yet.
- **A shared home for `Replay`.** It currently lives inside `replay-cli`. Capturing
  replays in the browser means moving it somewhere both can use.

## Do not break determinism

The client's simulation must match the server's exactly, or M3 has nothing to build on.

- `./scripts/wasm-parity.sh` runs every golden replay through native and wasm and
  compares checksums. Run it after touching the engine.
- `./scripts/engine-purity.sh` fails on floats, `HashMap`, clocks or I/O in the engine.
- Golden replay checksums are pinned. **If one moves, the rules moved.**

That last point matters for you specifically: DAS/ARR feel is a human playtesting problem
and M1 is where it gets tuned. Any change to handling semantics will move those
checksums. That is the mechanism working, not breaking — but regenerate the goldens
deliberately and bump `ENGINE_VER`, rather than reflexively re-pinning numbers until the
tests pass.
