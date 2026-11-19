#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Rung 6: seeded faults. One single-line fault at a time is planted in a copy
# of the core, and some rung must catch each (scripts/seeded_faults.py).
# Usage: scripts/rung6.sh CORPUS_DIR [--all]   (after scripts/rung3.sh, for Dormann's image)
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
CORPUS=${1:-"$ROOT/external/huc6280-sst/v1"}
shift || true
exec python3 "$ROOT/scripts/seeded_faults.py" --vectors "$CORPUS" "$@"
