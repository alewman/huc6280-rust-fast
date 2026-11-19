#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Rung 4: interrupts in lockstep with the reference: the three lines and
# their vectors, irq_disable, NMI, BRK, reset, the poll (CLI, SEI, PLP, a
# change in the last cycle) in each irq_poll_cycle mode, the vanished
# interrupt in each mode, around block transfers and CSL/CSH, under seeded
# random line changes and with one change swept across every cycle
# (scripts/make_scenarios.py).
# Usage: scripts/rung4.sh [REFERENCE_PYTHON]   (QUICK=1 for the smaller set)
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
PYTHON=${1:-"$ROOT/external/huc6280-python/.venv/bin/python"}
python3 "$ROOT/scripts/make_scenarios.py" ${QUICK:+--quick} > /dev/null
exec python3 "$ROOT/scripts/lockstep.py" "$ROOT"/external/scenarios/irq-*.json \
    "$ROOT"/external/scenarios/sweep-*.json --python "$PYTHON" --jobs "${JOBS:-4}"
