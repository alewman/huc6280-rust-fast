#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Rung 3, the reference's half: build Dormann's relocated test and run it on huc6280-python.

    reference_dormann.py OUT_DIR [--limit N]

Builds the image exactly as the reference's ``scripts/run_dormann.py`` does
(its ``tests/dormann.py``: ``scripts/dormann_to_ca65.py``, then ca65 and ld65
from cc65 2.19 under ~/.local/opt or on the PATH) into ``OUT_DIR``, leaves
``test.bin`` and ``labels.json`` (``start`` and ``success_address``) there for
``target/release/dormann``, runs the reference core and prints its result in
the line format the Rust binary prints. Run it with the reference's
interpreter, after ``scripts/fetch_dormann.sh``.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import huc6280

REFERENCE = Path(huc6280.__file__).resolve().parents[2]
sys.path.insert(0, str(REFERENCE / "tests"))

import dormann  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("out", type=Path)
    parser.add_argument("--limit", type=int, default=100_000_000)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    image, labels = dormann.build(args.out)
    wanted = {name: labels[name] for name in ("start", "success_address")}
    (args.out / "labels.json").write_text(json.dumps(wanted))
    result = dormann.run(image, labels, args.limit)
    print(
        f"{result.reason} at ${result.pc:04X} after {result.instructions} instructions "
        f"(test case {result.test_case})"
    )
    return 0 if result.reason == "success" else 1


if __name__ == "__main__":
    sys.exit(main())
