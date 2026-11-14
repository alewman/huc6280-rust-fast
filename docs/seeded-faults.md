# Seeded faults (rung 6)

A gate that cannot fail proves nothing. This is the record of the run that
certified the crate (2026-11-12, at `7f49bf8`; the core has not changed
since): each fault, the module it is in, what it changes, and the first
check that caught it.

`scripts/seeded_faults.py` (rung 6, `scripts/rung6.sh`): one single-line fault
at a time is planted in a copy of `src/`, built, and run through the rungs
cheapest first (the SingleStepTests sample of 100 cases an opcode with the
verdicts compared, Dormann, then rungs 1, 4 and 5 on the quick scenario set)
until one fails. Every module has at least one.

| Fault | Module | What it changes | Caught by |
| --- | --- | --- | --- |
| `pointer-leaves-zero-page` | `core.rs` | a zero-page pointer at $FF takes its high byte from $2100 (U8's other reading) | rung 2 (SingleStepTests sample) |
| `mednafen-adds-as-mesen` | `alu.rs` | the default decimal formula carries as Mesen's does (differs for invalid digits only) | rung 2 (SingleStepTests sample) |
| `decimal-touches-v` | `alu.rs` | decimal mode clears V as binary mode does (U14 says it is left alone) | rung 2 (SingleStepTests sample) |
| `memory-mode-keeps-a` | `alu.rs` | T mode writes the result to A as well | rung 2 (SingleStepTests sample) |
| `trb-z-from-test` | `bits.rs` | TRB's Z from the 65C02's test, not the result (C12 the other way) | rung 2 (SingleStepTests sample) |
| `zero-read-ends-early` | `block.rs` | the zero-read range ends at $13FF, pcetech's stated "$1400" (C9) | rung 1 (directed lockstep) |
| `block-length-zero` | `block.rs` | a length of 0 moves nothing instead of 65,536 bytes | rung 1 (directed lockstep) |
| `cli-no-delay` | `control.rs` | CLI's new I counts at once (U21) | rung 1 (directed lockstep) |
| `vanished-timer-taken` | `control.rs` | a timer interrupt whose source has gone is entered anyway (C18) | rung 4 (interrupt lockstep) |
| `lines-before-every-change` | `control.rs` | every change of the lines moves lines_before, not only the first since an access | rung 4 (interrupt lockstep) |
| `mednafen-timer-sampled` | `control.rs` | irq_poll_cycle = Mednafen samples the timer like the other two lines | rung 4 (interrupt lockstep) |
| `reset-keeps-speed` | `control.rs` | reset leaves the clock switch as it was (U10) | rung 4 (interrupt lockstep) |
| `entry-polls` | `control.rs` | an interrupt entry polls like an instruction | rung 4 (interrupt lockstep) |
| `st-through-mapper` | `hudson.rs` | ST0-ST2 write through the mapper (U12) | rung 1 (directed lockstep) |
| `speed-after-last-cycle` | `hudson.rs` | the new clock speed holds from the next instruction, not the last cycle (C7, MAME's way) | rung 5 (synthetic program) |
| `tam-zero-latches` | `hudson.rs` | TAM with a mask of $00 still loads the latch (U11) | rung 1 (directed lockstep) |
| `txs-sets-flags` | `loads.rs` | TXS sets N and Z like the other transfers | rung 2 (SingleStepTests sample) |
| `ror-carry-out` | `shifts.rs` | ROR takes its carry out from bit 7 | rung 2 (SingleStepTests sample) |
| `tst-form` | `dispatch.rs` | TST zp is given two bytes (the trace's instruction bytes; the core never reads the form) | rung 1 (directed lockstep) |
| `e2-is-cla` | `dispatch.rs` | the undefined opcode $E2 runs CLA | rung 2 (SingleStepTests sample) |
| `t-survives-fetch` | `cpu.rs` | T is not cleared by the fetch (U4) | rung 2 (SingleStepTests sample) |
| `restore-crosses-lines` | `cpu.rs` | restore_state crosses IRQ1 and IRQ2 | rung 4 (interrupt lockstep) |
| `restore-forgets-poll` | `state.rs` | a restored state forgets what the poll saw of the IRQ line | rung 4 (interrupt lockstep) |
| `boundary-priority` | `trace.rs` | the boundary a trace names puts IRQ1 above the timer | rung 4 (interrupt lockstep) |

**24 caught, 0 missed**: 9 by the SingleStepTests sample, 6 by rung 1, 8 by
rung 4, 1 by rung 5. Fifteen got past the SingleStepTests sample, most of
them out of that corpus's reach by construction (it runs one instruction, with
no interrupts, no hardware page and no hidden state): the zero-read range,
`ST0`'s address (which the corpus masks), the TAM latch, the trace's own
instruction bytes, everything about the poll and the lines, and `CSH`'s last
cycle, which no trace records: the synthetic program catches it, because its
bus folds `pins.high_speed` into the digest in every cycle (the MAME replay's
harness counts master clocks from it too, but the replay is not among the
checks the fault runner makes).
Dormann's test is second in the order and caught nothing first.
