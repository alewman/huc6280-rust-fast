# Changelog

huc6280-rust's, merged with it; this fork's own entries say so.

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
