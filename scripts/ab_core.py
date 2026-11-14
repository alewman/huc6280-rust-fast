#!/usr/bin/env python3
"""A/B two builds on this shared machine by CPU time, alternating them on one pinned CPU and
comparing each side's best run (other work only ever slows a run down), for the two cases the
board's scripts/ab.py does not cover:

    ab_core.py dormann BIN_A BIN_B [--rounds N] [--cpu N]
        Dormann's relocated 65C02 test (external/dormann, after scripts/rung3.sh): the core alone,
        20,936,652 instructions on flat RAM. The time is the child's user + system CPU time.
    ab_core.py cd BIN_A BIN_B [--frames N] [--rounds N] [--cpu N] [--disc NAME]
        pcenginecd-rust on a disc, from power-on (the System Card 3.0, the Arcade Card, RUN at
        frame 449): scripts/board.sh rev's pcecd-cpubench, timed by its own thread's CPU time.
        The .chd is extracted into scratch/disc/ for the run and removed after.

Either side's output apart from the time (the success line; the audio sample count and the
master clock) must agree, or the comparison is refused. An A/A run (one binary twice) shows the
noise.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
# The defaults are the author's layout: set SYSCARD (the System Card 3.0 image) and LIBCD (the
# directory of .chd discs) to your own.
SYSCARD = os.environ.get("SYSCARD", "/data/emu/share/batocera/bios/syscard3.pce")
LIBCD = os.environ.get("LIBCD", "/data/emu/roms-retrobat/pcenginecd")
DISC = "Ryuuko no Ken (Japan) (En,Ja,Es) (FACT)"


def run_dormann(binary: str, cpu: int) -> tuple[float, str]:
    out = ROOT / "external" / "dormann"
    labels = json.load(open(out / "labels.json"))
    cmd = ["taskset", "-c", str(cpu), binary, str(out / "test.bin"),
           "%04X" % labels["start"], "%04X" % labels["success_address"]]  # fmt: skip
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, preexec_fn=lambda: os.nice(10))
    text = proc.stdout.read().decode()
    _, status, usage = os.wait4(proc.pid, 0)
    return usage.ru_utime + usage.ru_stime, text.strip()


CD_LINE = re.compile(r"frames in ([\d.]+) s: [\d.]+ frames per second \((\d+) audio samples, mclk (\d+)\)")


def run_cd(binary: str, cpu: int, cue: Path, frames: int) -> tuple[float, str]:
    cmd = ["nice", "taskset", "-c", str(cpu), binary, SYSCARD, "--disc", str(cue), "--arcade",
           "--press", "run@449+20", "--frames", str(frames)]  # fmt: skip
    text = subprocess.run(cmd, capture_output=True, text=True, check=True).stdout
    seconds, samples, mclk = CD_LINE.search(text).groups()
    return float(seconds), f"{samples} samples, mclk {mclk}"


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("case", choices=["dormann", "cd"])
    p.add_argument("a")
    p.add_argument("b")
    p.add_argument("--rounds", type=int, default=12)
    p.add_argument("--cpu", type=int, default=31)
    p.add_argument("--frames", type=int, default=1500)
    p.add_argument("--disc", default=DISC)
    a = p.parse_args()
    work = None
    if a.case == "cd":
        work = ROOT / "scratch" / "disc" / re.sub(r"[^A-Za-z0-9]+", "_", a.disc)
        work.mkdir(parents=True, exist_ok=True)
        cue = work / "disc.cue"
        if not cue.is_file():
            subprocess.run(["chdman", "extractcd", "-i", f"{LIBCD}/{a.disc}.chd", "-o", str(cue),
                            "-ob", str(work / "disc.bin")], check=True, capture_output=True)  # fmt: skip
        once = lambda binary: run_cd(binary, a.cpu, cue, a.frames)  # noqa: E731
    else:
        once = lambda binary: run_dormann(binary, a.cpu)  # noqa: E731
    try:
        runs: dict[str, list[tuple[float, str]]] = {"a": [], "b": []}
        for r in range(a.rounds):
            order = ["a", "b"] if r % 2 == 0 else ["b", "a"]
            for side in order:
                runs[side].append(once(getattr(a, side)))
        checks = {c for side in runs.values() for _, c in side}
        if len(checks) != 1:
            print(f"the outputs differ {sorted(checks)}: the builds are not equivalent")
            return 1
        best_a = min(t for t, _ in runs["a"])
        best_b = min(t for t, _ in runs["b"])
        unit = f"{a.frames} frames" if a.case == "cd" else "Dormann"
        print(f"{a.case} ({unit}; {checks.pop()}): A {best_a:.3f} s, B {best_b:.3f} s CPU, "
              f"x{best_a / best_b:.3f} (best of {a.rounds} each)")  # fmt: skip
    finally:
        if work is not None and work.is_dir() and all(f.suffix in (".bin", ".cue") for f in work.iterdir()):
            for f in work.iterdir():
                f.unlink()
            work.rmdir()
    return 0


if __name__ == "__main__":
    sys.exit(main())
