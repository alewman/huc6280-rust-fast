#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Write src/dispatch.rs from the reference's ``OPCODES`` dict (run once, kept for the record).

    external/huc6280-python/.venv/bin/python scripts/make_dispatch.py > src/dispatch.rs

The table is the reference's, row for row; the enums and the two matches are
derived from the handler and mode names it uses, so nothing is typed twice.
"""

from __future__ import annotations

from huc6280._dispatch import FORM_BYTES, OPCODES

HEADER = '''\
// SPDX-License-Identifier: MIT
//! The opcode map: handler, addressing mode, mnemonic, operand form and base
//! cycles (`_dispatch.py`).
//!
//! One row per opcode, all 256: the 234 defined opcodes and the 22 undefined
//! ones, which are one-byte two-cycle `NOP`s (U3). The reference's rung 0
//! checks every row against Hudson's software manual; this table was
//! generated from the reference's `OPCODES` dict (`scripts/make_dispatch.py`).
//!
//! - the handler: [`Op`], the reference's `_op_<handler>` method;
//! - the mode: [`Mode`], the `_ea_<mode>` method that makes the addressing
//!   cycles and returns the operand's logical address, or `Implied` (the
//!   reference's `None`) when the handler makes its own cycles (implied,
//!   accumulator, the stack, branches, jumps, the Hudson instructions);
//! - the mnemonic and the [`Form`]: for a disassembler; [`Form::bytes`] gives
//!   the instruction length;
//! - the cycles: the base count with T = 0, D = 0 and no branch taken (SW),
//!   which rung 2 confirms by running the instruction. The block transfers
//!   print their fixed part; the rest is 6 cycles a byte (HW HB-6). The core
//!   does not read this column: every handler makes its cycles one by one and
//!   `step()` returns how many it made.
//!
//! Differences forced by Rust: the reference resolves the table into bound
//! methods once per class; here dispatch is the table and two explicit
//! matches, the handler's and (inside the handlers that take one, at the
//! point where the reference calls `mode(self)`) the mode's.

use crate::core::{Bus, HuC6280};
'''


def camel(name: str) -> str:
    return "".join(part.capitalize() for part in name.split("_"))


def main() -> None:
    handlers = sorted({row[0] for row in OPCODES.values()})
    modes = sorted({row[1] for row in OPCODES.values() if row[1]})
    with_mode = {row[0] for row in OPCODES.values() if row[1]}
    without = {row[0] for row in OPCODES.values() if not row[1]}
    assert not with_mode & without
    out = [HEADER]
    out.append("/// A handler, named as the reference's `_op_<name>` methods are.")
    out.append("#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub enum Op {")
    out += [f"    {camel(h)}," for h in handlers]
    out.append("}\n")
    out.append("/// An addressing mode, the reference's `_ea_<mode>` methods.")
    out.append("#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub enum Mode {")
    out.append("    /// The handler makes its own cycles (the reference's `None`).\n    Implied,")
    out += [f"    {camel(m)}," for m in modes]
    out.append("}\n")
    out.append("/// An operand form, for a disassembler and the instruction length (SW section 3).")
    out.append("#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub enum Form {")
    out += [f"    {camel(f)}," for f in FORM_BYTES]
    out.append("}\n")
    out.append("impl Form {\n    /// Instruction length in bytes (the reference's `FORM_BYTES`).")
    out.append("    pub const fn bytes(self) -> usize {\n        match self {")
    for size in sorted(set(FORM_BYTES.values())):
        forms = " | ".join(f"Form::{camel(f)}" for f, n in FORM_BYTES.items() if n == size)
        out.append(f"            {forms} => {size},")
    out.append("        }\n    }\n}\n")
    out.append("/// opcode: (handler, mode, mnemonic, form, base cycles)")
    out.append("pub type Row = (Op, Mode, &'static str, Form, u8);\n")
    out.append("use Form as F;\nuse Mode as M;\n")
    out.append("#[rustfmt::skip]\npub const OPCODES: [Row; 256] = [")
    for opcode in range(256):
        handler, mode, mnemonic, form, cycles = OPCODES[opcode]
        mode_text = f"M::{camel(mode)}" if mode else "M::Implied"
        out.append(
            f'    (Op::{camel(handler)}, {mode_text}, "{mnemonic}", F::{camel(form)}, {cycles}), // {opcode:02X}'
        )
    out.append("];\n")
    out.append("impl<B: Bus> HuC6280<B> {")
    out.append("    /// The operand's logical address, after the mode's addressing cycles.")
    out.append("    pub(crate) fn ea(&mut self, mode: Mode) -> u16 {\n        match mode {")
    out.append('            Mode::Implied => unreachable!("the handler makes its own cycles"),')
    out += [f"            Mode::{camel(m)} => self.ea_{m}()," for m in modes]
    out.append("        }\n    }\n")
    out.append("    /// Run the handler of one table row.")
    out.append("    pub(crate) fn dispatch(&mut self, op: Op, mode: Mode) {\n        match op {")
    for h in handlers:
        argument = "mode" if h in with_mode else ""
        out.append(f"            Op::{camel(h)} => self.op_{h}({argument}),")
    out.append("        }\n    }\n}")
    print("\n".join(out))


if __name__ == "__main__":
    main()
