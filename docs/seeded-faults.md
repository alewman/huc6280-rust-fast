<!-- SPDX-License-Identifier: MIT -->
# Seeded faults (rung 6)

A gate that cannot fail proves nothing. This is the record of the run that
certified the crate against huc6280-python `6ce348e` (2026-11-19, the clean
rewrite carried, [restart.md](restart.md)): each fault, the module it is in,
what it changes, and the first check that caught it. The run before it
(2026-11-13, against `v0.4.0`) had 32 faults, 32 caught; five of those targeted
code the rewrite withdrew (they quoted it) and were moved aside unread with it,
`entry-polls` was re-pointed (its anchor was the withdrawn function's doc
comment), and 13 were added on the new code.

`scripts/seeded_faults.py` (rung 6, `scripts/rung6.sh`): one single-line fault
at a time is planted in a copy of `src/`, built, and run through the rungs
cheapest first (the SingleStepTests sample of 100 cases an opcode with the
verdicts compared, Dormann, then rungs 1, 4 and 5 on the quick scenario set)
until one fails. Every module has at least one.

| Fault | Module | What it changes | Caught by |
| --- | --- | --- | --- |
| `pointer-leaves-zero-page` | `core.rs` | a zero-page pointer at $FF takes its high byte from $2100 (U8's other reading) | rung 2 (SingleStepTests sample) |
| `memory-mode-keeps-a` | `alu.rs` | T mode writes the result to A as well | rung 2 (SingleStepTests sample) |
| `trb-z-from-test` | `bits.rs` | TRB's Z from the 65C02's test, not the result (C12 the other way) | rung 2 (SingleStepTests sample) |
| `zero-read-ends-early` | `block.rs` | the zero-read range ends at $13FF, pcetech's stated "$1400" (C9) | rung 1 (directed lockstep) |
| `block-length-zero` | `block.rs` | a length of 0 moves nothing instead of 65,536 bytes | rung 1 (directed lockstep) |
| `cli-no-delay` | `control.rs` | CLI's new I counts at once (U21) | rung 1 (directed lockstep) |
| `polled-lines-ignore-host` | `control.rs` | a line the host changes between steps, after a change the poll missed, does not count for the next boundary (until issue 0005 this fault was `lines-before-every-change`, on the field the polled lines replaced) | rung 4 (interrupt lockstep) |
| `reset-keeps-speed` | `control.rs` | reset leaves the clock switch as it was (U10) | rung 4 (interrupt lockstep) |
| `entry-polls` | `control.rs` | an interrupt entry polls like an instruction | rung 4 (interrupt lockstep) |
| `vanished-skips-bookkeeping` | `control.rs` | issue 0005 item 1: the instruction a vanished entry runs skips step()'s deferred-to-polled bookkeeping | rung 4 (interrupt lockstep) |
| `swap-not-watched` | `control.rs` | item 2: a change of the lines that leaves IRQ_LINE as it was does not arm the frame's watch | rung 4 (interrupt lockstep) |
| `reset-keeps-polled-lines` | `control.rs` | reset leaves the lines the last poll saw | rung 4 (interrupt lockstep) |
| `brk-polls` | `control.rs` | item 4: BRK polls like an instruction | rung 1 (directed lockstep) |
| `st-through-mapper` | `hudson.rs` | ST0-ST2 write through the mapper (U12) | rung 1 (directed lockstep) |
| `speed-after-last-cycle` | `hudson.rs` | the new clock speed holds from the next instruction, not the last cycle (C7, MAME's way) | rung 5 (synthetic program) |
| `tam-zero-latches` | `hudson.rs` | TAM with a mask of $00 still loads the latch (U11) | rung 1 (directed lockstep) |
| `txs-sets-flags` | `loads.rs` | TXS sets N and Z like the other transfers | rung 2 (SingleStepTests sample) |
| `ror-carry-out` | `shifts.rs` | ROR takes its carry out from bit 7 | rung 2 (SingleStepTests sample) |
| `tst-form` | `dispatch.rs` | TST zp is given two bytes (the trace's instruction bytes; the core never reads the form) | rung 1 (directed lockstep) |
| `e2-is-cla` | `dispatch.rs` | the undefined opcode $E2 runs CLA | rung 2 (SingleStepTests sample) |
| `t-survives-fetch` | `cpu.rs` | T is not cleared by the fetch (U4) | rung 2 (SingleStepTests sample) |
| `polled-lines-never-set` | `cpu.rs` | a line changed after the last access is never remembered as changed after the poll | rung 4 (interrupt lockstep) |
| `restore-forgets-polled-lines` | `cpu.rs` | item 3: restore_state forgets the lines the poll saw | rung 4 (interrupt lockstep) |
| `restore-crosses-lines` | `cpu.rs` | restore_state crosses IRQ1 and IRQ2 | rung 4 (interrupt lockstep) |
| `restore-forgets-poll` | `state.rs` | a restored state forgets what the poll saw of the IRQ line | rung 4 (interrupt lockstep) |
| `boundary-priority` | `trace.rs` | the boundary a trace names puts IRQ1 above the timer | rung 4 (interrupt lockstep) |
| `boundary-live-lines` | `trace.rs` | the boundary a trace names goes by the lines as they stand, not as the poll saw them | rung 4 (interrupt lockstep) |
| `mednafen-adds-as-mesen` | `alu.rs` | the default decimal formula adds digit by digit, as `Mesen` does (the rewrite's code) | rung 2 (SingleStepTests sample) |
| `mednafen-subtract-borrow` | `alu.rs` | the default decimal SBC subtracts $60 only when the high digit borrows on its own | rung 2 (SingleStepTests sample) |
| `mesen-subtract-no-borrow` | `alu.rs` | `Mesen` SBC's high digit ignores the low digit's borrow | rung 2 (SingleStepTests sample) |
| `decimal-touches-v` | `alu.rs` | decimal ADC clears V (U14: left alone) | rung 2 (SingleStepTests sample) |
| `vanished-timer-taken` | `control.rs` | by default a timer whose source has gone is entered as the poll saw it (C18) | rung 4 (interrupt lockstep) |
| `drop-takes-polled` | `control.rs` | `Drop` takes polled or live sources | rung 4 (interrupt lockstep) |
| `vector-ignores-disable` | `control.rs` | the entry's vector ignores the disable register | rung 4 (interrupt lockstep) |
| `vector-priority` | `control.rs` | the entry takes IRQ1 before the timer | rung 4 (interrupt lockstep) |
| `polled-lines-kept` | `control.rs` | the boundary keeps the polled lines past it (the rewrite's rule: it clears them) | rung 4 (interrupt lockstep) |
| `boundary-counts-live-lines` | `control.rs` | the default boundary counts the live lines, not the polled ones | rung 4 (interrupt lockstep) |
| `last-counts-polled-lines` | `control.rs` | `Last` counts the lines the poll saw, as the default does | rung 4 (interrupt lockstep) |
| `last-ignores-polled-i` | `control.rs` | `Last` forgets that CLI, SEI and PLP count one instruction late | rung 4 (interrupt lockstep) |
| `last-ignores-i` | `control.rs` | `Last` takes a line while I is set | rung 4 (interrupt lockstep) |

**40 caught, 0 missed** (2026-11-19): 11 by the SingleStepTests sample, 7 by
rung 1, 21 by rung 4, 1 by rung 5. The first run of the 13 new faults had
`last-ignores-disable` (`Last` deciding on a masked line) as the last one: it
was MISSED, and is an equivalent fault, not a gap: the entry's vector then finds
no enabled source and runs the instruction with `step()`'s own bookkeeping, a
path no rung can tell from the plain one. It was replaced by `last-ignores-i`.

The record of 2026-11-13 (against `v0.4.0`), for comparison:
**32 caught, 0 missed**: 9 by the SingleStepTests sample, 7 by rung 1, 15 by
rung 4, 1 by rung 5 (the eight issue 0005 faults: seven by rung 4, `brk-polls`
by rung 1, whose traces record `skipped_poll` in the state after every BRK). Of the first 24, fifteen got past the SingleStepTests sample, most of
them out of that corpus's reach by construction (it runs one instruction, with
no interrupts, no hardware page and no hidden state): the zero-read range,
`ST0`'s address (which the corpus masks), the TAM latch, the trace's own
instruction bytes, everything about the poll and the lines, and `CSH`'s last
cycle, which no trace records: the synthetic program catches it, because its
bus folds `pins.high_speed` into the digest in every cycle (the MAME replay's
harness counts master clocks from it too, but the replay is not among the
checks the fault runner makes).
Dormann's test is second in the order and caught nothing first.

## This fork's three

huc6280-rust-fast keeps the 32 above, each still caught by the same check,
and adds one fault on each shortcut it takes (run of 2026-11-13 after
merging issue 0005 from huc6280-rust, against huc6280-python `v0.4.0`:
**35 caught, 0 missed**; the run before it, at `aad85b2` against `v0.3.0`,
had 27 of 27):

| Fault | Module | What it changes | Caught by |
| --- | --- | --- | --- |
| `row-mode-zpx-as-zp` | `dispatch.rs` | an arm of the dispatch from rows is built with zp for a zp,X row | rung 2 (SingleStepTests sample) |
| `step-keeps-polled-i` | `cpu.rs` | the one-test boundary leaves out `polled_i`, so after CLI, SEI or PLP with nothing requested the frame never consumes it | rung 1 (directed lockstep) |
| `step-drops-deferred` | `cpu.rs` | the one-test boundary forgets a request changed in the last cycle instead of leaving it for the next boundary | rung 1 (directed lockstep; rung 4 at `aad85b2`: BRK now leaves `NO_POLL` deferred, which this fault drops) |

(`e2-is-cla` and `tst-form` edit the table, which the dispatch from rows
reads at compile time: still caught by the same rungs.)
