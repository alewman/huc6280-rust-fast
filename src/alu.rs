// SPDX-License-Identifier: MIT
//! Arithmetic, logic and compare: ADC SBC AND ORA EOR CMP CPX CPY BIT
//! (`_alu.py`).
//!
//! C13, decimal ADC and SBC (rewritten 2026-11-19, docs/restart.md; the
//! reference's docs/restart.md). Neither Hudson manual says what the digit
//! adjust does with a digit above 9, and no hardware result is known, so two
//! emulators' behaviour is kept, each written in the reference from its own
//! result table alone:
//!
//! - [`DecimalFormula::Mednafen`] (default): the table recorded by running
//!   Mednafen 1.32.1 (patched for tracing only) as a closed box over every A,
//!   operand and carry (the reference's `scripts/decimal_tables.py`).
//!   OBSERVED, provisional; closed-box measurement (emulator tier, Mednafen
//!   1.32.1).
//! - [`DecimalFormula::Mesen`]: the 11,346 decimal ADC/SBC cases of
//!   SingleStepTests huc6280 (MIT test data recorded from Mesen). OBSERVED,
//!   provisional; closed-box measurement (emulator tier, the corpus's Mesen).
//!
//! The two agree whenever both digits of both inputs are 0-9. In both tables V
//! is left as it was and N and Z come from the stored result (TEST ROM tier for
//! the valid digits: the corpus and the Mednafen run agree, and both pass
//! Dormann's and Rockwell's decimal cases).
//!
//! Differences forced by Rust: the formula is an enum, so the reference's
//! `ValueError` for an unknown formula (raised when a decimal ADC or SBC runs)
//! has no counterpart; the digit arithmetic is in `i32`, where Python's
//! integers would go negative or past a byte.

use crate::core::{nz, Bus, HuC6280, C, D, N, V, Z, ZERO_PAGE};
use crate::dispatch::Mode;

/// C13: the decimal digit adjust of ADC and SBC (the reference's
/// `DECIMAL_FORMULAS = ("mednafen", "mesen")`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecimalFormula {
    /// `"mednafen"` (default): Mednafen 1.32.1's results, recorded closed box.
    Mednafen,
    /// `"mesen"`: the SingleStepTests corpus's (Mesen's) results.
    Mesen,
}

/// Decimal A + M + C: the 8-bit result and the carry out.
fn decimal_add(a: u8, operand: u8, carry: u8, formula: DecimalFormula) -> (u8, bool) {
    let (a, operand, carry) = (a as i32, operand as i32, carry as i32);
    let mut low = (a & 0x0F) + (operand & 0x0F) + carry;
    if formula == DecimalFormula::Mesen {
        // Each digit on its own: above 9 it gains 6, and passes a carry of 1 to the next.
        if low > 9 {
            low += 6;
        }
        let mut high = (a >> 4) + (operand >> 4) + (low > 0x0F) as i32;
        if high > 9 {
            high += 6;
        }
        return (((high << 4) | (low & 0x0F)) as u8, high > 0x0F);
    }
    // Mednafen: the binary sum, plus 6 when the low digits' sum is above 9, then plus $60
    // when what stands above the low digit is above 9 (so a low digit past $1F carries 2).
    let mut total = a + operand + carry;
    if low > 9 {
        total += 0x06;
    }
    if total >> 4 > 9 {
        total += 0x60;
    }
    (total as u8, total > 0xFF)
}

/// Decimal A - M - (1 - C): the 8-bit result and the carry out (no borrow).
fn decimal_subtract(a: u8, operand: u8, carry: u8, formula: DecimalFormula) -> (u8, bool) {
    let (a, operand) = (a as i32, operand as i32);
    let borrow = 1 - carry as i32;
    let mut low = (a & 0x0F) - (operand & 0x0F) - borrow;
    if formula == DecimalFormula::Mesen {
        // Each digit on its own: below 0 it loses 6, and takes a borrow of 1 from the next.
        if low < 0 {
            low -= 6;
        }
        let mut high = (a >> 4) - (operand >> 4) - (low < 0) as i32;
        if high < 0 {
            high -= 6;
        }
        return (((high << 4) | (low & 0x0F)) as u8, high >= 0);
    }
    // Mednafen: the binary difference, minus 6 when the low digits borrow, minus $60 when
    // the whole subtraction borrows; the carry is the binary one.
    let difference = a - operand - borrow;
    let mut result = difference;
    if low < 0 {
        result -= 0x06;
    }
    if difference < 0 {
        result -= 0x60;
    }
    (result as u8, difference >= 0)
}

/// ADC: (result, new P). Binary: N V Z C as the 6502 (SW SB-17); decimal: C13
/// (the module's head).
pub fn add(a: u8, operand: u8, p: u8, formula: DecimalFormula) -> (u8, u8) {
    let carry = p & C;
    if p & D != 0 {
        let (result, carry_out) = decimal_add(a, operand, carry, formula);
        return (
            result,
            (p & !(N | Z | C)) | nz(result) | if carry_out { C } else { 0 },
        );
    }
    let total = a as u16 + operand as u16 + carry as u16;
    let result = total as u8;
    let overflow = !(a ^ operand) & (a ^ result) & 0x80;
    let p = (p & !(N | V | Z | C)) | nz(result) | if overflow != 0 { V } else { 0 };
    (result, p | if total > 0xFF { C } else { 0 })
}

/// SBC: (result, new P). Binary: N V Z C as the 6502 (SW SB-74); decimal: C13
/// (the module's head).
pub fn subtract(a: u8, operand: u8, p: u8, formula: DecimalFormula) -> (u8, u8) {
    let carry = p & C;
    if p & D != 0 {
        let (result, carry_out) = decimal_subtract(a, operand, carry, formula);
        return (
            result,
            (p & !(N | Z | C)) | nz(result) | if carry_out { C } else { 0 },
        );
    }
    let difference = a as i32 - operand as i32 - (1 - carry as i32);
    let result = difference as u8;
    let overflow = (a ^ operand) & (a ^ result) & 0x80;
    let p = (p & !(N | V | Z | C)) | nz(result) | if overflow != 0 { V } else { 0 };
    (result, p | if difference >= 0 { C } else { 0 })
}

/// ADC's or SBC's arithmetic: [`add`] or [`subtract`].
type Arithmetic = fn(u8, u8, u8, DecimalFormula) -> (u8, u8);

impl<B: Bus> HuC6280<B> {
    /// U4: T set, so the operation's other operand is M($2000 + X) and the
    /// result goes back.
    ///
    /// The three extra cycles are a read, an internal cycle (two in decimal
    /// mode, where the adjust takes one more) and a write.
    fn memory_mode(
        &mut self,
        operand: u8,
        operation: impl FnOnce(&mut Self, u8, u8) -> u8,
        extra_cycle: bool,
    ) {
        let address = ZERO_PAGE | self.x as u16;
        let memory = self.cycle_read(address);
        self.io();
        if extra_cycle {
            self.io();
        }
        let result = operation(self, memory, operand);
        self.set_nz(result);
        self.cycle_write(address, result);
    }

    /// ORA -- A OR M -> A, or M(X) OR M -> M(X) with T set (SW SB-58).
    #[inline(always)]
    pub(crate) fn op_ora(&mut self, mode: Mode) {
        let address = self.ea(mode);
        let operand = self.load(address);
        if self.t != 0 {
            self.memory_mode(operand, |_, memory, value| memory | value, false);
        } else {
            self.a = self.set_nz(self.a | operand);
        }
    }

    /// AND -- A AND M -> A, or M(X) AND M -> M(X) with T set (SW SB-18).
    #[inline(always)]
    pub(crate) fn op_and(&mut self, mode: Mode) {
        let address = self.ea(mode);
        let operand = self.load(address);
        if self.t != 0 {
            self.memory_mode(operand, |_, memory, value| memory & value, false);
        } else {
            self.a = self.set_nz(self.a & operand);
        }
    }

    /// EOR -- A EOR M -> A, or M(X) EOR M -> M(X) with T set (SW SB-47).
    #[inline(always)]
    pub(crate) fn op_eor(&mut self, mode: Mode) {
        let address = self.ea(mode);
        let operand = self.load(address);
        if self.t != 0 {
            self.memory_mode(operand, |_, memory, value| memory ^ value, false);
        } else {
            self.a = self.set_nz(self.a ^ operand);
        }
    }

    #[inline(always)]
    fn arithmetic(&mut self, mode: Mode, operation: Arithmetic, uses_t: bool) {
        let address = self.ea(mode);
        let operand = self.load(address);
        let decimal = self.p & D != 0;
        if uses_t && self.t != 0 {
            // The adjust's extra cycle (U14) sits among the memory-mode cycles.
            self.memory_mode(
                operand,
                |cpu, memory, value| {
                    let (result, p) = operation(memory, value, cpu.p, cpu.decimal_formula);
                    cpu.p = p;
                    result
                },
                decimal,
            );
            return;
        }
        if decimal {
            // U14: the extra cycle. A dummy read at the next opcode, or an
            // internal cycle for SBC while T is set (it does not use the
            // memory mode but keeps its cycle; what the SingleStepTests
            // corpus records, emulator tier).
            if self.t != 0 {
                self.io();
            } else {
                self.dummy(self.pc);
            }
        }
        (self.a, self.p) = operation(self.a, operand, self.p, self.decimal_formula);
    }

    /// ADC -- A + M + C -> A, or M(X) + M + C -> M(X) with T set (SW SB-17).
    #[inline(always)]
    pub(crate) fn op_adc(&mut self, mode: Mode) {
        self.arithmetic(mode, add, true);
    }

    /// SBC -- A - M - (1 - C) -> A; T is ignored (C5) (SW SB-74).
    #[inline(always)]
    pub(crate) fn op_sbc(&mut self, mode: Mode) {
        self.arithmetic(mode, subtract, self.sbc_honours_t);
    }

    fn compare(&mut self, register: u8, operand: u8) {
        let difference = register.wrapping_sub(operand);
        let mut p = self.p & !(N | Z | C);
        if register >= operand {
            p |= C;
        }
        self.p = p | crate::core::nz(difference);
    }

    /// CMP -- A - M, flags only (SW SB-41).
    #[inline(always)]
    pub(crate) fn op_cmp(&mut self, mode: Mode) {
        let address = self.ea(mode);
        let operand = self.load(address);
        self.compare(self.a, operand);
    }

    /// CPX -- X - M, flags only (SW SB-42).
    #[inline(always)]
    pub(crate) fn op_cpx(&mut self, mode: Mode) {
        let address = self.ea(mode);
        let operand = self.load(address);
        self.compare(self.x, operand);
    }

    /// CPY -- Y - M, flags only (SW SB-43).
    #[inline(always)]
    pub(crate) fn op_cpy(&mut self, mode: Mode) {
        let address = self.ea(mode);
        let operand = self.load(address);
        self.compare(self.y, operand);
    }

    fn bit_flags(&mut self, operand: u8, mask_with: u8) {
        // U19: N and V come from the operand, Z from the AND; HW HB-5, SW BIT.
        let mut p = (self.p & !(N | V | Z)) | (operand & (N | V));
        if mask_with & operand == 0 {
            p |= Z;
        }
        self.p = p;
    }

    /// BIT -- A AND M sets Z; M bits 7 and 6 -> N and V, immediate form too
    /// (U19) (SW SB-25).
    #[inline(always)]
    pub(crate) fn op_bit(&mut self, mode: Mode) {
        let address = self.ea(mode);
        let operand = self.load(address);
        self.bit_flags(operand, self.a);
    }
}
