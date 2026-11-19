#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Rung 5: a long program on both cores.
#
# Where MAME traces of real games are present (the reference's
# tests/mame_traces/, large and not in git: a directory per game with cart.pce
# and error.log), each is replayed as the reference's tests/mame_replay.py
# replays it, and the Rust core must print the reference core's own report and
# the same digest of the core after every step.
#
# Always: a synthetic program of STEPS boundaries (default 3,000,000) on a
# seeded bus, and 1,000,000 more with the alternate switches, every cycle and
# every state folded into a digest that must agree every 250,000 steps.
# Usage: scripts/rung5.sh [TRACES_DIR] [REFERENCE_PYTHON]
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
TRACES=${1:-"$ROOT/../huc6280-python/tests/mame_traces"}
PYTHON=${2:-"$ROOT/external/huc6280-python/.venv/bin/python"}
STEPS=${STEPS:-3000000}
OUT="$ROOT/external/rung5"
export PYPY_GC_NURSERY=1M
mkdir -p "$OUT"
cargo build --release -q --manifest-path "$ROOT/Cargo.toml"
status=0

nice "$PYTHON" "$ROOT/scripts/reference_long.py" 6280 "$STEPS" > "$OUT/long.reference.txt" &
nice "$PYTHON" "$ROOT/scripts/reference_long.py" 1987 $((STEPS / 3)) --alternate > "$OUT/alternate.reference.txt" &

games=0
if [ -d "$TRACES" ]; then
    for game in "$TRACES"/*/; do
        [ -f "$game/error.log" ] && [ -f "$game/cart.pce" ] || continue
        name=$(basename "$game")
        games=$((games + 1))
        nice "$PYTHON" "$ROOT/scripts/reference_replay.py" "$game" > "$OUT/$name.reference.txt" &
        nice "$ROOT/target/release/replay" "$game" > "$OUT/$name.rust.txt" 2>/dev/null
        if [ $((games % 3)) -eq 0 ]; then wait; fi
    done
fi
wait
if [ -d "$TRACES" ]; then
    for game in "$TRACES"/*/; do
        [ -f "$game/error.log" ] && [ -f "$game/cart.pce" ] || continue
        name=$(basename "$game")
        if cmp -s "$OUT/$name.reference.txt" "$OUT/$name.rust.txt"; then
            echo "$name: the two reports are identical: $(head -1 "$OUT/$name.rust.txt" | cut -d';' -f1); $(tail -1 "$OUT/$name.rust.txt")"
        else
            echo "$name: the reports DIFFER"
            diff "$OUT/$name.reference.txt" "$OUT/$name.rust.txt" | head -10
            status=1
        fi
    done
fi
echo "$games MAME traces replayed (from $TRACES)"

nice "$ROOT/target/release/long" 6280 "$STEPS" > "$OUT/long.rust.txt" 2>/dev/null
nice "$ROOT/target/release/long" 1987 $((STEPS / 3)) --alternate > "$OUT/alternate.rust.txt" 2>/dev/null
for run in long alternate; do
    if cmp -s "$OUT/$run.reference.txt" "$OUT/$run.rust.txt"; then
        echo "synthetic ($run): identical: $(tail -1 "$OUT/$run.rust.txt")"
    else
        echo "synthetic ($run): the digests DIFFER"
        diff "$OUT/$run.reference.txt" "$OUT/$run.rust.txt" | head -6
        status=1
    fi
done
[ $status -eq 0 ] && echo "rung 5: PASS" || echo "rung 5: FAIL"
exit $status
