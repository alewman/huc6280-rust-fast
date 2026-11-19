#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Rung 2's verdict: do the two cores judge every SingleStepTests case alike?

    compare_sst.py REFERENCE_DIR RUST_DIR

Each directory holds ``XX.txt``, one line per case (``index verdict masks``),
written by ``scripts/reference_sst.py`` and ``target/release/sst --verdicts``.
Prints both cores' totals side by side and exits 0 when every case has the
same verdict and the same masks on both and none failed. Stdlib only.
"""

from __future__ import annotations

import sys
from collections import Counter
from pathlib import Path


def totals(lines: list[str]) -> tuple[int, int, Counter]:
    passed = failed = 0
    masks: Counter = Counter()
    for line in lines:
        _, verdict, used = line.split()
        passed += verdict == "P"
        failed += verdict == "F"
        if used != "-":
            for item in used.split(","):
                name, count = item.split("=")
                masks[name] += int(count)
    return passed, failed, masks


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    reference, rust = Path(sys.argv[1]), Path(sys.argv[2])
    sums = [[0, 0, Counter()], [0, 0, Counter()]]
    different = 0
    files = 0
    for opcode in range(256):
        name = f"{opcode:02x}.txt"
        left = (reference / name).read_text().splitlines()
        right = (rust / name).read_text().splitlines()
        files += 1
        for side, lines in enumerate((left, right)):
            passed, failed, masks = totals(lines)
            sums[side][0] += passed
            sums[side][1] += failed
            sums[side][2].update(masks)
        unequal = [(a, b) for a, b in zip(left, right) if a != b]
        different += len(unequal) + abs(len(left) - len(right))
        if unequal:
            print(f"{opcode:02X}: {len(unequal)} cases judged differently; first: reference {unequal[0][0]!r}, rust {unequal[0][1]!r}")
    for label, (passed, failed, masks) in zip(("huc6280-python", "huc6280-rust  "), sums):
        used = ", ".join(f"{name} {count}" for name, count in sorted(masks.items()))
        print(f"{label}: cases {passed + failed}, passed {passed}, failed {failed}; masks: {used}")
    ok = not different and sums[0][1] == 0 and sums[1][1] == 0
    print(f"{files} opcodes, {different} cases judged differently: {'PASS' if ok else 'FAIL'}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
