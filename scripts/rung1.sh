#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Rung 1: trace lockstep, short. The directed scenarios (every opcode many
# times, T and D both ways, the mapper, block transfers, every named switch;
# scripts/make_scenarios.py) on both cores, traces compared boundary by
# boundary with every access; and ADC's and SBC's whole result tables under
# both decimal formulas.
# Usage: scripts/rung1.sh [REFERENCE_PYTHON]   (QUICK=1 for the smaller set)
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
PYTHON=${1:-"$ROOT/external/huc6280-python/.venv/bin/python"}
cargo build --release -q --manifest-path "$ROOT/Cargo.toml"
python3 "$ROOT/scripts/make_scenarios.py" ${QUICK:+--quick}
if diff <("$PYTHON" "$ROOT/scripts/reference_alu.py") <("$ROOT/target/release/alu") > /dev/null; then
    echo "arithmetic tables: identical (ADC and SBC, 2 formulas, 8 P values, 65,536 operand pairs each)"
else
    echo "arithmetic tables: DIFFERENT"
    diff <("$PYTHON" "$ROOT/scripts/reference_alu.py") <("$ROOT/target/release/alu") | head -8
    exit 1
fi
exec python3 "$ROOT/scripts/lockstep.py" "$ROOT"/external/scenarios/directed-*.json \
    "$ROOT"/external/scenarios/sequences-*.json --python "$PYTHON" --jobs "${JOBS:-4}"
