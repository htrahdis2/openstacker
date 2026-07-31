#!/usr/bin/env bash
# Fails if the engine gains a construct that would break cross-platform determinism.
#
# The engine's whole value is that the same inputs produce the same outputs on every
# platform and every build. Each construct below quietly breaks that:
#
#   f32 / f64    rounding is not guaranteed identical across targets
#   HashMap      iteration order is not build-stable
#   SystemTime   the only clock may be the tick counter
#   Instant      same
#   std::fs      the engine does no I/O; that belongs to callers
#   async        introduces scheduling nondeterminism
#
# Line comments are stripped before matching, so documenting the ban does not trip it.
set -uo pipefail

SRC="${1:-crates/engine/src}"
BANNED='\bf32\b|\bf64\b|\bHashMap\b|\bHashSet\b|\bSystemTime\b|\bInstant\b|std::fs|\basync\b'

fail=0
while IFS= read -r file; do
  hits=$(sed 's://.*::' "$file" | grep -nE "$BANNED")
  if [ -n "$hits" ]; then
    echo "banned construct in $file:"
    echo "$hits" | sed 's/^/  /'
    fail=1
  fi
done < <(find "$SRC" -name '*.rs')

if [ "$fail" -ne 0 ]; then
  echo
  echo "The engine must stay deterministic across native and wasm builds."
  exit 1
fi
echo "engine purity: ok ($SRC)"
