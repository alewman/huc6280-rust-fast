<!-- SPDX-License-Identifier: MIT -->
# huc6280-rust-fast

A speed fork of [huc6280-rust](https://github.com/alewman/huc6280-rust), the
Rust **Hudson Soft HuC6280** CPU core (the processor of the PC Engine, the
TurboGrafx-16, the SuperGrafx and the CD-ROM systems), forked at `7f49bf8`,
merged with its publication cleanup at `65c18df`, with its carrying of
huc6280-python's issue 0005 at `43e5978` and with its carrying of the
reference's clean rewrite at `e1d24a3`. It is **held in lockstep
with it**: every change must leave every rung of huc6280-rust's ladder
identical (and so this core equal to
[huc6280-python](https://github.com/alewman/huc6280-python) `6ce348e` at every
processor boundary and every bus cycle), keep the speed fork of the board,
[pcengine-rust-fast](https://github.com/alewman/pcengine-rust-fast),
identical on its 275 lockstep cases and its save-state round trips, and
measure faster, or it is not kept. huc6280-rust is the readable one; this one
is the CPU of pcengine-rust-fast, which runs the PC Engine in the browser at
[play-pce.supertux.com](https://play-pce.supertux.com). The plain board is
[pcengine-rust](https://github.com/alewman/pcengine-rust). MIT licensed.
Built with Claude (Claude Opus 5.5, through Claude Code) for alewman.

**The crate is still `huc6280` and its public API is huc6280-rust's**: the
`Bus` trait and its three methods, `Pins` (its fields, `frame` included, and
its methods), `HuC6280::new` and `step`, the registers,
`capture_state`/`restore_state`/`CpuState`, the switches with their defaults,
the dispatch table, and the frame's types re-exported from
[6502-rust](https://github.com/alewman/6502-rust) as before. Nothing was
added. A board written for huc6280-rust changes one line, its dependency.
Since 0.2.0 (huc6280-rust 0.2.0, issue 0005) `CpuState` has one more field,
`polled_irq_lines: Option<u8>` (the lines the last poll saw when one changed
in the instruction's last cycle; `None` otherwise and for a state saved before
the field), so a board's saved states gain that key. Since the clean rewrite
(huc6280-rust `1c44044`, huc6280-python `6ce348e`) `VanishedInterrupt::Latched`
and `IrqPollCycle::Mednafen` no longer exist (the reference withdrew those
modes; under `Last`, `Drop` is what `Latched` was), and `polled_irq_lines` is
`None` right after a boundary that decides an interrupt.

## Status

**2026-11-19: two changes kept; huc6280-rust's carrying of the reference's
clean rewrite merged (the core gate and the seeded faults re-run against
huc6280-python `6ce348e`, green; the board gate below is the run of
2026-11-13, before both the issue 0005 merge and this one, not repeated here
while the board is being rewritten).** On the board
(pcengine-rust-fast, five titles) **x1.067** natively and x1.016 under wasm;
a CD game x1.04 to x1.07; the core alone (Dormann's test) **x1.388**; all
against the fork point (under "Speed", measured before the rewrite; the
rewrite touches only the interrupt decision and decimal arithmetic, and was not
re-measured). Gates, re-run on 2026-11-19 after the merge:

| Gate | What | Result |
| --- | --- | --- |
| `cargo test` | the directed tests, the replay tool's mirroring test and the doctests | 23 pass |
| Rung 1 | ADC/SBC result tables; directed and sequence trace lockstep | 2,097,152 table entries identical; 5 of 5 scenarios identical |
| Rung 2 | SingleStepTests huc6280, all 640,000 cases, both decimal formulas, verdicts compared case by case with the reference's | 0 cases judged differently under either formula |
| Rung 3 | Dormann's 65C02 extended opcodes test, relocated | `success at $5675 after 20936652 instructions (test case 240)`, the reference's line |
| Rung 4 | Interrupts in lockstep, the four kept mode pairs | 38 of 38 scenarios identical |
| Rung 5 | Six MAME 0.285 replays and two synthetic runs | 8 of 8 identical |
| Rung 6 | Seeded faults: huc6280-rust's 40 (13 on the rewrite's code) and three on this fork's shortcuts | 43 caught, 0 missed (2026-11-19, after the merge) |
| Board | pcengine-rust-fast on this crate: its tests, its 275 lockstep cases, the save-state round trip and run-ahead pass | 57 tests; 275 of 275 identical; 18 of 18 titles |

Rungs 1 to 6 and `cargo test` are the runs of 2026-11-19 after merging
huc6280-rust `e1d24a3` (the clean rewrite), against huc6280-python `6ce348e`
(fetched from a local clone: not on GitHub yet, so CI's rungs job fails until
it is pushed); the board row is the run of 2026-11-13. **Nothing here was run on a real console**: like
huc6280-rust, this core is proven equal to the reference, not to the chip.

## Using it

```toml
[dependencies]
huc6280 = { package = "huc6280-rust-fast", git = "https://github.com/alewman/huc6280-rust-fast", rev = "<commit>" }
```

(Pin the commit you have checked; the crate is not on crates.io.) Rust 1.85
or newer. A board on huc6280-rust swaps
`package = "huc6280-rust", git = "https://github.com/alewman/huc6280-rust"`
for the line above and nothing else; pcengine-rust-fast,
[supergrafx-rust](https://github.com/alewman/supergrafx-rust) and
[pcenginecd-rust](https://github.com/alewman/pcenginecd-rust) build and pass
that way (under "The add-ons").

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

This example is compiled and run by `cargo test` (as a doctest). The bus
contract, `Pins`, the interrupt lines, the disable register, state and the
named switches are exactly huc6280-rust's: its README's
[For a board author](https://github.com/alewman/huc6280-rust#for-a-board-author)
is the guide, and `tests/core.rs` shows each in a few lines.

## Speed

Measured on 2026-11-13 on an Intel i9-13900K (32 threads, shared with other
work: load average 15 to 21 throughout), rustc 1.93.1, Node 22 for wasm. The
fork point's build against this one, alternated on one pinned efficiency
core, timed by CPU time (best of 12 a side natively, of 8 under wasm), the
checksums of every picture, every sample and the final snapshot equal on both
sides:

| Title (600 frames, the board's picture-gate input) | fork point | now | | fork point, wasm | now, wasm | |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Blazing Lazers | 821.0 | 904.2 | x1.101 | 482.8 | 497.2 | x1.030 |
| Bonk's Adventure | 1005.7 | 1044.8 | x1.039 | 572.1 | 579.1 | x1.012 |
| R-Type | 921.2 | 975.8 | x1.059 | 507.8 | 512.0 | x1.008 |
| After Burner II | 818.4 | 894.0 | x1.092 | 468.4 | 477.2 | x1.019 |
| Street Fighter II' | 836.2 | 872.4 | x1.043 | 416.3 | 420.8 | x1.011 |
| all five (geometric mean) | | | **x1.067** | | | **x1.016** |

Frames per second of pcengine-rust-fast (`pce-bench`, its fat-LTO profile)
built against each core. Under wasm: the board's `wasm-bench` under Node 22
(`scripts/ab_wasm.py`, 1,200 frames).

Re-measured after the merge (2026-11-13, load average about 11 to 14), as
pcengine-rust-fast now adopts this crate: the board built against
huc6280-rust `65c18df` and against this tree, nothing else different, the
same five titles: **x1.075** and x1.073 in two runs on two efficiency cores
(every title up in both, x1.045 to x1.113), after an A/A run of x0.996.

**Issue 0005's cost** (the merge of huc6280-rust `43e5978`): the core now
records the three lines at every bus access (`Pins::access`, the reference's
`_confirm` override), a load and a store a cycle. Dormann's test, `2f4d4d2`
against the merge, alternated on one pinned core: **x0.950** and x0.945 (best
of 5 and of 7; an A/A run x1.002), 0.122 s against 0.129 s. Recording only
when a request is pending, as the reference's `_confirm` does, is not
equivalent here: the reference's confirming wrappers outlive an entry's or
reset's `_deferred = None`, `Frame::access` does not, and a request a board
makes through `pins.frame` arms the watch without passing through the lines.
The board was not re-measured with it (the board's references are changing).

| Other case | fork point | now | |
| --- | ---: | ---: | ---: |
| **A CD game**: Ryuuko no Ken on pcenginecd-rust (System Card 3.0, Arcade Card, 1,500 frames from power-on, fat LTO as play-pce builds) | 187.0 fps | 199.3 fps | **x1.065** (x1.039 in a first run; Mad Stalker x1.036) |
| **The core alone**: Dormann's relocated 65C02 test, 20,936,652 instructions on flat RAM (this crate's own profile, no LTO) | 0.210 s (99.7 M/s) | 0.152 s (137.7 M/s) | **x1.388** |

| Kept, in order | board, five titles | wasm | CD case | Dormann |
| --- | ---: | ---: | ---: | ---: |
| **The dispatch from constant rows, and the cycle primitives inlined into every handler** (`649e7ca`). `execute_sequence` matches on the opcode; each of the 256 arms reads its row of `OPCODES` at compile time and runs that handler with its addressing mode built in, and `cycle_read`, `cycle_write`, `dummy`, `io` and `fetch` are `inline(always)`, so an instruction is one stretch of code with one jump into it. The table stays the one place a handler and a mode are written | x1.078, x1.048 | x1.000 | x1.097 | x1.043 |
| **The usual boundary in one test** (`aad85b2`). `step` tests reset, halted, the requests and what the last poll left (`polled`, `polled_i`) together and, when all are clear, runs the instruction and the frame's `deferred` bookkeeping itself, as `Part::step` would; any other boundary is `Part::step`'s, out of line. `Part::step` itself is unchanged | x1.023, x1.024 | x1.016 | x1.017 | x1.387 |

Each factor is against the build before it (two board runs each, every title
up in all four). Chained, the steps give x1.07 to x1.10 on the board, x1.016
under wasm, x1.116 on the CD case and x1.45 on Dormann; measured directly
against the fork point (the tables above) they are x1.067, x1.016, x1.039 to
x1.065 and x1.388. A/A on the same machine: x0.95 to x1.00 a title natively,
x0.988 to x1.100 a title under wasm, x0.983 on the CD case, x0.989 on
Dormann; so the board's x1.067 is real but the wasm x1.016 is at the edge of
what this machine can tell. Tried and not kept, with their numbers:
[docs/not-kept.md](docs/not-kept.md).

**What this does not reach.** On the board the core is now 29% of Blazing
Lazers and 27% of Street Fighter II' (33% and 30% at the fork point), and no
line of it is above 3%: what is left is one call into the board per CPU cycle
and one indirect jump per instruction, which the bus contract (one callback
per cycle, one instruction per `step`) fixes. Under wasm the largest single
function is the board's `Bus::read` (about a fifth of a frame), not the core.

| Where the time goes | fork point | now |
| --- | ---: | ---: |
| Blazing Lazers on the board (`pce-profile`): the core and the 65xx frame under it | 33.2% | 29.0% |
| Street Fighter II' on the board | 30.0% | 27.3% |
| the hottest core lines on the board | the dispatch's `match` on the handler 3.8%, the `OPCODES` lookup 2.8%, `physical` 2.2% | `physical` 2.9%, the cycle's first store 2.3%, the opcode `match` 1.7% |
| Dormann alone: the frame's `step` | 7.8% | 3.6% |
| Dormann alone: the hottest lines | the `match` on the handler 10.9%, `physical` 8.9%, the `OPCODES` lookup 7.6% | `physical` 8.5%, the opcode `match` 6.6% and its arms |

(Shares of a smaller whole after: Dormann's samples went from 2,851 to 2,141
for the same 12 runs.)

## The quiet window: built, measured, not kept

The one gain that needs the board's cooperation: a board lends the core
stretches of cycles in which nothing it owns can happen, so the core makes
those cycles without calling it. It is on the branch `not-kept/quiet-window`
(not merged), with its test, its faults and the board patch.

**The core side** (on the branch): `Bus` gains a constant `QUIET` (default
`false`, which compiles everything below away, so a board that does not opt
in is unchanged) and four provided methods:

- `quiet_window(pins) -> QuietWindow`, at the start of a step: how many
  master clocks of quiet cycles the board lends, and what a cycle costs at
  each speed;
- `quiet_read(address) -> Option<u8>` and `quiet_write(address, value) ->
  bool`, inside the window: the byte of a page where an access would do
  nothing but move the byte;
- `quiet_spent(clocks, pins)`: the quiet cycles paid, before the next call
  and at the end of the step.

Inside the window an internal cycle, a dummy read and a lent access are made
with no call, their clocks added up. `tests/quiet.rs` on the branch runs a
model board with a moving event horizon, a device page and interrupt changes
both ways for 1.6 million steps and finds the same calls at the same master
clocks with the same pins, the same memory and the same state after every
step; five seeded faults on the window are all caught by it.

**What a board would need** to opt in (for pcengine-rust-fast,
`scripts/board-quiet.patch`, 45 lines in `board.rs`; `scripts/board.sh rev
REV quiet` applies it to a scratch copy, for revisions of the branch):

1. `const QUIET: bool = true` in its `Bus` implementation;
2. an event horizon: `quiet_window` returns the master clocks to the board's
   next event (`wake - mclk`), or none while anything watches every cycle (a
   tap, a device on the expansion port);
3. pages it can lend: `quiet_read`/`quiet_write` answer for the pages where an
   access only moves a byte (work RAM, the image of a plain ROM card) and
   decline everything else;
4. `quiet_spent` advances the board's master clock and stores what every
   callback stores (the opcode), so that the board is where it would have
   been after the calls it did not get.

Measured against the kept chain's first step (`649e7ca`) on the same machine:
Bonk's Adventure x1.088, R-Type x1.183, After Burner II x1.081, Blazing
Lazers x1.005, but Street Fighter II' x0.798; x1.022 overall, and under wasm
x0.95. Street Fighter II' is on a mapper card (`Sf2Card`), which the board
does not lend, so each step's first program read misses the window, looks the
page up twice and closes the window for the rest of the step; reopening the
window after every call instead cost more than it saved (x0.956 overall).
With a device on the expansion port (the CD unit) the board's horizon is
every cycle, so the window is never open and CD games gain nothing. For the
window to pay, a board would also have to lend the mapper cards' banks, and
the gain would be for HuCard games only; on this evidence it was not worth a
second path through every cycle primitive.

## How it is proven

**The rule.** A change is kept only if both gates stay green and an A/B by
CPU time shows a gain above the A/A noise; it is then one commit with its
factors in the message. A rejected one is a line in
[docs/not-kept.md](docs/not-kept.md). Every commit of this fork passed both
gates as a tree of its own.

**The core gate**, `scripts/ladder.sh` (a few minutes from the caches;
`--faults` adds rung 6, `JOBS=n` the parallelism, default 12): `cargo test`;
rung 1 (the arithmetic tables, the directed and sequence lockstep
scenarios); rung 2 (all 640,000 SingleStepTests cases under both decimal
formulas, the verdicts compared case for case with the reference's); rung 3
(Dormann, the reference's line); rung 4 (the 65 interrupt lockstep files);
rung 5 (the six MAME replays where their traces are present, and the two
synthetic runs). The rungs are huc6280-rust's, described there under
[How it is proven](https://github.com/alewman/huc6280-rust#how-it-is-proven).
The reference's side of each rung never changes, so it is computed once and
kept under `external/` (gitignored): `external/refcache` (rungs 1 and 4),
`external/sst/reference-*` (rung 2), `external/dormann/reference.txt`
(rung 3), `external/rung5/*.reference.txt` (rung 5). Delete a cache to
recompute it from the reference.

What each needs, as for huc6280-rust: Rust 1.85 or newer; Python 3 (standard
library) for the scripts; huc6280-python `6ce348e` in a venv
(`scripts/fetch_huc6280python.sh`; PyPy runs it several times faster:
`PYTHON=/path/to/pypy3`); the SingleStepTests vectors
(`scripts/fetch_vectors.sh`, 185 MB); Dormann's source
(`scripts/fetch_dormann.sh`) and cc65 2.19 on the PATH for rung 3. Rung 5's
MAME traces are made by huc6280-python's `scripts/mame_trace.py` from your
own ROMs and MAME 0.285, in a checkout of huc6280-python beside this one;
without them rung 5 is the synthetic runs alone. The single-rung scripts
(`scripts/rung1.sh` to `rung6.sh`, with `QUICK=1` and `LIMIT=n` for the
smaller sets) are huc6280-rust's and work here unchanged.

**The board gate**, `scripts/board.sh gate`: pcengine-rust-fast's own
`scripts/ladder.sh` (its tests, the 275 lockstep cases from its reference
cache, the save-state round trip and run-ahead pass on its 18 picture-gate
titles) on `scratch/board`, a copy of pcengine-rust-fast's committed HEAD
whose CPU dependency is pointed at this tree, made by `scripts/board.sh
setup`. It needs a checkout of pcengine-rust-fast beside this one
(`BOARD_REPO` overrides the place) with its reference cache, and your own
HuCard images where its cases name them (its README says how). Since
pcengine-rust-fast now depends on this crate itself, its own gate is this
one too.

**Measuring.** `scripts/board.sh rev REV` builds the board's `pce-bench`
and `pce-profile`, the CD add-on's bench timed by thread CPU time, Dormann
and the wasm bench against any revision of this crate, each in its own target
directory (it also needs pcenginecd-rust beside this one, `CD_REPO`).
pcengine-rust-fast's `scripts/ab.py` compares two `pce-bench` builds on its
five titles, `scripts/ab_wasm.py` two wasm builds, and `scripts/ab_core.py`
Dormann or the CD case (the System Card and the disc directory are
`SYSCARD` and `LIBCD`; their defaults are the author's layout). Profiles:
the board's `pce-profile` (SIGPROF sampling) with its `profile.py`, and the
same sampler around Dormann for the core alone.

**Seeded faults** (rung 6, `scripts/seeded_faults.py`): **43 caught, 0 missed**:
huc6280-rust's 40, each by the same check as there, and three on
this fork's shortcuts (the table in
[docs/seeded-faults.md](docs/seeded-faults.md)):

| Fault | Module | What it changes | Caught by |
| --- | --- | --- | --- |
| `row-mode-zpx-as-zp` | `dispatch.rs` | an arm of the dispatch from rows is built with zp for a zp,X row | rung 2 (SingleStepTests sample) |
| `step-keeps-polled-i` | `cpu.rs` | the one-test boundary leaves out `polled_i`, so after CLI, SEI or PLP with nothing requested the frame never consumes it | rung 1 (directed lockstep) |
| `step-drops-deferred` | `cpu.rs` | the one-test boundary forgets a request changed in the last cycle instead of leaving it for the next boundary | rung 1 (directed lockstep; rung 4 before issue 0005, now that BRK leaves `NO_POLL` deferred) |

**CI** (`.github/workflows/ci.yml`, main and pull requests): `cargo fmt
--check`, `cargo clippy -D warnings` and `cargo test`, and, against the
reference fetched from GitHub, rung 2 on a sample of 100 cases an opcode,
rung 3, rungs 1 and 4 on the quick scenario set and rung 5's synthetic
program. The full corpus, the MAME replays, the seeded faults and the board
gate run by hand.

## The add-ons

On scratch copies of supergrafx-rust (`1d0528a`) and pcenginecd-rust
(`96ae3f7`) whose dependency line names pcengine-rust-fast on this crate and
nothing else changed (2026-11-13): supergrafx-rust builds, its
52 tests pass and its 22 lockstep cases are identical from its reference
cache; pcenginecd-rust builds, its 109 tests pass and its 52 lockstep cases
(the probe card on synthetic discs, every switch, and six commercial discs on
the System Card) are identical from its reference cache.

## Notes on the reference, provenance

Nothing found here is new: the places where huc6280-python's behaviour looked
unintended were found while huc6280-rust was transcribed, and are its README's
[Notes on the reference](https://github.com/alewman/huc6280-rust#notes-on-the-reference),
filed in huc6280-python as
[issue 0005](https://github.com/alewman/huc6280-python/blob/main/docs/issues/0005-interrupt-edge-cases-found-by-the-rust-port.md)
and settled there in `v0.4.0`. This fork carries the settlement from
huc6280-rust, as the lockstep requires; its one-test boundary in `step`
needs nothing for it (the boundary it skips would not use the polled lines
in `Part::step` either, and `execute_sequence` rewrites them on both paths).
The same holds for the clean rewrite's rule that the boundary decision clears
the polled lines: the fast path is taken exactly when `Part::step` skips that
decision.

No GPL or LGPL source was copied, read or transcribed for this crate or for
huc6280-rust. The only source transcribed is huc6280-python, which is ours
and MIT. Its provenance audit (2026-11-13) found rules that had followed GPL
source; it rewrote them clean on 2026-11-19 (`6ce348e`), and huc6280-rust
replaced the functions that had transcribed them, moved aside unread and
transcribed anew ([docs/restart.md](docs/restart.md), verdicts in
[docs/provenance.md](docs/provenance.md); the MAME BSD-3 notice for the
replay tool's mirroring in [NOTICE.md](NOTICE.md)). This fork merged that;
its own changes are restructurings of huc6280-rust's code and touched none of
the replaced items.

[docs/fast-brief.md](docs/fast-brief.md) is the brief this fork was built
from, with what was built appended; [docs/handoff-brief.md](docs/handoff-brief.md)
is huc6280-rust's, inherited.

## Licence

MIT (see [LICENSE](LICENSE)). Nothing third-party is committed: the vectors,
Dormann's test and the MAME traces are fetched or read in place.
