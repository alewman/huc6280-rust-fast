# Brief: huc6280-rust-fast

> **This is the brief the fork was built from (2026-11-13), kept as written**, with "What was
> built" appended at the end by the session that built it. Its instructions ("do not push", the
> working rules, the local paths under `/data/emu/`, which are the author's machine) are history,
> not instructions to a reader. The [README](../README.md) is the current account; where the two
> differ, the README is right.

## The request

Aubrey, 2026-11-13: "we need the fast fork for these", after play-pce went live at
play-pce.supertux.com on pcengine-rust-fast. That board fork is now limited by the CPU core:
about **32% of a frame** is huc6280-rust plus 6502-rust's frame, and CD games (a port device on the
expansion port) gained least from the board fork. Phones are the target.

This repository is a fork of `/data/emu/huc6280-rust` at `7f49bf8` (remote `upstream`), in the
family's ladder-gated pattern: read `/data/emu/pcengine-rust-fast` (README, `scripts/ladder.sh`,
`scripts/ab.py`, `scripts/profile.py`, `docs/not-kept.md`, `docs/fast-brief.md` with its "What was
built") and `/data/emu/z80-rust-fast`, `/data/emu/m68000-rust-fast` for how a **core** fork in this
family is gated and measured.

Package `huc6280-rust-fast`, **crate name kept `huc6280`**, and **the public API kept**: the `Bus`
trait and its three methods, `Pins` (its public fields and methods: `set_irq_lines`,
`set_irq_disable`, `high_speed`, `kind()`, `opcode()`, `frame` and the frame's request methods),
`HuC6280::new`, `step`, the registers, `capture_state`/`restore_state`/`CpuState`, the switches as
public fields with their defaults. pcengine-rust-fast, supergrafx-rust and pcenginecd-rust must
build and behave identically with nothing but a dependency path changed. An addition (a new
optional trait method with a default, a new accessor) is allowed when nothing that does not use it
changes; if a gain needs the board to opt in (for example a fetch that reads from a page the board
lends instead of a `Bus::read` per byte), build the core side, measure it with a board patched in a
scratch copy, and report what the board fork would add; do not edit the board fork.

## What the board fork's profile pointed at (its report, unverified here)

- the dispatch: the `OPCODES` lookup, then two nested matches;
- per-cycle bookkeeping: `pins.kind`, the cycle count, `Frame::access`, `Part::step`;
- the per-cycle MPR lookup (logical to physical);
- the operand fetch through a `Bus::read` per byte.

Profile first and say where the time goes before changing anything.

## The rule

Behaviour is huc6280-rust's, bit for bit, at every boundary and every bus access. A change is kept
only if the **gate** stays green and an **A/B measurement of CPU time** shows a gain above the A/A
noise. One commit per kept change with its factor; rejected attempts in `docs/not-kept.md` with
their numbers. Stop when the profile is flat or three attempts in a row are not kept.

- **Core gate** (`scripts/ladder.sh`): the inherited rungs, all of them: `cargo test`; rung 1
  (trace lockstep against huc6280-python, from cached reference traces where the inherited scripts
  allow); rung 2 (all 640,000 SingleStepTests cases with the same mask counts, both decimal
  formulas); rung 3 (Dormann); rung 4 (the interrupt scenarios); rung 5 (the MAME replays, if the
  traces are present under `/data/emu/huc6280-python/tests/mame_traces/`, and the synthetic long
  run). The reference checkouts and caches under `/data/emu/huc6280-rust/external/` may be copied
  into this repository's `external/` (gitignored) to save rebuilding them.
- **Board gate**: pcengine-rust-fast's 275-case lockstep gate (`scripts/lockstep.py
  --reference-cache external/refcache`) and its save-state round trips, run on **a scratch copy**
  of pcengine-rust-fast whose manifest points at this crate by path. Do this on the untouched fork
  first (it must pass), then for every kept change. Do not modify `/data/emu/pcengine-rust-fast`
  (another session may be working in it).
- **Measure at the board**, because that is what play-pce runs: pcengine-rust-fast's `pce-bench`
  and `scripts/ab.py` method (CPU time, alternating, best of 12, A/A first) on its five titles, on
  the scratch copy, plus one CD case (pcenginecd-rust with a disc or its probe card, since CD games
  gained least) and Dormann's test as the core-only number. Also under **wasm** (the board fork's
  `scripts/wasm-bench.sh` shape), since the browser is the target.
- **Seeded faults**: the inherited 24 must still be caught, plus at least one new fault on every
  shortcut the fork adds.

## Rules

- Do not modify huc6280-rust, 6502-rust, pcengine-rust, pcengine-rust-fast, the add-on crates,
  play-pce or any Python repository. A bug found on the way is reported, not fixed here.
- If depending on 6502-rust's frame is itself the cost, the fork may carry its own frame **inside
  this crate** (MIT, ours) as long as the public `Pins`/frame API keeps working for the boards; say
  so plainly in the README and measure it like any other change.
- No GPL or LGPL code, copied or transcribed; do not open any `reference/` directory.
- Never commit ROMs or traces. At most 12 processes, `nice`; measure CPU time, not wall time.
  No `sed -i`, `rm -rf`, `ln -sfn`. Temp files under `external/`, `target/`, `scratch/`.
- clippy: the rustup 1.93.1 toolchain's cargo with its own target directory (`PATH=<toolchain
  bin>:$PATH CARGO_TARGET_DIR=target/clippy cargo clippy --release --all-targets -- -D warnings`;
  mixing it with `/usr/bin/cargo`'s target directory fails with E0514). `cargo fmt`.
- Commits locally, trailer `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. **Do not
  create a GitHub repository and do not push.**
- Report: the profile before and after (core alone and inside the board), each kept change with its
  factor, the chained factor against the fork point for Dormann and for the board's five titles
  natively and under wasm, and for a CD case; what was not kept; the faults; both gates' final
  results; confirmation that pcengine-rust-fast and both add-ons build and pass against this crate
  unchanged but for the path; what a board would add for any opt-in gain; the final commit.

## What was built

2026-11-13. Two kept changes, both gates green on every commit, 27 of 27 seeded faults caught
(the README has the tables; docs/not-kept.md the rest):

1. **The dispatch from constant rows with the cycle primitives inlined** (649e7ca): board
   x1.078 and x1.048, CD x1.097, Dormann x1.043, wasm x1.000.
2. **The usual boundary in one test** (aad85b2): board x1.023 and x1.024, CD x1.017, Dormann
   x1.387, wasm x1.016.

Directly against the fork point: the board's five titles **x1.067** natively and **x1.016** under
wasm, the CD case (Ryuuko no Ken on pcenginecd-rust) x1.039 and x1.065 (Mad Stalker x1.036),
Dormann **x1.388**. The board fork's profile was right about where the core's time shows, but
not about what removing it buys: the dispatch alone (two nested matches made one) measured
nothing, since the compiler had already made jump tables of both; what paid was a whole
instruction as one stretch of code, and the boundary's five tests made one.

**The opt-in** the brief anticipated (a board lending pages so a cycle needs no call) was built
as the quiet window, measured with pcengine-rust-fast patched in a scratch copy, and not kept:
+8% to +18% on three titles, -20% on Street Fighter II' (a mapper card the board does not lend),
x1.022 overall, x0.95 under wasm, and nothing for CD games (a port device makes every cycle an
event). It is on the branch `not-kept/quiet-window` with its test, its faults and the board
patch, for a board fork that wants to lend its mapper cards too.

**Stopped** because the profile is flat: on the board no core line is above 3% (`physical`, the
cycle's first store, the opcode `match`), and what remains is the contract itself, one call into
the board per CPU cycle and one indirect jump per instruction. Under wasm the largest single
function of a frame is the board's `Bus::read`, about a fifth: the next gain for phones is in
the board's quiet callback, not in this crate.

Not done here, by the brief's rules: nothing was written in pcengine-rust-fast, the add-ons,
huc6280-rust, 6502-rust or play-pce (their gates ran on scratch copies with one line changed); no
GitHub repository, nothing pushed. The fork still uses 6502-rust's frame: carrying it was not
needed for the boundary change, which reads the frame's public fields.
