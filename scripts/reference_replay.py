#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Rung 5, the reference's half: replay a MAME trace of a real game through huc6280-python.

    reference_replay.py TRACE_DIR [--limit N]

``TRACE_DIR`` is one the reference's ``scripts/mame_trace.py GAME --io`` left
(``cart.pce`` and ``error.log``). The replay is the reference's own
(``tests/mame_replay.py``, imported from the checkout this interpreter has
installed); this script prints its report and, so that the two cores are
compared on more than the report's counts, a digest of the core after every
``step()`` the replay made (the cycles it returned, PC A X Y S P, the mapping
registers, the clock switch and the harness's master clock count), which is
what ``target/release/replay`` prints for the Rust core. Run it with the
reference's interpreter.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import huc6280

REFERENCE = Path(huc6280.__file__).resolve().parents[2]
sys.path.insert(0, str(REFERENCE / "tests"))

import mame_replay  # noqa: E402

MASK = 0xFFFFFFFFFFFFFFFF
PRIME = 0x100000001B3
DIGEST = [0xCBF29CE484222325, 0]  # the digest, the steps folded into it


class Digested(mame_replay.Replay):
    """The reference's replay harness, folding the core into a digest after every step."""

    def __init__(self, rom: bytes) -> None:
        super().__init__(rom)
        cpu = self.cpu
        inner = cpu.step

        def step() -> int:
            cycles = inner()
            digest = DIGEST[0]
            for value in (cycles, cpu.pc, cpu.a, cpu.x, cpu.y, cpu.s, cpu.p, *cpu.mpr,
                          int(cpu.high_speed), self.master_clocks & 0xFFFFFFFF):
                digest = ((digest ^ value) * PRIME) & MASK
            DIGEST[0] = digest
            DIGEST[1] += 1
            return cycles

        cpu.step = step


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("trace", type=Path)
    parser.add_argument("--limit", type=int)
    args = parser.parse_args()
    mame_replay.Replay = Digested
    report = mame_replay.replay(
        (args.trace / "cart.pce").read_bytes(), args.trace / "error.log", args.limit
    )
    print(report.summary())
    print(f"digest {DIGEST[0]:016x} over {DIGEST[1]:,} steps")
    return 0


if __name__ == "__main__":
    sys.exit(main())
