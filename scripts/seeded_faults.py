#!/usr/bin/env python3
"""Rung 6. A gate that cannot fail proves nothing: build the crate with one fault at a
time and check that some rung catches each.

Each fault is one exact text replacement in a copy of ``src/``, built into its
own target directory; then the rungs run against it, cheapest first, until one
fails: the SingleStepTests sample (100 cases an opcode, both formulas, the
verdicts compared with the reference's case for case), Dormann's test, rung
1 (the arithmetic tables and the directed lockstep), rung 4 (the interrupt
lockstep) and rung 5's synthetic program, the lockstep rungs on the quick
scenario set. A fault no rung catches is reported as MISSED. ``--all`` runs
every rung against every fault and lists all that catch it.

    seeded_faults.py --vectors DIR [--work DIR] [--only NAME] [--all] [--python REFERENCE_PYTHON]

``--vectors`` is the SingleStepTests ``v1`` directory; Dormann's image comes
from ``scripts/rung3.sh`` (``external/dormann``), which must have run.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# name: (file, exact text, replacement)
FAULTS = {
    # core.rs: a zero-page pointer at $FF takes its high byte from $2100 (U8's other reading).
    "pointer-leaves-zero-page": (
        "core.rs",
        "            zp.wrapping_add(1) as u16\n",
        "            zp as u16 + 1\n",
    ),
    # alu.rs: the default decimal formula carries as Mesen's does (differs for invalid digits only).
    "mednafen-adds-as-mesen": (
        "alu.rs",
        "        DecimalFormula::Mednafen => 16 * sixteens + ones,",
        "        DecimalFormula::Mednafen => 16 * (sixteens + if ones >= 16 { 1 } else { 0 }) + ones % 16,",
    ),
    # alu.rs: decimal mode clears V as binary mode does (U14 says it is left alone).
    "decimal-touches-v": (
        "alu.rs",
        "        result = decimal_add(a, operand, carry, formula);\n        p &= !(N | Z | C);",
        "        result = decimal_add(a, operand, carry, formula);\n        p &= !(N | V | Z | C);",
    ),
    # alu.rs: T mode writes the result to A as well.
    "memory-mode-keeps-a": (
        "alu.rs",
        "        self.set_nz(result);\n        self.cycle_write(address, result);",
        "        self.a = self.set_nz(result);\n        self.cycle_write(address, result);",
    ),
    # bits.rs: TRB's Z from the 65C02's test, not the result (C12 the other way).
    "trb-z-from-test": (
        "bits.rs",
        "            result == 0\n",
        "            value & self.a == 0\n",
    ),
    # block.rs: the zero-read range ends at $13FF, pcetech's stated "$1400" (C9).
    "zero-read-ends-early": (
        "block.rs",
        "pub const HARDWARE_ZERO_END: u32 = 0x1FF7FF;",
        "pub const HARDWARE_ZERO_END: u32 = 0x1FF3FF;",
    ),
    # block.rs: a length of 0 moves nothing instead of 65,536 bytes.
    "block-length-zero": (
        "block.rs",
        "            0 => 0x10000,",
        "            0 => 0,",
    ),
    # control.rs: CLI's new I counts at once (U21).
    "cli-no-delay": (
        "control.rs",
        "        self.pins.frame.polled_i = Some(self.p & I); // U21\n        self.p &= !I;",
        "        self.p &= !I;",
    ),
    # control.rs: a timer interrupt whose source has gone is entered anyway (C18).
    "vanished-timer-taken": (
        "control.rs",
        "            VanishedInterrupt::Mesen2 => (polled & !TIQ_BIT) | (polled & live & TIQ_BIT),",
        "            VanishedInterrupt::Mesen2 => polled,",
    ),
    # control.rs: every change of the lines moves lines_before, not only the first since an access.
    "lines-before-every-change": (
        "control.rs",
        "        if self.frame.deferred.is_none() && lines != self.lines {",
        "        if lines != self.lines {",
    ),
    # control.rs: irq_poll_cycle = Mednafen samples the timer like the other two lines.
    "mednafen-timer-sampled": (
        "control.rs",
        "                _ => (pins.poll_lines & !TIQ_BIT) | (pins.lines & TIQ_BIT),",
        "                _ => pins.poll_lines,",
    ),
    # control.rs: reset leaves the clock switch as it was (U10).
    "reset-keeps-speed": (
        "control.rs",
        "        self.pins.high_speed = false;\n        self.pins.irq_disable = 0;",
        "        self.pins.irq_disable = 0;",
    ),
    # control.rs: an interrupt entry polls like an instruction.
    "entry-polls": (
        "control.rs",
        "        self.pins.frame.polled = Some(NO_POLL);\n        self.pins.frame.deferred = None;\n        self.cycles\n    }\n\n    /// The frame's boundary decision",
        "        self.pins.frame.deferred = None;\n        self.cycles\n    }\n\n    /// The frame's boundary decision",
    ),
    # hudson.rs: ST0-ST2 write through the mapper (U12).
    "st-through-mapper": (
        "hudson.rs",
        "        self.cycle_write_physical(VDC_ADDRESS + offset, value);",
        "        self.cycle_write((VDC_ADDRESS + offset) as u16, value);",
    ),
    # hudson.rs: the new clock speed holds from the next instruction, not the last cycle (C7, MAME's way).
    "speed-after-last-cycle": (
        "hudson.rs",
        "        self.pins.high_speed = high;\n        self.io();",
        "        self.io();\n        self.pins.high_speed = high;",
    ),
    # hudson.rs: TAM with a mask of $00 still loads the latch (U11).
    "tam-zero-latches": (
        "hudson.rs",
        "        if mask == 0 {\n            return; // U11: a mask of $00 writes nothing and leaves the latch alone\n        }\n        self.mpr_latch = self.a;",
        "        self.mpr_latch = self.a;",
    ),
    # loads.rs: TXS sets N and Z like the other transfers.
    "txs-sets-flags": (
        "loads.rs",
        "        self.s = self.x;",
        "        self.s = self.set_nz(self.x);",
    ),
    # shifts.rs: ROR takes its carry out from bit 7.
    "ror-carry-out": (
        "shifts.rs",
        "        self.shifted((value >> 1) | ((self.p & C) << 7), value & C)",
        "        self.shifted((value >> 1) | ((self.p & C) << 7), value >> 7)",
    ),
    # dispatch.rs: TST zp is given two bytes (the trace's instruction bytes; the core never reads the form).
    "tst-form": (
        "dispatch.rs",
        '(Op::Tst, M::Zp, "TST", F::Imzp, 7), // 83',
        '(Op::Tst, M::Zp, "TST", F::Zp, 7), // 83',
    ),
    # dispatch.rs: the undefined opcode $E2 runs CLA.
    "e2-is-cla": (
        "dispatch.rs",
        '(Op::Nop, M::Implied, "NOP", F::Imp, 2), // E2',
        '(Op::Cla, M::Implied, "NOP", F::Imp, 2), // E2',
    ),
    # cpu.rs: T is not cleared by the fetch (U4).
    "t-survives-fetch": (
        "cpu.rs",
        "        self.t = self.p & T;\n        self.p &= !T;",
        "        self.t = self.p & T;",
    ),
    # cpu.rs: restore_state crosses IRQ1 and IRQ2.
    "restore-crosses-lines": (
        "cpu.rs",
        "(if state.irq1 { IRQ1_BIT } else { 0 }) | (if state.irq2 { IRQ2_BIT } else { 0 })",
        "(if state.irq1 { IRQ2_BIT } else { 0 }) | (if state.irq2 { IRQ1_BIT } else { 0 })",
    ),
    # state.rs: a restored state forgets what the poll saw of the IRQ line.
    "restore-forgets-poll": (
        "state.rs",
        "            polled_maskable_interrupt: self.polled_maskable_interrupt,",
        "            polled_maskable_interrupt: None,",
    ),
    # trace.rs: the boundary a trace names puts IRQ1 above the timer.
    "boundary-priority": (
        "trace.rs",
        "        if state.tiq && state.irq_disable & 4 == 0 {\n            return BoundaryKind::Tiq;\n        }\n        if state.irq1 && state.irq_disable & 2 == 0 {\n            return BoundaryKind::Irq1;\n        }",
        "        if state.irq1 && state.irq_disable & 2 == 0 {\n            return BoundaryKind::Irq1;\n        }\n        if state.tiq && state.irq_disable & 4 == 0 {\n            return BoundaryKind::Tiq;\n        }",
    ),
    # -- the fork's shortcuts --------------------------------------------------------------
    # dispatch.rs (the dispatch from constant rows): an arm is built with zp for a zp,X row.
    "row-mode-zpx-as-zp": (
        "dispatch.rs",
        "                        self.dispatch(ROW.0, ROW.1)",
        "                        self.dispatch(ROW.0, if matches!(ROW.1, Mode::Zpx) { Mode::Zp } else { ROW.1 })",
    ),
    # cpu.rs (the usual boundary in one test): a boundary with only polled_i left (after CLI,
    # SEI or PLP with no request up) runs the instruction without the frame consuming it.
    "step-keeps-polled-i": (
        "cpu.rs",
        "            | frame.polled.is_some()\n            | frame.polled_i.is_some()\n",
        "            | frame.polled.is_some()\n",
    ),
    # cpu.rs (the usual boundary in one test): a request changed in the last cycle is forgotten,
    # not left for the next boundary's decision.
    "step-drops-deferred": (
        "cpu.rs",
        "            frame.deferred = None;\n            frame.polled = Some(deferred);",
        "            frame.deferred = None;\n            let _ = deferred;",
    ),
}


def run(command: list[str], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(["nice", *command], capture_output=True, text=True, **kwargs)


class Rungs:
    """The rungs, each a check of one built tree's binaries against the reference's cached answers."""

    def __init__(self, args: argparse.Namespace) -> None:
        self.args = args
        self.work: Path = args.work
        self.python: str = args.python
        self.cache = self.work / "reference"
        self.cache.mkdir(parents=True, exist_ok=True)
        self.scenarios = self.work / "scenarios"
        if not self.scenarios.exists():
            run([sys.executable, str(ROOT / "scripts" / "make_scenarios.py"), "--quick", "--out", str(self.scenarios)])
        labels = json.loads((ROOT / "external" / "dormann" / "labels.json").read_text())
        self.dormann = [str(ROOT / "external" / "dormann" / "test.bin"),
                        f"{labels['start']:04X}", f"{labels['success_address']:04X}"]

    def reference(self, name: str, command: list[str]) -> str:
        """The reference's output for a command, computed once."""
        path = self.cache / name
        if not path.exists():
            result = run([self.python, *command], env=dict(os.environ, PYPY_GC_NURSERY="1M"))
            path.write_text(result.stdout)
        return path.read_text()

    def rung2(self, binaries: Path, tree: Path) -> bool:
        for formula in ("default", "mesen"):
            flags = ["--formula", "mesen"] if formula == "mesen" else []
            verdicts = self.cache / f"sst-{formula}"
            if not verdicts.exists():
                run([self.python, str(ROOT / "scripts" / "reference_sst.py"), str(self.args.vectors),
                     str(verdicts), "--limit", "100", *flags])
            ours = tree / f"sst-{formula}"
            run([str(binaries / "sst"), str(self.args.vectors), "--limit", "100", "--verdicts", str(ours), *flags])
            if run([sys.executable, str(ROOT / "scripts" / "compare_sst.py"), str(verdicts), str(ours)]).returncode:
                return False
        return True

    def rung3(self, binaries: Path, tree: Path) -> bool:
        wanted = self.reference("dormann.txt", [str(ROOT / "scripts" / "reference_dormann.py"), str(self.work / "dormann")])
        return run([str(binaries / "dormann"), *self.dormann]).stdout == wanted

    def lockstep(self, binaries: Path, tree: Path, patterns: tuple[str, ...]) -> bool:
        scenarios = sorted(str(p) for pattern in patterns for p in self.scenarios.glob(pattern))
        return run([sys.executable, str(ROOT / "scripts" / "lockstep.py"), *scenarios, "--python", self.python,
                    "--rust", str(binaries / "trace"), "--traces", str(tree / "traces"),
                    "--reference-cache", str(self.cache / "traces"), "--jobs", "6"]).returncode == 0

    def rung1(self, binaries: Path, tree: Path) -> bool:
        if run([str(binaries / "alu")]).stdout != self.reference("alu.txt", [str(ROOT / "scripts" / "reference_alu.py")]):
            return False
        return self.lockstep(binaries, tree, ("directed-*.json", "sequences-*.json"))

    def rung4(self, binaries: Path, tree: Path) -> bool:
        return self.lockstep(binaries, tree, ("irq-*.json", "sweep-*.json"))

    def rung5(self, binaries: Path, tree: Path) -> bool:
        for name, command in (("long.txt", ["6280", "400000"]), ("alternate.txt", ["1987", "200000", "--alternate"])):
            wanted = self.reference(name, [str(ROOT / "scripts" / "reference_long.py"), *command])
            if run([str(binaries / "long"), *command]).stdout != wanted:
                return False
        return True


ORDER = (
    ("rung 2 (SingleStepTests sample)", "rung2"),
    ("rung 3 (Dormann)", "rung3"),
    ("rung 1 (directed lockstep)", "rung1"),
    ("rung 4 (interrupt lockstep)", "rung4"),
    ("rung 5 (synthetic program)", "rung5"),
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--vectors", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=ROOT / "external" / "faults")
    parser.add_argument("--python", default=str(ROOT / "external" / "huc6280-python" / ".venv" / "bin" / "python"))
    parser.add_argument("--only")
    parser.add_argument("--all", action="store_true", help="run every rung against every fault")
    args = parser.parse_args()
    args.work.mkdir(parents=True, exist_ok=True)
    rungs = Rungs(args)
    target = args.work / "target"

    def plant(name: str, file: str, old: str, new: str) -> Path | None:
        """A copy of the crate with one replacement made (none: the file is only rewritten)."""
        tree = args.work / name
        if tree.exists():
            shutil.rmtree(tree)
        shutil.copytree(ROOT / "src", tree / "src")
        shutil.copy(ROOT / "Cargo.toml", tree / "Cargo.toml")
        shutil.copy(ROOT / "Cargo.lock", tree / "Cargo.lock")
        source = tree / "src" / file
        text = source.read_text()
        if old and text.count(old) != 1:
            print(f"{name}: the fault's text is in {file} {text.count(old)} times, not once")
            return None
        # Every tree is built into one target directory, where Cargo takes them for one
        # package and goes by modification times: the copy keeps the originals', so the
        # file written here is what makes Cargo build this tree and not reuse the last.
        source.write_text(text.replace(old, new) if old else text)
        build = run(["cargo", "build", "--release", "-q", "--manifest-path", str(tree / "Cargo.toml"),
                     "--target-dir", str(target)])
        if build.returncode:
            print(f"{name}: does not build\n{build.stderr[-800:]}")
            return None
        return tree

    # The unfaulted tree must pass every rung here, or a catch means nothing.
    if plant("clean", "lib.rs", "", "") is None:
        return 2
    for label, method in ORDER:
        if not getattr(rungs, method)(target / "release", args.work / "clean"):
            print(f"the unfaulted tree fails {label}")
            return 2
    print("the unfaulted tree passes every rung used here", flush=True)

    missed = 0
    count = 0
    for name, (file, old, new) in FAULTS.items():
        if args.only and name != args.only:
            continue
        count += 1
        tree = plant(name, file, old, new)
        if tree is None:
            return 2
        caught = []
        for label, method in ORDER:
            if not getattr(rungs, method)(target / "release", tree):
                caught.append(label)
                if not args.all:
                    break
        print(f"{name} ({file}): {'caught by ' + ', '.join(caught) if caught else 'MISSED'}", flush=True)
        missed += not caught
    print(f"{count - missed} caught, {missed} missed")
    return 1 if missed else 0


if __name__ == "__main__":
    raise SystemExit(main())
