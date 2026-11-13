#!/usr/bin/env bash
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
#                              CPU time, built with the board's fat LTO as play-pce builds)
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

export_repo() { # REPO DEST
    mkdir -p "$2"
    git -C "$1" archive HEAD | tar -x -C "$2"
}

case "${1:-}" in
setup)
    [ -d "$S/board" ] || {
        export_repo "$BOARD_REPO" "$S/board"
        repoint "$S/board/Cargo.toml" 'huc6280 = { package = "huc6280-rust", path = "../huc6280-rust" }' \
            'huc6280 = { package = "huc6280-rust-fast", path = "../.." }'
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
    OUT="$S/bins/$REV"
    mkdir -p "$OUT"
    if [ "$REV" = work ]; then
        CORE="$ROOT"
    else
        CORE="$S/trees/$REV"
        [ -d "$CORE" ] || { mkdir -p "$CORE"; git -C "$ROOT" archive "$REV" | tar -x -C "$CORE"; }
    fi
    T="$S/revs/$REV"
    if [ ! -d "$T" ]; then
        mkdir -p "$T"
        export_repo "$BOARD_REPO" "$T/board"
        repoint "$T/board/Cargo.toml" 'huc6280 = { package = "huc6280-rust", path = "../huc6280-rust" }' \
            "huc6280 = { package = \"huc6280-rust-fast\", path = \"$CORE\" }"
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
    TD="$S/target-revs"
    nice cargo build --release -q --manifest-path "$T/board/Cargo.toml" --target-dir "$TD" --bin pce-bench
    cp "$TD/release/pce-bench" "$OUT/"
    nice cargo build --release -q --manifest-path "$T/board/Cargo.toml" --target-dir "$TD" --features profile --bin pce-profile
    cp "$TD/release/pce-profile" "$OUT/"
    nice cargo build --release -q --manifest-path "$T/pcenginecd/Cargo.toml" --target-dir "$TD" --bin pcecd-cpubench
    cp "$TD/release/pcecd-cpubench" "$OUT/"
    nice cargo build --release -q --manifest-path "$CORE/Cargo.toml" --target-dir "$TD-core" --bin dormann
    cp "$TD-core/release/dormann" "$OUT/"
    echo "$OUT: $(ls "$OUT" | tr '\n' ' ')"
    ;;
*)
    sed -n '2,/^set -euo/p' "$0" | sed '$d'
    exit 2
    ;;
esac
