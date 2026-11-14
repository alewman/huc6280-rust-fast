//! Registers, the memory mapper, the cycle primitives and the addressing
//! modes (`_core.py`).
//!
//! Every CPU cycle is one bus callback, in the chip's order, internal cycles
//! and dummy reads included. A cycle is one of four kinds, readable as
//! [`Pins::kind`] while the callback runs:
//!
//! - [`Kind::Read`]: [`Bus::read`]; the opcode fetch is the first cycle.
//! - [`Kind::Write`][]: [`Bus::write`], the value the chip drives.
//! - [`Kind::Dummy`]: [`Bus::idle`] with `Some(address)`; a read the chip
//!   makes and throws away, at the address given (the byte after the opcode
//!   for an implied instruction).
//! - [`Kind::Idle`]: [`Bus::idle`] with `None`; an internal cycle with
//!   nothing on the bus.
//!
//! The bus shapes (which cycle is which, and where the internal cycles sit)
//! are those of SingleStepTests huc6280 (the reference's docs/timing.md, "The
//! bus cycles"): Mesen's model, **emulator tier** (C8). Hudson's manual gives
//! totals only.
//!
//! The addresses the callbacks receive are 21-bit physical addresses: the
//! mapper is in the core (the reference's docs/handoff-brief.md, "The core
//! boundary"). Logical addresses are 16-bit; the top three bits pick one of
//! eight mapping registers (HW HB-7), whose 8-bit value replaces them (U1, HW
//! HB-3; pcetech 2.0).
//!
//! The U and C numbers are the entries of the reference's
//! docs/undocumented-behavior.md; HW and SW are Hudson's hardware and
//! software manuals.
//!
//! Differences forced by Rust:
//!
//! - The reference's host passes three callables and reaches the CPU from
//!   inside them (`cpu.set_irq_lines`, `cpu.kind`, `cpu.high_speed`). A Rust
//!   bus callback cannot borrow the CPU that is calling it, so what a
//!   callback may see or change lives in [`Pins`], which every callback
//!   receives: the frame (reset, NMI), the three maskable lines and their
//!   disable register, `high_speed`, the cycle's kind and the opcode.
//! - The reference's frame swaps its bus callables for confirming wrappers
//!   when a request changes; here every cycle calls `Frame::access` first,
//!   which does the same check (6502-rust's difference, inherited), through
//!   `Pins::access`, which also records the lines as the reference's
//!   `_confirm` override does.
//! - The mixins' class attributes (the named switches) are fields of
//!   [`HuC6280`], the string-valued ones enums.
//! - A handler that takes a function (`_modify`'s operation) takes a
//!   `fn(&mut Self, ...)` pointer.

use sixfiveohtwo::frame::Frame;

use crate::alu::DecimalFormula;
use crate::control::{IrqPollCycle, VanishedInterrupt};
use crate::hudson::TmaMultiple;

// P (HW HB-4, HB-5). B is never stored: it reads as 1 whenever P is pushed by
// PHP or BRK and as 0 when an interrupt pushes it (U5), and the corpus never
// sets it. T is bit 5, the memory-operation flag (U4).
pub const N: u8 = 0x80;
pub const V: u8 = 0x40;
pub const T: u8 = 0x20;
pub const B: u8 = 0x10;
pub const D: u8 = 0x08;
pub const I: u8 = sixfiveohtwo::frame::I; // bit 2 on every part: the frame's
pub const Z: u8 = 0x02;
pub const C: u8 = 0x01;

/// Zero page and stack are logical pages $20 and $21, through MPR1 (U1; HW
/// HB-3: the address bus high byte is $21 for the stack).
pub const ZERO_PAGE: u16 = 0x2000;
pub const STACK_PAGE: u16 = 0x2100;

/// A cycle's kind, as [`Pins::kind`] shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Read,
    Write,
    Dummy,
    Idle,
}

impl Kind {
    /// The reference's name for the kind (`cpu.kind`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Kind::Read => "read",
            Kind::Write => "write",
            Kind::Dummy => "dummy",
            Kind::Idle => "idle",
        }
    }
}

/// N and Z for an 8-bit result.
#[inline]
pub(crate) fn nz(value: u8) -> u8 {
    (value & N) | if value == 0 { Z } else { 0 }
}

/// The host's side of the bus: exactly one call per cycle, in the chip's
/// order, with 21-bit physical addresses.
///
/// Each call also gets the CPU's [`Pins`], so a device can change an
/// interrupt line during the cycle it happens in (the poll then decides
/// whether that was in time), and read the cycle's kind and the clock speed
/// it runs at.
pub trait Bus {
    fn read(&mut self, address: u32, pins: &mut Pins) -> u8;
    fn write(&mut self, address: u32, value: u8, pins: &mut Pins);
    /// A cycle with no data transfer: `Some(address)` is the dummy read the
    /// chip makes (the byte after the opcode of an implied instruction),
    /// `None` an internal cycle. The default touches nothing (the
    /// reference's `_no_idle`).
    fn idle(&mut self, address: Option<u32>, pins: &mut Pins) {
        let _ = (address, pins);
    }
}

/// What a bus callback can see and change of the CPU while it runs: the
/// frame's requests (reset, NMI), the three maskable inputs and their
/// disable register, the clock switch, the cycle's kind and the opcode.
///
/// Between steps a board reaches the same things as `cpu.pins`, or through
/// the methods of [`HuC6280`] that forward here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pins {
    /// 6502-rust's frame: reset, the NMI latch, and `IRQ_LINE`, which here
    /// means "an enabled maskable source is asserted" and is kept by
    /// [`Pins::set_irq_lines`] and [`Pins::set_irq_disable`]: a board does
    /// not call the frame's `request_maskable_interrupt` itself.
    pub frame: Frame,
    /// The clock switch: `false` at 1.79 MHz (12 master clocks a cycle),
    /// `true` at 7.16 MHz (3). Reset selects low (U10).
    pub high_speed: bool,
    pub(crate) kind: Kind,
    pub(crate) opcode: u8,
    // The three maskable sources in the layout of $1402/$1403 (control.rs).
    pub(crate) lines: u8,
    /// The lines as they stood at the last bus access (kept up to date by
    /// `Pins::access`): what the poll sees if a line changes in an
    /// instruction's last cycle.
    pub(crate) lines_at_access: u8,
    /// The lines the last instruction's poll saw, set when a line changed
    /// after it (in the last cycle) and only until the next boundary; `None`
    /// when the poll saw the lines as they stand.
    /// `CpuState::polled_irq_lines` (issue 0005).
    pub(crate) polled_lines: Option<u8>,
    /// The lines the boundary's interrupt decision counts, per
    /// `irq_poll_cycle` (U25).
    pub(crate) poll_lines: u8,
    pub(crate) irq_disable: u8,
}

impl Default for Pins {
    fn default() -> Self {
        Pins {
            frame: Frame::default(),
            high_speed: false,
            kind: Kind::Idle,
            opcode: 0,
            lines: 0,
            lines_at_access: 0,
            polled_lines: None,
            poll_lines: 0,
            irq_disable: 0,
        }
    }
}

impl Pins {
    /// Make a bus access: confirm a request change made since the last one
    /// (`Frame::access`), and record the lines as they stand (the reference's
    /// `_confirm` override, which sets `_lines_at_access`).
    ///
    /// The reference's `_confirm` runs only on the first access after a
    /// request change; every change of the lines arms it
    /// ([`Pins::set_irq_lines`]), so recording at every access keeps the same
    /// value.
    #[inline]
    pub(crate) fn access(&mut self) {
        self.frame.access();
        self.lines_at_access = self.lines;
    }

    /// The current cycle's kind.
    #[inline]
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// The opcode of the instruction being executed.
    ///
    /// It is set once the fetch cycle's read has returned, so every later
    /// callback of the instruction (operand reads, the write of an
    /// ST0/ST1/ST2) sees it; the fetch callback itself still sees the
    /// previous instruction's. An interrupt entry leaves it unchanged.
    #[inline]
    pub fn opcode(&self) -> u8 {
        self.opcode
    }
}

/// A Hudson HuC6280 core whose memory and devices are a host's [`Bus`].
///
/// `step()` calls the bus once per cycle it returns, in the chip's order.
/// Registers are plain fields: `a`, `x`, `y`, `s`, `pc` (logical, 16 bits),
/// `p` (8 bits; B is not stored) and `mpr` (eight 8-bit mapping registers).
/// The interrupt inputs, their poll and `step()`'s skeleton are 6502-rust's
/// frame; the three maskable sources come in through
/// [`Pins::set_irq_lines`] and [`Pins::set_irq_disable`].
pub struct HuC6280<B: Bus> {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub s: u8,
    pub pc: u16,
    pub p: u8,
    /// MPR0-MPR7, the 8-bit mapping registers (physical A13-A20).
    pub mpr: [u8; 8],
    pub bus: B,
    pub pins: Pins,
    /// The last value TAM or TMA moved: what TMA with an operand of $00
    /// returns (U11).
    pub(crate) mpr_latch: u8,
    /// T as the instruction fetch found it; the fetch clears it in P (U4).
    pub(crate) t: u8,
    pub(crate) cycles: u32,

    // -- the named switches, each with the reference's default ------------
    /// Named so a board or a later capture can change it: whether a pointer
    /// at zero-page offset $FF takes its high byte from offset $00 (Mesen's
    /// ReadZeroPageWrap, MAME's program_read16z; emulator tier, U8) or from
    /// logical $2100.
    pub zero_page_pointer_wraps: bool,
    /// C5: whether SBC honours T like ADC (`false`: SW and pcetech list only
    /// ORA AND EOR ADC).
    pub sbc_honours_t: bool,
    /// C13: the decimal digit adjust of ADC and SBC.
    pub decimal_formula: DecimalFormula,
    /// C12: what TSB and TRB's Z reports. `false` (default): the stored
    /// result is zero (pcetech 2.0 "Z = If result is zero", MAME, Mednafen
    /// and Mesen2's current source). `true`: the 65C02's A AND M is zero,
    /// which is what the SingleStepTests corpus (an older Mesen) records.
    pub tsb_z_from_test: bool,
    /// C9: pcetech says reads of $0800-$17FF return 0 in a block transfer
    /// (hw-verified); MAME and Mednafen read the device. `false` reads the
    /// device like any access.
    pub block_reads_hardware_page_as_zero: bool,
    /// C4: pcetech tested opcode $63 as a two-cycle NOP; MAME bills four.
    /// Emulator-tier knob for a board that wants MAME's count.
    pub nop_63_cycles: u32,
    /// C3: an interrupt entry takes 8 cycles in Mesen, Mednafen, Geargrafx
    /// and ares, 7 in MAME (emulator tier; no manual states it).
    pub interrupt_entry_cycles: u32,
    /// C18: what an interrupt does when its source has gone between the
    /// poll and the entry.
    pub vanished_interrupt: VanishedInterrupt,
    /// U25: which cycle of an instruction the interrupt lines are sampled at.
    pub irq_poll_cycle: IrqPollCycle,
    /// C6: TMA with several mask bits.
    pub tma_multiple: TmaMultiple,
}

impl<B: Bus> HuC6280<B> {
    /// A HuC6280 on `bus`.
    pub fn new(bus: B) -> Self {
        HuC6280 {
            // Power-up values are random on hardware (A X Y S, MPR0-6; U2).
            // Zero makes traces reproducible; reset sets only what HW HB-18
            // lists.
            a: 0,
            x: 0,
            y: 0,
            s: 0xFF,
            pc: 0,
            p: I,
            mpr: [0; 8],
            bus,
            pins: Pins::default(),
            mpr_latch: 0,
            t: 0,
            cycles: 0,
            zero_page_pointer_wraps: true,
            sbc_honours_t: false,
            decimal_formula: DecimalFormula::Mednafen,
            tsb_z_from_test: false,
            block_reads_hardware_page_as_zero: true,
            nop_63_cycles: 2,
            interrupt_entry_cycles: 8,
            vanished_interrupt: VanishedInterrupt::Mesen2,
            irq_poll_cycle: IrqPollCycle::SecondToLast,
            tma_multiple: TmaMultiple::Or,
        }
    }

    /// The current cycle's kind ([`Pins::kind`]).
    pub fn kind(&self) -> Kind {
        self.pins.kind
    }

    /// The opcode of the instruction being executed ([`Pins::opcode`]): the
    /// public accessor the SuperGrafx uses.
    pub fn opcode(&self) -> u8 {
        self.pins.opcode
    }

    /// The clock switch ([`Pins::high_speed`]).
    pub fn high_speed(&self) -> bool {
        self.pins.high_speed
    }

    // -- the mapper -------------------------------------------------------

    /// The 21-bit physical address of a 16-bit logical one (HW HB-7).
    #[inline]
    pub fn physical(&self, address: u16) -> u32 {
        ((self.mpr[(address >> 13) as usize] as u32) << 13) | (address & 0x1FFF) as u32
    }

    // -- cycles ----------------------------------------------------------

    /// A data or program read at a logical address.
    #[inline(always)]
    pub(crate) fn cycle_read(&mut self, address: u16) -> u8 {
        self.pins.kind = Kind::Read;
        self.cycles += 1;
        self.pins.access();
        let physical = self.physical(address);
        self.bus.read(physical, &mut self.pins)
    }

    /// A write at a logical address.
    #[inline(always)]
    pub(crate) fn cycle_write(&mut self, address: u16, value: u8) {
        self.pins.kind = Kind::Write;
        self.cycles += 1;
        self.pins.access();
        let physical = self.physical(address);
        self.bus.write(physical, value, &mut self.pins);
    }

    /// A write at a physical address, bypassing the mapper (ST0-ST2, U12).
    pub(crate) fn cycle_write_physical(&mut self, address: u32, value: u8) {
        self.pins.kind = Kind::Write;
        self.cycles += 1;
        self.pins.access();
        self.bus.write(address, value, &mut self.pins);
    }

    /// A read the chip makes and discards, at a logical address (C8).
    #[inline(always)]
    pub(crate) fn dummy(&mut self, address: u16) {
        self.pins.kind = Kind::Dummy;
        self.cycles += 1;
        self.pins.access();
        let physical = self.physical(address);
        self.bus.idle(Some(physical), &mut self.pins);
    }

    /// An internal cycle: nothing on the bus (C8).
    #[inline(always)]
    pub(crate) fn io(&mut self) {
        self.pins.kind = Kind::Idle;
        self.cycles += 1;
        self.pins.access();
        self.bus.idle(None, &mut self.pins);
    }

    /// The dummy read after an opcode that has no operand: PC, the byte after it.
    #[inline]
    pub(crate) fn implied(&mut self) {
        self.dummy(self.pc);
    }

    /// One operand byte at PC.
    #[inline(always)]
    pub(crate) fn fetch(&mut self) -> u8 {
        let pc = self.pc;
        self.pc = pc.wrapping_add(1);
        self.cycle_read(pc)
    }

    pub(crate) fn fetch_word(&mut self) -> u16 {
        let low = self.fetch() as u16;
        ((self.fetch() as u16) << 8) | low
    }

    // -- operands --------------------------------------------------------

    #[inline]
    pub(crate) fn load(&mut self, address: u16) -> u8 {
        self.cycle_read(address)
    }

    #[inline]
    pub(crate) fn store(&mut self, address: u16, value: u8) {
        self.cycle_write(address, value);
    }

    /// A read-modify-write: read, an internal cycle, write. No dummy write
    /// (the reference's docs/timing.md).
    pub(crate) fn modify(&mut self, address: u16, operation: fn(&mut Self, u8) -> u8) {
        let value = self.cycle_read(address);
        self.io();
        let result = operation(self, value);
        self.cycle_write(address, result);
    }

    #[inline]
    pub(crate) fn set_nz(&mut self, value: u8) -> u8 {
        self.p = (self.p & !(N | Z)) | nz(value);
        value
    }

    // -- the stack (logical $2100 + S; S wraps in 8 bits) -----------------

    pub(crate) fn push(&mut self, value: u8) {
        let s = self.s;
        self.cycle_write(STACK_PAGE | s as u16, value);
        self.s = s.wrapping_sub(1);
    }

    pub(crate) fn pull(&mut self) -> u8 {
        let s = self.s.wrapping_add(1);
        self.s = s;
        self.cycle_read(STACK_PAGE | s as u16)
    }

    // -- addressing modes -------------------------------------------------
    // Each makes its addressing cycles and returns the operand's logical
    // address. Reads, writes and read-modify-writes share one shape: there is
    // no index cost and no page-crossing cost on this chip (pcetech 2.0, U9).

    /// `#imm`: the operand is the byte at PC; the handler's load is its cycle.
    pub(crate) fn ea_imm(&mut self) -> u16 {
        let pc = self.pc;
        self.pc = pc.wrapping_add(1);
        pc
    }

    /// `zp`: logical $2000 + zp (U1), then an internal cycle.
    pub(crate) fn ea_zp(&mut self) -> u16 {
        let zp = self.fetch();
        self.io();
        ZERO_PAGE | zp as u16
    }

    /// `zp,X`: the sum wraps within the zero page.
    pub(crate) fn ea_zpx(&mut self) -> u16 {
        let zp = self.fetch();
        self.io();
        ZERO_PAGE | zp.wrapping_add(self.x) as u16
    }

    /// `zp,Y` (LDX and STX only).
    pub(crate) fn ea_zpy(&mut self) -> u16 {
        let zp = self.fetch();
        self.io();
        ZERO_PAGE | zp.wrapping_add(self.y) as u16
    }

    /// `abs`, then an internal cycle.
    pub(crate) fn ea_abs(&mut self) -> u16 {
        let address = self.fetch_word();
        self.io();
        address
    }

    /// `abs,X`: a 16-bit sum, no extra cycle for a page crossing (pcetech 2.0).
    pub(crate) fn ea_abx(&mut self) -> u16 {
        let address = self.fetch_word();
        self.io();
        address.wrapping_add(self.x as u16)
    }

    /// `abs,Y`.
    pub(crate) fn ea_aby(&mut self) -> u16 {
        let address = self.fetch_word();
        self.io();
        address.wrapping_add(self.y as u16)
    }

    /// A two-byte pointer in the zero page (U8).
    pub(crate) fn pointer(&mut self, zp: u8) -> u16 {
        let low = self.cycle_read(ZERO_PAGE | zp as u16) as u16;
        let high_zp = if self.zero_page_pointer_wraps {
            zp.wrapping_add(1) as u16
        } else {
            zp as u16 + 1
        };
        ((self.cycle_read(ZERO_PAGE + high_zp) as u16) << 8) | low
    }

    /// `(zp,X)`: the index is added before the pointer is read.
    pub(crate) fn ea_idx(&mut self) -> u16 {
        let zp = self.fetch();
        self.io();
        let pointer = self.pointer(zp.wrapping_add(self.x));
        self.io();
        pointer
    }

    /// `(zp),Y`: the index is added after, 16 bits.
    pub(crate) fn ea_idy(&mut self) -> u16 {
        let zp = self.fetch();
        self.io();
        let pointer = self.pointer(zp);
        self.io();
        pointer.wrapping_add(self.y as u16)
    }

    /// `(zp)`, the 65C02 addition.
    pub(crate) fn ea_ind(&mut self) -> u16 {
        let zp = self.fetch();
        self.io();
        let pointer = self.pointer(zp);
        self.io();
        pointer
    }
}
