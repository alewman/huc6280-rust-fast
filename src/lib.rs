//! A Rust Hudson HuC6280 core transcribed from
//! [huc6280-python](https://github.com/alewman/huc6280-python) at `v0.3.0`
//! (`bf20330`) and proven equivalent to it at every processor boundary.
//!
//! Each module is the transcription of one `huc6280/*.py` module, with the
//! same handler names, the same explicit dispatch table, the named switches
//! as fields and the hardware comments carried over: `core`, `alu`, `bits`,
//! `block`, `control`, `hudson`, `loads`, `shifts`, `dispatch`, `cpu`,
//! [`state`] and [`trace`]. It builds on 6502-rust's frame (crate
//! `sixfiveohtwo`: the interrupt requests, the poll and `step()`'s
//! skeleton) as the reference builds on 6502-python's `CPUFrame`. Where
//! Rust forced a difference, the module header says so.
//!
//! ```
//! use huc6280::{Bus, HuC6280, Pins};
//!
//! /// 2 MB of flat RAM over the 21-bit physical space.
//! struct Ram(Vec<u8>);
//! impl Bus for Ram {
//!     fn read(&mut self, address: u32, _: &mut Pins) -> u8 {
//!         self.0[address as usize]
//!     }
//!     fn write(&mut self, address: u32, value: u8, _: &mut Pins) {
//!         self.0[address as usize] = value;
//!     }
//! }
//!
//! let mut ram = Ram(vec![0; 0x200000]);
//! ram.0[0x1F0000] = 0xE8; // INX, in work RAM (page $F8)
//! let mut cpu = HuC6280::new(ram);
//! cpu.mpr[2] = 0xF8; // logical $4000-$5FFF is page $F8
//! cpu.pc = 0x4000;
//! assert_eq!(cpu.step(), 2);
//! assert_eq!(cpu.x, 1);
//! ```

mod alu;
mod bits;
mod block;
mod control;
mod core;
mod cpu;
mod dispatch;
mod hudson;
mod loads;
mod shifts;
pub mod state;
pub mod trace;

pub use sixfiveohtwo::frame::{Frame, Part, PollFields, IRQ_LINE, NMI_LATCH, NO_POLL};

pub use crate::alu::{add, subtract, DecimalFormula};
pub use crate::block::{HARDWARE_ZERO_END, HARDWARE_ZERO_START};
pub use crate::control::{
    IrqPollCycle, VanishedInterrupt, IRQ1_BIT, IRQ2_BIT, TIQ_BIT, VECTOR_IRQ1, VECTOR_IRQ2,
    VECTOR_NMI, VECTOR_RESET, VECTOR_TIQ,
};
pub use crate::core::{Bus, HuC6280, Kind, Pins, B, C, D, I, N, STACK_PAGE, T, V, Z, ZERO_PAGE};
pub use crate::dispatch::{Form, Mode, Op, Row, OPCODES};
pub use crate::hudson::{TmaMultiple, VDC_ADDRESS};
pub use crate::state::CpuState;

/// The README's example compiles and runs as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
