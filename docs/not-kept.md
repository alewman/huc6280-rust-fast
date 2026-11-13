# Tried and not kept

Each with its A/B: two builds alternated on one pinned efficiency core, by CPU time, the best
run of each side compared. The board's five titles with pcengine-rust-fast's `scripts/ab.py`
(600 frames, best of 10 or 12 a side) on a scratch copy of the board built against each core
(`scripts/board.sh rev`); the same five under wasm with `scripts/ab_wasm.py` (Node 22, 1,200
frames, best of 6); Dormann and the CD case with `scripts/ab_core.py`. A/A on this machine
(load 15 to 21 while these ran): the board x0.950 to x1.000 a title, the CD case x0.983,
Dormann x0.989, wasm x0.988 to x1.100 a title (x1.020 overall).

- **The dispatch from constant rows, alone.** `execute_sequence` matched on the opcode, each of
  the 256 arms reading its row of `OPCODES` at compile time and calling the handler with its
  mode inlined (`#[inline(always)]` on `dispatch`, `ea` and the handlers that take a mode), in
  place of the table lookup and the two nested matches at run time. Board x1.023 (Bonk's
  Adventure x0.992), wasm x0.999, Dormann x1.001: the compiler had already made the two
  matches into jump tables, and one indirect jump an instruction is what is left either way.
  Kept later together with the cycle primitives inlined (README, change 1), which is what
  measured.
- **The quiet window, first form** (the opt-in described in the README): the window's test
  written into the bodies of the cycle primitives. With the board *not* opting in, where the
  test is compiled away, the board went x0.912 (Street Fighter II' x0.854, twice): a cycle
  primitive with the dead window code in its body is inlined differently. Rewritten so a
  primitive's body is the old one with `if B::QUIET { return self.quiet_...() }` first, the
  plain board was x0.994 (noise), and the patched board taking the window x1.014.
- **The quiet window on the kept first step** (branch `not-kept/quiet-window`; the README's
  section on it says what it is and what a board would add). Against 649e7ca with the plain board,
  the same core with the window compiled away: x0.997 (noise). With the board patched to take it
  (`scripts/board-quiet.patch`): Blazing Lazers x1.005, Bonk's Adventure x1.088, R-Type x1.183,
  After Burner II x1.081, Street Fighter II' x0.798, x1.022 overall; under wasm, against the fork
  point, x0.950 (Street Fighter II' x0.830). Street Fighter II' runs from a mapper card the board
  does not lend: every step's first program read misses the window, looks its page up twice and
  closes the window for the rest of the step. With 95% of Blazing Lazers' cycles inside the window
  (counted), it still gained nothing there: the board's quiet callback it replaces was already
  cheap next to the core's own work per cycle.
- **The quiet window reopened after every call** (so a missed read does not close it for the rest
  of the step): x0.956 overall against 649e7ca, Street Fighter II' x0.842, the others x0.97 to
  x1.00. The window's question to the board on every call costs more than the quiet cycles save.
- **Not an attempt, a check of a guess:** that the per-cycle `cycles += 1` (a read-modify-write
  of memory every cycle) was a dependency chain worth breaking. With it removed altogether
  (wrong, but the speed is what was asked) Dormann ran x1.013: it is not.
