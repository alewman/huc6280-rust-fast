#!/usr/bin/env bash
# The fork's core gate: every change must leave this core identical to huc6280-rust (and so
# to huc6280-python v0.3.0) on every inherited rung. The reference's side of each rung does not
# change, so it is computed once and kept under external/ (rung 1 and 4 traces in
# external/refcache, rung 2 verdicts in external/sst/reference-*, rung 3's line in
# external/dormann/reference.txt, rung 5's reports in external/rung5/*.reference.txt); delete
# a cache to recompute it. `--faults` adds the seeded faults (rung 6).
#
#   scripts/ladder.sh [--faults]          JOBS=12 by default; everything niced
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
PYTHON="$ROOT/external/huc6280-python/.venv/bin/python"
CORPUS="$ROOT/external/huc6280-sst/v1"
TRACES="$ROOT/../huc6280-python/tests/mame_traces"
JOBS=${JOBS:-12}
export PYPY_GC_NURSERY=1M
status=0
nice cargo build --release -q --manifest-path "$ROOT/Cargo.toml"

if ! TESTS=$(nice cargo test --release -q --manifest-path "$ROOT/Cargo.toml" 2>&1); then
    printf '%s\n' "$TESTS" | tail -30
    echo "cargo test: FAILED"
    exit 1
fi
echo "cargo test: $(printf '%s\n' "$TESTS" | grep -o '[0-9]* passed' | awk '{ n += $1 } END { print n }') tests pass"

# rung 1: the arithmetic tables and the directed lockstep
python3 "$ROOT/scripts/make_scenarios.py" > /dev/null
if [ ! -f "$ROOT/external/alu.reference.txt" ]; then
    nice "$PYTHON" "$ROOT/scripts/reference_alu.py" > "$ROOT/external/alu.reference.txt"
fi
if cmp -s "$ROOT/external/alu.reference.txt" <(nice "$ROOT/target/release/alu"); then
    echo "rung 1: arithmetic tables identical"
else
    echo "rung 1: arithmetic tables DIFFER"; status=1
fi
nice python3 "$ROOT/scripts/lockstep.py" "$ROOT"/external/scenarios/directed-*.json \
    "$ROOT"/external/scenarios/sequences-*.json --jobs "$JOBS" \
    --reference-cache "$ROOT/external/refcache" | tail -1 | sed 's/^/rung 1: /' || status=1

# rung 2: all 640,000 SingleStepTests cases, both formulas, verdicts compared case for case
for formula in default mesen; do
    flags=()
    [ "$formula" = mesen ] && flags=(--formula mesen)
    if [ ! -d "$ROOT/external/sst/reference-$formula" ]; then
        nice "$PYTHON" "$ROOT/scripts/reference_sst.py" "$CORPUS" "$ROOT/external/sst/reference-$formula" \
            --jobs "$JOBS" "${flags[@]}" > /dev/null
    fi
    nice "$ROOT/target/release/sst" "$CORPUS" --verdicts "$ROOT/external/sst/rust-$formula" \
        --jobs "$JOBS" "${flags[@]}" > "$ROOT/external/sst/rust-$formula.log" || true
    if out=$(python3 "$ROOT/scripts/compare_sst.py" "$ROOT/external/sst/reference-$formula" \
        "$ROOT/external/sst/rust-$formula" 2>&1); then
        echo "rung 2 ($formula): $(printf '%s\n' "$out" | tail -1)"
    else
        printf '%s\n' "$out" | tail -8; echo "rung 2 ($formula): FAIL"; status=1
    fi
done

# rung 3: Dormann
OUT="$ROOT/external/dormann"
if [ ! -f "$OUT/reference.txt" ]; then
    nice "$PYTHON" "$ROOT/scripts/reference_dormann.py" "$OUT" > "$OUT/reference.txt"
fi
START=$(python3 -c "import json,sys; print('%04X' % json.load(open(sys.argv[1]))['start'])" "$OUT/labels.json")
SUCCESS=$(python3 -c "import json,sys; print('%04X' % json.load(open(sys.argv[1]))['success_address'])" "$OUT/labels.json")
RUST=$(nice "$ROOT/target/release/dormann" "$OUT/test.bin" "$START" "$SUCCESS" 2>/dev/null) || true
if [ "$RUST" = "$(cat "$OUT/reference.txt")" ]; then
    echo "rung 3: $RUST"
else
    echo "rung 3: FAIL ($RUST)"; status=1
fi

# rung 4: the interrupt lockstep
nice python3 "$ROOT/scripts/lockstep.py" "$ROOT"/external/scenarios/irq-*.json \
    "$ROOT"/external/scenarios/sweep-*.json --jobs "$JOBS" \
    --reference-cache "$ROOT/external/refcache" | tail -1 | sed 's/^/rung 4: /' || status=1

# rung 5: the MAME replays and the synthetic long run
R5="$ROOT/external/rung5"
mkdir -p "$R5"
[ -f "$R5/long.reference.txt" ] || nice "$PYTHON" "$ROOT/scripts/reference_long.py" 6280 3000000 > "$R5/long.reference.txt"
[ -f "$R5/alternate.reference.txt" ] || nice "$PYTHON" "$ROOT/scripts/reference_long.py" 1987 1000000 --alternate > "$R5/alternate.reference.txt"
pids=()
games=0
if [ -d "$TRACES" ]; then
    for game in "$TRACES"/*/; do
        [ -f "$game/error.log" ] && [ -f "$game/cart.pce" ] || continue
        name=$(basename "$game")
        games=$((games + 1))
        [ -f "$R5/$name.reference.txt" ] || nice "$PYTHON" "$ROOT/scripts/reference_replay.py" "$game" > "$R5/$name.reference.txt"
        nice "$ROOT/target/release/replay" "$game" > "$R5/$name.rust.txt" 2>/dev/null &
        pids+=($!)
    done
fi
nice "$ROOT/target/release/long" 6280 3000000 > "$R5/long.rust.txt" 2>/dev/null &
pids+=($!)
nice "$ROOT/target/release/long" 1987 1000000 --alternate > "$R5/alternate.rust.txt" 2>/dev/null &
pids+=($!)
for pid in "${pids[@]}"; do wait "$pid" || true; done
same=0
for ref in "$R5"/*.reference.txt; do
    name=$(basename "$ref" .reference.txt)
    if cmp -s "$ref" "$R5/$name.rust.txt"; then
        same=$((same + 1))
    else
        echo "rung 5: $name DIFFERS"; status=1
    fi
done
echo "rung 5: $same of $((games + 2)) identical ($games MAME replays, 2 synthetic runs)"

if [ "${1:-}" = "--faults" ]; then
    nice python3 "$ROOT/scripts/seeded_faults.py" --vectors "$CORPUS" | tail -3
fi
[ $status -eq 0 ] && echo "core gate: PASS" || echo "core gate: FAIL"
exit $status
