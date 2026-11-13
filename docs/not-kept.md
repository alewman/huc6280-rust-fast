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
- **Not an attempt, a check of a guess:** that the per-cycle `cycles += 1` (a read-modify-write
  of memory every cycle) was a dependency chain worth breaking. With it removed altogether
  (wrong, but the speed is what was asked) Dormann ran x1.013: it is not.
