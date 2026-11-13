#!/usr/bin/env python3
"""A/B two wasm builds of the board (scripts/board.sh rev's pce_wasm_bench.wasm) under Node, by
the process's CPU time, alternating them title by title on one pinned CPU, each side's best of
ROUNDS runs (the board's wasm-bench/bench.mjs, which takes the best of its own --rounds).

    ab_wasm.py WASM_A WASM_B [--rounds N] [--frames N] [--cpu N]

The five titles are the board's scripts/ab.py's, extracted by it into scratch/board/scratch/bench.
The output and state checksums must agree between the two sides.
"""

from __future__ import annotations

import argparse
import re
import statistics
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BOARD = ROOT / "scratch" / "board"
BENCH = BOARD / "scratch" / "bench"
TITLES = [("Blazing_Lazers", "tg16"), ("Bonk_s_Adventure", "tg16"), ("R_Type", "tg16"),
          ("After_Burner_II", ""), ("Street_Fighter_II_", "")]  # fmt: skip
LINE = re.compile(r"([\d.]+) frames/s, output ([0-9A-F]+), check ([0-9A-F]+)")


def one(wasm: str, title: str, machine: str, frames: int, cpu: int) -> tuple[float, str]:
    cmd = ["nice", "taskset", "-c", str(cpu), "node", str(BOARD / "wasm-bench" / "bench.mjs"), wasm,
           str(BENCH / f"{title}.pce"), *([machine] if machine else []), "--frames", str(frames), "--rounds", "1"]  # fmt: skip
    out = subprocess.run(cmd, capture_output=True, text=True, check=True).stdout
    fps, output, check = LINE.search(out).groups()
    return float(fps), output + check


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("a")
    p.add_argument("b")
    p.add_argument("--rounds", type=int, default=6)
    p.add_argument("--frames", type=int, default=600)
    p.add_argument("--cpu", type=int, default=31)
    a = p.parse_args()
    ratios = []
    for title, machine in TITLES:
        runs: dict[str, list[tuple[float, str]]] = {"a": [], "b": []}
        for r in range(a.rounds):
            for side in (["a", "b"] if r % 2 == 0 else ["b", "a"]):
                runs[side].append(one(getattr(a, side), title, machine, a.frames, a.cpu))
        if len({c for side in runs.values() for _, c in side}) != 1:
            print(f"{title}: checksums differ: the builds are not equivalent")
            return 1
        fa, fb = max(f for f, _ in runs["a"]), max(f for f, _ in runs["b"])
        ratios.append(fb / fa)
        print(f"{title}: A {fa:.1f} fps, B {fb:.1f} fps, x{fb / fa:.3f}", flush=True)
    print(f"overall x{statistics.geometric_mean(ratios):.3f} (wasm, {a.rounds} rounds a title)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
