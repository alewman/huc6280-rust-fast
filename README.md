# huc6280-rust-fast

The speed fork of [huc6280-rust](../huc6280-rust), the Rust Hudson HuC6280 core (itself
transcribed from, and proven equivalent to,
[huc6280-python](https://github.com/alewman/huc6280-python) `v0.3.0`), forked at `7f49bf8` and
**held in lockstep with it**: every change must leave every rung of the base core's ladder
identical (the traces against huc6280-python, all 640,000 SingleStepTests verdicts, Dormann, the
interrupt lockstep, the MAME replays), keep pcengine-rust-fast's 275 lockstep cases and its
save-state round trips identical, and measure faster, or it is not kept. The base core is the
readable one; this one is for play-pce, and above all for its wasm build on phones.

**The crate is still `huc6280` and its public API is the base core's**: the `Bus` trait and its
three methods, `Pins` (its fields, `frame` included, and its methods), `HuC6280::new` and `step`,
the registers, `capture_state`/`restore_state`/`CpuState`, the switches with their defaults, the
dispatch table, and the frame's types re-exported from 6502-rust as before. Nothing was added. A
board or an add-on changes one line:

```toml
huc6280 = { package = "huc6280-rust-fast", path = "../huc6280-rust-fast" }
```

pcengine-rust-fast, supergrafx-rust and pcenginecd-rust build and pass on this crate with that
line changed and nothing else (below, "The add-ons").

## Speed

Measured directly, the fork point's build against this one, alternated on one pinned efficiency
core by CPU time (best of 12 a side natively, of 8 under wasm), the checksums of every picture,
every sample and the final snapshot equal on both sides:

| Title (600 frames, the board's picture-gate input) | fork point | now | | fork point, wasm | now, wasm | |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Blazing Lazers | 821.0 | 904.2 | x1.101 | 482.8 | 497.2 | x1.030 |
| Bonk's Adventure | 1005.7 | 1044.8 | x1.039 | 572.1 | 579.1 | x1.012 |
| R-Type | 921.2 | 975.8 | x1.059 | 507.8 | 512.0 | x1.008 |
| After Burner II | 818.4 | 894.0 | x1.092 | 468.4 | 477.2 | x1.019 |
| Street Fighter II' | 836.2 | 872.4 | x1.043 | 416.3 | 420.8 | x1.011 |
| all five (geometric mean) | | | **x1.067** | | | **x1.016** |

Frames per second of pcengine-rust-fast (`pce-bench`, its fat-LTO profile) on a scratch copy built
against each core. Under wasm: the board's `wasm-bench` under Node 22 (`scripts/ab_wasm.py`, 1,200
frames).

| Other case | fork point | now | |
| --- | ---: | ---: | ---: |
| **A CD game**: Ryuuko no Ken on pcenginecd-rust (System Card 3.0, Arcade Card, 1,500 frames from power-on, fat LTO as play-pce builds) | 187.0 fps | 199.3 fps | **x1.065** (x1.039 in a first run; Mad Stalker x1.036) |
| **The core alone**: Dormann's relocated 65C02 test, 20,936,652 instructions on flat RAM (this crate's own profile, no LTO) | 0.210 s (99.7 M/s) | 0.152 s (137.7 M/s) | **x1.388** |

| Kept, in order | board, five titles | wasm | CD case | Dormann |
| --- | ---: | ---: | ---: | ---: |
| **The dispatch from constant rows, and the cycle primitives inlined into every handler.** `execute_sequence` matches on the opcode; each of the 256 arms reads its row of `OPCODES` at compile time and runs that handler with its addressing mode built in, and `cycle_read`, `cycle_write`, `dummy`, `io` and `fetch` are `inline(always)`, so an instruction is one stretch of code with one jump into it. The table stays the one place a handler and a mode are written | x1.078, x1.048 | x1.000 | x1.097 | x1.043 |
| **The usual boundary in one test.** `step` tests reset, halted, the requests and what the last poll left (`polled`, `polled_i`) together and, when all are clear, runs the instruction and the frame's `deferred` bookkeeping itself, as `Part::step` would; any other boundary is `Part::step`'s, out of line | x1.023, x1.024 | x1.016 | x1.017 | x1.387 |

Each factor is against the build before it (two board runs each, every title up in all four).
Chained, the steps give x1.07 to x1.10 on the board, x1.016 under wasm, x1.116 on the CD case and
x1.45 on Dormann; measured directly against the fork point (the tables above) they are x1.067,
x1.016, x1.039 to x1.065 and x1.388. A/A on this machine (load 15 to 21 throughout): x0.95 to x1.00 a title
natively, x0.988 to x1.100 a title under wasm, x0.983 on the CD case, x0.989 on Dormann; so the
board's x1.067 is real but the wasm x1.016 is at the edge of what this machine can tell.
Tried and not kept, with their numbers: [docs/not-kept.md](docs/not-kept.md) (the dispatch alone,
and the quiet window below).

**What this does not reach.** On the board the core is now 29% of Blazing Lazers and 27% of Street
Fighter II' (33% and 30% at the fork point), and no line of it is above 3%: what is left is one
call into the board per CPU cycle and one indirect jump per instruction, which the bus contract
(one callback per cycle, one instruction per `step`) fixes. Under wasm the largest single
function is the board's `Bus::read` (about a fifth of a frame), not the core.

## The quiet window: built, measured, not kept (what a board would add)

The brief's opt-in: a gain that needs the board's cooperation, built on the core's side and
measured with a board patched in a scratch copy. Built on the branch `not-kept/quiet-window`
(not merged):

- the core side: `Bus` gains a constant `QUIET` (default `false`, which compiles everything below
  away) and four provided methods: `quiet_window(pins) -> QuietWindow` (at the start of a step:
  how many master clocks of quiet cycles the board lends, and what a cycle costs at each speed),
  `quiet_read(address) -> Option<u8>` and `quiet_write(address, value) -> bool` (inside the
  window, the byte of a page where an access would do nothing but move the byte), and
  `quiet_spent(clocks, pins)` (the quiet cycles paid, before the next call and at the end of the
  step). Inside the window an internal cycle, a dummy read and a lent access are made with no
  call, their clocks added up; `tests/quiet.rs` on the branch runs a model board with a moving
  event horizon, a device page and interrupt changes both ways for 1.6 million steps and finds
  the same calls at the same master clocks with the same pins, the same memory and the same
  state after every step; five seeded faults on the window are all caught by it.
- the board side, what pcengine-rust-fast would add (`scripts/board-quiet.patch`, 45 lines in
  `board.rs`; `scripts/board.sh rev REV quiet` applies it, for revisions of that branch): `QUIET = true`; the window is `wake - mclk` unless a tap is on; a quiet
  read or write is the board's own quiet case (`quiet_page`: work RAM, or the lent `RomCard`
  image); `quiet_spent` moves `mclk` and stores the opcode as every callback does.

Measured against the kept chain's first step (the core with the dispatch from rows): Bonk's
Adventure x1.088, R-Type x1.183, After Burner II x1.081, Blazing Lazers x1.005, but Street
Fighter II' x0.798, x1.022 overall, and under wasm x0.95. Street Fighter II' is on a mapper card
(`Sf2Card`), which the board does not lend, so each step's first program read misses the window,
looks the page up twice and closes the window for the rest of the step; reopening the window
after every call instead cost more than it saved (x0.956 overall). With a device on the
expansion port (the CD unit) the board's horizon is every cycle, so the window is never open and
CD games gain nothing from it. For the window to pay, a board would have to lend the mapper
cards' banks too (the board fork's not-kept list explains why it does not today), and the gain
would be for HuCard games only; on this evidence it was not worth the second path through every
cycle primitive.

## The rule and the gate

A change is kept only if both gates stay green and an A/B by CPU time shows a gain above the A/A
noise; it is then one commit with its factors in the message. A rejected one is a line in
[docs/not-kept.md](docs/not-kept.md). Every commit of this fork passed both gates as a tree of
its own.

- **The core gate**, `scripts/ladder.sh` (about two minutes; `--faults` adds rung 6): `cargo
  test`; rung 1 (the arithmetic tables, the directed and sequence lockstep scenarios); rung 2 (all
  640,000 SingleStepTests cases under both decimal formulas, the verdicts compared case for case
  with the reference's); rung 3 (Dormann, the reference's line); rung 4 (the 65 interrupt
  lockstep files); rung 5 (the six MAME replays and the two synthetic runs). The reference's side
  of each rung never changes, so it is computed once on PyPy and kept under `external/`
  (`external/refcache`, `external/sst/reference-*`, `external/dormann/reference.txt`,
  `external/rung5/*.reference.txt`; the vectors from `scripts/fetch_vectors.sh`, the reference
  checkout and caches copied from huc6280-rust's `external/`).
- **The board gate**, `scripts/board.sh gate`: pcengine-rust-fast's own `scripts/ladder.sh`
  (`cargo test`, the 275 lockstep cases from its reference cache, the save-state round trip and
  run-ahead pass on the 18 picture-gate titles) on `scratch/board`, a copy of pcengine-rust-fast's
  HEAD (`git archive`) whose manifest points at this crate by path, made by `scripts/board.sh
  setup`; nothing is written in ../pcengine-rust-fast. It passed on the untouched fork first.
- **Measuring**: `scripts/board.sh rev REV` builds the board's `pce-bench` and `pce-profile`, the
  CD add-on's bench timed by thread CPU time, Dormann and the wasm bench against any revision of
  this crate, each in its own target directory; pcengine-rust-fast's `scripts/ab.py` compares
  two `pce-bench` builds on its five titles, `scripts/ab_wasm.py` two wasm builds, and
  `scripts/ab_core.py` Dormann or the CD case (the disc extracted with `chdman` into `scratch/`
  for the run and removed after). Profiles: the board's `pce-profile` (SIGPROF sampling, `perf`
  is not allowed here) with its `profile.py`, and the same sampler around Dormann for the core
  alone.

## Where the time goes

| | fork point | now |
| --- | ---: | ---: |
| Blazing Lazers on the board (`pce-profile`): the core and the 65xx frame under it | 33.2% | 29.0% |
| Street Fighter II' on the board | 30.0% | 27.3% |
| the hottest core lines on the board | the dispatch's `match` on the handler 3.8%, the `OPCODES` lookup 2.8%, `physical` 2.2% | `physical` 2.9%, the cycle's first store 2.3%, the opcode `match` 1.7% |
| Dormann alone: the frame's `step` | 7.8% | 3.6% |
| Dormann alone: the hottest lines | the `match` on the handler 10.9%, `physical` 8.9%, the `OPCODES` lookup 7.6% | `physical` 8.5%, the opcode `match` 6.6% and its arms |

(Shares of a smaller whole after: Dormann's samples went from 2,851 to 2,141 for the same 12 runs.)

## Seeded faults

**27 caught, 0 missed** (`scripts/seeded_faults.py`, rung 6): the base core's 24, each by the same
check as below, and three on the fork's shortcuts:

| Fault | Module | What it changes | Caught by |
| --- | --- | --- | --- |
| `row-mode-zpx-as-zp` | `dispatch.rs` | an arm of the dispatch from rows is built with zp for a zp,X row | rung 2 (SingleStepTests sample) |
| `step-keeps-polled-i` | `cpu.rs` | the one-test boundary leaves out `polled_i`, so after CLI, SEI or PLP with nothing requested the frame never consumes it | rung 1 (directed lockstep) |
| `step-drops-deferred` | `cpu.rs` | the one-test boundary forgets a request changed in the last cycle instead of leaving it for the next boundary | rung 4 (interrupt lockstep) |

(`e2-is-cla` and `tst-form` edit the table, which the dispatch from rows reads at compile time:
still caught by the same rungs.)

## The add-ons

On scratch copies of supergrafx-rust (`1d0528a`) and pcenginecd-rust (`96ae3f7`) whose dependency
line names `scratch/board` (pcengine-rust-fast on this crate) and nothing else changed:
supergrafx-rust builds, its 52 tests pass and its 22 lockstep cases are identical from its
reference cache; pcenginecd-rust builds, its 109 tests pass and its 52 lockstep cases (the probe
card on synthetic discs, every switch, and six commercial discs on the System Card) are identical
from its reference cache. (Their reference checkouts, probe cards and synthetic discs were
copied from the add-ons' own `external/` and `scratch/`, nothing written there.)

## The base core

From here on this is huc6280-rust's README at the fork point (`7f49bf8`): what the core is, how
it was certified against the reference, how a board uses it. All of it holds for this fork,
which passes the same rungs; its Speed section is the fork point's.

A Rust Hudson HuC6280 core (the CPU of the PC Engine, the TurboGrafx-16 and
the SuperGrafx) transcribed from
[huc6280-python](https://github.com/alewman/huc6280-python) at `v0.3.0` and
proven equivalent to it at every processor boundary: boundary kind, cycles,
every bus cycle in order (address, value, kind), and all of `CPUState`, not
merely "passes the functional test at the end". The same revision is what
pcengine-python 0.1.0 pins, so a Rust board can sit on this crate as the
Python board sits on the Python core.

It builds on [6502-rust](https://github.com/alewman/6502-rust) (crate
`sixfiveohtwo`) exactly as huc6280-python builds on 6502-python's `CPUFrame`:
the frame supplies the interrupt requests, the poll and `step()`'s skeleton;
this crate supplies the HuC6280's four sequences and inherits none of the NMOS
handlers.

The crate is `huc6280` (package `huc6280-rust`).

```rust
use huc6280::{Bus, HuC6280, Pins};

struct Board { ram: Vec<u8> /* , the VDC, the timer, ... */ }

impl Bus for Board {
    fn read(&mut self, address: u32, pins: &mut Pins) -> u8 { self.ram[address as usize] }
    fn write(&mut self, address: u32, value: u8, pins: &mut Pins) {
        if address == 0x1FF402 { pins.set_irq_disable(value); } // the board decodes $1402
        self.ram[address as usize] = value;
    }
    fn idle(&mut self, address: Option<u32>, pins: &mut Pins) {}
}

let mut cpu = HuC6280::new(Board { ram: vec![0; 0x200000] });
cpu.request_reset();
let cycles = cpu.step(); // 7: the start sequence, one bus call per cycle
```

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
  register and the frame's poll fields) and `cpu.restore_state(&state) ->
  Result<(), String>`. Memory and devices are the board's.
- **The named switches** are public fields with the reference's defaults:

  | Field | Default | Other values |
  | --- | --- | --- |
  | `decimal_formula` (C13) | `DecimalFormula::Mednafen` | `Mesen` |
  | `sbc_honours_t` (C5) | `false` | `true` |
  | `irq_poll_cycle` (U25) | `IrqPollCycle::SecondToLast` | `Last`, `Mednafen` |
  | `vanished_interrupt` (C18) | `VanishedInterrupt::Mesen2` | `Latched`, `Drop` |
  | `tma_multiple` (C6) | `TmaMultiple::Or` | `Highest`, `Lowest` |
  | `tsb_z_from_test` (C12) | `false` | `true` |
  | `zero_page_pointer_wraps` (U8) | `true` | `false` |
  | `block_reads_hardware_page_as_zero` (C9) | `true` | `false` |
  | `nop_63_cycles` (C4) | `2` | any count from 2 |
  | `interrupt_entry_cycles` (C3) | `8` | `7` |

  The U and C numbers are the entries of huc6280-python's
  `docs/undocumented-behavior.md`, which says what each is and how well it is
  known.

`tests/core.rs` shows each of these in a few lines.

### Layout

Each `src/*.rs` file is the transcription of one `huc6280/*.py` module, with
the same handler names, the same 256-row dispatch table (generated from the
reference's by `scripts/make_dispatch.py`) and the hardware comments carried
over: `core` (`_core.py`), `alu`, `bits`, `block`, `control`, `hudson`,
`loads`, `shifts`, `dispatch`, `cpu`, `state`, `trace`. The decimal routines
are the reference's own (written from a description, its
`docs/issues/0004`); no emulator's source was opened for this crate.

Not transcribed: the reference's debugger, console, disassembler, harness and
command line (Python-side tooling). A trace written here carries each
instruction's address and bytes, and the reference's reader disassembles
them. `src/bin/` holds the rungs' tools, among them the part of the
reference's harness that the MAME replay needs.

#### Where Rust forced a difference

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
  enums, so the reference's `ValueError` for an unknown name has no
  counterpart.
- **Arithmetic** (`alu.rs`). The decimal digit arithmetic is in `i32` with
  `rem_euclid` where Python's `%` meets a negative number.
- **State** (`state.rs`). The field widths are the types', so
  `CpuState::validate` checks only what is left: `irq_disable` is three bits
  and B is clear.
- **No halted state.** `Part::halted_cycle` is `unimplemented!`, as the
  reference's `_halted` raises.

#### What 6502-rust's frame would need to make this cleaner

Nothing here needed a change to 6502-rust; these are the places where this
crate works around the frame rather than with it (the reference does the same
around `CPUFrame`):

1. **Several maskable sources.** The frame has one `IRQ_LINE`. The three
   lines, their mask and the copy "as the poll saw them" (`lines`,
   `lines_before`, `poll_lines`) are kept beside it, and `lines_before`
   repeats by hand the frame's own `deferred` rule (first change since the
   last access). A frame that carried a part's auxiliary request bits through
   `deferred` and `polled` would remove all three fields, and the two corners
   listed under "Notes on the reference" with them.
2. **An entry that declines.** `Part::step` returns whatever
   `enter_interrupt` returns, so an entry that finds its source gone has to
   run the instruction itself, past the bookkeeping `step` does after
   `execute`. `enter_interrupt` returning `Option<u32>` (none: run the
   instruction) would keep that path the same as any other.
3. **The poll point.** `irq_poll_cycle` rewrites the frame's `polled` before
   calling its `boundary_interrupt`. A frame with the poll point as a setting
   (second-to-last cycle or last) would own that.
4. **`halted_cycle`** is a required method of `Part`; a default that panics
   would spare a part with no halted state.
5. **The bus context.** 6502-rust's own `Bus` hands a callback `&mut Frame`;
   a part with more inputs needs its own context type (`Pins` here). That
   works as it is; a note in 6502-rust's README that this is the intended
   shape would have saved a decision.

### Certification

| What | Version |
| --- | --- |
| huc6280-python (the reference) | tag `v0.3.0`, `bf20330d4ce4e0e126331433bad03f4e4d0e9a2e` (which pins 6502-python at `2146eba`) |
| 6502-rust (`sixfiveohtwo`) | `71747eb9036702109fb206eb9fe88174a9dcc989`, the pin a2600-rust uses |
| Trace schema | version 1 (6502-python `docs/trace-schema.md`), with the reference's `HuC6280Record` |
| SingleStepTests huc6280 | revision `4ce378e04e243437e6f74dfe76c3cb7c70e45bf1` (MIT; Mesen-generated: a detector, not a judge) |
| Klaus Dormann's 65C02 extended opcodes test | `Klaus2m5/6502_65C02_functional_tests` at `7954e2db`, source SHA-256 `72b1f57d…2f76736`, relocated by the reference's `scripts/dormann_to_ca65.py`, assembled with cc65 2.19 |

Ladder status at this commit, each rung reproduced with the script named, on
Linux x86_64, rustc 1.93.1, PyPy 7.3.20 (Python 3.11):

| Rung | What | Result | Script |
| --- | --- | --- | --- |
| 1 | Trace lockstep, short: every opcode 40 times under each of three sets of switches (random registers, T and D both ways, random mapping registers, zero-page pointers at `$FF`, TMA/TAM masks of no, one and many bits, block transfers across the edges of the zero-read range and one of 65,536 bytes), and sequences where an instruction depends on the one before (SET or PLP then each T instruction, CLI/SEI/PLP, CSL/CSH); every cycle compared. And ADC's and SBC's whole result tables under both decimal formulas | 5 of 5 scenarios `traces are identical` (37,356 boundaries, 2,796,641 cycles, all 256 opcodes); 2,097,152 table entries identical | `rung1.sh` |
| 2 | SingleStepTests huc6280, all 640,000 cases, judged as the reference's `tests/single_step.py` judges them, the verdict and the masks of every case compared between the two cores | see below: the same counts, 0 cases judged differently | `rung2.sh` |
| 3 | Dormann's 65C02 extended opcodes test, relocated, natively | `success at $5675 after 20936652 instructions (test case 240)`, the reference's line | `rung3.sh` |
| 4 | Interrupts in lockstep: a workout (the three lines with their vectors, `irq_disable` written through the bus, NMI, BRK, reset, CLI/SEI/PLP, a handler that acknowledges the timer in its last cycle, SET, a block transfer and CSL/CSH in the main loop) under seeded random line changes, masks, NMIs, resets and state restores between steps and inside bus cycles, in all nine combinations of `irq_poll_cycle` and `vanished_interrupt` and with 7-cycle entries; and one change swept across every cycle of the workout's first 200 | 65 of 65 files `traces are identical`: 925,600 boundaries; in the 29 random runs 61,845 maskable interrupts due (5,709 of them dropped at entry, the instruction running instead), 14,127 NMIs, 625 resets; 5,760 swept scenarios | `rung4.sh` |
| 5 | A real program: MAME 0.285's traces of six games replayed as the reference's `tests/mame_replay.py` replays them, the report and a digest of the core after every step compared. And a synthetic program on a seeded bus (every cycle, what the pins show in it, and every state folded into a digest) | 6 of 6 reports and digests identical, 21,013,980 instructions; synthetic: 3,000,000 boundaries (196,517,500 cycles) and 1,000,000 with the alternate switches (66,633,861 cycles), digests identical at every checkpoint | `rung5.sh` |
| 6 | Seeded faults | 24 caught, 0 missed (below) | `rung6.sh` |

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

**Rung 5** used the MAME traces (the reference's `tests/mame_traces/`, large
and not in git): Blazing Lazers (USA), Bomberman '94 (Japan), Bonk's
Adventure (USA), Final Soldier (Japan), Gradius (Japan) and R-Type I (Japan),
3.3 to 3.8 million instructions each. The report is the reference's own
(Gradius: 3,458,889 instructions, 234 IRQ1 and 4,555 timer interrupts, 0
register divergences, 3,454,098 of 3,454,099 instruction lengths equal to
MAME's); the rung is that the Rust core prints it too. Where the traces are
absent (CI) the rung is the synthetic program alone.

**Rung 6, seeded faults** (`scripts/seeded_faults.py`): one single-line fault
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

CI (`.github/workflows/ci.yml`) runs `cargo fmt --check`, `cargo clippy -D
warnings` and `cargo test`, and against the reference rung 2 on a sample of
100 cases an opcode, rung 3, rungs 1 and 4 on the quick scenario set and rung
5's synthetic program, on every push to main and every pull request.

#### Speed

Klaus Dormann's relocated test (20,936,652 instructions on flat RAM, no
tracing), one core of a busy 32-thread machine, best of five:

| Core | Time | Instructions per second |
| --- | --- | --- |
| huc6280-rust (rustc 1.93.1, `--release`) | 0.173 s | 121 million |
| huc6280-python on PyPy 7.3.20, warmed up (third run in one process) | 1.13 s | 18.5 million |
| huc6280-python on PyPy 7.3.20, a fresh process | 3.36 s | 6.2 million |

About 6.5 times the reference at its best and 20 times a fresh run of it. A
MAME replay (Gradius, 3,458,889 instructions, reading the 160 MB log
included) takes 2.1 s here and 19.8 s on the reference. This crate is
the plain transcription: correct first, and the place to measure a speed fork
against.

#### Notes on the reference

Nothing found here is a divergence: the Rust core does what huc6280-python
v0.3.0 does in each case (rung 4 runs through all of them). They are places
where the reference's behaviour looked unintended while it was being
transcribed, left for its maintainers; `scripts/reference_notes.py` shows
the first three on the reference itself in a few lines each:

1. **A vanished interrupt's instruction skips the last-cycle rule.** When an
   entry finds its source gone, `_enter_interrupt` runs the instruction and
   returns past the frame's bookkeeping after `_execute`. A line raised in
   that instruction's last cycle is then taken at the very next boundary,
   where after any other instruction it waits one more (U21).
2. **A swap of sources in the last cycle is taken at once.** With TIQ up at
   the poll, TIQ dropping and IRQ1 rising in the last cycle leaves the frame's
   `IRQ_LINE` unchanged, so nothing is deferred and IRQ1 is entered at that
   boundary although it rose after the poll.
3. **Capture and restore is not transparent at one kind of boundary.**
   `_lines_before` (which lines the poll saw) is not in `CPUState`; a state
   captured after a line dropped in the last cycle and restored takes a
   different decision from the run it was captured from (IRQ1 up, dropped in
   the last cycle: entered without the restore, not entered with it).
4. `CPUState`'s docstring says BRK does not poll; `_op_brk` leaves the poll
   as any instruction's (only entries and reset set `NO_POLL`).
5. In the test tooling, `tests/mame_replay.py`'s `_take_interrupt` returns
   early with the line it has just asserted still asserted when the core's
   PC is already at the handler.

#### Reproducing

```text
scripts/fetch_huc6280python.sh          # huc6280-python v0.3.0 into external/, with a PyPy venv
scripts/fetch_vectors.sh                # SingleStepTests huc6280 into external/huc6280-sst (185 MB)
scripts/fetch_dormann.sh                # Dormann's source, where the reference's build looks for it
scripts/rung1.sh
scripts/rung2.sh [CORPUS_DIR]           # default external/huc6280-sst/v1
scripts/rung3.sh                        # needs cc65 (ca65, ld65)
scripts/rung4.sh
scripts/rung5.sh [TRACES_DIR]           # default ../huc6280-python/tests/mame_traces
scripts/rung6.sh [CORPUS_DIR]           # after rung3.sh
```

`conformance/README.md` describes the lockstep scenarios.
`docs/handoff-brief.md` is the brief this was built from, with what was built
appended.

### Licence

MIT (see `LICENSE`). Nothing third-party is committed: the vectors, Dormann's
test and the MAME traces are fetched or read in place. No GPL or LGPL source
was read or transcribed for this crate; the only source transcribed is
huc6280-python, which is ours.
