# Handoff brief: huc6280-rust

> In huc6280-rust-fast: this is the parent crate's brief, inherited with the fork. The fork's
> own brief is [fast-brief.md](fast-brief.md).
>
> **This is the brief the crate was built from (2026-11-12), kept as written**, with "What was
> built" appended at the end by the session that built it. Its instructions ("do not push", the
> working rules, the local paths under `/data/emu/` and `~/.local/opt/`, which are the author's
> machine) are history, not instructions to a reader. The [README](../README.md) is the current
> account; where the two differ, the README is right (for example, clippy now runs and passes
> locally and in CI, which the last paragraph below says it could not).

## The request

Aubrey, 2026-11-12, after the PC Engine family's Python release (huc6280-python 0.3.0,
pcengine-python 0.1.0, supergrafx-python 0.1.0, pcenginecd-python 0.1.0): "get our rust version and
rust fast and the play-pce site ready." The order is the family's (megadrive, a2600, famicom):

1. **huc6280-rust** (this repository): the CPU core.
2. pcengine-rust: the board (HuCards first; the SuperGrafx and CD add-ons after).
3. pcengine-rust-fast: the ladder-gated speed fork.
4. play-pce: the board in the browser (wasm), like play-2600 and play-nes.

## What it is

A Rust HuC6280 core **transcribed from**
[huc6280-python](https://github.com/alewman/huc6280-python) at **`v0.3.0`**
(`bf20330d4ce4e0e126331433bad03f4e4d0e9a2e`; local clone `/data/emu/huc6280-python`) and proven
equivalent to it at every processor boundary: boundary kind, cycles, every bus access in order
(address, value, kind), and all of `CPUState`. The same revision is what pcengine-python 0.1.0 pins,
so `pcengine-rust` can sit on this crate.

It builds on [6502-rust](https://github.com/alewman/6502-rust) (crate `sixfiveohtwo`, local
`/data/emu/6502-rust`, pin `71747eb9036702109fb206eb9fe88174a9dcc989` as a2600-rust does) exactly as
huc6280-python builds on 6502-python's `CPUFrame`: the `Frame` struct and the `Part` trait supply
the interrupt requests, the poll and `step()`'s skeleton; this crate supplies the HuC6280's four
sequences and inherits none of the NMOS handlers. Read 6502-rust's `src/frame.rs`, `src/core.rs`
and `src/cpu.rs` first, and a2600-rust for how a board owns the bus. If the frame lacks something
the HuC6280 needs (three maskable lines with their own vectors and a disable register, a poll point
switch, an interrupt dropped when its source vanished), **do not edit 6502-rust**: keep the
difference in this crate (as the Python core overrides `_boundary_interrupt` and friends) and list
in the README what 6502-rust would need to gain to make it cleaner.

Crate name `huc6280`, package `huc6280-rust`, edition 2021, MIT, `publish = false`.

## What to transcribe

From `/data/emu/huc6280-python/src/huc6280/` at `v0.3.0`, module for module, with the same names,
the same explicit dispatch, the switches as fields, and the hardware comments (the U and C entry
numbers of `docs/undocumented-behavior.md`) carried over:

- `_core.py`, `_alu.py`, `_bits.py`, `_block.py`, `_control.py`, `_hudson.py`, `_loads.py`,
  `_shifts.py`, `_dispatch.py`, `cpu.py`: the `HuC6280`: the 21-bit mapper (eight MPRs), the T
  flag, block transfers (one `step()` per transfer), `CSL`/`CSH` and `high_speed`, the three
  interrupt lines and `irq_disable`, the bus contract (one callback per cycle: `read_byte`,
  `write_byte`, `idle(address | None)`; in Rust a `Bus` trait the board implements, taking the
  21-bit physical address).
- **Every named switch, with the Python default:** `decimal_formula` (`"mednafen"` default,
  `"mesen"`), `sbc_honours_t`, `irq_poll_cycle` (`second_to_last` default, `last`, `mednafen`),
  `vanished_interrupt` (`"mesen2"` default), and any other class attribute the modules hold. As
  enums on the struct.
- `state.py` (`CPUState`, every field) with `capture_state`/`restore_state`, and `trace.py` (the
  trace schema), so both cores write the same JSON Lines; `HuC6280.opcode` (the public accessor
  the SuperGrafx uses).

Not transcribed (Python-side tooling): `debug.py`, `console.py`, `disasm.py`, `harness.py`,
`__main__.py`, beyond what a rung needs.

**The decimal routines** (`_alu.py` `_decimal_add`, `_decimal_subtract`) are our own code, written
from a description (huc6280-python `docs/issues/0004`): transcribe *ours*. Do not open Mednafen's or
Mesen2's source for them or for anything else; see "Working rules".

## The ladder

Each rung has a script in `scripts/` (`rungN.sh`) and passes only when its output says so. Look at
6502-rust's `scripts/` for the pattern and reuse its shape (`reference_trace.py`, `lockstep.py`,
`compare_traces.py`, `make_scenarios.py`, `seeded_faults.py`, `fetch_*python.sh`).

1. **Trace lockstep, short**: the same programs (directed scenarios covering every opcode, the T
   flag paths, block transfers, MPR changes, both decimal formulas) on both cores, traces compared
   boundary by boundary. Identical.
2. **SingleStepTests `huc6280`** (the corpus huc6280-python's `scripts/fetch_references.py` pins;
   it is already fetched under `/data/emu/huc6280-python/reference/`, read it in place or fetch it
   by the same pin): all 640,000 cases under the same masks huc6280-python's rung 1 counts
   (`tests/single_step.py`, `scripts/run_single_step.py`: C1 `ST0-2`, C2 BRK bytes, C4 `$63`,
   truncated block transfers, and the decimal cases under each formula). The Rust core must give
   **the same pass/mask counts as the Python core**, case for case; report both.
3. **Dormann's 65C02 extended opcodes test, relocated**, natively to its success trap, the binary
   built as huc6280-python's `scripts/run_dormann.py` builds it (cc65 at
   `~/.local/opt/cc65-2.19`); same instruction count as the Python run.
4. **Interrupts in lockstep**: the three lines, `irq_disable`, the poll cases (`CLI`/`SEI`/`PLP`,
   the last-cycle change, each `irq_poll_cycle` mode), the vanished interrupt in each mode, reset,
   interrupts around block transfers and `CSL`/`CSH`, as traced scenarios including seeded random
   line changes, diffed against the reference.
5. **A real program**: where MAME traces are present in
   `/data/emu/huc6280-python/tests/mame_traces/` (they may not be: they are large and gitignored),
   replay a game as `tests/mame_replay.py` does and get the Python core's own result; else lockstep
   the two cores on a synthetic long program (a few million instructions) with the bus answering
   from a seeded generator on both sides. Say which was done.
6. **Seeded faults**: at least 12 single-line faults planted in the Rust core (one per module at
   least), each caught by some rung; a fault that changes no behaviour is not a fault (6502-rust's
   lesson). Report which rung caught each.

The reference runs under PyPy (`~/.local/opt/pypy3.11-v7.3.20-linux64/bin/pypy3.11`; there is no
`pypy3` on PATH; set `PYPY_GC_NURSERY=1M` for parallel runs) from a fetched checkout in
`external/` (`scripts/fetch_huc6280python.sh`, by the pinned commit).

CI (`.github/workflows/ci.yml`, kept small: main and pull requests, concurrency cancel): `cargo fmt
--check`, `cargo clippy --release --all-targets -- -D warnings`, `cargo test --release`, and rungs
1, 3 where feasible, 4, and a sample of rung 2. The local `cargo clippy` may be an old toolchain
that cannot build: CI is where clippy runs; write clippy-clean code regardless.

## Deliverables

- The crate, the scripts, a README (what it is, the pin, the ladder with the measured numbers,
  the seeded faults, the Rust-forced differences, speed in instructions per second against the
  Python core on PyPy), `docs/handoff-brief.md` (this file, with a "What was built" section
  appended), CI.
- Commits at good points, locally. **Do not create a GitHub repository and do not push**: the
  session that started you does that after reading your report.

## Working rules

- Do not modify huc6280-python, 6502-python or 6502-rust. If the reference looks wrong, stop and
  show the divergence; the vectors are the authority, not either core.
- Transcribe; do not redesign. Where Rust forces a difference, say so in the module header.
- **No GPL or LGPL code, copied or transcribed** (Aubrey, 2026-11-12; pcengine-python
  `docs/provenance.md`). The only source you transcribe is huc6280-python, which is ours and MIT.
  The trees under `reference/` of the Python repositories (Mednafen, Mesen2, Geargrafx, MAME, the
  MiSTer core) are not to be opened for this work. Nothing third-party is committed (vectors and
  test binaries are fetched by script or read in place).
- No dangerous-looking shell one-liners (`rm -rf`, `sed -i`, `ln -sfn`): use small Python helpers.
- `nice`, at most about 12 processes on the shared machine; temp files only under `external/`,
  `target/` or a scratch directory you make inside the repository (gitignored).
- Commit trailer: `Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>`.
- Report at the end: the rung results with numbers, the faults table, the Rust-forced differences,
  anything in the Python core that looked wrong, and what `pcengine-rust` should know about the
  crate's API (the `Bus` trait, how a board delivers interrupt lines and reads `high_speed`).

## What was built

Written 2026-11-12 by the session that did the work (Claude Fable 5.2), for
whoever picks the family's order up at step 2 (`pcengine-rust`).

**The crate.** `huc6280` (package `huc6280-rust`), twelve modules, each the
transcription of the Python module of the same name at `v0.3.0`
(`src/core.rs` is `_core.py`, and so on), on `sixfiveohtwo` at `71747eb` by
its GitHub URL. Every named switch is a public field with the reference's
default. `src/dispatch.rs` is generated from the reference's `OPCODES`
(`scripts/make_dispatch.py`). Not transcribed: `debug.py`, `console.py`,
`disasm.py`, `__main__.py`; of `harness.py` only what the MAME replay uses,
in `src/bin/replay.rs`.

**The ladder**, all six rungs passing at the commit this section went in with
(numbers in the README's "Certification"):

| Rung | Script | Judge |
| --- | --- | --- |
| 1 | `scripts/rung1.sh` | the reference's trace comparison on generated directed scenarios; the arithmetic tables by digest |
| 2 | `scripts/rung2.sh` | the 640,000 SingleStepTests cases on both cores, verdict and masks compared case for case, under both decimal formulas |
| 3 | `scripts/rung3.sh` | Dormann's relocated test to its success trap; the reference's instruction count |
| 4 | `scripts/rung4.sh` | the reference's trace comparison on the interrupt workout, random and swept, in every poll and vanished-interrupt mode |
| 5 | `scripts/rung5.sh` | six MAME traces replayed as the reference replays them (they were present), report and digest compared; and a synthetic program by digest, which is all CI has |
| 6 | `scripts/rung6.sh` | 24 seeded faults, each caught |

**Decisions made here that the brief left open.**

- *What a bus callback gets.* Not `&mut Frame` as in 6502-rust but `&mut Pins`
  (the frame, the three lines, the disable register, `high_speed`, the
  cycle's kind, the opcode): everything the reference's host reaches through
  `cpu.` from inside a callback. `pcengine-rust` sets the lines with
  `pins.set_irq_lines(irq1, irq2, tiq)`, writes `$1402` with
  `pins.set_irq_disable(value)`, and reads `pins.high_speed` for the length of
  the cycle it is in. The README's "For a board author" is the whole surface.
- *The scenarios are generated, not committed* (`scripts/make_scenarios.py`
  writes them to `external/scenarios/`; they are seeded and the large ones
  are 3 MB each). `conformance/README.md` is the format.
- *Rung 2 compares verdicts, not only totals*: `scripts/reference_sst.py`
  and `sst --verdicts` each write a line per case.
- *Rung 5 did both* things the brief offers: the MAME replay, because the
  traces were there, and the synthetic program, because CI has no traces.
- *Dormann's source* (GPL-3.0, a test program) is fetched by
  `scripts/fetch_dormann.sh` into the reference checkout under `external/`
  and built there by the reference's own scripts; it was not opened.

**Found in the reference** (the README's "Notes on the reference" has the
five items and `scripts/reference_notes.py` shows the first three on the
reference). None is a divergence: the Rust
core reproduces each. Three are about interrupt lines changing in an
instruction's last cycle (the vanished-interrupt path skipping the frame's
deferral; a swap of sources that leaves `IRQ_LINE` unchanged; `_lines_before`
missing from `CPUState`), and they share a cause: the three lines live beside
the frame instead of in it. If huc6280-python changes any of them, this crate
follows by re-transcription and the rungs say whether it did.

**A trap in the fault runner, for the next transcription.** Building every
faulted tree into one target directory makes Cargo treat them as one package
and decide by modification times, and `copytree` keeps the originals' times:
a tree with no freshly written file silently reuses the previous tree's
build. `scripts/seeded_faults.py` rewrites a file in every tree, the
unfaulted one included, and refuses to go on unless the unfaulted tree passes
every rung.

**Not done.** No GitHub repository, no push (as instructed). `cargo clippy`
could not run locally (the installed clippy is 1.78, the crate needs 1.85):
the code was written to be clippy-clean and CI is where that is checked.
`pcengine-rust`, the speed fork and play-pce are the next steps of the order
above.
