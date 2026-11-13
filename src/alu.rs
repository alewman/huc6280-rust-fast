//! Arithmetic, logic, compare and bit test: ADC SBC AND ORA EOR CMP CPX CPY
//! BIT (`_alu.py`).
//!
//! `ORA`, `AND`, `EOR` and `ADC` honour the T flag (U4): with T set the
//! accumulator is replaced by the zero-page byte at X (logical `$2000 + X`),
//! the result goes back there, A is untouched and the instruction takes three
//! more cycles (read, internal, write). `SBC` does not (C5: SW's entry has no
//! T clause; MAME's yes is a guess), unless `sbc_honours_t` is set.
//!
//! Decimal mode (U14, HW HB-4, SW `ADC`): one more cycle, N, Z and C taken
//! from the decimal result, **V left alone**. How invalid digits adjust is
//! C13: neither manual says. `decimal_formula` picks the digit adjust. The
//! default, `Mednafen`, gives the results Mednafen's HuC6280 gives
//! ("corrected ... per tests on a HuC6280", its ChangeLog, 2015-04-29: the
//! one author statement of hardware testing); `Mesen` gives Mesen's, the
//! results the 640,000-case SingleStepTests corpus was made with. They
//! differ only with an invalid BCD digit (the reference's docs/issues/0004).
//! Both are transcribed from the reference's own routines, which were
//! written from the rules stated in words there and checked against recorded
//! result tables; neither emulator's code is copied.
//!
//! Differences forced by Rust: the digit arithmetic is done in `i32`, with
//! `rem_euclid` where the reference's `%` meets a negative number (Python's
//! `%` takes the sign of the divisor); an unknown formula cannot be named,
//! so the reference's `ValueError` has no counterpart.

use crate::core::{Bus, HuC6280, C, D, N, V, Z, ZERO_PAGE};
use crate::dispatch::Mode;

/// C13: the decimal digit adjust of ADC and SBC (`decimal_formula`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecimalFormula {
    /// The default: the results Mednafen gives.
    Mednafen,
    /// The results Mesen gives, which the SingleStepTests corpus records.
    Mesen,
}

/// N and Z for a result that has not been masked to 8 bits yet.
fn nz(value: i32) -> u8 {
    (value as u8 & N) | if value & 0xFF != 0 { 0 } else { Z }
}

/// The decimal sum before masking: 256 and up is the carry out.
///
/// Written from the rule each emulator's result table follows (the
/// reference's docs/issues/0004 states both in words), digit by digit; no
/// emulator's source is reproduced here.
fn decimal_add(a: i32, operand: i32, carry: i32, formula: DecimalFormula) -> i32 {
    let mut ones = a % 16 + operand % 16 + carry;
    let sixteens = a / 16 + operand / 16;
    if ones >= 10 {
        ones += 6; // a digit past 9 skips the six unused codes
    }
    let mut total = match formula {
        // Everything the ones digit overflowed by is carried, however much that is.
        DecimalFormula::Mednafen => 16 * sixteens + ones,
        // One carry at most, and only the low four bits of the ones digit are kept.
        DecimalFormula::Mesen => 16 * (sixteens + if ones >= 16 { 1 } else { 0 }) + ones % 16,
    };
    if total >= 0xA0 {
        total += 0x60;
    }
    total
}

/// The decimal difference: 256 and up means no borrow (carry out). Written as above.
fn decimal_subtract(a: i32, operand: i32, carry: i32, formula: DecimalFormula) -> i32 {
    match formula {
        DecimalFormula::Mednafen => {
            // Digit differences with a borrow between them; each digit that
            // borrowed is corrected (by 6, by $60) on the assembled byte,
            // modulo 256.
            let ones = a % 16 - operand % 16 - (1 - carry);
            let sixteens = a / 16 - operand / 16 - if ones < 0 { 1 } else { 0 };
            let mut byte = 16 * sixteens.rem_euclid(16) + ones.rem_euclid(16);
            if ones < 0 {
                byte -= 0x06;
            }
            if sixteens < 0 {
                byte -= 0x60;
            }
            byte.rem_euclid(256) + if sixteens < 0 { 0 } else { 256 }
        }
        DecimalFormula::Mesen => {
            // The sum with the operand's complement; a digit that produced no
            // carry is corrected before the next is formed, and the
            // correction is not reduced modulo 256 here.
            let complement = 255 - operand;
            let mut ones = a % 16 + complement % 16 + carry;
            if ones < 16 {
                ones -= 6;
            }
            let mut total = 16 * (a / 16 + complement / 16 + if ones >= 16 { 1 } else { 0 })
                + ones.rem_euclid(16);
            if total < 256 {
                total -= 0x60;
            }
            total
        }
    }
}

/// A + operand + C, binary or decimal; return (result, P).
pub fn add(a: u8, operand: u8, p: u8, formula: DecimalFormula) -> (u8, u8) {
    let (a, operand) = (a as i32, operand as i32);
    let carry = (p & C) as i32;
    let mut p = p;
    let result;
    if p & D != 0 {
        result = decimal_add(a, operand, carry, formula);
        p &= !(N | Z | C); // V is unchanged in decimal mode (U14)
    } else {
        result = a + operand + carry;
        p &= !(N | V | Z | C);
        if !(a ^ operand) & (a ^ result) & 0x80 != 0 {
            p |= V;
        }
    }
    if result > 0xFF {
        p |= C;
    }
    (result as u8, p | nz(result))
}

/// A - operand - (1 - C), binary or decimal; return (result, P).
pub fn subtract(a: u8, operand: u8, p: u8, formula: DecimalFormula) -> (u8, u8) {
    let (a, operand) = (a as i32, operand as i32);
    let carry = (p & C) as i32;
    let mut p = p;
    let result;
    if p & D != 0 {
        result = decimal_subtract(a, operand, carry, formula);
        p &= !(N | Z | C);
    } else {
        let inverted = !operand & 0xFF;
        result = a + inverted + carry;
        p &= !(N | V | Z | C);
        if !(a ^ inverted) & (a ^ result) & 0x80 != 0 {
            p |= V;
        }
    }
    if result > 0xFF {
        p |= C;
    }
    (result as u8, p | nz(result))
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
            // memory mode but keeps its cycle; Mesen).
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
