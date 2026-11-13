//! Loads, stores, register transfers and the index counters: LDA LDX LDY STA
//! STX STY STZ TAX TAY TXA TYA TSX TXS INX INY DEX DEY (`_loads.py`).
//!
//! The implied ones are two cycles, the opcode fetch and a dummy read of the
//! byte after it.

use crate::core::{Bus, HuC6280};
use crate::dispatch::Mode;

impl<B: Bus> HuC6280<B> {
    /// LDA -- M -> A (SW SB-53).
    #[inline(always)]
    pub(crate) fn op_lda(&mut self, mode: Mode) {
        let address = self.ea(mode);
        let value = self.load(address);
        self.a = self.set_nz(value);
    }

    /// LDX -- M -> X (SW SB-54).
    #[inline(always)]
    pub(crate) fn op_ldx(&mut self, mode: Mode) {
        let address = self.ea(mode);
        let value = self.load(address);
        self.x = self.set_nz(value);
    }

    /// LDY -- M -> Y (SW SB-55).
    #[inline(always)]
    pub(crate) fn op_ldy(&mut self, mode: Mode) {
        let address = self.ea(mode);
        let value = self.load(address);
        self.y = self.set_nz(value);
    }

    /// STA -- A -> M (SW SB-83).
    #[inline(always)]
    pub(crate) fn op_sta(&mut self, mode: Mode) {
        let address = self.ea(mode);
        self.store(address, self.a);
    }

    /// STX -- X -> M (SW SB-84).
    #[inline(always)]
    pub(crate) fn op_stx(&mut self, mode: Mode) {
        let address = self.ea(mode);
        self.store(address, self.x);
    }

    /// STY -- Y -> M (SW SB-85).
    #[inline(always)]
    pub(crate) fn op_sty(&mut self, mode: Mode) {
        let address = self.ea(mode);
        self.store(address, self.y);
    }

    /// STZ -- 00 -> M (SW SB-86).
    #[inline(always)]
    pub(crate) fn op_stz(&mut self, mode: Mode) {
        let address = self.ea(mode);
        self.store(address, 0);
    }

    /// TAX -- A -> X (SW SB-91).
    pub(crate) fn op_tax(&mut self) {
        self.implied();
        self.x = self.set_nz(self.a);
    }

    /// TAY -- A -> Y (SW SB-92).
    pub(crate) fn op_tay(&mut self) {
        self.implied();
        self.y = self.set_nz(self.a);
    }

    /// TXA -- X -> A (SW SB-106).
    pub(crate) fn op_txa(&mut self) {
        self.implied();
        self.a = self.set_nz(self.x);
    }

    /// TYA -- Y -> A (SW SB-108).
    pub(crate) fn op_tya(&mut self) {
        self.implied();
        self.a = self.set_nz(self.y);
    }

    /// TSX -- S -> X (SW SB-105).
    pub(crate) fn op_tsx(&mut self) {
        self.implied();
        self.x = self.set_nz(self.s);
    }

    /// TXS -- X -> S; no flags (SW SB-107).
    pub(crate) fn op_txs(&mut self) {
        self.implied();
        self.s = self.x;
    }

    /// INX -- X + 1 -> X (SW SB-49).
    pub(crate) fn op_inx(&mut self) {
        self.implied();
        self.x = self.set_nz(self.x.wrapping_add(1));
    }

    /// INY -- Y + 1 -> Y (SW SB-50).
    pub(crate) fn op_iny(&mut self) {
        self.implied();
        self.y = self.set_nz(self.y.wrapping_add(1));
    }

    /// DEX -- X - 1 -> X (SW SB-45).
    pub(crate) fn op_dex(&mut self) {
        self.implied();
        self.x = self.set_nz(self.x.wrapping_sub(1));
    }

    /// DEY -- Y - 1 -> Y (SW SB-46).
    pub(crate) fn op_dey(&mut self) {
        self.implied();
        self.y = self.set_nz(self.y.wrapping_sub(1));
    }
}
