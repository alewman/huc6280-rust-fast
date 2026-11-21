// SPDX-License-Identifier: MIT
//! Branches, jumps, calls, returns, the stack, flags, BRK, interrupts and
//! reset (`_control.py`).
//!
//! Vectors (HW HB-12 to HB-18; the reference's start-here.md): RESET `$FFFE`,
//! NMI `$FFFC`, TIQ `$FFFA`, IRQ1 `$FFF8`, IRQ2 and BRK `$FFF6`, read through
//! the mapper (after reset MPR7 = 0, so physical `$001FFE`).
//!
//! The interrupt design is the reference's docs/handoff-brief.md's: the
//! frame's `IRQ_LINE` means "an enabled maskable source is asserted";
//! [`Pins::set_irq_lines`] and the `irq_disable` register (the `$1402`
//! layout, 1 = disabled) feed it, and the vector is chosen at entry from the
//! lines in priority order TIQ, IRQ1, IRQ2. The timer and the
//! `$1402`/`$1403` registers themselves are the board's.
//!
//! Differences forced by Rust: the interrupt lines and the disable register
//! live in [`Pins`] (core.rs), so their methods are `Pins`'s, with
//! forwarders on the CPU; the reference's `_boundary_interrupt` override
//! that calls `super()` is `boundary_interrupt` here, called from the
//! `Part` implementation in cpu.rs, ending in the frame's own; the
//! reference's `ValueError` for an unknown `irq_poll_cycle` or
//! `vanished_interrupt` has no counterpart, the modes being enums without
//! the withdrawn values.
//!
//! The two switches' enums, `boundary_interrupt_sequence` and
//! `maskable_vector` were transcribed anew from the reference's clean rewrite
//! (huc6280-python `6ce348e`; docs/restart.md); the code they replace was
//! moved aside unread.

use sixfiveohtwo::frame::{IRQ_LINE, NMI_LATCH, NO_POLL};

use crate::core::{Bus, HuC6280, Pins, B, C, D, I, N, T, V, Z};

pub const VECTOR_RESET: u16 = 0xFFFE;
pub const VECTOR_NMI: u16 = 0xFFFC;
pub const VECTOR_TIQ: u16 = 0xFFFA;
pub const VECTOR_IRQ1: u16 = 0xFFF8;
pub const VECTOR_IRQ2: u16 = 0xFFF6; // also BRK (U15)

// The three maskable sources, in the layout of $1402/$1403 (pcetech 2.1).
pub const IRQ2_BIT: u8 = 0x01;
pub const IRQ1_BIT: u8 = 0x02;
pub const TIQ_BIT: u8 = 0x04;

/// C18: what an entry does when a source the poll saw has gone by the
/// boundary (it was acknowledged in the instruction's last cycle). When
/// nothing is left, the instruction runs instead.
///
/// The reference's `vanished_interrupt` keeps `"mesen2"` and `"drop"`; its
/// `"latched"` was withdrawn in the clean rewrite (it raises `ValueError`
/// there) and has no variant here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VanishedInterrupt {
    /// `"mesen2"` (default): IRQ1 and IRQ2 are taken as the poll saw them,
    /// through their own vectors; the timer is taken only if it is still
    /// asserted. OBSERVED, provisional: Mesen2 2.1.1 and Mednafen 1.32.1 run
    /// as closed boxes on the reference's validation/vanish_rom.py cards (a
    /// timer acknowledged by `STZ $1403`, a VDC interrupt by a status read,
    /// each in the last cycle) both drop the timer and take IRQ1; no hardware
    /// result is known. A timer that appears in the last cycle waits for the
    /// next poll, like any other source (a choice: neither run shows it).
    Mesen2,
    /// `"drop"`: only sources the poll saw and that are still asserted are
    /// taken.
    Drop,
}

/// U25: when the boundary samples the lines. Hudson's hardware manual says
/// only that the inputs "are sampled in the last bus cycle of one
/// instruction" (HW HB-12), not whether a change during that cycle counts.
///
/// The reference's `irq_poll_cycle` keeps `"second_to_last"` and `"last"`;
/// its `"mednafen"` was withdrawn in the clean rewrite (it raises
/// `ValueError` there) and has no variant here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrqPollCycle {
    /// `"second_to_last"` (default): the 6502 rule, the lines as they stood
    /// at the end of the instruction's second-to-last cycle (the frame's
    /// poll; NESdev for the 6502), which the MAME replays and Covell's spread
    /// are judged with (the reference's issue 0003).
    SecondToLast,
    /// `"last"`: the levels at the boundary, so a line that appears in the
    /// last cycle is taken there and one that goes is not, every source
    /// alike. OBSERVED: srg320's HUC6280_CPU run as a closed box under GHDL
    /// (the reference's validation/vanish_vhdl.py) does exactly this; MAME
    /// 0.285 also decides at the boundary on the levels then (h6280.cpp,
    /// BSD-3-Clause, Bryan McPhail: the check after each instruction reads the
    /// lines' current state).
    Last,
}

impl Pins {
    /// Assert the frame's IRQ_LINE when an enabled maskable source is asserted.
    pub(crate) fn sync_irq(&mut self) {
        let asserted = self.lines & !self.irq_disable & 7 != 0;
        if asserted != (self.frame.requests & IRQ_LINE != 0) {
            if asserted {
                self.frame.request_maskable_interrupt();
            } else {
                self.frame.clear_maskable_interrupt();
            }
        }
    }

    /// Set the three maskable inputs (levels: they stay until the device
    /// lets go).
    ///
    /// IRQ1 is the VDC's, IRQ2 the HuCard connector's and the CD-ROM's, TIQ
    /// the timer's (pcetech 2.1). A source masked by `irq_disable` is
    /// remembered, not taken.
    pub fn set_irq_lines(&mut self, irq1: bool, irq2: bool, tiq: bool) {
        let lines = (if irq2 { IRQ2_BIT } else { 0 })
            | (if irq1 { IRQ1_BIT } else { 0 })
            | (if tiq { TIQ_BIT } else { 0 });
        let changed = lines ^ self.lines;
        if changed == 0 {
            return;
        }
        if let Some(polled_lines) = self.polled_lines {
            // Between steps after a change the poll missed: what the host
            // changes now counts for the next boundary, as the frame's request
            // methods treat polled.
            self.polled_lines = Some((polled_lines & !changed) | (lines & changed));
        }
        self.lines = lines;
        self.sync_irq();
        if self.frame.deferred.is_none() {
            // Learn whether a bus access follows, even when the frame's
            // IRQ_LINE is unchanged (a source swapped for another): no access
            // means the change came after the poll.
            let requests = self.frame.requests;
            self.frame.request(requests);
        }
    }

    /// The (IRQ1, IRQ2, TIQ) levels last set.
    pub fn irq_lines(&self) -> (bool, bool, bool) {
        let lines = self.lines;
        (
            lines & IRQ1_BIT != 0,
            lines & IRQ2_BIT != 0,
            lines & TIQ_BIT != 0,
        )
    }

    /// The interrupt disable register's bits 2-0 (IRQ2, IRQ1, TIQ; 1 =
    /// masked), as `$1402`.
    pub fn irq_disable(&self) -> u8 {
        self.irq_disable
    }

    /// Write the interrupt disable register (bits 2-0 are kept).
    pub fn set_irq_disable(&mut self, value: u8) {
        self.irq_disable = value & 7;
        self.sync_irq();
    }
}

impl<B: Bus> HuC6280<B> {
    // -- branches ----------------------------------------------------------

    fn branch(&mut self, taken: bool) {
        let offset = self.fetch();
        if !taken {
            return;
        }
        // A taken branch costs 2 more cycles whatever page it lands in (SW;
        // pcetech 2.0, U9).
        self.dummy(self.pc);
        self.io();
        self.pc = self.pc.wrapping_add(offset as i8 as u16);
    }

    /// BPL -- branch if N = 0 (SW SB-28).
    pub(crate) fn op_bpl(&mut self) {
        self.branch(self.p & N == 0);
    }

    /// BMI -- branch if N = 1 (SW SB-26).
    pub(crate) fn op_bmi(&mut self) {
        self.branch(self.p & N != 0);
    }

    /// BVC -- branch if V = 0 (SW SB-32).
    pub(crate) fn op_bvc(&mut self) {
        self.branch(self.p & V == 0);
    }

    /// BVS -- branch if V = 1 (SW SB-33).
    pub(crate) fn op_bvs(&mut self) {
        self.branch(self.p & V != 0);
    }

    /// BCC -- branch if C = 0 (SW SB-22).
    pub(crate) fn op_bcc(&mut self) {
        self.branch(self.p & C == 0);
    }

    /// BCS -- branch if C = 1 (SW SB-23).
    pub(crate) fn op_bcs(&mut self) {
        self.branch(self.p & C != 0);
    }

    /// BNE -- branch if Z = 0 (SW SB-27).
    pub(crate) fn op_bne(&mut self) {
        self.branch(self.p & Z == 0);
    }

    /// BEQ -- branch if Z = 1 (SW SB-24).
    pub(crate) fn op_beq(&mut self) {
        self.branch(self.p & Z != 0);
    }

    /// BRA -- branch always, always 4 cycles (SW SB-29).
    pub(crate) fn op_bra(&mut self) {
        let offset = self.fetch();
        self.io();
        self.io();
        self.pc = self.pc.wrapping_add(offset as i8 as u16);
    }

    // -- jumps, calls, returns ----------------------------------------------

    /// JMP -- abs: AA -> PC (SW SB-51).
    pub(crate) fn op_jmp_abs(&mut self) {
        self.pc = self.fetch_word();
        self.io();
    }

    fn read_pointer(&mut self, address: u16) -> u16 {
        // U9: the high byte is at address + 1 even across a page; no NMOS bug
        // (pcetech 2.0).
        let low = self.cycle_read(address) as u16;
        ((self.cycle_read(address.wrapping_add(1)) as u16) << 8) | low
    }

    /// JMP -- (abs): the pointer's address, then the word there -> PC (SW SB-51).
    pub(crate) fn op_jmp_ind(&mut self) {
        let pointer = self.fetch_word();
        self.io();
        self.pc = self.read_pointer(pointer);
        self.io();
    }

    /// JMP -- (abs,X): the pointer is at abs + X (SW SB-51).
    pub(crate) fn op_jmp_idx(&mut self) {
        let pointer = self.fetch_word().wrapping_add(self.x as u16);
        self.io();
        self.pc = self.read_pointer(pointer);
        self.io();
    }

    /// JSR -- push the address of the operand's last byte, then AA -> PC (SW SB-52).
    pub(crate) fn op_jsr(&mut self) {
        let low = self.fetch() as u16;
        self.io();
        let pc = self.pc; // the high byte's address, the last byte of JSR (U20)
        self.push((pc >> 8) as u8);
        self.push(pc as u8);
        self.pc = ((self.fetch() as u16) << 8) | low;
        self.io();
    }

    /// BSR -- push the address of the offset byte, then PC + offset (8 cycles)
    /// (SW SB-31).
    pub(crate) fn op_bsr(&mut self) {
        let offset = self.fetch();
        self.io();
        let pc = self.pc.wrapping_sub(1); // U20: the last byte of BSR
        self.push((pc >> 8) as u8);
        self.push(pc as u8);
        self.io();
        self.io();
        self.io();
        self.pc = self.pc.wrapping_add(offset as i8 as u16);
    }

    /// RTS -- pull PC, then PC + 1 -> PC (SW SB-71).
    pub(crate) fn op_rts(&mut self) {
        self.implied();
        self.io();
        let low = self.pull() as u16;
        let pc = ((self.pull() as u16) << 8) | low;
        self.io();
        self.io();
        self.pc = pc.wrapping_add(1);
    }

    /// RTI -- pull P, then PC (SW SB-70).
    pub(crate) fn op_rti(&mut self) {
        self.implied();
        self.io();
        self.p = self.pull() & !B; // U20: P first, then PCL, PCH; B is not stored (U5)
        let low = self.pull() as u16;
        self.pc = ((self.pull() as u16) << 8) | low;
        self.io();
    }

    // -- the stack ---------------------------------------------------------

    /// PHA -- push A (SW SB-59).
    pub(crate) fn op_pha(&mut self) {
        self.implied();
        self.push(self.a);
    }

    /// PHP -- push P with B set and T clear (SW SB-60).
    pub(crate) fn op_php(&mut self) {
        self.implied();
        // U5: B reads as 1 in a pushed P. T reads as 0: the fetch cleared it (U4).
        self.push(self.p | B);
    }

    /// PHX -- push X (SW SB-61).
    pub(crate) fn op_phx(&mut self) {
        self.implied();
        self.push(self.x);
    }

    /// PHY -- push Y (SW SB-62).
    pub(crate) fn op_phy(&mut self) {
        self.implied();
        self.push(self.y);
    }

    fn pull_register(&mut self) -> u8 {
        self.implied();
        self.io();
        self.pull()
    }

    /// PLA -- pull A (SW SB-63).
    pub(crate) fn op_pla(&mut self) {
        let value = self.pull_register();
        self.a = self.set_nz(value);
    }

    /// PLX -- pull X (SW SB-65).
    pub(crate) fn op_plx(&mut self) {
        let value = self.pull_register();
        self.x = self.set_nz(value);
    }

    /// PLY -- pull Y (SW SB-66).
    pub(crate) fn op_ply(&mut self) {
        let value = self.pull_register();
        self.y = self.set_nz(value);
    }

    /// PLP -- pull P; T can be set from the stack, B is not kept (SW SB-64).
    pub(crate) fn op_plp(&mut self) {
        let value = self.pull_register();
        // U21: the new I counts one instruction late, as on the 6502 (the
        // frame's polled_i).
        self.pins.frame.polled_i = Some(self.p & I);
        self.p = value & !B;
    }

    // -- flags and NOP -------------------------------------------------------

    /// CLC -- 0 -> C (SW SB-35).
    pub(crate) fn op_clc(&mut self) {
        self.implied();
        self.p &= !C;
    }

    /// SEC -- 1 -> C (SW SB-75).
    pub(crate) fn op_sec(&mut self) {
        self.implied();
        self.p |= C;
    }

    /// CLD -- 0 -> D (SW SB-36).
    pub(crate) fn op_cld(&mut self) {
        self.implied();
        self.p &= !D;
    }

    /// SED -- 1 -> D (SW SB-76).
    pub(crate) fn op_sed(&mut self) {
        self.implied();
        self.p |= D;
    }

    /// CLV -- 0 -> V (SW SB-38).
    pub(crate) fn op_clv(&mut self) {
        self.implied();
        self.p &= !V;
    }

    /// CLI -- 0 -> I, after this instruction's interrupt poll (SW SB-37).
    pub(crate) fn op_cli(&mut self) {
        self.implied();
        self.pins.frame.polled_i = Some(self.p & I); // U21
        self.p &= !I;
    }

    /// SEI -- 1 -> I, after this instruction's interrupt poll (SW SB-77).
    pub(crate) fn op_sei(&mut self) {
        self.implied();
        self.pins.frame.polled_i = Some(self.p & I); // U21
        self.p |= I;
    }

    /// NOP -- no operation; the 22 undefined opcodes are the same (U3) (SW SB-57).
    pub(crate) fn op_nop(&mut self) {
        self.implied();
        if self.pins.opcode == 0x63 {
            for _ in 2..self.nop_63_cycles {
                self.io();
            }
        }
    }

    // -- BRK, interrupts, reset ------------------------------------------------

    /// BRK -- push PC + 2 and P with B set, jump through the IRQ2 vector (SW SB-30).
    pub(crate) fn op_brk(&mut self) {
        self.fetch(); // C2: the signature byte is skipped, so BRK is two bytes
        self.interrupt(VECTOR_IRQ2, self.p | B); // U15
        self.io();
        // BRK's cycles are those SingleStepTests huc6280 records (emulator
        // tier); an entry makes the same pushes and vector reads (C3, C8).
        // Like an entry, BRK does not poll: the handler's first instruction
        // runs first (the 6502 frame's rule; issue 0005). step() moves this
        // to polled.
        self.pins.frame.deferred = Some(NO_POLL);
    }

    fn interrupt(&mut self, vector: u16, pushed_p: u8) {
        let pc = self.pc;
        self.push((pc >> 8) as u8);
        self.push(pc as u8);
        self.push(pushed_p);
        // HW HB-14, pcetech 2.0: the entry sets I and clears D. C16: it also
        // clears T, so the handler's first instruction runs normally while the
        // pushed P keeps T (pcetech 2.3). OBSERVED, provisional: Mesen2 and
        // Mednafen run as closed boxes on a timer taken between SET and its
        // target (the reference's validation/vanish_rom.py `build_t_entry`)
        // run the handler's ORA on A; ares (ISC) clears T at entry too.
        // srg320's VHDL under GHDL dissents: its handler's first ORA works on
        // zero page (validation/vanish_vhdl.py --t-entry).
        self.p = (self.p | I) & !(D | T);
        let low = self.cycle_read(vector) as u16;
        self.pc = ((self.cycle_read(vector + 1) as u16) << 8) | low;
    }

    /// Take NMI or the highest-priority enabled maskable source at a boundary.
    pub(crate) fn enter_interrupt_sequence(&mut self, kind: u8) -> u32 {
        self.cycles = 0;
        self.pins.frame.polled_i = None;
        let vector;
        if kind == NMI_LATCH {
            vector = VECTOR_NMI;
            self.pins.frame.requests &= !NMI_LATCH;
        } else {
            match self.maskable_vector() {
                Some(taken) => vector = taken,
                None => {
                    // Nothing is left to enter: the instruction runs instead
                    // (issue 0002, C18), with the bookkeeping step() does
                    // after any instruction, so a request changed in its last
                    // cycle waits for the next poll as it would anywhere else
                    // (issue 0005).
                    self.pins.frame.polled = None;
                    let cycles = self.execute_sequence();
                    let frame = &mut self.pins.frame;
                    if let Some(deferred) = frame.deferred.take() {
                        frame.polled = Some(deferred);
                    }
                    return cycles;
                }
            }
        }
        // Two dummy reads (the opcode and the byte after it, both discarded),
        // the pushes, the vector, and an internal cycle (C3, C8). OBSERVED,
        // provisional: Mesen2's read and write callbacks, run as a closed box,
        // show two reads at PC, PCH, PCL and P pushed, the vector read, then a
        // cycle with no access before the handler's first fetch. srg320's VHDL
        // under GHDL takes 9: three reads at PC and an internal cycle before
        // the pushes (the reference's validation/vanish_vhdl.py --dump).
        self.dummy(self.pc);
        self.dummy(self.pc);
        // U4, U5: P is pushed with B clear and T as it stands, so an
        // interrupt between SET and its target leaves T set in the pushed
        // byte and RTI restores it.
        self.interrupt(vector, self.p & !B);
        if self.interrupt_entry_cycles >= 8 {
            self.io();
        }
        // An entry does not poll: the handler's first instruction runs before
        // anything requested since is taken.
        self.pins.frame.polled = Some(NO_POLL);
        self.pins.frame.deferred = None;
        self.cycles
    }

    /// The interrupt this boundary takes (the frame's hook), with the lines
    /// it counts.
    pub(crate) fn boundary_interrupt_sequence(&mut self) -> u8 {
        let mode = self.irq_poll_cycle;
        let pins = &mut self.pins;
        let polled_lines = pins.polled_lines;
        if mode == IrqPollCycle::SecondToLast {
            // The frame's poll; the lines it saw are the ones a change in the last cycle left.
            // They hold until this boundary only (CpuState::polled_irq_lines).
            pins.poll_lines = polled_lines.unwrap_or(pins.lines);
            pins.polled_lines = None;
            return pins.frame.boundary_interrupt(self.p);
        }
        // IrqPollCycle::Last (the reference raises ValueError for any other name).
        pins.poll_lines = pins.lines;
        pins.polled_lines = None;
        let frame = &mut pins.frame;
        let requests = match frame.polled.take() {
            None => frame.requests,
            Some(polled) => polled,
        };
        if requests & NMI_LATCH != 0 {
            return NMI_LATCH;
        }
        let i_flag = match frame.polled_i.take() {
            None => self.p & I,
            // U21: CLI, SEI and PLP still count one instruction late.
            Some(polled_i) => polled_i,
        };
        if pins.lines & !pins.irq_disable & 7 != 0 && i_flag == 0 {
            return IRQ_LINE;
        }
        0
    }

    /// The vector of the highest-priority source this entry takes, or `None`
    /// if none is left.
    fn maskable_vector(&self) -> Option<u16> {
        let enabled = !self.pins.irq_disable & 7;
        let seen = self.pins.poll_lines & enabled;
        let live = self.pins.lines & enabled;
        let taking = match self.vanished_interrupt {
            VanishedInterrupt::Mesen2 => (seen & (IRQ1_BIT | IRQ2_BIT)) | (seen & live & TIQ_BIT),
            VanishedInterrupt::Drop => seen & live,
        };
        // Priority TIQ, IRQ1, IRQ2 (HW Fig. 2-4-1).
        for (bit, vector) in [
            (TIQ_BIT, VECTOR_TIQ),
            (IRQ1_BIT, VECTOR_IRQ1),
            (IRQ2_BIT, VECTOR_IRQ2),
        ] {
            if taking & bit != 0 {
                return Some(vector);
            }
        }
        None
    }

    /// The start sequence (HW HB-18; U2): MPR7 = 0, I set, D and T clear, low speed.
    pub(crate) fn reset_sequence(&mut self) -> u32 {
        self.cycles = 0;
        self.pins.frame.reset_pending = false;
        self.pins.frame.polled_i = None;
        self.mpr[7] = 0;
        self.p = (self.p | I) & !(D | T | B);
        self.pins.high_speed = false;
        self.pins.irq_disable = 0;
        // No source counts reset's cycles (the reference's docs/timing.md);
        // the core makes five internal cycles and the two vector reads, an
        // arbitrary and unobserved choice (C8).
        for _ in 0..5 {
            self.io();
        }
        let low = self.cycle_read(VECTOR_RESET) as u16;
        self.pc = ((self.cycle_read(VECTOR_RESET + 1) as u16) << 8) | low;
        self.pins.sync_irq();
        self.pins.frame.polled = Some(NO_POLL);
        self.pins.frame.deferred = None;
        self.pins.polled_lines = None;
        self.cycles
    }

    // -- the interrupt lines (forwarders to Pins) ---------------------------

    /// Set the three maskable inputs ([`Pins::set_irq_lines`]).
    pub fn set_irq_lines(&mut self, irq1: bool, irq2: bool, tiq: bool) {
        self.pins.set_irq_lines(irq1, irq2, tiq);
    }

    /// The (IRQ1, IRQ2, TIQ) levels last set.
    pub fn irq_lines(&self) -> (bool, bool, bool) {
        self.pins.irq_lines()
    }

    /// The interrupt disable register ([`Pins::irq_disable`]).
    pub fn irq_disable(&self) -> u8 {
        self.pins.irq_disable
    }

    /// Write the interrupt disable register ([`Pins::set_irq_disable`]).
    pub fn set_irq_disable(&mut self, value: u8) {
        self.pins.set_irq_disable(value);
    }
}
