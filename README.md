<!-- SPDX-License-Identifier: MIT -->
# huc6280-rust

A Rust **Hudson Soft HuC6280** CPU core (the processor of the PC Engine, the
TurboGrafx-16, the SuperGrafx and the CD-ROM systems) transcribed from
[huc6280-python](https://github.com/alewman/huc6280-python) (built from the
tag `v0.3.0`, carried to `v0.4.0` and to its clean rewrite `6ce348e`) and proven equivalent to it at
every processor boundary:
boundary kind, cycles, every bus cycle in order (address, value, kind), and
all of `CPUState`, not merely "passes the functional test at the end". It
builds on [6502-rust](https://github.com/alewman/6502-rust) (crate
`sixfiveohtwo`) exactly as huc6280-python builds on 6502-python's
`CPUFrame`. It is the CPU of the Rust PC Engine board,
[pcengine-rust](https://github.com/alewman/pcengine-rust), and so of its
add-ons [supergrafx-rust](https://github.com/alewman/supergrafx-rust) and
[pcenginecd-rust](https://github.com/alewman/pcenginecd-rust);
[huc6280-rust-fast](https://github.com/alewman/huc6280-rust-fast) is its
speed fork. MIT licensed. Built with Claude (Claude Fable 5.2, through Claude
Code) for alewman, and prepared for publication by Claude Opus 5.5.

The crate is `huc6280` (package `huc6280-rust`).

## Status

**2026-11-19: built (2026-11-12) from huc6280-python `v0.3.0`; carried to
`v0.4.0` (issue 0005) on 2026-11-13 and to the reference's clean rewrite
`6ce348e` on 2026-11-19 ([docs/restart.md](docs/restart.md)); all six rungs
pass against `6ce348e`.**
Every rung compares this core with the reference, huc6280-python `6ce348e`, which is itself judged
against Hudson's manuals, SingleStepTests, Dormann's test, MAME, Mednafen and
a VHDL model (its [docs/validation.md](https://github.com/alewman/huc6280-python/blob/main/docs/validation.md)).
Results (details under "How it is proven"):

| Rung | What | Result |
| --- | --- | --- |
| 1 | Directed trace lockstep: every opcode under three sets of switches, dependent sequences, ADC/SBC result tables | 5 of 5 scenarios identical (37,356 boundaries, every cycle); 2,097,152 table entries identical |
| 2 | SingleStepTests huc6280, all 640,000 cases, verdicts compared case by case | the same counts on both cores, 0 cases judged differently, under both decimal formulas |
| 3 | Dormann's 65C02 extended opcodes test, relocated | success at `$5675` after 20,936,652 instructions, the reference's line |
| 4 | Interrupts in lockstep, random and swept, in every poll and vanished-interrupt mode | 38 of 38 identical (510,400 boundaries; the withdrawn modes' 27 files are gone, 4 sweeps under `Drop` added) |
| 5 | MAME 0.285 traces of six games replayed; a synthetic program by digest | 6 of 6 reports and digests identical (21,013,980 instructions); synthetic digests identical |
| 6 | Seeded faults | 40 caught, 0 missed (27 kept, 13 on the rewrite's code; one equivalent fault replaced) |

Every rung and `cargo test` (20 directed tests, 6 of them issue 0005's and 3
added with the rewrite, mirroring the reference's, among them a capture and
restore before every step; and the replay tool's mirroring test) were run on
2026-11-19 against `6ce348e` (local then; on GitHub since 2026-11-20). Rungs 1 to 3 and
rung 5 (the six MAME replays and both synthetic runs, the alternate one now
with `Drop` for the withdrawn `Latched`) print exactly what they printed
against `v0.4.0`. **Nothing here was run on a real
console**, and there is no hardware-captured per-instruction data or die model
of this chip: the reference's judges are emulator tier, and this crate is
proven equal to the reference, not to the chip. Not covered: the reference's
debugger, console, disassembler and command line (Python-side tooling).

## Using it

```toml
[dependencies]
huc6280 = { package = "huc6280-rust", git = "https://github.com/alewman/huc6280-rust", rev = "<commit>" }
```

(Pin the commit you have checked; the crate is not on crates.io.) Rust 1.85
or newer.

```rust
use huc6280::{Bus, HuC6280, Pins};

struct Board { ram: Vec<u8> /* , the VDC, the timer, ... */ }

impl Bus for Board {
    fn read(&mut self, address: u32, _pins: &mut Pins) -> u8 { self.ram[address as usize] }
    fn write(&mut self, address: u32, value: u8, pins: &mut Pins) {
        if address == 0x1FF402 { pins.set_irq_disable(value); } // the board decodes $1402
        self.ram[address as usize] = value;
    }
    fn idle(&mut self, _address: Option<u32>, _pins: &mut Pins) {}
}

let mut cpu = HuC6280::new(Board { ram: vec![0; 0x200000] });
cpu.request_reset();
let cycles = cpu.step(); // the start sequence, one bus call per cycle
assert_eq!(cycles, 7);
```

This example is compiled and run by `cargo test` (as a doctest), as is the
one in `src/lib.rs`.

### For a board author

- **The bus.** `Bus` has three methods and the core calls exactly one of them
  per CPU cycle, in the chip's order: `read(address, pins) -> u8`,
  `write(address, value, pins)` and `idle(address, pins)`, which is a cycle
  with no data transfer: `Some(address)` for a dummy read the chip makes and
  discards, `None` for an internal cycle (the default `idle` does nothing).
  Addresses are **21-bit physical** (`u32`): the mapper (`cpu.mpr`, eight
  registers) is in the core, as are `ST0`/`ST1`/`ST2`'s fixed addresses
  (`$1FE000`, `$1FE002`, `$1FE003`). `step()` returns how many cycles, and
  so callbacks, it made. A block transfer is one `step()`.
- **`Pins`.** A callback cannot borrow the CPU that is calling it, so every
  callback gets `&mut Pins`, the part of the CPU a device may see or change
  mid-cycle; between steps the same thing is `cpu.pins`.
  - `pins.high_speed`: the clock switch (`false` 1.79 MHz, 12 master clocks a
    cycle; `true` 7.16 MHz, 3) **for this cycle**: `CSL`/`CSH` change it
    before their own last cycle (C7), reset selects low. Also
    `cpu.high_speed()`.
  - `pins.kind()` (`Kind::Read`, `Write`, `Dummy`, `Idle`) and `pins.opcode()`
    (the instruction being executed, set once its fetch has returned; the
    accessor the SuperGrafx board uses). Also `cpu.kind()`,
    `cpu.opcode()`.
- **The three interrupt lines** are levels the board holds:
  `pins.set_irq_lines(irq1, irq2, tiq)` sets all three at once (IRQ1 the
  VDC's, IRQ2 the HuCard connector's and the CD-ROM's, TIQ the timer's), from
  inside a callback or between steps (`cpu.set_irq_lines`);
  `pins.irq_lines()` reads them back. A line changed during an instruction's
  last cycle is seen one instruction later (`irq_poll_cycle`'s default).
  Taking an interrupt does not clear its line: the device lets go when its
  handler acknowledges it. Do not call the frame's
  `request_maskable_interrupt` yourself: its `IRQ_LINE` is derived ("an
  enabled source is asserted").
- **The disable register** (`$1402`, bit 0 IRQ2, bit 1 IRQ1, bit 2 TIQ,
  1 = masked): the board decodes the address and calls
  `pins.set_irq_disable(value)`; `pins.irq_disable()` reads it. Reset clears
  it. The timer and the `$1402`/`$1403` read values are the board's.
- **NMI and reset** are the frame's: `cpu.request_non_maskable_interrupt()`,
  `cpu.request_reset()`, or `pins.frame.request_*()` in a callback.
- **Registers** are plain fields: `a x y s` (`u8`), `pc` (logical, `u16`),
  `p` (`u8`: T is bit 5, B is never stored), `mpr` (`[u8; 8]`);
  `cpu.physical(logical)` maps an address. `cpu.bus` is the board.
- **State.** `cpu.capture_state() -> CpuState` (a `Copy` struct: the
  registers, `mpr`, `high_speed`, `mpr_latch`, the three lines, the disable
  register, the frame's poll fields and `polled_irq_lines`) and
  `cpu.restore_state(&state) -> Result<(), String>`, which refuses a state
  the reference's constructor would (`irq_disable` or `polled_irq_lines`
  above 7, B set). `polled_irq_lines: Option<u8>` (since `v0.4.0` of the
  reference, issue 0005) is the lines the last poll saw when one changed
  after it, in the instruction's last cycle, else `None`; a board that
  serializes `CpuState` gains that one field, and a state saved without it
  loads as `None`, which is what every state meant before. Memory and devices
  are the board's.
- **The named switches** are public fields with the reference's defaults:

  | Field | Default | Other values |
  | --- | --- | --- |
  | `decimal_formula` (C13) | `DecimalFormula::Mednafen` | `Mesen` |
  | `sbc_honours_t` (C5) | `false` | `true` |
  | `irq_poll_cycle` (U25) | `IrqPollCycle::SecondToLast` | `Last` |
  | `vanished_interrupt` (C18) | `VanishedInterrupt::Mesen2` | `Drop` |
  | `tma_multiple` (C6) | `TmaMultiple::Or` | `Highest`, `Lowest` |
  | `tsb_z_from_test` (C12) | `false` | `true` |
  | `zero_page_pointer_wraps` (U8) | `true` | `false` |
  | `block_reads_hardware_page_as_zero` (C9) | `true` | `false` |
  | `nop_63_cycles` (C4) | `2` | any count from 2 |
  | `interrupt_entry_cycles` (C3) | `8` | `7` |

  `IrqPollCycle::Mednafen` and `VanishedInterrupt::Latched` were removed in
  the clean rewrite ([docs/restart.md](docs/restart.md)), as the reference
  withdrew them; under `Last`, `Drop` is what `Latched` was.

  The U and C numbers are the entries of huc6280-python's
  [`docs/undocumented-behavior.md`](https://github.com/alewman/huc6280-python/blob/main/docs/undocumented-behavior.md),
  which says what each is and how well it is known.

`tests/core.rs` shows each of these in a few lines.

### Layout

Each `src/*.rs` file is the transcription of one `huc6280/*.py` module, with
the same handler names, the same 256-row dispatch table (generated from the
reference's by `scripts/make_dispatch.py`) and the hardware comments carried
over: `core` (`_core.py`), `alu`, `bits`, `block`, `control`, `hudson`,
`loads`, `shifts`, `dispatch`, `cpu`, `state`, `trace`. A trace written here
carries each instruction's address and bytes, and the reference's reader
disassembles them. `src/bin/` holds the rungs' tools (`alu`, `dormann`,
`long`, `replay`, `sst`, `trace`), among them the part of the reference's
harness that the MAME replay needs.

## How it is proven

| What | Version |
| --- | --- |
| huc6280-python (the reference) | `6ce348e13c9ef3009bd07e7938c4b48956eeadc6`, the clean rewrite after `v0.4.0` (which pins 6502-python at `2146eba`) |
| 6502-rust (`sixfiveohtwo`) | `71747eb9036702109fb206eb9fe88174a9dcc989`, the pin a2600-rust uses |
| Trace schema | version 1 (6502-python `docs/trace-schema.md`), with the reference's `HuC6280Record` |
| SingleStepTests huc6280 | revision `4ce378e04e243437e6f74dfe76c3cb7c70e45bf1` (MIT; Mesen-generated: a detector, not a judge) |
| Klaus Dormann's 65C02 extended opcodes test | `Klaus2m5/6502_65C02_functional_tests` at `7954e2db`, source SHA-256 `72b1f57d…2f76736`, relocated by the reference's `scripts/dormann_to_ca65.py`, assembled with cc65 2.19 |

Each rung reproduced with the script named, on Linux x86_64, rustc 1.93.1,
the reference on PyPy 7.3.20 (Python 3.11):

| Rung | What | Result | Script |
| --- | --- | --- | --- |
| 1 | Trace lockstep, short: every opcode 40 times under each of three sets of switches (random registers, T and D both ways, random mapping registers, zero-page pointers at `$FF`, TMA/TAM masks of no, one and many bits, block transfers across the edges of the zero-read range and one of 65,536 bytes), and sequences where an instruction depends on the one before (SET or PLP then each T instruction, CLI/SEI/PLP, CSL/CSH); every cycle compared. And ADC's and SBC's whole result tables under both decimal formulas | 5 of 5 scenarios `traces are identical` (37,356 boundaries, 2,796,641 cycles, all 256 opcodes); 2,097,152 table entries identical | `rung1.sh` |
| 2 | SingleStepTests huc6280, all 640,000 cases, judged as the reference's `tests/single_step.py` judges them, the verdict and the masks of every case compared between the two cores | below: the same counts, 0 cases judged differently | `rung2.sh` |
| 3 | Dormann's 65C02 extended opcodes test, relocated, natively | `success at $5675 after 20936652 instructions (test case 240)`, the reference's line | `rung3.sh` |
| 4 | Interrupts in lockstep: a workout (the three lines with their vectors, `irq_disable` written through the bus, NMI, BRK, reset, CLI/SEI/PLP, a handler that acknowledges the timer in its last cycle, SET, a block transfer and CSL/CSH in the main loop) under seeded random line changes, masks, NMIs, resets and state restores between steps and inside bus cycles, in all four combinations of `irq_poll_cycle` and `vanished_interrupt` and with 7-cycle entries; and one change swept across every cycle of the workout's first 200 | 38 of 38 files `traces are identical`: 510,400 boundaries (14 random runs, 280,000; 24 sweep files, 3,840 swept scenarios, 230,400). Before the rewrite, in all nine combinations: 65 of 65, 925,600 | `rung4.sh` |
| 5 | A real program: MAME 0.285's traces of six games replayed as the reference's `tests/mame_replay.py` replays them, the report and a digest of the core after every step compared. And a synthetic program on a seeded bus (every cycle, what the pins show in it, and every state folded into a digest) | 6 of 6 reports and digests identical, 21,013,980 instructions; synthetic: 3,000,000 boundaries (196,517,500 cycles) and 1,000,000 with the alternate switches (66,666,983 cycles; 66,633,861 at `v0.3.0`), digests identical at every checkpoint | `rung5.sh` |
| 6 | Seeded faults: one single-line fault at a time in a copy of `src/`, run through the rungs cheapest first until one fails; every module has at least one | 40 caught, 0 missed (27 kept, 13 on the rewrite's code; one equivalent fault replaced) ([docs/seeded-faults.md](docs/seeded-faults.md)) | `rung6.sh` |

**Rung 2, both cores side by side** (`scripts/compare_sst.py`; the masks are
the reference's readings of the corpus, explained in its
`tests/single_step.py` and in `src/bin/sst.rs`):

| | Cases | Passed | Failed | `st_write` | `truncated` | `block_unknown` | `block_self_overlap` | `block_final_unexplained` | `tma_latch` | `tsb_z` | `decimal_formula` |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| huc6280-python, default formula | 640,000 | 640,000 | 0 | 7,500 | 12,481 | 11,177 | 5,031 | 16 | 13 | 1,492 | 1,552 |
| huc6280-rust, default formula | 640,000 | 640,000 | 0 | 7,500 | 12,481 | 11,177 | 5,031 | 16 | 13 | 1,492 | 1,552 |
| huc6280-python, `mesen` | 640,000 | 640,000 | 0 | 7,500 | 12,481 | 11,177 | 5,031 | 16 | 13 | 1,492 | 0 |
| huc6280-rust, `mesen` | 640,000 | 640,000 | 0 | 7,500 | 12,481 | 11,177 | 5,031 | 16 | 13 | 1,492 | 0 |

Under each formula all 640,000 verdict lines (index, pass or fail, the masks
used) are the same on both cores.

**Rung 5** used the MAME traces the reference records (its
`scripts/mame_trace.py`; large, made from the user's ROMs, never committed):
Blazing Lazers (USA), Bomberman '94 (Japan), Bonk's Adventure (USA), Final
Soldier (Japan), Gradius (Japan) and R-Type I (Japan), 3.3 to 3.8 million
instructions each. The report is the reference's own (Gradius: 3,458,889
instructions, 234 IRQ1 and 4,555 timer interrupts, 0 register divergences,
3,454,098 of 3,454,099 instruction lengths equal to MAME's); the rung is that
the Rust core prints it too. Where the traces are absent (CI) the rung is the
synthetic program alone.

**Rung 6** caught 11 faults with the SingleStepTests sample, 7 with rung 1,
21 with rung 4 and 1 with rung 5's synthetic program; 29 got past the
SingleStepTests sample, most of them out of that corpus's reach by
construction (it runs one instruction, with no interrupts, no hardware page
and no hidden state). The table and the reasoning (and the one equivalent
fault found and replaced) are in
[docs/seeded-faults.md](docs/seeded-faults.md).

### Reproducing

```text
scripts/fetch_huc6280python.sh          # huc6280-python 6ce348e into external/, with a venv
scripts/fetch_vectors.sh                # SingleStepTests huc6280 into external/huc6280-sst (185 MB)
scripts/fetch_dormann.sh                # Dormann's source, where the reference's build looks for it
scripts/rung1.sh                        # QUICK=1 for the smaller set; JOBS=n parallel scenarios
scripts/rung2.sh [CORPUS_DIR]           # default external/huc6280-sst/v1; LIMIT=n cases an opcode
scripts/rung3.sh                        # needs cc65 (ca65, ld65) on the PATH
scripts/rung4.sh                        # QUICK=1 for the smaller set
scripts/rung5.sh [TRACES_DIR]           # default ../huc6280-python/tests/mame_traces
scripts/rung6.sh [CORPUS_DIR]           # after rung3.sh
cargo test --release                    # the directed tests and the doctests
```

What each needs: Rust 1.85 or newer; Python 3 for the scripts (standard
library only); the reference in a venv (PyPy runs rungs 2, 4 and 5 several
times faster: `PYTHON=/path/to/pypy3 scripts/fetch_huc6280python.sh`; the
script's default is the author's PyPy, and it falls back to `python3`); cc65
2.19 for rung 3 (`reference_dormann.py` also looks under `~/.local/opt`, the
author's layout). Rung 5's MAME traces are made by the reference's
`scripts/mame_trace.py` from your own ROMs and MAME 0.285, in a checkout of
huc6280-python beside this one, or anywhere named by `TRACES_DIR`.
[conformance/README.md](conformance/README.md) describes the lockstep
scenarios, which `scripts/make_scenarios.py` generates (seeded, so the same
every time) into `external/`.

CI's rungs job fetches the reference from GitHub, so it fails until
huc6280-python `6ce348e` is pushed there; until then fetch it
from a local clone (`scripts/fetch_huc6280python.sh "" /path/to/huc6280-python`).
CI (`.github/workflows/ci.yml`, main and pull requests) runs `cargo fmt
--check`, `cargo clippy -D warnings` and `cargo test`, and, against the
reference fetched from GitHub, rung 2 on a sample of 100 cases an opcode,
rung 3, rungs 1 and 4 on the quick scenario set and rung 5's synthetic
program.

## Where Rust forced a difference

Each module's header says so; together:

- **The bus and `Pins`** (`core.rs`). The reference's host passes three
  callables and reaches the CPU from inside them. Here the bus is a trait the
  board implements, and what a callback may reach is the `Pins` struct it is
  handed: the frame, the three lines and the disable register, `high_speed`,
  the cycle's kind and the opcode. `cpu.high_speed` in Python is
  `cpu.pins.high_speed` (or `cpu.high_speed()`).
- **Confirming a request** (`core.rs`). The reference's frame swaps its bus
  callables for confirming wrappers when a request changes; here every cycle
  calls `Frame::access` first (6502-rust's difference, inherited).
- **Mixins** (`cpu.rs`). The nine mixins are `impl` blocks, one a module; the
  frame is a field reached through 6502-rust's `Part` trait, whose provided
  `step()` is the reference's.
- **Dispatch** (`dispatch.rs`). The reference resolves the table into bound
  methods; here it is the table and two matches. A handler that takes a mode
  calls `self.ea(mode)` where the reference calls `mode(self)`.
- **Switches.** Class attributes are fields; the string-valued ones are
  enums, so the reference's `ValueError` for an unknown name (or a withdrawn
  one: `"latched"`, `"mednafen"`) has no counterpart.
- **Arithmetic** (`alu.rs`). The decimal digit arithmetic is in `i32`, where
  Python's integers go negative or past a byte; `as u8` is Python's `& 0xFF`.
- **State** (`state.rs`). The field widths are the types', so
  `CpuState::validate` checks only what is left: `irq_disable` is three bits
  and B is clear.
- **No halted state.** `Part::halted_cycle` is `unimplemented!`, as the
  reference's `_halted` raises.

### What 6502-rust's frame would need to make this cleaner

Nothing here needed a change to 6502-rust; these are the places where this
crate works around the frame rather than with it (the reference does the same
around `CPUFrame`):

1. **Several maskable sources.** The frame has one `IRQ_LINE`. The three
   lines, their mask and what the poll saw of them (`lines`,
   `lines_at_access`, `polled_lines`, `poll_lines`) are kept beside it, and
   `set_irq_lines` arms the frame's `deferred` watch by hand when a source is
   swapped for another and `IRQ_LINE` does not change. A frame that carried a
   part's auxiliary request bits through `deferred` and `polled` would remove
   all of that.
2. **An entry that declines.** `Part::step` returns whatever
   `enter_interrupt` returns, so an entry that finds its source gone has to
   run the instruction itself and repeat the bookkeeping `step` does after
   `execute` (as the reference does since issue 0005). `enter_interrupt`
   returning `Option<u32>` (none: run the instruction) would keep that path
   the same as any other.
3. **The poll point.** `irq_poll_cycle` rewrites the frame's `polled` before
   calling its `boundary_interrupt`. A frame with the poll point as a setting
   (second-to-last cycle or last) would own that.
4. **`halted_cycle`** is a required method of `Part`; a default that panics
   would spare a part with no halted state.
5. **The bus context.** 6502-rust's own `Bus` hands a callback `&mut Frame`;
   a part with more inputs needs its own context type (`Pins` here). That
   works as it is; a note in 6502-rust's README that this is the intended
   shape would have saved a decision.

## Speed

Klaus Dormann's relocated test (20,936,652 instructions on flat RAM, no
tracing), one core of a busy 32-thread machine, best of five:

| Core | Time | Instructions per second |
| --- | --- | --- |
| huc6280-rust (rustc 1.93.1, `--release`) | 0.173 s | 121 million |
| huc6280-python on PyPy 7.3.20, warmed up (third run in one process) | 1.13 s | 18.5 million |
| huc6280-python on PyPy 7.3.20, a fresh process | 3.36 s | 6.2 million |

About 6.5 times the reference at its best and 20 times a fresh run of it. A
MAME replay (Gradius, 3,458,889 instructions, reading the 160 MB log
included) takes 2.1 s here and 19.8 s on the reference. This crate is the
plain transcription: correct first, and the place to measure a speed fork
against.

## Notes on the reference

Nothing found here was a divergence: the Rust core did what huc6280-python
v0.3.0 did in each case. Five places where the reference's behaviour looked
unintended while it was being transcribed were filed in huc6280-python as
[issue 0005](https://github.com/alewman/huc6280-python/blob/main/docs/issues/0005-interrupt-edge-cases-found-by-the-rust-port.md),
**settled in v0.4.0** (each fixed against the reference's own stated rule;
no switch added, no default changed), and carried here.
`scripts/reference_notes.py` shows the first three on the reference itself;
its docstring has what v0.3.0 printed:

1. **A vanished interrupt's instruction skipped the last-cycle rule.** When
   an entry found its source gone, the instruction it ran returned past the
   frame's bookkeeping after `execute`, so a line raised in its last cycle
   was taken one boundary early. Now the vanished path moves a pending
   `deferred` to `polled` as `step` does.
2. **A swap of sources in the last cycle was taken at once.** With TIQ up
   at the poll, TIQ dropping and IRQ1 rising in the last cycle left the
   frame's `IRQ_LINE` unchanged, so IRQ1 was entered although it rose after
   the poll. Now the core keeps the lines as the poll saw them
   (`polled_lines`, from the lines at the last access) and any change of the
   lines arms the frame's watch.
3. **Capture and restore was not transparent at one kind of boundary.**
   What the poll saw of the lines was not in `CpuState`. Now it is,
   `polled_irq_lines`; a capture and restore before any step changes nothing
   (`tests/core.rs`, `issue_0005_a_round_trip_at_any_boundary_changes_nothing`).
4. **BRK polled.** `CPUState`'s docstring said BRK does not poll; it now
   does not (`deferred = NO_POLL`, as an entry and as on the 6502).
5. In the test tooling, the MAME replay's `_take_interrupt` returned with a
   line asserted when the core's PC was already at the handler; it now
   checks first, and its forced path also clears the polled lines
   (`src/bin/replay.rs` follows).

## Provenance

No GPL or LGPL source was copied, read or transcribed for this crate. The
only source transcribed is huc6280-python, which is ours and MIT. Its
provenance audit (2026-11-13) found that some of its rules had followed GPL
source (its `docs/provenance.md`), and on 2026-11-19 it rewrote them clean:
the decimal ADC/SBC from closed-box result tables, the vanished-interrupt and
poll-point switches re-established from closed-box runs (two modes withdrawn),
the comments re-cited (its `docs/restart.md`). This crate carried that rewrite
the same way: the Rust functions that had transcribed the replaced code were
moved aside unread and transcribed anew from the new Python
([docs/restart.md](docs/restart.md); verdicts in
[docs/provenance.md](docs/provenance.md)). Emulators are named in the
comments as the reference names them: as closed boxes that were run, or
permissive sources (MAME's BSD-3 files, ares), not as sources of code. The
MAME replay tool's HuCard mirroring is the reference's port of MAME's BSD-3
`rom_map_setup`; its notice is in [NOTICE.md](NOTICE.md). Dormann's test
(GPL-3.0, a test program) is fetched and built in the reference's checkout by
its own scripts; it is not in this repository.

[docs/handoff-brief.md](docs/handoff-brief.md) is the brief this crate was
built from, kept as written, with what was built appended (it predates the
rewrite and still names the withdrawn modes).

## Licence

MIT (see [LICENSE](LICENSE)). Nothing third-party is committed: the vectors,
Dormann's test and the MAME traces are fetched or read in place.
