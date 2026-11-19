#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The board side of the fork's gate and measurements, on scratch copies: nothing is written in
# ../pcengine-rust-fast, ../supergrafx-rust or ../pcenginecd-rust (another session may be working
# there). Each copy is that repository's committed HEAD (git archive) with ONE line changed, the
# dependency that brings this crate:
#
#   scripts/board.sh setup     scratch/board (pcengine-rust-fast on this tree), scratch/supergrafx
#                              and scratch/pcenginecd (each add-on on scratch/board), with the
#                              reference caches and the probe card copied in
#   scripts/board.sh gate      the board's own gate on this tree (scripts/ladder.sh there: cargo
#                              test, the 275 lockstep cases from external/refcache, the save-state
#                              round trips)
#   scripts/board.sh rev REV   the benchmark binaries with this crate at REV (a commit, or `work`
#                              for the working tree) into scratch/bins/REV/: pce-bench,
#                              pce-profile, and pcecd-cpubench (pcecd-bench timed by the thread's
#                              CPU time, built with the board's fat LTO as play-pce builds),
#                              dormann (the core alone) and pce_wasm_bench.wasm
#   scripts/board.sh rev REV quiet   the same with scripts/board-quiet.patch applied to the
#                              board copy: the board taking the core's quiet window (opt-in)
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
S="$ROOT/scratch"
BOARD_REPO=${BOARD_REPO:-"$ROOT/../pcengine-rust-fast"}
SGX_REPO=${SGX_REPO:-"$ROOT/../supergrafx-rust"}
CD_REPO=${CD_REPO:-"$ROOT/../pcenginecd-rust"}

# replace exactly one dependency line in a manifest
repoint() {
    python3 - "$1" "$2" "$3" <<'PY'
import sys
path, old, new = sys.argv[1:]
text = open(path).read()
assert text.count(old) == 1, (path, old)
open(path, "w").write(text.replace(old, new))
PY
}

# replace the board's CPU core dependency (the line starting `huc6280 = `, whichever crate and
# source it names: pcengine-rust-fast now depends on this crate itself)
repoint_core() {
    python3 - "$1" "$2" <<'PY'
import re
import sys
path, new = sys.argv[1:]
text = open(path).read()
text, count = re.subn(r"(?m)^huc6280 = \{.*\}$", lambda _: new, text)
assert count == 1, (path, count)
open(path, "w").write(text)
PY
}

export_repo() { # REPO DEST
    mkdir -p "$2"
    git -C "$1" archive HEAD | tar -x -C "$2"
}

case "${1:-}" in
setup)
    [ -d "$S/board" ] || {
        export_repo "$BOARD_REPO" "$S/board"
        repoint_core "$S/board/Cargo.toml" 'huc6280 = { package = "huc6280-rust-fast", path = "../.." }'
        mkdir -p "$S/board/external" "$S/board/scratch"
        cp -a "$BOARD_REPO/external/refcache" "$S/board/external/"
        cp "$BOARD_REPO/scratch/probe.pce" "$S/board/scratch/"
    }
    for pair in "supergrafx:$SGX_REPO" "pcenginecd:$CD_REPO"; do
        name=${pair%%:*} repo=${pair#*:}
        [ -d "$S/$name" ] && continue
        export_repo "$repo" "$S/$name"
        repoint "$S/$name/Cargo.toml" 'pcengine_rust = { package = "pcengine-rust", path = "../pcengine-rust" }' \
            'pcengine_rust = { package = "pcengine-rust-fast", path = "../board" }'
        mkdir -p "$S/$name/external"
        cp -a "$repo/external/refcache" "$S/$name/external/"
    done
    echo "scratch copies ready: $S/board $S/supergrafx $S/pcenginecd"
    ;;
gate)
    JOBS=${JOBS:-12} nice "$S/board/scripts/ladder.sh"
    ;;
rev)
    REV=${2:?REV}
    VARIANT=${3:+-$3}
    OUT="$S/bins/$REV$VARIANT"
    mkdir -p "$OUT"
    if [ "$REV" = work ]; then
        CORE="$ROOT"
    else
        CORE="$S/trees/$REV"
        [ -d "$CORE" ] || { mkdir -p "$CORE"; git -C "$ROOT" archive "$REV" | tar -x -C "$CORE"; }
    fi
    T="$S/revs/$REV$VARIANT"
    if [ ! -d "$T" ]; then
        mkdir -p "$T"
        export_repo "$BOARD_REPO" "$T/board"
        repoint_core "$T/board/Cargo.toml" "huc6280 = { package = \"huc6280-rust-fast\", path = \"$CORE\" }"
        if [ "$VARIANT" = -quiet ]; then
            # what the board fork would add to take the core's quiet window
            patch -s -d "$T/board" -p1 < "$ROOT/scripts/board-quiet.patch"
        fi
        export_repo "$CD_REPO" "$T/pcenginecd"
        repoint "$T/pcenginecd/Cargo.toml" 'pcengine_rust = { package = "pcengine-rust", path = "../pcengine-rust" }' \
            'pcengine_rust = { package = "pcengine-rust-fast", path = "../board" }'
        # the measurement only: the board's release profile (fat LTO, one codegen unit, as
        # play-pce builds) and pcecd-bench timed by this thread's CPU time
        printf 'lto = "fat"\ncodegen-units = 1\n' >> "$T/pcenginecd/Cargo.toml"
        python3 - "$T/pcenginecd/src/bin/pcecd-bench.rs" "$T/pcenginecd/src/bin/pcecd-cpubench.rs" <<'PY'
import sys
source, out = sys.argv[1:]
text = open(source).read()
cpu = '''fn cpu_seconds() -> f64 {
    let text = std::fs::read_to_string("/proc/thread-self/schedstat").unwrap();
    text.split_whitespace().next().unwrap().parse::<u64>().unwrap() as f64 / 1e9
}

fn main() {'''
for old, new in [
    ("use std::time::Instant;\n", ""),
    ("fn main() {", cpu),
    ("let began = Instant::now();", "let began = cpu_seconds();"),
    ("let seconds = began.elapsed().as_secs_f64();", "let seconds = cpu_seconds() - began;"),
]:
    assert text.count(old) == 1, old
    text = text.replace(old, new)
open(out, "w").write(text)
PY
    fi
    # a target directory per revision and variant: the binaries share their names
    TD="$S/target-revs/$REV$VARIANT"
    nice cargo build --release -q --manifest-path "$T/board/Cargo.toml" --target-dir "$TD" --bin pce-bench
    cp "$TD/release/pce-bench" "$OUT/"
    nice cargo build --release -q --manifest-path "$T/board/Cargo.toml" --target-dir "$TD" --features profile --bin pce-profile
    cp "$TD/release/pce-profile" "$OUT/"
    nice cargo build --release -q --manifest-path "$T/pcenginecd/Cargo.toml" --target-dir "$TD" --bin pcecd-cpubench
    cp "$TD/release/pcecd-cpubench" "$OUT/"
    nice cargo build --release -q --manifest-path "$CORE/Cargo.toml" --target-dir "$TD-core" --bin dormann
    cp "$TD-core/release/dormann" "$OUT/"
    # the board under wasm, as the board's scripts/wasm-bench.sh builds it (fat LTO, panic=abort)
    TOOLCHAIN=${TOOLCHAIN:-"$HOME/.rustup/toolchains/1.93.1-x86_64-unknown-linux-gnu"}
    RUSTC="$TOOLCHAIN/bin/rustc" nice "$TOOLCHAIN/bin/cargo" build --release -q \
        --manifest-path "$T/board/wasm-bench/Cargo.toml" --target-dir "$TD-wasm" \
        --target wasm32-unknown-unknown --lib
    cp "$TD-wasm/wasm32-unknown-unknown/release/pce_wasm_bench.wasm" "$OUT/"
    echo "$OUT: $(ls "$OUT" | tr '\n' ' ')"
    ;;
*)
    sed -n '2,/^set -euo/p' "$0" | sed '$d'
    exit 2
    ;;
esac
