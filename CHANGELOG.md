<!-- SPDX-License-Identifier: MIT -->
# Changelog

huc6280-rust's, merged with it; this fork's own entries say so.

## huc6280-rust-fast, unreleased

Merges huc6280-rust's clean-rewrite carry (`1c44044`, huc6280-python `6ce348e`, below). The
fork's two speed changes are kept: the dispatch from rows, and the one-test usual boundary in
`step`, which needed no change: it takes the fast path exactly when `Part::step` would skip the
boundary decision, and the new rule (the boundary decision clears the polled lines) only acts
inside that decision, so both paths leave them as before. The core gate
(`scripts/ladder.sh`, against `6ce348e`): `cargo test` 23, rung 1 tables and 5 of 5, rung 2
640,000 cases under both formulas with 0 judged differently, rung 3 Dormann `$5675`, rung 4 38 of
38, rung 5 8 of 8. **API change**, inherited: `VanishedInterrupt::Latched` and
`IrqPollCycle::Mednafen` are gone. Rung 6: 43 seeded faults caught, 0 missed (huc6280-rust's 40 and the
fork's three). Speed not re-measured (the rewrite touches only the interrupt decision and the
decimal arithmetic).

## huc6280-rust, unreleased (`1c44044`)

Carries huc6280-python's clean rewrite (`6ce348e`, after `v0.4.0`; local, not yet on
GitHub): the parts its provenance audit marked rewrite or unclear were replaced there, and the
Rust functions that transcribed them are replaced here, moved aside unread and transcribed anew
from the new Python ([docs/restart.md](docs/restart.md)). The reference pin moves from `265df88`
to `6ce348e`.

- **API change (switches):** `VanishedInterrupt::Latched` and `IrqPollCycle::Mednafen` are
  removed, as the reference withdrew `"latched"` and `"mednafen"` (`ValueError` there). The
  defaults (`Mesen2`, `SecondToLast`), `Drop` and `Last` behave as before; their code is new
  (`maskable_vector`, `boundary_interrupt_sequence`). Under `Last`, `Drop` is what `Latched` was.
- **Behaviour change:** the boundary clears the lines the poll saw (`polled_lines`), so
  `CpuState::polled_irq_lines` is `None` in a capture right after an interrupt entry.
- C13 decimal `ADC`/`SBC` (`decimal_add`, `decimal_subtract`, `add`, `subtract`) transcribed from
  the reference's rewrite; results identical for every input of both formulas.
- Tooling: the MAME replay's HuCard mirroring is the reference's port of MAME's BSD-3
  `rom_map_setup` (NOTICE.md; 512 KB cards repeat whole, 192 KB images map `0 1 2 2 0 1 2 2`);
  `long --alternate` and `reference_long.py` use `Drop` with poll `Last`; rung 4's scenarios run
  the kept modes (the kept random scenarios keep their seeds; sweeps under `Drop` where
  `Latched` was); rung 6's five faults on the withdrawn code moved aside, `entry-polls`
  re-pointed, 13 new faults.
- Comments, README and docs re-cited without GPL internals; SPDX headers on every source file;
  NOTICE.md; `docs/provenance.md` verdicts updated item by item; three directed tests added
  (C16's entry between SET and its target, a source still asserted in every mode, the polled
  lines cleared at the boundary) and the replay's mirroring test.

## huc6280-rust-fast 0.2.0 (2026-11-13)

Merges huc6280-rust 0.2.0 (`43e5978`, issue 0005, below). The fork's two
speed changes are kept; its one-test boundary in `step` needed no change.
The core gate (rungs 1-5) and 35 seeded faults pass against huc6280-python
`v0.4.0`. Recording the lines at every access costs about 5% on Dormann's
test (x0.950, x0.945); the board was not re-measured.

## 0.2.0 (2026-11-13)

Carries huc6280-python issue 0005, settled in its `v0.4.0` (`265df88`); the
reference pin moves from `v0.3.0` (`bf20330`) to it. Every rung re-run against
the new reference (README, "Status").

- **`CpuState::polled_irq_lines: Option<u8>`** (new field, `None` or 0-7: the
  lines the last poll saw, IRQ2/IRQ1/TIQ in bits 0-2, when one changed in the
  instruction's last cycle). `capture_state` writes it, `restore_state` reads
  it and refuses a value above 7. A struct literal without `..` needs the
  field; a board's serialized state gains one key, and a state saved without
  it should load as `None`.
- The poll keeps the three lines as it saw them (`Pins::lines_at_access`,
  `polled_lines`, replacing `lines_before`): `set_irq_lines` returns early
  when nothing changed, arms the frame's watch on any change (a swap of
  sources included) and updates the polled lines bit by bit between steps;
  the boundary decides on them per `irq_poll_cycle`; reset clears them.
- An entry whose source has vanished runs its instruction with `step`'s
  deferred-to-polled bookkeeping.
- BRK does not poll (`deferred = NO_POLL`).
- `trace::next_boundary` decides from the polled lines and drops a vanished
  timer, as the default switches do.
- Tooling: the MAME replay checks for "already at the handler" before
  asserting a line and clears the polled lines on its forced path; the
  synthetic program's digest folds `polled_irq_lines`;
  `fetch_huc6280python.sh` fetches the pinned commit from its SOURCE argument;
  rung 6 has 8 new faults on the new bookkeeping (32 in all).

## 0.1.0 (2026-11-12)

Transcribed from huc6280-python `v0.3.0`; rungs 1-6 pass.
