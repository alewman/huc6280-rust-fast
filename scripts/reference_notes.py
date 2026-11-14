#!/usr/bin/env python3
"""Three corners of the reference's interrupt lines, shown on the reference itself.

    external/huc6280-python/.venv/bin/python scripts/reference_notes.py

The README's "Notes on the reference", items 1 to 3: behaviour of
huc6280-python v0.3.0 that looked unintended while it was being transcribed,
filed there as issue 0005 and settled in v0.4.0 (the pinned reference). On
v0.3.0 this printed a vanished entry's trail one step short (`$5100` at the
fourth step), `$5100` for the swap (IRQ1 taken at once) and `$5100` without
the round trip but `$4003` with it. On v0.4.0 each pair agrees, as the rule
says it should: the same trail after a vanished entry as after a plain
instruction; `$4003` (the NOP ran) for the swap; `$5100` with and without the
round trip. The Rust core does the same at each pin (rung 4 runs through all
three). The machine is 2 MB of RAM with every mapping register on page $F8,
code at logical $4000, and the handlers' addresses in the vectors: TIQ $5000,
IRQ1 $5100, IRQ2 $5200. A line change is given as the cycle it happens in
(counted from 0 over the run) and the new (IRQ1, IRQ2, TIQ) levels.
"""

from __future__ import annotations

from huc6280 import HuC6280

CLI, NOP = 0x58, 0xEA


def machine(code: list[int], during: dict[int, tuple[bool, bool, bool]]) -> HuC6280:
    memory = bytearray(0x200000)
    memory[0x1F0000 : 0x1F0000 + len(code)] = bytes(code)
    for vector, target in ((0xFFFA, 0x5000), (0xFFF8, 0x5100), (0xFFF6, 0x5200)):
        memory[0x1F0000 | vector & 0x1FFF] = target & 0xFF
        memory[(0x1F0000 | vector & 0x1FFF) + 1] = target >> 8
    count = [0]

    def tick() -> None:
        lines = during.get(count[0])
        count[0] += 1
        if lines is not None:
            cpu.set_irq_lines(*lines)

    def read_byte(address: int) -> int:
        tick()
        return memory[address]

    def write_byte(address: int, value: int) -> None:
        tick()
        memory[address] = value

    def idle(address: int | None) -> None:
        tick()

    cpu = HuC6280(read_byte, write_byte, idle)
    cpu.mpr[:] = [0xF8] * 8
    cpu.pc = 0x4000
    return cpu


def main() -> None:
    # 1. The instruction a vanished interrupt runs keeps the last-cycle rule (v0.3.0: it did not).
    #    CLI; NOP; NOP; NOP; NOP. First run: TIQ is up, drops in the first NOP's last cycle
    #    (cycle 3), so the entry after it finds nothing and runs the second NOP; IRQ1 rises
    #    in that NOP's last cycle (cycle 5). Second run: no TIQ, IRQ1 rises in the same cycle.
    for name, start, during in (
        ("after a vanished entry  ", (False, False, True),
         {3: (False, False, False), 5: (True, False, False)}),
        ("after a plain instruction", (False, False, False), {5: (True, False, False)}),
    ):
        cpu = machine([CLI, NOP, NOP, NOP, NOP], during)
        cpu.set_irq_lines(*start)
        trail = []
        for _ in range(5):
            cpu.step()
            trail.append(f"${cpu.pc:04X}")
        print(f"1. IRQ1 raised in the second NOP's last cycle, {name}: PC after each step {' '.join(trail)}")

    # 2. A swap of sources in the last cycle waits for the next poll (v0.3.0: taken at once).
    #    CLI; NOP; NOP with TIQ up; in the NOP's last cycle TIQ drops and IRQ1 rises. The
    #    frame's IRQ_LINE never changes; the core keeps the lines the poll saw (TIQ, gone by
    #    the boundary, so dropped by C18's default) and runs the NOP.
    cpu = machine([CLI, NOP, NOP], {3: (True, False, False)})
    cpu.set_irq_lines(False, False, True)
    for _ in range(3):
        cpu.step()
    print(f"2. TIQ swapped for IRQ1 in the NOP's last cycle: PC after the next boundary ${cpu.pc:04X}")

    # 3. Capture and restore at a boundary where the poll saw a line that has since dropped
    #    (CPUState.polled_irq_lines; v0.3.0 lost it and ran the NOP after the restore).
    #    CLI; NOP; NOP with IRQ1 up, dropped in the NOP's last cycle.
    for restore in (False, True):
        cpu = machine([CLI, NOP, NOP], {3: (False, False, False)})
        cpu.set_irq_lines(True, False, False)
        cpu.step()
        cpu.step()
        if restore:
            cpu.restore_state(cpu.capture_state())
        cpu.step()
        how = "with   " if restore else "without"
        print(f"3. IRQ1 dropped in the NOP's last cycle, {how} a capture and restore: PC after the next boundary ${cpu.pc:04X}")


if __name__ == "__main__":
    main()
