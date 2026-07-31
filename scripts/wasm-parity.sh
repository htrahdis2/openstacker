#!/usr/bin/env bash
# Fails if the simulation produces different results on native and wasm.
#
# The engine's whole value is that the same inputs give the same outputs everywhere. A
# client runs it in a browser and a server runs it natively, so a difference between the
# two is not a portability nit: it is two peers disagreeing about a game in progress,
# surfacing as a desync long after the cause is gone.
#
# Every golden replay is run through both builds and their checksums compared. Catching a
# divergence here costs a CI run; catching it in a match costs the match.
#
# Needs: rustup target wasm32-unknown-unknown, and node.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="${TMPDIR:-/tmp}/wasm-parity-$$"
mkdir -p "$WORK/src"
trap 'rm -rf "$WORK"' EXIT

# A tiny crate that runs one replay and returns its checksum, built for both targets.
cat > "$WORK/Cargo.toml" <<EOF
[workspace]

[package]
name = "parity"
version = "0.0.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
engine = { path = "$ROOT/crates/engine" }
serde_json = "1"

[profile.release]
opt-level = 2
EOF

# The replay is baked in as JSON so the wasm build needs no imports at all: no
# filesystem, no clock, nothing that could differ between the two hosts.
cat > "$WORK/src/lib.rs" <<'EOF'
use engine::{Buttons, Engine, Handling, MatchConfig};

const REPLAY: &str = include_str!("replay.json");

pub fn checksum() -> u64 {
    let v: serde_json::Value = serde_json::from_str(REPLAY).expect("valid replay");
    let seed = v["seed"].as_u64().expect("seed");
    let config: MatchConfig = serde_json::from_value(v["config"].clone()).expect("config");
    let handling: Handling = serde_json::from_value(v["handling"].clone()).expect("handling");

    let mut engine = Engine::new(seed, &config, &handling);
    for pair in v["inputs"].as_array().expect("inputs") {
        let bits = pair[0].as_u64().expect("bits") as u8;
        let run = pair[1].as_u64().expect("run");
        for _ in 0..run {
            engine.tick(Buttons::from_bits_retain(bits));
        }
    }
    engine.checksum()
}

#[unsafe(no_mangle)]
pub extern "C" fn parity_checksum() -> u64 {
    checksum()
}
EOF

cat > "$WORK/src/main.rs" <<'EOF'
fn main() {
    println!("{}", parity::checksum());
}
EOF

cat > "$WORK/run.mjs" <<'EOF'
import { readFileSync } from 'node:fs';
const { instance } = await WebAssembly.instantiate(readFileSync(process.argv[2]), {});
// A wasm i64 return arrives as a signed BigInt; reinterpret it the way Rust wrote it.
console.log(BigInt.asUintN(64, instance.exports.parity_checksum()).toString());
EOF

failures=0
checked=0

for replay in "$ROOT"/testdata/replays/*.replay; do
  name="$(basename "$replay")"
  cp "$replay" "$WORK/src/replay.json"

  ( cd "$WORK" && cargo build --quiet --release >/dev/null )
  ( cd "$WORK" && cargo build --quiet --release --target wasm32-unknown-unknown >/dev/null )

  native="$("$WORK/target/release/parity")"
  wasm="$(node "$WORK/run.mjs" "$WORK/target/wasm32-unknown-unknown/release/parity.wasm")"

  checked=$((checked + 1))
  if [ "$native" = "$wasm" ]; then
    printf '  %-24s ok  %s\n' "$name" "$native"
  else
    printf '  %-24s MISMATCH  native=%s wasm=%s\n' "$name" "$native" "$wasm"
    failures=$((failures + 1))
  fi
done

if [ "$checked" -eq 0 ]; then
  echo "no golden replays found; nothing was actually checked"
  exit 1
fi

if [ "$failures" -ne 0 ]; then
  echo
  echo "$failures of $checked replays differ between native and wasm."
  echo "The simulation is not portable, which means two peers can disagree mid-match."
  exit 1
fi

echo "wasm parity: ok ($checked replays)"
