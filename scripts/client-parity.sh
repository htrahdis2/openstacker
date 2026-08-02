#!/usr/bin/env bash
# Fails if the built client disagrees with a recorded game's pinned checksum.
#
# `wasm-parity.sh` proves the engine crate behaves the same on both targets. This proves
# the same thing about the artifact a player actually loads: the wasm module with its
# bindings, its frame block, and its replay capture built in. A bug introduced between the
# engine and the browser lives in exactly that gap.
#
# Every golden replay is decoded, driven through the client one tick at a time, and its
# final checksum compared with the one recorded in the file.
#
# Needs: rustup target wasm32-unknown-unknown, wasm-pack, and node.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${TMPDIR:-/tmp}/client-parity-$$"
trap 'rm -rf "$OUT"' EXIT

if ! command -v wasm-pack >/dev/null; then
  echo "wasm-pack is not installed: cargo install wasm-pack" >&2
  exit 1
fi

wasm-pack build "$ROOT/crates/client-wasm" \
  --target nodejs --out-dir "$OUT" --out-name client --release >/dev/null 2>&1

node --input-type=module -e "
import { readFileSync, readdirSync } from 'node:fs';
const { Game } = await import('$OUT/client.js');

const dir = '$ROOT/testdata/replays';
let checked = 0, failures = 0;

// Seeds and checksums are 64-bit. JSON.parse would round them through a double, so they
// are taken from the text and kept as BigInt; everything else in a replay is small.
const u64 = (text, key) => BigInt(new RegExp(\`\"\${key}\"\\\\s*:\\\\s*(\\\\d+)\`).exec(text)[1]);

for (const name of readdirSync(dir).filter((f) => f.endsWith('.replay')).sort()) {
  const text = readFileSync(\`\${dir}/\${name}\`, 'utf8');
  const r = JSON.parse(text);
  const game = new Game(u64(text, 'seed'), JSON.stringify(r.config), JSON.stringify(r.handling));

  // Garbage goes in on the tick it was scheduled on, the way the client schedules it.
  // The pending queue is part of the checksum and is what a clear cancels against, so a
  // batch replayed a tick out is a different game.
  const garbage = r.garbage ?? [];
  let tick = 0;
  for (const [bits, run] of r.inputs) {
    for (let i = 0; i < run; i++) {
      tick++;
      for (const g of garbage) {
        if (g.at_tick === tick) {
          game.scheduleGarbage(g.garbage.apply_at_tick, g.garbage.amount, g.garbage.hole_col);
        }
      }
      game.tick(bits);
    }
  }

  const actual = game.checksum();
  const expected = u64(text, 'checksum');
  checked++;
  if (actual === expected) {
    console.log(\`  \${name.padEnd(24)} ok  \${actual}\`);
  } else {
    console.log(\`  \${name.padEnd(24)} MISMATCH  client=\${actual} recorded=\${expected}\`);
    failures++;
  }
}

if (checked === 0) {
  console.error('no golden replays found; nothing was actually checked');
  process.exit(1);
}
if (failures !== 0) {
  console.error(\`\n\${failures} of \${checked} replays differ in the client.\`);
  console.error('The browser would play a different game from the one that was recorded.');
  process.exit(1);
}
console.log(\`client parity: ok (\${checked} replays)\`);
"
