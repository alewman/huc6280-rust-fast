#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Rung 2: SingleStepTests huc6280, all 640,000 cases (LIMIT=N for the first N
# of each opcode), on both cores under the reference's masks, the verdicts
# compared case for case: under the core's default decimal formula (C13
# counted as a mask) and under Mesen's, the corpus's own (no mask).
# Usage: scripts/rung2.sh CORPUS_DIR [REFERENCE_PYTHON]
#   CORPUS_DIR holds XX.json.bin: reference/src/huc6280/v1 of a huc6280-python
#   tree that has run its scripts/fetch_references.py, or scripts/fetch_vectors.sh's.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
CORPUS=${1:-"$ROOT/external/huc6280-sst/v1"}
PYTHON=${2:-"$ROOT/external/huc6280-python/.venv/bin/python"}
OUT="$ROOT/external/sst"
cargo build --release -q --manifest-path "$ROOT/Cargo.toml"
status=0
for formula in default mesen; do
    flags=()
    [ "$formula" = mesen ] && flags=(--formula mesen)
    echo "decimal_formula: $formula"
    PYPY_GC_NURSERY=1M nice "$PYTHON" "$ROOT/scripts/reference_sst.py" "$CORPUS" "$OUT/reference-$formula" \
        ${LIMIT:+--limit "$LIMIT"} --jobs "${JOBS:-8}" "${flags[@]}" > /dev/null
    nice "$ROOT/target/release/sst" "$CORPUS" --verdicts "$OUT/rust-$formula" \
        ${LIMIT:+--limit "$LIMIT"} --jobs "${JOBS:-8}" "${flags[@]}" > "$OUT/rust-$formula.log" || { cat "$OUT/rust-$formula.log"; }
    python3 "$ROOT/scripts/compare_sst.py" "$OUT/reference-$formula" "$OUT/rust-$formula" || status=1
done
exit $status
