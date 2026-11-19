#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SingleStepTests huc6280 (MIT, about 185 MB) at the revision the reference's
# scripts/pinned_repos.json pins, into external/huc6280-sst.
# Usage: scripts/fetch_vectors.sh [DEST]
set -euo pipefail
REV=4ce378e04e243437e6f74dfe76c3cb7c70e45bf1
ROOT=$(cd "$(dirname "$0")/.." && pwd)
DEST=${1:-"$ROOT/external/huc6280-sst"}
if [ ! -d "$DEST/.git" ]; then
    git init -q "$DEST"
    git -C "$DEST" remote add origin https://github.com/SingleStepTests/huc6280
fi
git -C "$DEST" fetch -q --depth 1 origin "$REV"
git -C "$DEST" checkout -q FETCH_HEAD
echo "SingleStepTests huc6280 $(git -C "$DEST" rev-parse --short HEAD) in $DEST: $(ls "$DEST/v1" | wc -l) files"
