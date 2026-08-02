#!/usr/bin/env bash
# Fails if a game captured in the client is not a replay the tools accept.
#
# The client records what it played and claims a result. This drives the built client
# through a scripted game, writes the recording it produces, and hands it to `replay-cli`
# to re-simulate and check. A capture that cannot be verified is a recording nobody can
# trust, which is the whole point of keeping them.
#
# Needs: rustup target wasm32-unknown-unknown, wasm-pack, and node.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${TMPDIR:-/tmp}/capture-roundtrip-$$"
trap 'rm -rf "$OUT"' EXIT
mkdir -p "$OUT"

if ! command -v wasm-pack >/dev/null; then
  echo "wasm-pack is not installed: cargo install wasm-pack" >&2
  exit 1
fi

wasm-pack build "$ROOT/crates/client-wasm" \
  --target nodejs --out-dir "$OUT/pkg" --out-name client --release >/dev/null 2>&1

node --input-type=module -e "
import { readFileSync, writeFileSync } from 'node:fs';
const { Game } = await import('$OUT/pkg/client.js');

const modes = JSON.parse(readFileSync('$ROOT/modes.generated.json', 'utf8'));
const sprint = modes.modes.find((m) => m.id === 'sprint40');
if (!sprint) { console.error('sprint40 is missing from the generated modes'); process.exit(1); }

const handling = { das_ms: 100, arr_ms: 16, sdf_ms_per_row: 0, dcd_ms: 0,
  das_cut_delay_ms: 0, irs: 'hold', ihs: true, prevent_misdrop_ms: 0, soft_drop_lock: false };

const LEFT = 1, RIGHT = 2, CW = 4, HOLD = 32, HARD_DROP = 128;
const game = new Game(123456789n, JSON.stringify(sprint.config), JSON.stringify(handling));

// A game with something of everything in it: movement, rotation, hold and drops.
// Pieces are tapped across the field rather than dropped in one column, so rows fill
// and clear instead of the run ending in a tower.
const script = [];
for (let piece = 0; piece < 60; piece++) {
  if (piece % 7 === 3) script.push(HOLD, 0);
  if (piece % 3 === 0) script.push(CW, 0);
  const dir = piece % 2 === 0 ? LEFT : RIGHT;
  for (let tap = 0; tap < piece % 5; tap++) script.push(dir, 0);
  script.push(HARD_DROP, 0, 0);
}

for (const buttons of script) {
  if (game.isOver()) break;
  game.tick(buttons);
}

const replay = game.finishReplay();
writeFileSync('$OUT/captured.replay', replay);

// A sparring game as well: the opponent is the only thing that writes the second input
// channel during play, and a recording of it has to verify like any other.
const versus = modes.modes.find((m) => m.id === 'versus');
if (!versus) { console.error('versus is missing from the generated modes'); process.exit(1); }
const spar = new Game(987654321n, JSON.stringify(versus.config), JSON.stringify(handling),
  JSON.stringify(versus.sparring));
for (let i = 0; i < 60 * 60 && !spar.isOver(); i++) spar.tick(0);
const sparred = spar.finishReplay();
writeFileSync('$OUT/sparring.replay', sparred);
const sparredJson = JSON.parse(sparred);
if (!sparredJson.garbage || sparredJson.garbage.length === 0) {
  console.error('a minute of the sparring mode sent nothing');
  process.exit(1);
}
console.log('  sparred   ' + sparredJson.garbage.length + ' batches, ' +
  sparredJson.claimed.final_tick + ' ticks');

const u64 = (text, key) => BigInt(new RegExp('\"' + key + '\"\\\\s*:\\\\s*(\\\\d+)').exec(text)[1]);

const parsed = JSON.parse(replay);
if (u64(replay, 'checksum') !== game.checksum()) {
  console.error('the recording claims a different checksum from the game it recorded');
  process.exit(1);
}
console.log('  captured  ' + parsed.inputs.length + ' runs, ' + parsed.claimed.pieces + ' pieces, ' + parsed.claimed.lines + ' lines');

// Replaying a known recording through the client has to produce that recording back.
// The goldens carry clears, spins and holds that a scripted game does not reach.
import { readdirSync } from 'node:fs';
const dir = '$ROOT/testdata/replays';
for (const name of readdirSync(dir).filter((f) => f.endsWith('.replay')).sort()) {
  const text = readFileSync(dir + '/' + name, 'utf8');
  const original = JSON.parse(text);
  const g = new Game(u64(text, 'seed'), JSON.stringify(original.config), JSON.stringify(original.handling));
  // Rows the recording received go back in on the tick they arrived on, and have to come
  // out of the re-capture on the same tick again.
  const garbage = original.garbage ?? [];
  let tick = 0;
  for (const [bits, run] of original.inputs) {
    for (let i = 0; i < run; i++) {
      tick++;
      for (const gb of garbage) {
        if (gb.at_tick === tick) {
          g.scheduleGarbage(gb.garbage.apply_at_tick, gb.garbage.amount, gb.garbage.hole_col);
        }
      }
      g.tick(bits);
    }
  }
  const recaptured = g.finishReplay();
  if (u64(recaptured, 'checksum') !== u64(text, 'checksum')) {
    console.error('  ' + name + ': re-capturing it produced a different game');
    process.exit(1);
  }
  const back = JSON.parse(recaptured);
  if (back.claimed.lines !== original.claimed.lines || back.claimed.pieces !== original.claimed.pieces) {
    console.error('  ' + name + ': the re-captured recording claims a different result');
    process.exit(1);
  }
  if (JSON.stringify(back.garbage ?? []) !== JSON.stringify(garbage)) {
    console.error('  ' + name + ': the re-captured recording lost or moved the rows it received');
    process.exit(1);
  }
  console.log('  recaptured ' + name.padEnd(20) + original.claimed.lines + ' lines, ' + original.claimed.pieces + ' pieces');
}
"

# The tools have to accept it exactly as the client wrote it.
for file in captured sparring; do
  output="$(cd "$ROOT" && cargo run -q -p replay-cli --bin replay -- verify "$OUT/$file.replay")"
  echo "  $output"

  if ! grep -q "verified" <<<"$output"; then
    echo
    echo "The client produced a recording that replay-cli does not accept." >&2
    exit 1
  fi
done

echo "capture round trip: ok"
