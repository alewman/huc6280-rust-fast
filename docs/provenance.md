<!-- SPDX-License-Identifier: MIT -->
# Provenance

huc6280-rust-fast is the speed fork of huc6280-rust, a Rust transcription of huc6280-python `v0.3.0` (`bf20330`, after
the decimal rewrite), carried forward to `265df88` (issue 0005) and, on 2026-11-19, to
`6ce348e`, the reference's clean rewrite ([restart.md](restart.md)). It inherits
huc6280-python's verdicts: see that repository's `docs/provenance.md` (audit of 2026-11-13,
wave 1, verdicts updated by its rewrite). The items that needed work there were carried here,
in the transcribed file, item by item (in huc6280-rust `1c44044`, merged here; the fork's own changes touch none of these items):

| Inherited item | Audit (2026-11-13) | Here, after the rewrite (2026-11-19) |
| --- | --- | --- |
| C13 decimal `ADC`/`SBC` | **unclear** | **clean**: `src/alu.rs` moved aside unread and transcribed from the reference's new `_alu.py` (written from closed-box tables); results identical (rungs 1 and 2) |
| C18 `vanished_interrupt` `Mesen2` default | **rewrite** | **clean**: `VanishedInterrupt` and `maskable_vector` re-transcribed from the reference's new code (OBSERVED closed box, Mesen2 and Mednafen) |
| C18 `Latched` | **rewrite** | **withdrawn**: the variant is gone, as the reference refuses the mode |
| U25 `irq_poll_cycle` `Mednafen` mode | **rewrite** | **withdrawn**: the variant is gone, as the reference refuses the mode |
| U25 `Last` and the boundary decision | (own rule) | **clean**: `boundary_interrupt_sequence` re-transcribed (OBSERVED, srg320's VHDL as a closed box; MAME BSD-3) |
| C6, C7, C16 | **unclear** | **cite**: comments carry the reference's new sources (SST cases; the traced Mednafen closed box; Mesen2/Mednafen closed box, ares ISC) in `src/hudson.rs`, `src/control.rs`, `src/core.rs` |
| Bus shapes, block transfers, U8, C3, C12 | **cite** | **cite, done**: SingleStepTests' records, MAME BSD-3 (named with its licence and author), ares ISC, Mesen2 run as a closed box; no GPL internal is named (`src/core.rs`, `src/block.rs`, `src/control.rs`) |
| HuCard mirroring "as the reference states it" (`src/bin/replay.rs`) | **unclear** | **clean, with notice**: the reference's port of MAME's BSD-3 `rom_map_setup` ([NOTICE.md](../NOTICE.md)); test tooling only |

No GPL or LGPL source was opened for the audit or for the rewrite. What this repository adds:

| Part | Source | Verdict |
| --- | --- | --- |
| `src/*.rs` core | transcribed from huc6280-python (MIT) | inherits (clean after 6ce348e) |
| `scripts/reference_*.py`, `compare_*.py`, `lockstep.py`, `make_*.py`, `seeded_faults.py`, `rung*.sh` | own; drive the pinned Python reference | clean (`seeded_faults.py`'s faults on the withdrawn code were moved aside unread with it) |
| `scripts/fetch_*.sh` | fetch SingleStepTests (MIT), Dormann's tests (GPL-3.0) and the reference; nothing committed | clean |
| `tests/core.rs`, `docs/seeded-faults.md`, `conformance/` | own | clean |
| `scratch/withdraw.py` (git-ignored, with `scratch/withdrawn/`) | own helper of the rewrite | not committed |
| `scripts/ab_core.py`, `ab_wasm.py`, `ladder.sh`, `board.sh`, `scripts/board-quiet.patch` (a patch to our own pcengine board), `docs/not-kept.md`, `docs/fast-brief.md` | own speed work | clean |

## History (PUBLIC)

Every commit transcribes `v0.3.0` or later, so the Mesen2-shaped and
Mednafen-transcribed decimal code that huc6280-python's `929c9f7`..`9ea9415`
carried never entered this repository. The commits before the rewrite do hold the
transcriptions of the reference's pre-rewrite C18/U25 code and decimal routines; whether
to rewrite public history is Aubrey's decision (no history was rewritten here). No file
was ever deleted; all commits are authored `alewman`.
