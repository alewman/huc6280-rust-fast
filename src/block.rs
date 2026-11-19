// SPDX-License-Identifier: MIT
//! Block transfers: TII TDD TIN TIA TAI (SW SB-88 to SB-100; HW HB-6;
//! pcetech 2.0) (`_block.py`).
//!
//! All five push Y, A, X, load the source, destination and length from the
//! three operand words, move one byte per six cycles, pop X, A, Y, for
//! 17 + 6n cycles (n = 0 means 65,536). The whole transfer is **one step**
//! and cannot be interrupted: the interrupt is taken when it ends (U6,
//! pcetech). The shape is the one SingleStepTests huc6280 records (Mesen's
//! output, a closed box; emulator tier): 13 cycles of entry, `i R i W i i` a
//! byte, 4 of exit (the reference's docs/timing.md).

use crate::core::{Bus, HuC6280};

// U7: a block transfer's reads of the hardware page's $0800-$17FF (PSG,
// timer, port, interrupt registers) return 0 without touching the device,
// whatever is there; writes work (pcetech 2.0, hw-verified). The end is the
// parenthesis's reading, not the stated "$1400" (C9, issue 0001).
pub const HARDWARE_ZERO_START: u32 = 0x1FE800;
pub const HARDWARE_ZERO_END: u32 = 0x1FF7FF;

impl<B: Bus> HuC6280<B> {
    fn block_read(&mut self, address: u16) -> u8 {
        if self.block_reads_hardware_page_as_zero
            && (HARDWARE_ZERO_START..=HARDWARE_ZERO_END).contains(&self.physical(address))
        {
            self.dummy(address); // the cycle happens; the device is not asked
            return 0;
        }
        self.cycle_read(address)
    }

    /// Run one transfer; each step slice is the per-byte increment, cycled
    /// (1, -1 alternates).
    fn transfer(&mut self, source_steps: &[i16], destination_steps: &[i16]) {
        self.implied();
        self.io();
        // HW HB-6: Y, then A, then X go on the stack; A, X and Y hold the low
        // bytes while it runs and come back at the end.
        self.push(self.y);
        self.push(self.a);
        self.push(self.x);
        let mut source = self.fetch_word();
        let mut destination = self.fetch_word();
        let length = match self.fetch_word() {
            0 => 0x10000,
            length => length as usize,
        };
        self.io();
        for index in 0..length {
            self.io();
            let value = self.block_read(source);
            self.io();
            self.cycle_write(destination, value);
            self.io();
            self.io();
            source = source.wrapping_add(source_steps[index % source_steps.len()] as u16);
            destination =
                destination.wrapping_add(destination_steps[index % destination_steps.len()] as u16);
        }
        self.io();
        self.x = self.pull();
        self.a = self.pull();
        self.y = self.pull();
    }

    /// TII -- transfer, source +1, destination +1 (SW SB-97).
    pub(crate) fn op_tii(&mut self) {
        self.transfer(&[1], &[1]);
    }

    /// TDD -- transfer, source -1, destination -1 (SW SB-93).
    pub(crate) fn op_tdd(&mut self) {
        self.transfer(&[-1], &[-1]);
    }

    /// TIN -- transfer, source +1, destination fixed (SW SB-99).
    pub(crate) fn op_tin(&mut self) {
        self.transfer(&[1], &[0]);
    }

    /// TIA -- transfer, source +1, destination alternating +1, -1 (SW SB-95).
    pub(crate) fn op_tia(&mut self) {
        self.transfer(&[1], &[1, -1]);
    }

    /// TAI -- transfer, source alternating +1, -1, destination +1 (SW SB-88).
    pub(crate) fn op_tai(&mut self) {
        self.transfer(&[1, -1], &[1]);
    }
}
