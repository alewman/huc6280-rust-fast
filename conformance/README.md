<!-- SPDX-License-Identifier: MIT -->
# Lockstep scenarios

A scenario is a program, a host and a number of boundaries, run on both cores:
`target/release/trace` here and `scripts/reference_trace.py` on huc6280-python
(its own `HuC6280Session`, access tracking on, written with its own
`write_trace`). `scripts/lockstep.py` runs both and compares the two traces
with the reference's `first_trace_divergence`; a scenario passes when it
prints `traces are identical`.

The scenarios are written by `scripts/make_scenarios.py` into
`external/scenarios/` (seeded, so the same every time; generated and
hand-assembled, nothing third-party; not committed, the larger ones are 3 MB).
`directed-*` and `sequences-*` are rung 1's, `irq-*` and `sweep-*` rung 4's.

## Format

A file holds one scenario, or a list of scenarios that are run one after
another (each on a new CPU and new memory), their records following one
another in the trace.

```json
{
  "fill": 1,
  "board": true,
  "memory": [{"address": 2031616, "hex": "e8e8"}],
  "registers": {"pc": 16384, "mpr": [255, 248, 0, 0, 0, 0, 0, 0]},
  "switches": {"decimal_formula": "mesen", "irq_poll_cycle": "last"},
  "reset": true,
  "steps": 40,
  "events": [
    {"step": 20, "action": "reset"},
    {"access": 77, "action": "lines", "value": 4},
    {"step": 3, "action": "set", "registers": {"pc": 4096, "p": 32}}
  ],
  "accesses": true
}
```

| Key | Meaning |
| --- | --- |
| `fill` | Seed of the noise the 2 MB starts as (a 64-bit linear congruential generator, its top byte a cell); absent, the memory is zero. |
| `memory` | Blocks of `hex` bytes at 21-bit **physical** addresses, over the fill. The whole 2 MB is RAM. |
| `board` | The host is a sliver of a PC Engine: a write to physical `$1FF402` is the interrupt disable register, a write to `$1FF403` lets the timer's line go, a read of `$1FE000` lets IRQ1 go. Default false. |
| `registers` | Any of `a x y s pc p` and `mpr` (eight values) set before the first step. |
| `switches` | The core's named switches, by the reference's attribute names and values: `decimal_formula` (`"mednafen"`, `"mesen"`), `sbc_honours_t`, `irq_poll_cycle` (`"second_to_last"`, `"last"`), `vanished_interrupt` (`"mesen2"`, `"drop"`; the reference withdrew `"mednafen"` and `"latched"` in its clean rewrite, `6ce348e`), `tma_multiple` (`"or"`, `"highest"`, `"lowest"`), `tsb_z_from_test`, `zero_page_pointer_wraps`, `block_reads_hardware_page_as_zero`, `nop_63_cycles`, `interrupt_entry_cycles`. |
| `reset` | Request a reset before the first step. |
| `steps` | Boundaries to run and record. |
| `events` | Host actions. With `step: n`, made before boundary `n` (from 0). With `access: k`, made inside the bus callback of cycle `k` (counted from 0 over the scenario: reads, writes, dummy reads and internal cycles alike), before the access completes; several at one cycle run in the order listed. Actions: `lines` (`value`: the three inputs in the `$1402` layout, bit 0 IRQ2, bit 1 IRQ1, bit 2 TIQ), `disable` (`value`: the disable register), `nmi`, `nmi_clear`, `reset`; and between steps only, `set` (`registers`: load these registers, as at the start) and `restore` (`restore_state(capture_state())`, on both cores at the same boundary). |
| `accesses` | Record every cycle in the trace (default true). |
