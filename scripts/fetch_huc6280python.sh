#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The reference, huc6280-python at the pinned commit (6ce348e, the clean rewrite after
# v0.4.0; local until pushed), into external/
# with a venv (its 6502-python dependency comes with it, by its own pin).
# Usage: scripts/fetch_huc6280python.sh [DEST] [SOURCE]
#   SOURCE: a git URL or a local clone (default: GitHub); the commit is fetched
#   from it, so a local clone that has the commit works before GitHub does.
#   PYTHON picks the interpreter for the venv (PyPy runs the long rungs several
#   times faster).
set -euo pipefail
COMMIT=6ce348e13c9ef3009bd07e7938c4b48956eeadc6
ROOT=$(cd "$(dirname "$0")/.." && pwd)
DEST=${1:-"$ROOT/external/huc6280-python"}
SOURCE=${2:-https://github.com/alewman/huc6280-python}
PYTHON=${PYTHON:-"$HOME/.local/opt/pypy3.11-v7.3.20-linux64/bin/pypy3.11"}
command -v "$PYTHON" > /dev/null || PYTHON=python3
if [ ! -d "$DEST/.git" ]; then
    git clone -q "$SOURCE" "$DEST"
fi
git -C "$DEST" fetch -q "$SOURCE" "$COMMIT" 2>/dev/null || true
git -C "$DEST" checkout -q "$COMMIT"
if [ ! -x "$DEST/.venv/bin/python" ]; then
    "$PYTHON" -m venv "$DEST/.venv"
fi
"$DEST/.venv/bin/python" -m pip install -q -e "$DEST"
echo "huc6280-python $(git -C "$DEST" rev-parse --short HEAD) in $DEST"
