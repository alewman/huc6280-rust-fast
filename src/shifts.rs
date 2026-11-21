// SPDX-License-Identifier: MIT
//! Shifts, rotates, INC and DEC, on the accumulator and on memory: ASL LSR
//! ROL ROR INC DEC (`_shifts.py`).
//!
//! The accumulator forms are two cycles; the memory forms read, spend an
//! internal cycle and write (no dummy write, unlike the NMOS 6502).

use crate::core::{nz, Bus, HuC6280, C, N, Z};
use crate::dispatch::Mode;

impl<B: Bus> HuC6280<B> {
    fn shifted(&mut self, result: u8, carry: u8) -> u8 {
        self.p = (self.p & !(N | Z | C)) | nz(result) | carry;
        result
    }

    fn asl(&mut self, value: u8) -> u8 {
        self.shifted(value << 1, if value & 0x80 != 0 { C } else { 0 })
    }

    fn lsr(&mut self, value: u8) -> u8 {
        self.shifted(value >> 1, value & C)
    }

    fn rol(&mut self, value: u8) -> u8 {
        self.shifted(
            (value << 1) | (self.p & C),
            if value & 0x80 != 0 { C } else { 0 },
        )
    }

    fn ror(&mut self, value: u8) -> u8 {
        self.shifted((value >> 1) | ((self.p & C) << 7), value & C)
    }

    fn inc(&mut self, value: u8) -> u8 {
        self.set_nz(value.wrapping_add(1))
    }

    fn dec(&mut self, value: u8) -> u8 {
        self.set_nz(value.wrapping_sub(1))
    }

    fn accumulator(&mut self, operation: fn(&mut Self, u8) -> u8) {
        self.implied();
        self.a = operation(self, self.a);
    }

    /// ASL -- shift M left, bit 7 -> C (SW SB-19).
    pub(crate) fn op_asl(&mut self, mode: Mode) {
        let address = self.ea(mode);
        self.modify(address, Self::asl);
    }

    /// ASL A -- shift A left, bit 7 -> C (SW SB-19).
    pub(crate) fn op_asl_a(&mut self) {
        self.accumulator(Self::asl);
    }

    /// LSR -- shift M right, bit 0 -> C (SW SB-56).
    pub(crate) fn op_lsr(&mut self, mode: Mode) {
        let address = self.ea(mode);
        self.modify(address, Self::lsr);
    }

    /// LSR A -- shift A right, bit 0 -> C (SW SB-56).
    pub(crate) fn op_lsr_a(&mut self) {
        self.accumulator(Self::lsr);
    }

    /// ROL -- rotate M left through C (SW SB-68).
    pub(crate) fn op_rol(&mut self, mode: Mode) {
        let address = self.ea(mode);
        self.modify(address, Self::rol);
    }

    /// ROL A -- rotate A left through C (SW SB-68).
    pub(crate) fn op_rol_a(&mut self) {
        self.accumulator(Self::rol);
    }

    /// ROR -- rotate M right through C (SW SB-69).
    pub(crate) fn op_ror(&mut self, mode: Mode) {
        let address = self.ea(mode);
        self.modify(address, Self::ror);
    }

    /// ROR A -- rotate A right through C (SW SB-69).
    pub(crate) fn op_ror_a(&mut self) {
        self.accumulator(Self::ror);
    }

    /// INC -- M + 1 -> M (SW SB-48).
    pub(crate) fn op_inc(&mut self, mode: Mode) {
        let address = self.ea(mode);
        self.modify(address, Self::inc);
    }

    /// INC A -- A + 1 -> A (SW SB-48).
    pub(crate) fn op_inc_a(&mut self) {
        self.accumulator(Self::inc);
    }

    /// DEC -- M - 1 -> M (SW SB-44).
    pub(crate) fn op_dec(&mut self, mode: Mode) {
        let address = self.ea(mode);
        self.modify(address, Self::dec);
    }

    /// DEC A -- A - 1 -> A (SW SB-44).
    pub(crate) fn op_dec_a(&mut self) {
        self.accumulator(Self::dec);
    }
}
