#!/usr/bin/env python3
"""Rung 5 without MAME traces, the reference's half: ``target/release/long`` on huc6280-python.

    reference_long.py SEED STEPS [--alternate]

The same seeded bus (every read from a 64-bit linear congruential generator,
the interrupt inputs driven from it every 1,024 cycles), the same digest of
every cycle and of the whole ``CPUState`` after every step, printed every
250,000 steps and at the end (src/bin/long.rs says why and how). Run it with
the reference's interpreter.
"""

from __future__ import annotations

import sys

from huc6280 import HuC6280

MASK = 0xFFFFFFFFFFFFFFFF
PRIME = 0x100000001B3
KINDS = {"read": 1, "write": 2, "dummy": 3, "idle": 4}
BLOCK = frozenset((0x73, 0xC3, 0xD3, 0xE3, 0xF3))
ALTERNATE = {
    "decimal_formula": "mesen", "sbc_honours_t": True, "tsb_z_from_test": True,
    "tma_multiple": "highest", "zero_page_pointer_wraps": False,
    "block_reads_hardware_page_as_zero": False, "nop_63_cycles": 4, "interrupt_entry_cycles": 7,
    "irq_poll_cycle": "last", "vanished_interrupt": "latched",
}


class Noise:
    def __init__(self, seed: int) -> None:
        self.state = seed
        self.count = 0
        self.digest = 0xCBF29CE484222325
        self.cpu: HuC6280 | None = None

    def tick(self) -> None:
        """Every cycle: count it, and every 1,024th drive the interrupt inputs."""
        self.count = count = self.count + 1
        if not count & 0x3FF:
            self.state = (self.state * 6364136223846793005 + 1442695040888963407) & MASK
            roll = self.state >> 32
            cpu = self.cpu
            cpu.set_irq_lines(bool(roll & 2), bool(roll & 1), bool(roll & 4))
            if not (roll >> 3) & 7:
                cpu.irq_disable = (roll >> 6) & 7
            if not (roll >> 9) & 0x1F:
                cpu.request_non_maskable_interrupt()
            if not (roll >> 14) & 0x3FF:
                cpu.request_reset()

    def seen(self) -> int:
        """What a callback can see of the CPU: the cycle's kind, the clock switch, the opcode."""
        cpu = self.cpu
        return KINDS[cpu.kind] << 32 | int(cpu.high_speed) << 36 | cpu.opcode << 40

    def read_byte(self, address: int) -> int:
        self.tick()
        self.state = state = (self.state * 6364136223846793005 + 1442695040888963407) & MASK
        value = state >> 56
        if value in BLOCK and (state >> 48) & 0x3F:
            value ^= 0x01
        self.digest = ((self.digest ^ (self.seen() | address << 8 | value)) * PRIME) & MASK
        return value

    def write_byte(self, address: int, value: int) -> None:
        self.tick()
        self.digest = ((self.digest ^ (self.seen() | address << 8 | value)) * PRIME) & MASK

    def idle(self, address: int | None) -> None:
        self.tick()
        self.digest = ((self.digest ^ (self.seen() | (address or 0) << 8)) * PRIME) & MASK


def tri(value: bool | None) -> int:
    return 2 if value is None else int(value)


def main() -> int:
    seed, steps = int(sys.argv[1]), int(sys.argv[2])
    bus = Noise(seed)
    cpu = HuC6280(bus.read_byte, bus.write_byte, bus.idle)
    bus.cpu = cpu
    if "--alternate" in sys.argv[3:]:
        for name, value in ALTERNATE.items():
            setattr(cpu, name, value)
    cpu.request_reset()
    total = 0
    for step in range(1, steps + 1):
        cycles = cpu.step()
        total += cycles
        state = cpu.capture_state()
        digest = bus.digest
        for value in (
            cycles, state.a, state.x, state.y, state.s, state.pc, state.p, *state.mpr,
            int(state.high_speed), state.mpr_latch, int(state.irq1), int(state.irq2),
            int(state.tiq), state.irq_disable, int(state.halted), int(state.reset_pending),
            int(state.maskable_interrupt_pending), int(state.non_maskable_interrupt_pending),
            tri(state.polled_i), tri(state.polled_maskable_interrupt),
            tri(state.polled_non_maskable_interrupt), int(state.skipped_poll),
            8 if state.polled_irq_lines is None else state.polled_irq_lines,
        ):
            digest = ((digest ^ value) * PRIME) & MASK
        bus.digest = digest
        if step % 250_000 == 0 or step == steps:
            print(f"step {step} cycles {total} digest {digest:016x}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
