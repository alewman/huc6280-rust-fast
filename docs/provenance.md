# Provenance

huc6280-rust-fast is a Rust transcription of huc6280-python `v0.3.0` (`bf20330`, after
the decimal rewrite), carried forward to `265df88` (issue 0005). It
inherits huc6280-python's verdicts: see that repository's
`docs/provenance.md` (audit of 2026-11-13, wave 1). Briefly, the items that
need work there need the same work here, in the transcribed file:

| Inherited item | Here |
| --- | --- |
| C13 decimal `ADC`/`SBC` (**unclear**) | `src/alu.rs` `DecimalFormula` |
| C18 `vanished_interrupt` `Mesen2` default and `Latched` (**rewrite**: re-establish closed box) | `src/control.rs` |
| U25 `irq_poll_cycle` `Mednafen` mode (**rewrite**) | `src/control.rs` |
| C6, C7, C16 (**unclear**) | `src/hudson.rs`, `src/control.rs` |
| Bus shapes, block transfers, U8, C3, C12 (**cite**: name SST / MAME BSD-3 / ares ISC, not Mesen) | `src/core.rs`, `src/block.rs` |
| HuCard mirroring "as the reference states it" (**unclear**) | `src/bin/replay.rs` |

No GPL or LGPL source was opened for this audit. What this repository adds:

| Part | Source | Verdict |
| --- | --- | --- |
| `src/*.rs` core | transcribed from huc6280-python (MIT) | inherits |
| `scripts/reference_*.py`, `compare_*.py`, `lockstep.py`, `make_*.py`, `seeded_faults.py`, `rung*.sh` | own; drive the pinned Python reference | clean |
| `scripts/fetch_*.sh` | fetch SingleStepTests (MIT), Dormann's tests (GPL-3.0) and the reference; nothing committed | clean |
| `tests/core.rs`, `docs/seeded-faults.md`, `conformance/` | own | clean |
| `scripts/ab_core.py`, `ab_wasm.py`, `ladder.sh`, `board.sh`, `scripts/board-quiet.patch` (a patch to our own pcengine board), `docs/not-kept.md`, `docs/fast-brief.md` | own speed work | clean |

## History (PUBLIC)

Every commit transcribes `v0.3.0` or later, so the Mesen2-shaped and
Mednafen-transcribed decimal code that huc6280-python's `929c9f7`..`9ea9415`
carried never entered this repository. No file was ever deleted; all commits
are authored `alewman`.
