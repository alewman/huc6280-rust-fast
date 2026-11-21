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

/// A handler, named as the reference's `_op_<name>` methods are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Adc,
    And,
    Asl,
    AslA,
    Bbr,
    Bbs,
    Bcc,
    Bcs,
    Beq,
    Bit,
    Bmi,
    Bne,
    Bpl,
    Bra,
    Brk,
    Bsr,
    Bvc,
    Bvs,
    Cla,
    Clc,
    Cld,
    Cli,
    Clv,
    Clx,
    Cly,
    Cmp,
    Cpx,
    Cpy,
    Csh,
    Csl,
    Dec,
    DecA,
    Dex,
    Dey,
    Eor,
    Inc,
    IncA,
    Inx,
    Iny,
    JmpAbs,
    JmpIdx,
    JmpInd,
    Jsr,
    Lda,
    Ldx,
    Ldy,
    Lsr,
    LsrA,
    Nop,
    Ora,
    Pha,
    Php,
    Phx,
    Phy,
    Pla,
    Plp,
    Plx,
    Ply,
    Rmb,
    Rol,
    RolA,
    Ror,
    RorA,
    Rti,
    Rts,
    Sax,
    Say,
    Sbc,
    Sec,
    Sed,
    Sei,
    Set,
    Smb,
    St0,
    St1,
    St2,
    Sta,
    Stx,
    Sty,
    Stz,
    Sxy,
    Tai,
    Tam,
    Tax,
    Tay,
    Tdd,
    Tia,
    Tii,
    Tin,
    Tma,
    Trb,
    Tsb,
    Tst,
    Tsx,
    Txa,
    Txs,
    Tya,
}

/// An addressing mode, the reference's `_ea_<mode>` methods.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The handler makes its own cycles (the reference's `None`).
    Implied,
    Abs,
    Abx,
    Aby,
    Idx,
    Idy,
    Imm,
    Ind,
    Zp,
    Zpx,
    Zpy,
}

/// An operand form, for a disassembler and the instruction length (SW section 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    Imp,
    Imm,
    Zp,
    Zpx,
    Zpy,
    Abs,
    Abx,
    Aby,
    Ind,
    Idx,
    Idy,
    Zpind,
    Abxind,
    Rel,
    Zprel,
    Block,
    Imzp,
    Imzpx,
    Imabs,
    Imabsx,
}

impl Form {
    /// Instruction length in bytes (the reference's `FORM_BYTES`).
    pub const fn bytes(self) -> usize {
        match self {
            Form::Imp => 1,
            Form::Imm
            | Form::Zp
            | Form::Zpx
            | Form::Zpy
            | Form::Idx
            | Form::Idy
            | Form::Zpind
            | Form::Rel => 2,
            Form::Abs
            | Form::Abx
            | Form::Aby
            | Form::Ind
            | Form::Abxind
            | Form::Zprel
            | Form::Imzp
            | Form::Imzpx => 3,
            Form::Imabs | Form::Imabsx => 4,
            Form::Block => 7,
        }
    }
}

/// opcode: (handler, mode, mnemonic, form, base cycles)
pub type Row = (Op, Mode, &'static str, Form, u8);

use Form as F;
use Mode as M;

#[rustfmt::skip]
pub const OPCODES: [Row; 256] = [
    (Op::Brk, M::Implied, "BRK", F::Imm, 8), // 00
    (Op::Ora, M::Idx, "ORA", F::Idx, 7), // 01
    (Op::Sxy, M::Implied, "SXY", F::Imp, 3), // 02
    (Op::St0, M::Implied, "ST0", F::Imm, 4), // 03
    (Op::Tsb, M::Zp, "TSB", F::Zp, 6), // 04
    (Op::Ora, M::Zp, "ORA", F::Zp, 4), // 05
    (Op::Asl, M::Zp, "ASL", F::Zp, 6), // 06
    (Op::Rmb, M::Zp, "RMB0", F::Zp, 7), // 07
    (Op::Php, M::Implied, "PHP", F::Imp, 3), // 08
    (Op::Ora, M::Imm, "ORA", F::Imm, 2), // 09
    (Op::AslA, M::Implied, "ASL", F::Imp, 2), // 0A
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // 0B
    (Op::Tsb, M::Abs, "TSB", F::Abs, 7), // 0C
    (Op::Ora, M::Abs, "ORA", F::Abs, 5), // 0D
    (Op::Asl, M::Abs, "ASL", F::Abs, 7), // 0E
    (Op::Bbr, M::Implied, "BBR0", F::Zprel, 6), // 0F
    (Op::Bpl, M::Implied, "BPL", F::Rel, 2), // 10
    (Op::Ora, M::Idy, "ORA", F::Idy, 7), // 11
    (Op::Ora, M::Ind, "ORA", F::Zpind, 7), // 12
    (Op::St1, M::Implied, "ST1", F::Imm, 4), // 13
    (Op::Trb, M::Zp, "TRB", F::Zp, 6), // 14
    (Op::Ora, M::Zpx, "ORA", F::Zpx, 4), // 15
    (Op::Asl, M::Zpx, "ASL", F::Zpx, 6), // 16
    (Op::Rmb, M::Zp, "RMB1", F::Zp, 7), // 17
    (Op::Clc, M::Implied, "CLC", F::Imp, 2), // 18
    (Op::Ora, M::Aby, "ORA", F::Aby, 5), // 19
    (Op::IncA, M::Implied, "INC", F::Imp, 2), // 1A
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // 1B
    (Op::Trb, M::Abs, "TRB", F::Abs, 7), // 1C
    (Op::Ora, M::Abx, "ORA", F::Abx, 5), // 1D
    (Op::Asl, M::Abx, "ASL", F::Abx, 7), // 1E
    (Op::Bbr, M::Implied, "BBR1", F::Zprel, 6), // 1F
    (Op::Jsr, M::Implied, "JSR", F::Abs, 7), // 20
    (Op::And, M::Idx, "AND", F::Idx, 7), // 21
    (Op::Sax, M::Implied, "SAX", F::Imp, 3), // 22
    (Op::St2, M::Implied, "ST2", F::Imm, 4), // 23
    (Op::Bit, M::Zp, "BIT", F::Zp, 4), // 24
    (Op::And, M::Zp, "AND", F::Zp, 4), // 25
    (Op::Rol, M::Zp, "ROL", F::Zp, 6), // 26
    (Op::Rmb, M::Zp, "RMB2", F::Zp, 7), // 27
    (Op::Plp, M::Implied, "PLP", F::Imp, 4), // 28
    (Op::And, M::Imm, "AND", F::Imm, 2), // 29
    (Op::RolA, M::Implied, "ROL", F::Imp, 2), // 2A
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // 2B
    (Op::Bit, M::Abs, "BIT", F::Abs, 5), // 2C
    (Op::And, M::Abs, "AND", F::Abs, 5), // 2D
    (Op::Rol, M::Abs, "ROL", F::Abs, 7), // 2E
    (Op::Bbr, M::Implied, "BBR2", F::Zprel, 6), // 2F
    (Op::Bmi, M::Implied, "BMI", F::Rel, 2), // 30
    (Op::And, M::Idy, "AND", F::Idy, 7), // 31
    (Op::And, M::Ind, "AND", F::Zpind, 7), // 32
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // 33
    (Op::Bit, M::Zpx, "BIT", F::Zpx, 4), // 34
    (Op::And, M::Zpx, "AND", F::Zpx, 4), // 35
    (Op::Rol, M::Zpx, "ROL", F::Zpx, 6), // 36
    (Op::Rmb, M::Zp, "RMB3", F::Zp, 7), // 37
    (Op::Sec, M::Implied, "SEC", F::Imp, 2), // 38
    (Op::And, M::Aby, "AND", F::Aby, 5), // 39
    (Op::DecA, M::Implied, "DEC", F::Imp, 2), // 3A
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // 3B
    (Op::Bit, M::Abx, "BIT", F::Abx, 5), // 3C
    (Op::And, M::Abx, "AND", F::Abx, 5), // 3D
    (Op::Rol, M::Abx, "ROL", F::Abx, 7), // 3E
    (Op::Bbr, M::Implied, "BBR3", F::Zprel, 6), // 3F
    (Op::Rti, M::Implied, "RTI", F::Imp, 7), // 40
    (Op::Eor, M::Idx, "EOR", F::Idx, 7), // 41
    (Op::Say, M::Implied, "SAY", F::Imp, 3), // 42
    (Op::Tma, M::Implied, "TMA", F::Imm, 4), // 43
    (Op::Bsr, M::Implied, "BSR", F::Rel, 8), // 44
    (Op::Eor, M::Zp, "EOR", F::Zp, 4), // 45
    (Op::Lsr, M::Zp, "LSR", F::Zp, 6), // 46
    (Op::Rmb, M::Zp, "RMB4", F::Zp, 7), // 47
    (Op::Pha, M::Implied, "PHA", F::Imp, 3), // 48
    (Op::Eor, M::Imm, "EOR", F::Imm, 2), // 49
    (Op::LsrA, M::Implied, "LSR", F::Imp, 2), // 4A
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // 4B
    (Op::JmpAbs, M::Implied, "JMP", F::Abs, 4), // 4C
    (Op::Eor, M::Abs, "EOR", F::Abs, 5), // 4D
    (Op::Lsr, M::Abs, "LSR", F::Abs, 7), // 4E
    (Op::Bbr, M::Implied, "BBR4", F::Zprel, 6), // 4F
    (Op::Bvc, M::Implied, "BVC", F::Rel, 2), // 50
    (Op::Eor, M::Idy, "EOR", F::Idy, 7), // 51
    (Op::Eor, M::Ind, "EOR", F::Zpind, 7), // 52
    (Op::Tam, M::Implied, "TAM", F::Imm, 5), // 53
    (Op::Csl, M::Implied, "CSL", F::Imp, 3), // 54
    (Op::Eor, M::Zpx, "EOR", F::Zpx, 4), // 55
    (Op::Lsr, M::Zpx, "LSR", F::Zpx, 6), // 56
    (Op::Rmb, M::Zp, "RMB5", F::Zp, 7), // 57
    (Op::Cli, M::Implied, "CLI", F::Imp, 2), // 58
    (Op::Eor, M::Aby, "EOR", F::Aby, 5), // 59
    (Op::Phy, M::Implied, "PHY", F::Imp, 3), // 5A
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // 5B
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // 5C
    (Op::Eor, M::Abx, "EOR", F::Abx, 5), // 5D
    (Op::Lsr, M::Abx, "LSR", F::Abx, 7), // 5E
    (Op::Bbr, M::Implied, "BBR5", F::Zprel, 6), // 5F
    (Op::Rts, M::Implied, "RTS", F::Imp, 7), // 60
    (Op::Adc, M::Idx, "ADC", F::Idx, 7), // 61
    (Op::Cla, M::Implied, "CLA", F::Imp, 2), // 62
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // 63
    (Op::Stz, M::Zp, "STZ", F::Zp, 4), // 64
    (Op::Adc, M::Zp, "ADC", F::Zp, 4), // 65
    (Op::Ror, M::Zp, "ROR", F::Zp, 6), // 66
    (Op::Rmb, M::Zp, "RMB6", F::Zp, 7), // 67
    (Op::Pla, M::Implied, "PLA", F::Imp, 4), // 68
    (Op::Adc, M::Imm, "ADC", F::Imm, 2), // 69
    (Op::RorA, M::Implied, "ROR", F::Imp, 2), // 6A
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // 6B
    (Op::JmpInd, M::Implied, "JMP", F::Ind, 7), // 6C
    (Op::Adc, M::Abs, "ADC", F::Abs, 5), // 6D
    (Op::Ror, M::Abs, "ROR", F::Abs, 7), // 6E
    (Op::Bbr, M::Implied, "BBR6", F::Zprel, 6), // 6F
    (Op::Bvs, M::Implied, "BVS", F::Rel, 2), // 70
    (Op::Adc, M::Idy, "ADC", F::Idy, 7), // 71
    (Op::Adc, M::Ind, "ADC", F::Zpind, 7), // 72
    (Op::Tii, M::Implied, "TII", F::Block, 17), // 73
    (Op::Stz, M::Zpx, "STZ", F::Zpx, 4), // 74
    (Op::Adc, M::Zpx, "ADC", F::Zpx, 4), // 75
    (Op::Ror, M::Zpx, "ROR", F::Zpx, 6), // 76
    (Op::Rmb, M::Zp, "RMB7", F::Zp, 7), // 77
    (Op::Sei, M::Implied, "SEI", F::Imp, 2), // 78
    (Op::Adc, M::Aby, "ADC", F::Aby, 5), // 79
    (Op::Ply, M::Implied, "PLY", F::Imp, 4), // 7A
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // 7B
    (Op::JmpIdx, M::Implied, "JMP", F::Abxind, 7), // 7C
    (Op::Adc, M::Abx, "ADC", F::Abx, 5), // 7D
    (Op::Ror, M::Abx, "ROR", F::Abx, 7), // 7E
    (Op::Bbr, M::Implied, "BBR7", F::Zprel, 6), // 7F
    (Op::Bra, M::Implied, "BRA", F::Rel, 4), // 80
    (Op::Sta, M::Idx, "STA", F::Idx, 7), // 81
    (Op::Clx, M::Implied, "CLX", F::Imp, 2), // 82
    (Op::Tst, M::Zp, "TST", F::Imzp, 7), // 83
    (Op::Sty, M::Zp, "STY", F::Zp, 4), // 84
    (Op::Sta, M::Zp, "STA", F::Zp, 4), // 85
    (Op::Stx, M::Zp, "STX", F::Zp, 4), // 86
    (Op::Smb, M::Zp, "SMB0", F::Zp, 7), // 87
    (Op::Dey, M::Implied, "DEY", F::Imp, 2), // 88
    (Op::Bit, M::Imm, "BIT", F::Imm, 2), // 89
    (Op::Txa, M::Implied, "TXA", F::Imp, 2), // 8A
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // 8B
    (Op::Sty, M::Abs, "STY", F::Abs, 5), // 8C
    (Op::Sta, M::Abs, "STA", F::Abs, 5), // 8D
    (Op::Stx, M::Abs, "STX", F::Abs, 5), // 8E
    (Op::Bbs, M::Implied, "BBS0", F::Zprel, 6), // 8F
    (Op::Bcc, M::Implied, "BCC", F::Rel, 2), // 90
    (Op::Sta, M::Idy, "STA", F::Idy, 7), // 91
    (Op::Sta, M::Ind, "STA", F::Zpind, 7), // 92
    (Op::Tst, M::Abs, "TST", F::Imabs, 8), // 93
    (Op::Sty, M::Zpx, "STY", F::Zpx, 4), // 94
    (Op::Sta, M::Zpx, "STA", F::Zpx, 4), // 95
    (Op::Stx, M::Zpy, "STX", F::Zpy, 4), // 96
    (Op::Smb, M::Zp, "SMB1", F::Zp, 7), // 97
    (Op::Tya, M::Implied, "TYA", F::Imp, 2), // 98
    (Op::Sta, M::Aby, "STA", F::Aby, 5), // 99
    (Op::Txs, M::Implied, "TXS", F::Imp, 2), // 9A
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // 9B
    (Op::Stz, M::Abs, "STZ", F::Abs, 5), // 9C
    (Op::Sta, M::Abx, "STA", F::Abx, 5), // 9D
    (Op::Stz, M::Abx, "STZ", F::Abx, 5), // 9E
    (Op::Bbs, M::Implied, "BBS1", F::Zprel, 6), // 9F
    (Op::Ldy, M::Imm, "LDY", F::Imm, 2), // A0
    (Op::Lda, M::Idx, "LDA", F::Idx, 7), // A1
    (Op::Ldx, M::Imm, "LDX", F::Imm, 2), // A2
    (Op::Tst, M::Zpx, "TST", F::Imzpx, 7), // A3
    (Op::Ldy, M::Zp, "LDY", F::Zp, 4), // A4
    (Op::Lda, M::Zp, "LDA", F::Zp, 4), // A5
    (Op::Ldx, M::Zp, "LDX", F::Zp, 4), // A6
    (Op::Smb, M::Zp, "SMB2", F::Zp, 7), // A7
    (Op::Tay, M::Implied, "TAY", F::Imp, 2), // A8
    (Op::Lda, M::Imm, "LDA", F::Imm, 2), // A9
    (Op::Tax, M::Implied, "TAX", F::Imp, 2), // AA
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // AB
    (Op::Ldy, M::Abs, "LDY", F::Abs, 5), // AC
    (Op::Lda, M::Abs, "LDA", F::Abs, 5), // AD
    (Op::Ldx, M::Abs, "LDX", F::Abs, 5), // AE
    (Op::Bbs, M::Implied, "BBS2", F::Zprel, 6), // AF
    (Op::Bcs, M::Implied, "BCS", F::Rel, 2), // B0
    (Op::Lda, M::Idy, "LDA", F::Idy, 7), // B1
    (Op::Lda, M::Ind, "LDA", F::Zpind, 7), // B2
    (Op::Tst, M::Abx, "TST", F::Imabsx, 8), // B3
    (Op::Ldy, M::Zpx, "LDY", F::Zpx, 4), // B4
    (Op::Lda, M::Zpx, "LDA", F::Zpx, 4), // B5
    (Op::Ldx, M::Zpy, "LDX", F::Zpy, 4), // B6
    (Op::Smb, M::Zp, "SMB3", F::Zp, 7), // B7
    (Op::Clv, M::Implied, "CLV", F::Imp, 2), // B8
    (Op::Lda, M::Aby, "LDA", F::Aby, 5), // B9
    (Op::Tsx, M::Implied, "TSX", F::Imp, 2), // BA
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // BB
    (Op::Ldy, M::Abx, "LDY", F::Abx, 5), // BC
    (Op::Lda, M::Abx, "LDA", F::Abx, 5), // BD
    (Op::Ldx, M::Aby, "LDX", F::Aby, 5), // BE
    (Op::Bbs, M::Implied, "BBS3", F::Zprel, 6), // BF
    (Op::Cpy, M::Imm, "CPY", F::Imm, 2), // C0
    (Op::Cmp, M::Idx, "CMP", F::Idx, 7), // C1
    (Op::Cly, M::Implied, "CLY", F::Imp, 2), // C2
    (Op::Tdd, M::Implied, "TDD", F::Block, 17), // C3
    (Op::Cpy, M::Zp, "CPY", F::Zp, 4), // C4
    (Op::Cmp, M::Zp, "CMP", F::Zp, 4), // C5
    (Op::Dec, M::Zp, "DEC", F::Zp, 6), // C6
    (Op::Smb, M::Zp, "SMB4", F::Zp, 7), // C7
    (Op::Iny, M::Implied, "INY", F::Imp, 2), // C8
    (Op::Cmp, M::Imm, "CMP", F::Imm, 2), // C9
    (Op::Dex, M::Implied, "DEX", F::Imp, 2), // CA
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // CB
    (Op::Cpy, M::Abs, "CPY", F::Abs, 5), // CC
    (Op::Cmp, M::Abs, "CMP", F::Abs, 5), // CD
    (Op::Dec, M::Abs, "DEC", F::Abs, 7), // CE
    (Op::Bbs, M::Implied, "BBS4", F::Zprel, 6), // CF
    (Op::Bne, M::Implied, "BNE", F::Rel, 2), // D0
    (Op::Cmp, M::Idy, "CMP", F::Idy, 7), // D1
    (Op::Cmp, M::Ind, "CMP", F::Zpind, 7), // D2
    (Op::Tin, M::Implied, "TIN", F::Block, 17), // D3
    (Op::Csh, M::Implied, "CSH", F::Imp, 3), // D4
    (Op::Cmp, M::Zpx, "CMP", F::Zpx, 4), // D5
    (Op::Dec, M::Zpx, "DEC", F::Zpx, 6), // D6
    (Op::Smb, M::Zp, "SMB5", F::Zp, 7), // D7
    (Op::Cld, M::Implied, "CLD", F::Imp, 2), // D8
    (Op::Cmp, M::Aby, "CMP", F::Aby, 5), // D9
    (Op::Phx, M::Implied, "PHX", F::Imp, 3), // DA
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // DB
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // DC
    (Op::Cmp, M::Abx, "CMP", F::Abx, 5), // DD
    (Op::Dec, M::Abx, "DEC", F::Abx, 7), // DE
    (Op::Bbs, M::Implied, "BBS5", F::Zprel, 6), // DF
    (Op::Cpx, M::Imm, "CPX", F::Imm, 2), // E0
    (Op::Sbc, M::Idx, "SBC", F::Idx, 7), // E1
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // E2
    (Op::Tia, M::Implied, "TIA", F::Block, 17), // E3
    (Op::Cpx, M::Zp, "CPX", F::Zp, 4), // E4
    (Op::Sbc, M::Zp, "SBC", F::Zp, 4), // E5
    (Op::Inc, M::Zp, "INC", F::Zp, 6), // E6
    (Op::Smb, M::Zp, "SMB6", F::Zp, 7), // E7
    (Op::Inx, M::Implied, "INX", F::Imp, 2), // E8
    (Op::Sbc, M::Imm, "SBC", F::Imm, 2), // E9
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // EA
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // EB
    (Op::Cpx, M::Abs, "CPX", F::Abs, 5), // EC
    (Op::Sbc, M::Abs, "SBC", F::Abs, 5), // ED
    (Op::Inc, M::Abs, "INC", F::Abs, 7), // EE
    (Op::Bbs, M::Implied, "BBS6", F::Zprel, 6), // EF
    (Op::Beq, M::Implied, "BEQ", F::Rel, 2), // F0
    (Op::Sbc, M::Idy, "SBC", F::Idy, 7), // F1
    (Op::Sbc, M::Ind, "SBC", F::Zpind, 7), // F2
    (Op::Tai, M::Implied, "TAI", F::Block, 17), // F3
    (Op::Set, M::Implied, "SET", F::Imp, 2), // F4
    (Op::Sbc, M::Zpx, "SBC", F::Zpx, 4), // F5
    (Op::Inc, M::Zpx, "INC", F::Zpx, 6), // F6
    (Op::Smb, M::Zp, "SMB7", F::Zp, 7), // F7
    (Op::Sed, M::Implied, "SED", F::Imp, 2), // F8
    (Op::Sbc, M::Aby, "SBC", F::Aby, 5), // F9
    (Op::Plx, M::Implied, "PLX", F::Imp, 4), // FA
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // FB
    (Op::Nop, M::Implied, "NOP", F::Imp, 2), // FC
    (Op::Sbc, M::Abx, "SBC", F::Abx, 5), // FD
    (Op::Inc, M::Abx, "INC", F::Abx, 7), // FE
    (Op::Bbs, M::Implied, "BBS7", F::Zprel, 6), // FF
];

impl<B: Bus> HuC6280<B> {
    /// The operand's logical address, after the mode's addressing cycles.
    pub(crate) fn ea(&mut self, mode: Mode) -> u16 {
        match mode {
            Mode::Implied => unreachable!("the handler makes its own cycles"),
            Mode::Abs => self.ea_abs(),
            Mode::Abx => self.ea_abx(),
            Mode::Aby => self.ea_aby(),
            Mode::Idx => self.ea_idx(),
            Mode::Idy => self.ea_idy(),
            Mode::Imm => self.ea_imm(),
            Mode::Ind => self.ea_ind(),
            Mode::Zp => self.ea_zp(),
            Mode::Zpx => self.ea_zpx(),
            Mode::Zpy => self.ea_zpy(),
        }
    }

    /// Run the handler of one table row.
    pub(crate) fn dispatch(&mut self, op: Op, mode: Mode) {
        match op {
            Op::Adc => self.op_adc(mode),
            Op::And => self.op_and(mode),
            Op::Asl => self.op_asl(mode),
            Op::AslA => self.op_asl_a(),
            Op::Bbr => self.op_bbr(),
            Op::Bbs => self.op_bbs(),
            Op::Bcc => self.op_bcc(),
            Op::Bcs => self.op_bcs(),
            Op::Beq => self.op_beq(),
            Op::Bit => self.op_bit(mode),
            Op::Bmi => self.op_bmi(),
            Op::Bne => self.op_bne(),
            Op::Bpl => self.op_bpl(),
            Op::Bra => self.op_bra(),
            Op::Brk => self.op_brk(),
            Op::Bsr => self.op_bsr(),
            Op::Bvc => self.op_bvc(),
            Op::Bvs => self.op_bvs(),
            Op::Cla => self.op_cla(),
            Op::Clc => self.op_clc(),
            Op::Cld => self.op_cld(),
            Op::Cli => self.op_cli(),
            Op::Clv => self.op_clv(),
            Op::Clx => self.op_clx(),
            Op::Cly => self.op_cly(),
            Op::Cmp => self.op_cmp(mode),
            Op::Cpx => self.op_cpx(mode),
            Op::Cpy => self.op_cpy(mode),
            Op::Csh => self.op_csh(),
            Op::Csl => self.op_csl(),
            Op::Dec => self.op_dec(mode),
            Op::DecA => self.op_dec_a(),
            Op::Dex => self.op_dex(),
            Op::Dey => self.op_dey(),
            Op::Eor => self.op_eor(mode),
            Op::Inc => self.op_inc(mode),
            Op::IncA => self.op_inc_a(),
            Op::Inx => self.op_inx(),
            Op::Iny => self.op_iny(),
            Op::JmpAbs => self.op_jmp_abs(),
            Op::JmpIdx => self.op_jmp_idx(),
            Op::JmpInd => self.op_jmp_ind(),
            Op::Jsr => self.op_jsr(),
            Op::Lda => self.op_lda(mode),
            Op::Ldx => self.op_ldx(mode),
            Op::Ldy => self.op_ldy(mode),
            Op::Lsr => self.op_lsr(mode),
            Op::LsrA => self.op_lsr_a(),
            Op::Nop => self.op_nop(),
            Op::Ora => self.op_ora(mode),
            Op::Pha => self.op_pha(),
            Op::Php => self.op_php(),
            Op::Phx => self.op_phx(),
            Op::Phy => self.op_phy(),
            Op::Pla => self.op_pla(),
            Op::Plp => self.op_plp(),
            Op::Plx => self.op_plx(),
            Op::Ply => self.op_ply(),
            Op::Rmb => self.op_rmb(mode),
            Op::Rol => self.op_rol(mode),
            Op::RolA => self.op_rol_a(),
            Op::Ror => self.op_ror(mode),
            Op::RorA => self.op_ror_a(),
            Op::Rti => self.op_rti(),
            Op::Rts => self.op_rts(),
            Op::Sax => self.op_sax(),
            Op::Say => self.op_say(),
            Op::Sbc => self.op_sbc(mode),
            Op::Sec => self.op_sec(),
            Op::Sed => self.op_sed(),
            Op::Sei => self.op_sei(),
            Op::Set => self.op_set(),
            Op::Smb => self.op_smb(mode),
            Op::St0 => self.op_st0(),
            Op::St1 => self.op_st1(),
            Op::St2 => self.op_st2(),
            Op::Sta => self.op_sta(mode),
            Op::Stx => self.op_stx(mode),
            Op::Sty => self.op_sty(mode),
            Op::Stz => self.op_stz(mode),
            Op::Sxy => self.op_sxy(),
            Op::Tai => self.op_tai(),
            Op::Tam => self.op_tam(),
            Op::Tax => self.op_tax(),
            Op::Tay => self.op_tay(),
            Op::Tdd => self.op_tdd(),
            Op::Tia => self.op_tia(),
            Op::Tii => self.op_tii(),
            Op::Tin => self.op_tin(),
            Op::Tma => self.op_tma(),
            Op::Trb => self.op_trb(mode),
            Op::Tsb => self.op_tsb(mode),
            Op::Tst => self.op_tst(mode),
            Op::Tsx => self.op_tsx(),
            Op::Txa => self.op_txa(),
            Op::Txs => self.op_txs(),
            Op::Tya => self.op_tya(),
        }
    }
}
