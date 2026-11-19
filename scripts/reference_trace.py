#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Run a scenario on huc6280-python and write its boundary trace (the reference's half).

The same scenario format as ``src/bin/trace.rs`` (conformance/README.md), run
on the reference core through its own ``HuC6280Session`` with access tracking,
written with its own ``write_trace``. Run it with the reference's interpreter
(``external/huc6280-python/.venv/bin/python``, from
``scripts/fetch_huc6280python.sh``).

    reference_trace.py SCENARIO.json [--out TRACE.jsonl]
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from huc6280 import HuC6280
from huc6280.debug import HuC6280Session
from huc6280.trace import write_trace


def set_registers(cpu: HuC6280, registers: dict) -> None:
    for name, value in registers.items():
        if name == "mpr":
            cpu.mpr[:] = value
        else:
            setattr(cpu, name, value)


def act(cpu: HuC6280, event: dict) -> None:
    action, value = event["action"], event.get("value", 0)
    if action == "lines":
        # The three lines in the $1402 layout: bit 0 IRQ2, bit 1 IRQ1, bit 2 TIQ.
        cpu.set_irq_lines(bool(value & 2), bool(value & 1), bool(value & 4))
    elif action == "disable":
        cpu.irq_disable = value
    elif action == "nmi":
        cpu.request_non_maskable_interrupt()
    elif action == "nmi_clear":
        cpu.clear_non_maskable_interrupt()
    elif action == "reset":
        cpu.request_reset()
    elif action == "restore":
        # Capture and restore at this boundary, as src/bin/trace.rs does.
        cpu.restore_state(cpu.capture_state())
    elif action == "set":
        set_registers(cpu, event["registers"])
    else:
        raise ValueError(f"unknown action {action}")


# The sliver of a PC Engine the interrupt workout uses ("board": true).
IRQ_DISABLE = 0x1FF402
TIMER_ACKNOWLEDGE = 0x1FF403
VDC_STATUS = 0x1FE000


class Host:
    """2 MB of flat RAM and the scenario's in-cycle actions."""

    def __init__(self, memory: bytearray, during: dict, board: bool) -> None:
        self.memory = memory
        self.during = during
        self.board = board
        self.count = 0
        self.cpu: HuC6280 | None = None

    def tick(self) -> None:
        index = self.count
        self.count += 1
        for event in self.during.get(index, ()):
            act(self.cpu, event)

    def read_byte(self, address: int) -> int:
        self.tick()
        if self.board and address == VDC_STATUS:
            _, irq2, tiq = self.cpu.irq_lines
            self.cpu.set_irq_lines(False, irq2, tiq)
        return self.memory[address]

    def write_byte(self, address: int, value: int) -> None:
        self.tick()
        if self.board and address == IRQ_DISABLE:
            self.cpu.irq_disable = value
        elif self.board and address == TIMER_ACKNOWLEDGE:
            irq1, irq2, _ = self.cpu.irq_lines
            self.cpu.set_irq_lines(irq1, irq2, False)
        self.memory[address] = value

    def idle(self, address: int | None) -> None:
        self.tick()


def records(path: Path):
    # A file holds one scenario or a list of them, run one after another.
    loaded = json.loads(path.read_text())
    for scenario in loaded if isinstance(loaded, list) else [loaded]:
        yield from run(scenario)


def run(scenario: dict):
    memory = bytearray(0x200000)
    if scenario.get("fill") is not None:
        # The same generator as src/bin/trace.rs's.
        state = scenario["fill"]
        for index in range(len(memory)):
            state = (state * 6364136223846793005 + 1442695040888963407) & 0xFFFFFFFFFFFFFFFF
            memory[index] = state >> 56
    for block in scenario["memory"]:
        for offset, byte in enumerate(bytes.fromhex(block["hex"])):
            memory[(block["address"] + offset) & 0x1FFFFF] = byte
    during: dict[int, list] = {}
    between: dict[int, list] = {}
    for event in scenario.get("events", ()):
        if "step" in event:
            between.setdefault(event["step"], []).append(event)
        else:
            during.setdefault(event["access"], []).append(event)
    host = Host(memory, during, scenario.get("board", False))
    cpu = HuC6280(host.read_byte, host.write_byte, host.idle)
    host.cpu = cpu
    set_registers(cpu, scenario.get("registers", {}))
    for name, value in scenario.get("switches", {}).items():
        if not hasattr(type(cpu), name):
            raise ValueError(f"unknown switch {name}")
        setattr(cpu, name, value)
    if scenario.get("reset"):
        cpu.request_reset()
    session = HuC6280Session(
        cpu,
        peek_byte=lambda logical: memory[cpu.physical(logical)],
        track_accesses=scenario.get("accesses", True),
    )
    for sequence in range(scenario["steps"]):
        for event in between.get(sequence, ()):
            act(cpu, event)
        yield session.step()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("scenario", type=Path)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    stream = args.out.open("w") if args.out else sys.stdout
    with stream:
        write_trace(records(args.scenario), stream)
    return 0


if __name__ == "__main__":
    sys.exit(main())
