#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Rung 2, the reference's half: every SingleStepTests huc6280 case on huc6280-python.

    reference_sst.py CORPUS_DIR OUT_DIR [--limit N] [--formula mesen] [--jobs N]

The readings and the masks are the reference's own (its ``tests/single_step.py``,
imported from the checkout this interpreter has installed); this script only
writes one line per case, ``index verdict masks``, into ``OUT_DIR/XX.txt`` (one
file per opcode), which is what ``target/release/sst --verdicts`` writes for the
Rust core, so the two can be compared case for case (``scripts/compare_sst.py``).
Run it with the reference's interpreter.
"""

from __future__ import annotations

import argparse
import multiprocessing
import sys
from collections import Counter
from pathlib import Path

import huc6280

REFERENCE = Path(huc6280.__file__).resolve().parents[2]
sys.path.insert(0, str(REFERENCE / "tests"))
sys.path.insert(0, str(REFERENCE / "scripts"))

import single_step  # noqa: E402
import sst_reader  # noqa: E402

ARGS: argparse.Namespace


def verdict_line(index: int, problems: list[str], masks: Counter) -> str:
    used = ",".join(f"{name}={count}" for name, count in sorted(masks.items()))
    return f"{index} {'F' if problems else 'P'} {used or '-'}"


def run_opcode(opcode: int) -> tuple[int, int, int, Counter]:
    passed = failed = 0
    masks: Counter = Counter()
    lines = []
    path = Path(ARGS.corpus) / f"{opcode:02x}.json.bin"
    for index, case in enumerate(sst_reader.tests(path)):
        if ARGS.limit is not None and index >= ARGS.limit:
            break
        problems, used = single_step.run_case(case)
        masks.update(used)
        if problems:
            failed += 1
        else:
            passed += 1
        lines.append(verdict_line(index, problems, used))
    (Path(ARGS.out) / f"{opcode:02x}.txt").write_text("\n".join(lines) + "\n")
    return opcode, passed, failed, masks


def init(args: argparse.Namespace) -> None:
    global ARGS
    ARGS = args
    single_step.FORMULA = args.formula


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("corpus", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("--limit", type=int)
    parser.add_argument("--formula", choices=("mednafen", "mesen"))
    parser.add_argument("--jobs", type=int, default=8)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    passed = failed = 0
    masks: Counter = Counter()
    with multiprocessing.Pool(args.jobs, initializer=init, initargs=(args,)) as pool:
        for _, ok, bad, used in pool.imap_unordered(run_opcode, range(256)):
            passed += ok
            failed += bad
            masks.update(used)
    used = ", ".join(f"{name} {count}" for name, count in sorted(masks.items()))
    print(f"cases {passed + failed}, passed {passed}, failed {failed}; masks: {used}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
