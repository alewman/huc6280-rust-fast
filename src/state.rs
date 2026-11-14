//! Values for capturing and restoring HuC6280 processor state (`state.py`).
//!
//! Differences forced by Rust: the field widths are the types', so of the
//! reference's constructor checks only three remain for
//! [`CpuState::validate`]: `irq_disable` and `polled_irq_lines` are three
//! bits and B is not stored.

use sixfiveohtwo::frame::PollFields;

/// Complete CPU-owned state at an instruction boundary.
///
/// Registers, the mapping registers, the clock switch, the three maskable
/// interrupt inputs and their disable gate, and the frame's poll state (the
/// requests the host has made and the core has not yet taken, and what
/// decides the next `step()`). Host memory, devices, scheduling and cycle
/// counters are excluded: restoring this value restores the processor, not a
/// machine. The fields from `halted` to `skipped_poll` are 6502-rust's
/// frame's, as in its `CpuState`; `polled_irq_lines` is this core's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuState {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub s: u8,
    /// Logical, 16 bits.
    pub pc: u16,
    /// P as the core keeps it: T is bit 5, B (bit 4) is never stored (U4, U5).
    pub p: u8,
    /// MPR0-MPR7.
    pub mpr: [u8; 8],
    pub high_speed: bool,
    /// The last value TAM or TMA moved (what TMA #$00 returns; U11).
    pub mpr_latch: u8,
    /// The IRQ1, IRQ2 and TIQ input levels and the disable register (IRQ2,
    /// IRQ1, TIQ; 1 = masked).
    pub irq1: bool,
    pub irq2: bool,
    pub tiq: bool,
    pub irq_disable: u8,
    /// Always false: the HuC6280 has no JAM, WAI or STP.
    pub halted: bool,
    pub reset_pending: bool,
    /// An enabled maskable source is asserted.
    pub maskable_interrupt_pending: bool,
    /// An NMI edge latched and not yet taken.
    pub non_maskable_interrupt_pending: bool,
    /// The I flag the last instruction's poll saw, when that differs in
    /// timing from P (CLI, SEI, PLP); `None` otherwise.
    pub polled_i: Option<bool>,
    /// The IRQ line and the NMI latch as the last poll saw them, when that
    /// is not what the requests above say; `None` when the poll agrees with
    /// them.
    pub polled_maskable_interrupt: Option<bool>,
    pub polled_non_maskable_interrupt: Option<bool>,
    /// The last step was BRK, an interrupt entry or reset, which do not
    /// poll: the next boundary runs an instruction.
    pub skipped_poll: bool,
    /// The maskable lines the last instruction's poll saw (IRQ2, IRQ1, TIQ in
    /// bits 0-2, the `$1403` layout), when a line changed after the poll, in
    /// the instruction's last cycle; `None` when the poll saw `irq1`, `irq2`
    /// and `tiq` as they stand. It picks the vector and decides a vanished
    /// interrupt (C18). Added in issue 0005; `None`, the default, is how a
    /// state from before it loads.
    pub polled_irq_lines: Option<u8>,
}

impl Default for CpuState {
    fn default() -> Self {
        CpuState {
            a: 0,
            x: 0,
            y: 0,
            s: 0xFF,
            pc: 0,
            p: 0x04,
            mpr: [0; 8],
            high_speed: false,
            mpr_latch: 0,
            irq1: false,
            irq2: false,
            tiq: false,
            irq_disable: 0,
            halted: false,
            reset_pending: false,
            maskable_interrupt_pending: false,
            non_maskable_interrupt_pending: false,
            polled_i: None,
            polled_maskable_interrupt: None,
            polled_non_maskable_interrupt: None,
            skipped_poll: false,
            polled_irq_lines: None,
        }
    }
}

impl CpuState {
    /// The frame's fields of this state.
    pub fn poll_fields(&self) -> PollFields {
        PollFields {
            halted: self.halted,
            reset_pending: self.reset_pending,
            maskable_interrupt_pending: self.maskable_interrupt_pending,
            non_maskable_interrupt_pending: self.non_maskable_interrupt_pending,
            polled_i: self.polled_i,
            polled_maskable_interrupt: self.polled_maskable_interrupt,
            polled_non_maskable_interrupt: self.polled_non_maskable_interrupt,
            skipped_poll: self.skipped_poll,
        }
    }

    /// Check what the reference's constructor enforces beyond the field
    /// widths: `irq_disable` in 0..=7, `polled_irq_lines` `None` or in
    /// 0..=7, and B (bit 4 of P) clear.
    pub fn validate(&self) -> Result<(), String> {
        if self.irq_disable > 7 {
            return Err("irq_disable must be an integer in range 0x00..0x07".into());
        }
        if matches!(self.polled_irq_lines, Some(lines) if lines > 7) {
            return Err("polled_irq_lines must be an integer in range 0x00..0x07".into());
        }
        if self.p & 0x10 != 0 {
            return Err("p must have B (bit 4) clear: B is not stored".into());
        }
        Ok(())
    }
}
