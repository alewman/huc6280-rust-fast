#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Rung 3: Klaus Dormann's 65C02 extended opcodes test, relocated for the
# HuC6280 as the reference's scripts/run_dormann.py builds it, natively to its
# success trap, with the instruction count the reference core takes.
# Usage: scripts/rung3.sh [REFERENCE_PYTHON]   (after scripts/fetch_dormann.sh)
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
PYTHON=${1:-"$ROOT/external/huc6280-python/.venv/bin/python"}
OUT="$ROOT/external/dormann"
cargo build --release -q --manifest-path "$ROOT/Cargo.toml"
REFERENCE=$(nice "$PYTHON" "$ROOT/scripts/reference_dormann.py" "$OUT")
START=$(python3 -c "import json,sys; print('%04X' % json.load(open(sys.argv[1]))['start'])" "$OUT/labels.json")
SUCCESS=$(python3 -c "import json,sys; print('%04X' % json.load(open(sys.argv[1]))['success_address'])" "$OUT/labels.json")
RUST=$(nice "$ROOT/target/release/dormann" "$OUT/test.bin" "$START" "$SUCCESS" 2>/dev/null) || true
echo "huc6280-python: $REFERENCE"
echo "huc6280-rust:   $RUST"
case "$RUST" in
    success*) ;;
    *) echo "rung 3: FAIL (the Rust core did not reach the success trap)"; exit 1 ;;
esac
if [ "$REFERENCE" != "$RUST" ]; then
    echo "rung 3: FAIL (the two runs differ)"
    exit 1
fi
echo "rung 3: PASS"
