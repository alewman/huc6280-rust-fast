#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Klaus Dormann's 65C02 extended opcodes test (GPL-3.0; a test program, fetched,
# never committed), where the reference's scripts/dormann_to_ca65.py looks for
# it: external/huc6280-python/reference/src/dormann/.
# Usage: scripts/fetch_dormann.sh [SOURCE_FILE]
#   SOURCE_FILE: a local copy of 65C02_extended_opcodes_test.a65c (default:
#   GitHub, at the revision the reference's scripts/pinned_repos.json pins).
set -euo pipefail
REV=7954e2dbb49c469ea286070bf46cdd71aeb29e4b
NAME=65C02_extended_opcodes_test.a65c
SHA256=72b1f57dc8f22f418ac2e23fc57c43821da84060679a7fcc3302071aa2f76736
ROOT=$(cd "$(dirname "$0")/.." && pwd)
DEST="$ROOT/external/huc6280-python/reference/src/dormann"
mkdir -p "$DEST"
if [ -n "${1:-}" ]; then
    cp "$1" "$DEST/$NAME"
elif [ ! -f "$DEST/$NAME" ]; then
    curl -sSfL -o "$DEST/$NAME" \
        "https://raw.githubusercontent.com/Klaus2m5/6502_65C02_functional_tests/$REV/$NAME"
fi
GOT=$(sha256sum "$DEST/$NAME" | cut -d' ' -f1)
if [ "$GOT" != "$SHA256" ]; then
    echo "$NAME: SHA-256 $GOT, expected $SHA256" >&2
    exit 1
fi
echo "$NAME in $DEST"
