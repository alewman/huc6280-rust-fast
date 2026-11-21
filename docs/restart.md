<!-- SPDX-License-Identifier: MIT -->
# The clean rewrite, carried (2026-11-19)

huc6280-python rewrote the parts its provenance audit of 2026-11-13 marked **rewrite** or
**unclear** (its `docs/restart.md`, commit `6ce348e`, after `v0.4.0`): the replaced code had
followed GPL source, so the Rust functions that transcribed it are equally tainted. This pass
replaces them here with transcriptions of the new Python, under the PC Engine family's rules for
the rewrite (adapted from sn76489-python's restart rules and the family charter's provenance
rules).

## The rules

1. **No GPL or LGPL source is read**: Mesen2, Mednafen and its Beetle ports, Geargrafx, srg320's
   TurboGrafx16 MiSTer VHDL, MAME's GPL files. Not to learn, not to check, not when a comparison
   fails. None was opened for this pass, nor any `reference/` tree of the Python repositories.
2. **The code being replaced is not read.** Before anything was written, the tainted items were
   moved, unread, into the git-ignored `scratch/withdrawn/` with a helper that prints only item
   names and line ranges (`scratch/withdraw.py`): `src/alu.rs` lines 1-147 (the module head,
   `DecimalFormula`, the decimal add and subtract, `add`, `subtract`); `src/control.rs` the enums
   `VanishedInterrupt` and `IrqPollCycle` (lines 38-74) and `boundary_interrupt_sequence` and
   `maskable_vector` (lines 470-535); and from `scripts/seeded_faults.py` the five faults whose
   target text was that code (they quoted it). The replacements were transcribed from the new
   Python alone (huc6280-python `6ce348e`: `_alu.py`, `_control.py`), line by line, same names,
   its comments and evidence tags carried. Call sites, the tests and the untainted modules were
   read.
3. **The gate is the reference**, as always for this tier: every rung compares this core with
   huc6280-python `6ce348e`, bus cycle by bus cycle. The withdrawn code's results are recorded
   below as "before", not used as a target.
4. House rules: only this repository is edited; no push, no tag.

## What was replaced

| Item | Before | After |
| --- | --- | --- |
| C13 decimal `ADC`/`SBC` (`alu.rs`) | transcribed from the reference's v0.3.0 routines, which followed a prose restatement of GPL code | `decimal_add`/`decimal_subtract`, `add`, `subtract` transcribed from the reference's new functions, written there from two closed-box result tables; identical results (rung 1's tables, rung 2) |
| `DecimalFormula` | `Mednafen`, `Mesen` | the same two (the reference's `DECIMAL_FORMULAS`); the reference's `ValueError` for an unknown name, raised when a decimal ADC/SBC runs, has no counterpart (an enum) |
| C18 `VanishedInterrupt` | `Mesen2`, `Latched`, `Drop` | **`Latched` removed** (the reference withdrew `"latched"`: `ValueError`); `Mesen2` (default) and `Drop` re-transcribed |
| U25 `IrqPollCycle` | `SecondToLast`, `Last`, `Mednafen` | **`Mednafen` removed** (the reference withdrew `"mednafen"`: `ValueError`); `SecondToLast` (default) and `Last` re-transcribed |
| `boundary_interrupt_sequence` | old code | new: records `poll_lines` (the polled lines, or the live lines under `Last`) and **clears `polled_lines` at the boundary**, so a capture right after an interrupt entry reports `polled_irq_lines = None` |
| `maskable_vector` | old code | new: under `Mesen2` the polled IRQ1/IRQ2 plus the timer only if polled and still live; under `Drop` polled and live; priority TIQ, IRQ1, IRQ2 |
| `src/bin/replay.rs` `bank_offsets` (tooling) | the reference's old map, stated as Mesen2's | the reference's new map, MAME's `rom_map_setup` (BSD-3, ported by the reference; NOTICE.md): 512 KB cards repeat whole, 192 KB images map `0 1 2 2 0 1 2 2` |
| comments | named GPL internals (BRK as "Mesen2's ProcessIrq", "Mesen's ReadZeroPageWrap", "Mesen's" bus shapes) | the reference's new wording: observed behaviour, closed-box runs and permissive sources named |
| `src/bin/long.rs` `--alternate`, `scripts/reference_long.py` | `Latched` with poll `Last` | `Drop` with poll `Last`: under `Last` the poll lines are the live lines, so the two take the same sources and the run is the same |
| `scripts/make_scenarios.py` | rung 4 in all nine mode pairs, sweeps in six | the four kept pairs (the kept random scenarios keep their seeds), sweeps in four (`Drop` swept where `Latched` was) |
| rung 6 | 32 faults, 5 on the withdrawn code | those 5 moved aside unread; `entry-polls` re-pointed; 13 new faults on the new code |

SPDX headers were added to every source file and every document touched.

## Behaviour changes a board must know

- `VanishedInterrupt::Latched` and `IrqPollCycle::Mednafen` no longer exist: a board that set
  either does not compile. Under `IrqPollCycle::Last`, `Drop` is what `Latched` was.
- `CpuState::polled_irq_lines` is `None` right after any boundary that decides an interrupt (the
  boundary uses the lines up); a state captured after an interrupt entry reads `None` where it
  could read `Some` before. A board's save states load as before.
- Everything else, the defaults included, is cycle for cycle what it was.

## Judges, before (reference `265df88`, this crate `f2eeece`) and after (reference `6ce348e`)

Run on 2026-11-19 (Linux x86_64, rustc 1.93.1, the reference on PyPy 7.3.20 with
`PYPY_GC_NURSERY=1M`). The before column is the record of 2026-11-13 (`external/before-6ce348e/`
locally).

| Judge | Before | After |
| --- | --- | --- |
| `cargo test` | 17 directed tests, 2 doctests | 20 directed tests (C16's entry between SET and its target, a source still asserted in every mode, the polled lines cleared at the boundary; the `Latched` cases replaced by `Drop`), 2 doctests, 1 replay-tool test |
| rung 1: ADC/SBC tables, both formulas | 2,097,152 entries identical | the same |
| rung 1: directed lockstep | 5 of 5, 37,356 boundaries | the same |
| rung 2: SingleStepTests, default formula | 640,000 / 640,000 on both cores, 0 judged differently; masks `st_write` 7,500, `truncated` 12,481, `block_unknown` 11,177, `block_self_overlap` 5,031, `block_final_unexplained` 16, `tma_latch` 13, `tsb_z` 1,492, `decimal_formula` 1,552 | the same counts, 0 judged differently (the reference's own counts at 6ce348e) |
| rung 2: `--formula mesen` | the same masks, no `decimal_formula` | the same |
| rung 3: Dormann | success at `$5675` after 20,936,652 instructions | the same |
| rung 4: interrupt lockstep | 65 of 65 files (nine mode pairs), 925,600 boundaries | 38 of 38 (four mode pairs), 510,400 boundaries |
| rung 5: six MAME replays and the synthetic runs | 6 of 6 reports and digests, 21,013,980 instructions; synthetic digests identical | byte for byte the same output (the alternate run under `Drop` where it had `Latched`: the same digests, as expected under poll `Last`) |
| rung 6: seeded faults | 32 caught, 0 missed | 40 caught, 0 missed (27 kept, 13 on the rewrite's code; one equivalent fault replaced) |
| clippy `-D warnings`, rustup 1.93.1 and stable 1.99.1 (own target dirs) | clean | clean |

Nothing here was run on hardware; the gate is the reference, which is itself judged by the
external judges its `docs/restart.md` lists.
