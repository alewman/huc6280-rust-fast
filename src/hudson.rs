//! Hudson's additions: SXY SAX SAY CLA CLX CLY ST0 ST1 ST2 TMA TAM CSL CSH
//! SET TST (`_hudson.py`).
//!
//! None exists on the 65C02; all but the swaps and clears are about the
//! chip's own extras: the VDC ports, the mapper, the clock switch and the T
//! flag.

use crate::core::{Bus, HuC6280, N, T, V, Z};
use crate::dispatch::Mode;

/// U12: ST0-ST2 write physical $1FE000, $1FE002 and $1FE003 whatever the
/// mapper holds (HW HB-8, Table 2-2-3: A20 high, A1:A0 = 00, 10, 11; pcetech
/// 2.0). The VDC's registers are every 4 bytes.
pub const VDC_ADDRESS: u32 = 0x1FE000;

/// C6: TMA with several mask bits. Mesen ORs the registers, MAME takes the
/// highest bit's; pcetech: "combined together... I have not figured out
/// exactly how this works".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TmaMultiple {
    /// The default (emulator tier).
    Or,
    Highest,
    Lowest,
}

impl<B: Bus> HuC6280<B> {
    /// SXY -- swap X and Y (SW SB-87).
    pub(crate) fn op_sxy(&mut self) {
        self.implied();
        self.io();
        std::mem::swap(&mut self.x, &mut self.y);
    }

    /// SAX -- swap A and X (SW SB-72).
    pub(crate) fn op_sax(&mut self) {
        self.implied();
        self.io();
        std::mem::swap(&mut self.a, &mut self.x);
    }

    /// SAY -- swap A and Y (SW SB-73).
    pub(crate) fn op_say(&mut self) {
        self.implied();
        self.io();
        std::mem::swap(&mut self.a, &mut self.y);
    }

    /// CLA -- 00 -> A, no flags (SW SB-34).
    pub(crate) fn op_cla(&mut self) {
        self.implied();
        self.a = 0;
    }

    /// CLX -- 00 -> X, no flags (SW SB-39).
    pub(crate) fn op_clx(&mut self) {
        self.implied();
        self.x = 0;
    }

    /// CLY -- 00 -> Y, no flags (SW SB-40).
    pub(crate) fn op_cly(&mut self) {
        self.implied();
        self.y = 0;
    }

    fn store_vdc(&mut self, offset: u32) {
        let value = self.fetch();
        self.io();
        self.cycle_write_physical(VDC_ADDRESS + offset, value);
    }

    /// ST0 -- immediate -> VDC register 0, physical $1FE000 (U12) (SW SB-80).
    pub(crate) fn op_st0(&mut self) {
        self.store_vdc(0);
    }

    /// ST1 -- immediate -> VDC register 2, physical $1FE002 (U12) (SW SB-81).
    pub(crate) fn op_st1(&mut self) {
        self.store_vdc(2);
    }

    /// ST2 -- immediate -> VDC register 3, physical $1FE003 (U12) (SW SB-82).
    pub(crate) fn op_st2(&mut self) {
        self.store_vdc(3);
    }

    /// TMAi -- the mapping register named by the mask -> A (SW SB-101).
    pub(crate) fn op_tma(&mut self) {
        let mask = self.fetch();
        self.io();
        self.io();
        if mask == 0 {
            // U11: an operand of $00 returns the last value moved by TAM or
            // TMA (pcetech 2.0).
            self.a = self.mpr_latch;
            return;
        }
        let mut chosen = (0..8)
            .filter(|bit| (mask >> bit) & 1 != 0)
            .map(|bit| self.mpr[bit]);
        let value = match self.tma_multiple {
            TmaMultiple::Or => chosen.fold(0, |value, register| value | register),
            TmaMultiple::Highest => chosen.next_back().unwrap_or(0),
            TmaMultiple::Lowest => chosen.next().unwrap_or(0),
        };
        self.a = value;
        self.mpr_latch = value;
    }

    /// TAMi -- A -> every mapping register the mask names (SW SB-90).
    pub(crate) fn op_tam(&mut self) {
        let mask = self.fetch();
        self.io();
        self.io();
        self.io();
        if mask == 0 {
            return; // U11: a mask of $00 writes nothing and leaves the latch alone
        }
        self.mpr_latch = self.a;
        for (bit, register) in self.mpr.iter_mut().enumerate() {
            if (mask >> bit) & 1 != 0 {
                *register = self.a;
            }
        }
    }

    fn speed(&mut self, high: bool) {
        self.implied();
        // C7: the new speed holds for the instruction's last cycle (Mesen,
        // Mednafen); MAME applies it after. 3 cycles when the speed does not
        // change (pcetech 2.4, U10).
        self.pins.high_speed = high;
        self.io();
    }

    /// CSL -- select the low clock speed, 1.79 MHz (HW HB-25; pcetech 2.4).
    pub(crate) fn op_csl(&mut self) {
        self.speed(false);
    }

    /// CSH -- select the high clock speed, 7.16 MHz (HW HB-25; pcetech 2.4).
    pub(crate) fn op_csh(&mut self) {
        self.speed(true);
    }

    /// SET -- 1 -> T, for the next instruction only (SW SB-78).
    pub(crate) fn op_set(&mut self) {
        self.implied();
        // U4: the next instruction's fetch clears it again.
        self.p |= T;
    }

    /// TST -- immediate AND M sets Z; M bits 7 and 6 -> N and V (SW SB-104).
    #[inline(always)]
    pub(crate) fn op_tst(&mut self, mode: Mode) {
        let mask = self.fetch();
        let address = self.ea(mode);
        self.io();
        let value = self.cycle_read(address);
        self.io();
        // U19: N and V come from the memory operand, Z from the AND.
        self.p =
            (self.p & !(N | V | Z)) | (value & (N | V)) | if value & mask != 0 { 0 } else { Z };
    }
}
