// SPDX-License-Identifier: MIT
//! Bit instructions: RMB SMB BBR BBS TSB TRB (`_bits.py`).
//!
//! `RMBi`/`SMBi`/`BBRi`/`BBSi` take the bit number from the opcode
//! (`10i + code`, SW). They are the Rockwell/WDC 65C02 additions (U22); the
//! cycle counts are not the 65C02's.

use crate::core::{Bus, HuC6280, N, V, Z, ZERO_PAGE};
use crate::dispatch::Mode;

impl<B: Bus> HuC6280<B> {
    fn bit_mask(&self) -> u8 {
        1 << ((self.pins.opcode >> 4) & 7)
    }

    fn bit_branch(&mut self, branch_if_set: bool) {
        let zp = self.fetch();
        self.io();
        let offset = self.fetch();
        self.io();
        let value = self.cycle_read(ZERO_PAGE | zp as u16);
        if (value & self.bit_mask() != 0) == branch_if_set {
            self.io();
            self.io();
            self.pc = self.pc.wrapping_add(offset as i8 as u16);
        }
    }

    /// BBRi -- branch if bit i of the zero-page byte is 0 (SW SB-20).
    pub(crate) fn op_bbr(&mut self) {
        self.bit_branch(false);
    }

    /// BBSi -- branch if bit i of the zero-page byte is 1 (SW SB-21).
    pub(crate) fn op_bbs(&mut self) {
        self.bit_branch(true);
    }

    fn bit_write(&mut self, address: u16, clear: bool) {
        let value = self.cycle_read(address);
        self.io();
        self.io();
        let mask = self.bit_mask();
        self.cycle_write(address, if clear { value & !mask } else { value | mask });
    }

    /// RMBi -- clear bit i of the zero-page byte (SW SB-67).
    pub(crate) fn op_rmb(&mut self, mode: Mode) {
        let address = self.ea(mode);
        self.bit_write(address, true);
    }

    /// SMBi -- set bit i of the zero-page byte (SW SB-79).
    pub(crate) fn op_smb(&mut self, mode: Mode) {
        let address = self.ea(mode);
        self.bit_write(address, false);
    }

    fn test_and_modify(&mut self, address: u16, operation: fn(&Self, u8) -> u8) {
        // N and V from the memory byte before the change (HW HB-5, SW TSB).
        let value = self.cycle_read(address);
        let result = operation(self, value);
        let zero = if self.tsb_z_from_test {
            value & self.a == 0
        } else {
            result == 0
        };
        self.p = (self.p & !(N | V | Z)) | (value & (N | V)) | if zero { Z } else { 0 };
        self.io();
        self.cycle_write(address, result);
    }

    /// TSB -- A OR M -> M; N and V from M, Z from the result (U23) (SW SB-103).
    pub(crate) fn op_tsb(&mut self, mode: Mode) {
        let address = self.ea(mode);
        self.test_and_modify(address, |cpu, value| value | cpu.a);
    }

    /// TRB -- (NOT A) AND M -> M; N and V from M, Z from the result (U23)
    /// (SW SB-102).
    pub(crate) fn op_trb(&mut self, mode: Mode) {
        let address = self.ea(mode);
        self.test_and_modify(address, |cpu, value| value & !cpu.a);
    }
}
