#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Write the lockstep scenarios (rungs 1 and 4) into external/scenarios/.

Every program here is generated or hand-assembled from the bytes below, so
nothing third-party is involved; the files are deterministic (seeded) and are
written, not committed. The format is conformance/README.md's.

    make_scenarios.py [--out DIR] [--quick]

Rung 1 (``directed-*``): every opcode, many times each, one boundary a case:
before each case the host sets every register (random A X Y S P with T and D
both ways, random mapping registers, a random PC), the case's bytes sit where
that PC maps, and the rest of the 2 MB is seeded noise. Cases are biased to
the corners: a zero-page pointer at $FF, TMA/TAM masks of none, one and many
bits, block transfers of a few bytes (and one of 65,536) whose source runs
across the edges of the hardware page's zero-read range, ST0-ST2. One file per
set of switches, so both decimal formulas and every other named switch run.
``sequences-*`` are short programs where an instruction depends on the one
before (SET then each T instruction, PLP setting T, CLI/SEI/PLP, CSL/CSH).

Rung 4 (``irq-*``): the interrupt workout (the three maskable lines with their
vectors, ``irq_disable``, NMI, BRK, reset, CLI/SEI/PLP, a handler that
acknowledges the timer in its last cycle, block transfers and CSL/CSH in the
main loop) under seeded random line changes between steps and inside bus
cycles, in every ``irq_poll_cycle`` and ``vanished_interrupt`` mode;
``sweep-*`` raise or drop one line at each cycle of a fixed program in turn.
The modes are the reference's since its clean rewrite (6ce348e): ``"latched"``
and ``"mednafen"`` are withdrawn there (``ValueError``). The random scenarios
keep the seeds they had when those modes were run, so a kept one is the same file.
"""

from __future__ import annotations

import argparse
import json
import random
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "external" / "scenarios"

BLOCK = (0x73, 0xC3, 0xD3, 0xE3, 0xF3)
POINTER = {0x01, 0x11, 0x12}  # low nibbles' pattern: (zp,X) (zp),Y (zp)
T_OPCODES = (0x09, 0x05, 0x15, 0x0D, 0x1D, 0x19, 0x01, 0x11, 0x12,  # ORA
             0x29, 0x25, 0x35, 0x2D, 0x3D, 0x39, 0x21, 0x31, 0x32,  # AND
             0x49, 0x45, 0x55, 0x4D, 0x5D, 0x59, 0x41, 0x51, 0x52,  # EOR
             0x69, 0x65, 0x75, 0x6D, 0x7D, 0x79, 0x61, 0x71, 0x72,  # ADC
             0xE9, 0xE5, 0xF5, 0xED, 0xFD, 0xF9, 0xE1, 0xF1, 0xF2)  # SBC

ALTERNATE = {
    "decimal_formula": "mesen",
    "sbc_honours_t": True,
    "tsb_z_from_test": True,
    "tma_multiple": "highest",
    "zero_page_pointer_wraps": False,
    "block_reads_hardware_page_as_zero": False,
    "nop_63_cycles": 4,
}


def physical(mpr: list[int], logical: int) -> int:
    logical &= 0xFFFF
    return mpr[logical >> 13] << 13 | logical & 0x1FFF


def directed(seed: int, switches: dict, per_opcode: int, big_block: bool) -> dict:
    """Every opcode ``per_opcode`` times, one boundary a case."""
    rng = random.Random(seed)
    used: set[int] = set()
    memory: list[dict] = []
    events: list[dict] = []
    step = 0
    for opcode in range(256):
        for case in range(per_opcode):
            while True:
                mpr = [rng.randrange(256) for _ in range(8)]
                pc = rng.randrange(0x10000)
                cells = [physical(mpr, pc + n) for n in range(7)]
                if not used.intersection(cells):
                    break
            used.update(cells)
            code = [opcode] + [rng.randrange(256) for _ in range(6)]
            registers = {
                "a": rng.randrange(256), "x": rng.randrange(256), "y": rng.randrange(256),
                "s": rng.randrange(256), "p": rng.randrange(256) & ~0x10, "pc": pc,
            }
            roll = rng.random()
            if roll < 0.15:
                code[1] = 0xFF  # a zero-page operand at the page's last byte (U8)
            elif roll < 0.25:
                registers[rng.choice("xy")] = rng.choice((0x00, 0xFF))
            elif roll < 0.30:
                registers["s"] = rng.choice((0x00, 0x01, 0x02, 0xFF))
            if opcode in (0x43, 0x53):  # TMA, TAM: no bit, one bit, several
                roll = rng.random()
                if roll < 0.2:
                    code[1] = 0
                elif roll < 0.6:
                    code[1] = 1 << rng.randrange(8)
            if opcode in BLOCK:
                length = 0 if big_block and case == 0 else rng.randrange(1, 25)
                code[5], code[6] = length & 0xFF, length >> 8
                if rng.random() < 0.4:
                    # The source runs across an edge of the zero-read range (U7, C9).
                    edge = rng.choice((0x0800, 0x1800))
                    source = (rng.randrange(8) << 13) | (edge + rng.randrange(-12, 4)) & 0x1FFF
                    if source >> 13 != pc >> 13 and source >> 13 != (pc + 6 & 0xFFFF) >> 13:
                        mpr[source >> 13] = 0xFF
                        code[1], code[2] = source & 0xFF, source >> 8
            registers["mpr"] = mpr
            for n, byte in enumerate(code):
                memory.append({"address": physical(mpr, pc + n), "hex": f"{byte:02x}"})
            events.append({"step": step, "action": "set", "registers": registers})
            if rng.random() < 0.05:
                events.append({"step": step, "action": "restore"})
            step += 1
    return {"fill": seed, "memory": memory, "switches": switches, "steps": step, "events": events}


def sequences(seed: int, switches: dict) -> dict:
    """Short programs where an instruction depends on the one before it."""
    rng = random.Random(seed)
    mpr = [0xFF, 0xF8, 0xF9, 0xFA, 0xFB, 0x10, 0x11, 0x00]
    memory: list[dict] = []
    events: list[dict] = []
    step = 0
    pc = 0xA000  # pages $10 and $11: the cases follow one another, 16 bytes apart

    def case(code: bytes, steps: int, **registers: int) -> None:
        nonlocal step, pc
        state = {
            "a": rng.randrange(256), "x": rng.randrange(256), "y": rng.randrange(256),
            "s": 0xFF, "p": rng.randrange(256) & ~0x30, "pc": pc, "mpr": mpr,
        }
        state.update(registers)
        memory.append({"address": physical(mpr, pc), "hex": code.hex()})
        events.append({"step": step, "action": "set", "registers": state})
        step += steps
        pc += 16

    for opcode in T_OPCODES:
        for _ in range(12):
            operands = bytes((rng.randrange(256), rng.randrange(0x20, 0x60)))
            case(bytes((0xF4, opcode)) + operands + b"\xea\xea", 3)  # SET, op, NOP
            # PLP pulls a P with T set, then the instruction runs in memory mode.
            case(bytes((0x28, opcode)) + operands + b"\xea\xea", 3, s=0x7F)
    # The stack byte PLP pulls ($2180): T and I both ways, D both ways.
    for value in (0x20, 0x24, 0x28, 0x2C, 0x00, 0x04, 0xFF, 0xEF):
        case(bytes((0xA9, value, 0x48, 0x28, 0x08, 0x68, 0xEA)), 6, s=0x80)
    for code in ("58 78 58 ea 78 ea", "d4 ea 54 ea d4 54", "f4 f4 ea", "f4 08 68", "f4 00 ea",
                 "f4 d4 65 10", "f4 73 00 22 40 22 03 00"):
        case(bytes.fromhex(code), 6 if len(code) > 12 else 3)
    return {"fill": seed, "memory": memory, "switches": switches, "steps": step, "events": events}


# -- rung 4 ---------------------------------------------------------------

RESET_MPR = [0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47]  # before the program maps itself


def workout() -> list[dict]:
    """The interrupt workout: code in bank 0 (logical $E000 after reset), handlers, vectors.

    With ``"board": true`` the host is a sliver of a PC Engine: a write to
    physical $1FF402 is the interrupt disable register, a write to $1FF403
    lets the timer's line go, a read of $1FE000 lets IRQ1 go.
    """
    main = bytes.fromhex(
        "a9 ff"                 # E000 LDA #$FF
        "53 01"                 # E002 TAM #$01     MPR0 = $FF: the hardware page at $0000
        "a9 f8"                 # E004 LDA #$F8
        "53 02"                 # E006 TAM #$02     MPR1 = $F8: zero page and stack
        "a2 ff"                 # E008 LDX #$FF
        "9a"                    # E00A TXS
        "d4"                    # E00B CSH
        "58"                    # E00C CLI
        "e6 10"                 # E00D INC $10      main:
        "a5 10"                 # E00F LDA $10
        "29 03"                 # E011 AND #$03
        "d0 03"                 # E013 BNE $E018
        "54"                    # E015 CSL
        "80 01"                 # E016 BRA $E019
        "d4"                    # E018 CSH
        "78"                    # E019 SEI
        "ea"                    # E01A NOP
        "58"                    # E01B CLI
        "08"                    # E01C PHP
        "78"                    # E01D SEI
        "28"                    # E01E PLP          restores I clear, one boundary late
        "f4"                    # E01F SET
        "65 10"                 # E020 ADC $10      memory mode: M($20FF) += M($2010)
        "73 00 22 40 22 05 00"  # E022 TII $2200,$2240,5
        "9c 03 14"              # E029 STZ $1403    lets the timer go
        "ad 00 00"              # E02C LDA $0000    reads the VDC status: lets IRQ1 go
        "a9 00"                 # E02F LDA #$00
        "8d 02 14"              # E031 STA $1402    all three sources enabled
        "43 01"                 # E034 TMA #$01
        "00 ea"                 # E036 BRK, signature byte
        "a5 10"                 # E038 LDA $10
        "10 03"                 # E03A BPL $E03F
        "4c 0d e0"              # E03C JMP main
        "a9 05"                 # E03F LDA #$05
        "8d 02 14"              # E041 STA $1402    IRQ2 and TIQ masked for a lap
        "4c 0d e0"              # E044 JMP main
    )
    irq2 = bytes.fromhex(
        "48"        # E100 PHA              IRQ2 and BRK
        "e6 20"     # E101 INC $20
        "a5 20"     # E103 LDA $20
        "29 07"     # E105 AND #$07
        "d0 01"     # E107 BNE $E10A
        "58"        # E109 CLI              nested interrupts now and then
        "68"        # E10A PLA
        "40"        # E10B RTI
    )
    irq1 = bytes.fromhex(
        "48"        # E120 PHA
        "ad 00 00"  # E121 LDA $0000        lets IRQ1 go
        "68"        # E124 PLA
        "40"        # E125 RTI
    )
    tiq = bytes.fromhex(
        "58"        # E140 CLI
        "9c 03 14"  # E141 STZ $1403        the acknowledge in the last cycle (C18)
        "40"        # E144 RTI
    )
    nmi = bytes.fromhex(
        "e6 21"     # E160 INC $21
        "40"        # E162 RTI
    )
    vectors = bytes.fromhex("00e1" "20e1" "40e1" "60e1" "00e0")  # IRQ2 IRQ1 TIQ NMI RESET
    return [
        {"address": 0x0000, "hex": main.hex()},
        {"address": 0x0100, "hex": irq2.hex()},
        {"address": 0x0120, "hex": irq1.hex()},
        {"address": 0x0140, "hex": tiq.hex()},
        {"address": 0x0160, "hex": nmi.hex()},
        {"address": 0x1FF6, "hex": vectors.hex()},
    ]


def fuzz(seed: int, steps: int, switches: dict, accesses_per_step: float = 9.0) -> dict:
    """The workout under seeded random line changes, masks, NMIs, resets and restores."""
    rng = random.Random(seed)
    span = int(steps * accesses_per_step)
    events: list[dict] = []
    lines = 0
    for access in range(span):
        roll = rng.random()
        if roll < 0.020:
            lines ^= 1 << rng.randrange(3)  # one line rises or falls
            events.append({"access": access, "action": "lines", "value": lines})
        elif roll < 0.026:
            lines = rng.randrange(8)
            events.append({"access": access, "action": "lines", "value": lines})
        elif roll < 0.031:
            # Two changes in one cycle: a pulse, or a swap of sources.
            other = lines ^ (1 << rng.randrange(3))
            events.append({"access": access, "action": "lines", "value": other})
            events.append({"access": access, "action": "lines", "value": rng.choice((lines, other ^ 7))})
            lines = events[-1]["value"]
        elif roll < 0.035:
            events.append({"access": access, "action": "disable", "value": rng.randrange(8)})
        elif roll < 0.039:
            events.append({"access": access, "action": "nmi"})
        elif roll < 0.040:
            events.append({"access": access, "action": "nmi_clear"})
    for step in range(steps):
        roll = rng.random()
        if roll < 0.010:
            events.append({"step": step, "action": "lines", "value": rng.randrange(8)})
        elif roll < 0.014:
            events.append({"step": step, "action": "disable", "value": rng.randrange(8)})
        elif roll < 0.018:
            events.append({"step": step, "action": "nmi"})
        elif roll < 0.019:
            events.append({"step": step, "action": "reset"})
        elif roll < 0.060:
            events.append({"step": step, "action": "restore"})
    return {
        "memory": workout(), "board": True, "registers": {"mpr": RESET_MPR}, "reset": True,
        "switches": switches, "steps": steps, "events": events,
    }


def sweep(access: int, changes: list[dict], switches: dict) -> dict:
    """The workout with one change (or a change and its undoing) at one cycle."""
    events = [
        {"access": access + change.get("after", 0), "action": change["action"],
         "value": change.get("value", 0)}
        for change in changes
    ]
    return {
        "memory": workout(), "board": True, "registers": {"mpr": RESET_MPR}, "reset": True,
        "switches": switches, "steps": 60, "events": events,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--out", type=Path, default=OUT)
    parser.add_argument("--quick", action="store_true", help="a smaller set, for CI")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for old in args.out.glob("*.json"):
        old.unlink()
    scenarios: dict[str, dict | list[dict]] = {}

    per_opcode = 12 if args.quick else 40
    scenarios["directed-default"] = directed(1, {}, per_opcode, big_block=True)
    scenarios["directed-alternate"] = directed(2, ALTERNATE, per_opcode, big_block=False)
    scenarios["directed-lowest"] = directed(
        3, {"tma_multiple": "lowest", "sbc_honours_t": True, "nop_63_cycles": 3}, per_opcode, False
    )
    scenarios["sequences-default"] = sequences(4, {})
    scenarios["sequences-alternate"] = sequences(5, ALTERNATE)

    seeds = 1 if args.quick else 3
    steps = 4_000 if args.quick else 20_000
    withdrawn = ("mednafen", "latched")
    index = len(scenarios)
    for poll in ("second_to_last", "last", "mednafen"):
        for vanished in ("mesen2", "latched", "drop"):
            for seed in range(seeds):
                index += 1
                if poll in withdrawn or vanished in withdrawn:
                    continue  # the seed it had stays reserved
                switches = {"irq_poll_cycle": poll, "vanished_interrupt": vanished}
                name = f"irq-{poll}-{vanished}-{seed}"
                scenarios[name] = fuzz(100 + index - 1, steps, switches)
    scenarios["irq-entry7"] = fuzz(900, steps, {"interrupt_entry_cycles": 7})
    scenarios["irq-entry7-last"] = fuzz(901, steps, {"interrupt_entry_cycles": 7, "irq_poll_cycle": "last"})

    # One change at each cycle of the first laps of the workout (about 150 cycles a lap).
    changes = {
        "tiq": [{"action": "lines", "value": 4}],
        "irq1": [{"action": "lines", "value": 2}],
        "irq2": [{"action": "lines", "value": 1}],
        "nmi": [{"action": "nmi"}],
        "tiq-pulse": [{"action": "lines", "value": 4}, {"action": "lines", "value": 0, "after": 1}],
        "all-then-tiq-drops": [{"action": "lines", "value": 6, "after": -40},
                               {"action": "lines", "value": 2}],
    }
    # A file may hold a list of scenarios, run one after another: one file a line and mode.
    span = range(40, 200, 4 if args.quick else 1)
    for poll in ("second_to_last", "last"):
        for vanished in ("mesen2", "drop"):
            switches = {"irq_poll_cycle": poll, "vanished_interrupt": vanished}
            for name, action in changes.items():
                scenarios[f"sweep-{poll}-{vanished}-{name}"] = [
                    sweep(access, action, switches) for access in span
                ]

    for name, scenario in scenarios.items():
        (args.out / f"{name}.json").write_text(json.dumps(scenario, separators=(",", ":")))
    print(f"{len(scenarios)} scenarios in {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
