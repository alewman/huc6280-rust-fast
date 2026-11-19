#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Rung 1, the arithmetic tables on the reference: what ``target/release/alu`` prints.

ADC's and SBC's result and flags for every pair of operands, binary and
decimal, under both decimal formulas, one FNV-1a digest a line. Run it with
the reference's interpreter.
"""

from huc6280._alu import add, subtract

P_VALUES = (0x00, 0x01, 0x08, 0x09, 0xC2, 0xC3, 0xEA, 0xEB)
MASK = 0xFFFFFFFFFFFFFFFF


def main() -> None:
    for name, operation in (("add", add), ("subtract", subtract)):
        for formula in ("mednafen", "mesen"):
            for p in P_VALUES:
                digest = 0xCBF29CE484222325
                for a in range(256):
                    for operand in range(256):
                        result, flags = operation(a, operand, p, formula)
                        digest = ((digest ^ result) * 0x100000001B3) & MASK
                        digest = ((digest ^ flags) * 0x100000001B3) & MASK
                print(f"{name} {formula} p={p:02X} {digest:016x}")


if __name__ == "__main__":
    main()
