//! Boundary traces in the reference's version 1 JSON Lines schema
//! (`trace.py` and the parts of `debug.py` a trace needs; 6502-python
//! `docs/trace-schema.md`).
//!
//! One JSON object per boundary: `kind` one of [`BoundaryKind`]'s values,
//! `before` and `after` every field of [`CpuState`], `instruction.address`
//! the 16-bit logical PC and `instruction.data` the instruction's bytes (no
//! mnemonic: the reference disassembles the bytes itself), and `accesses`
//! `[kind, address, value]` with kinds `"r"`, `"w"`, `"d"` (a dummy read,
//! value 0) and `"i"` (an internal cycle, recorded at address 0, value 0)
//! and 21-bit physical addresses, one per cycle. The reference's reader,
//! `read_trace`, and `first_trace_divergence` compare a trace written here
//! with one its `HuC6280Session` wrote. Only writing is transcribed;
//! comparing is the reference's job.

use serde_json::{json, Map, Value};

use crate::core::I;
use crate::dispatch::OPCODES;
use crate::state::CpuState;

pub const TRACE_SCHEMA_VERSION: u32 = 1;

/// Kind of work performed by one `HuC6280::step()` call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundaryKind {
    Instruction,
    Reset,
    Nmi,
    Irq1,
    Irq2,
    Tiq,
}

impl BoundaryKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            BoundaryKind::Instruction => "instruction",
            BoundaryKind::Reset => "reset",
            BoundaryKind::Nmi => "nmi",
            BoundaryKind::Irq1 => "irq1",
            BoundaryKind::Irq2 => "irq2",
            BoundaryKind::Tiq => "tiq",
        }
    }
}

/// What the next `step()` will do, decided as the reference's
/// `debug.next_boundary` decides it from a state.
pub fn next_boundary(state: &CpuState) -> BoundaryKind {
    if state.reset_pending {
        return BoundaryKind::Reset;
    }
    if state.skipped_poll {
        return BoundaryKind::Instruction;
    }
    let nmi = state
        .polled_non_maskable_interrupt
        .unwrap_or(state.non_maskable_interrupt_pending);
    if nmi {
        return BoundaryKind::Nmi;
    }
    let irq = state
        .polled_maskable_interrupt
        .unwrap_or(state.maskable_interrupt_pending);
    let masked = state.polled_i.unwrap_or(state.p & I != 0);
    if irq && !masked {
        // The vector is chosen from the lines the poll saw, in priority order
        // (HW HB-13). The switches are not state: this is the defaults'
        // decision (irq_poll_cycle SecondToLast, vanished_interrupt Mesen2:
        // the timer must still be asserted, and with no source left an
        // instruction runs; C18, issue 0005).
        let enabled = !state.irq_disable & 7;
        let live = ((state.tiq as u8) << 2 | (state.irq1 as u8) << 1 | state.irq2 as u8) & enabled;
        let polled = match state.polled_irq_lines {
            None => live,
            Some(lines) => lines & enabled,
        };
        if polled & live & 4 != 0 {
            return BoundaryKind::Tiq;
        }
        if polled & 2 != 0 {
            return BoundaryKind::Irq1;
        }
        if polled & 1 != 0 {
            return BoundaryKind::Irq2;
        }
    }
    BoundaryKind::Instruction
}

/// How many bytes the instruction with this opcode occupies (the
/// reference's `FORM_BYTES` by its form).
pub fn instruction_length(opcode: u8) -> usize {
    OPCODES[opcode as usize].3.bytes()
}

/// One cycle as the trace records it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read(u32, u8),
    Write(u32, u8),
    /// A dummy read at this address.
    Dummy(u32),
    /// An internal cycle.
    Idle,
}

/// One processor boundary.
#[derive(Clone, Debug)]
pub struct StepRecord {
    pub sequence: u64,
    pub kind: BoundaryKind,
    pub cycles: u32,
    /// The instruction's logical address and bytes, for an instruction boundary.
    pub instruction: Option<(u16, Vec<u8>)>,
    pub before: CpuState,
    pub after: CpuState,
    pub accesses: Option<Vec<Access>>,
}

fn state_value(state: &CpuState) -> Value {
    json!({
        "a": state.a,
        "x": state.x,
        "y": state.y,
        "s": state.s,
        "pc": state.pc,
        "p": state.p,
        "mpr": state.mpr,
        "high_speed": state.high_speed,
        "mpr_latch": state.mpr_latch,
        "irq1": state.irq1,
        "irq2": state.irq2,
        "tiq": state.tiq,
        "irq_disable": state.irq_disable,
        "halted": state.halted,
        "reset_pending": state.reset_pending,
        "maskable_interrupt_pending": state.maskable_interrupt_pending,
        "non_maskable_interrupt_pending": state.non_maskable_interrupt_pending,
        "polled_i": state.polled_i,
        "polled_maskable_interrupt": state.polled_maskable_interrupt,
        "polled_non_maskable_interrupt": state.polled_non_maskable_interrupt,
        "skipped_poll": state.skipped_poll,
        "polled_irq_lines": state.polled_irq_lines,
    })
}

impl StepRecord {
    /// The versioned JSON representation.
    pub fn to_value(&self) -> Value {
        let mut record = Map::new();
        record.insert("version".into(), json!(TRACE_SCHEMA_VERSION));
        record.insert("sequence".into(), json!(self.sequence));
        record.insert("kind".into(), json!(self.kind.as_str()));
        record.insert("cycles".into(), json!(self.cycles));
        let instruction = match &self.instruction {
            None => Value::Null,
            Some((address, data)) => {
                let hex: String = data.iter().map(|b| format!("{b:02x}")).collect();
                json!({"address": address, "data": hex})
            }
        };
        record.insert("instruction".into(), instruction);
        record.insert("before".into(), state_value(&self.before));
        record.insert("after".into(), state_value(&self.after));
        if let Some(accesses) = &self.accesses {
            let list: Vec<Value> = accesses
                .iter()
                .map(|access| match *access {
                    Access::Read(address, value) => json!(["r", address, value]),
                    Access::Write(address, value) => json!(["w", address, value]),
                    Access::Dummy(address) => json!(["d", address, 0]),
                    Access::Idle => json!(["i", 0, 0]),
                })
                .collect();
            record.insert("accesses".into(), Value::Array(list));
        }
        Value::Object(record)
    }

    /// One JSON Lines line, without the newline.
    pub fn to_line(&self) -> String {
        self.to_value().to_string()
    }
}
