// SPDX-License-Identifier: MIT
//! Run a scenario and write its boundary trace (rungs 1 and 4).
//!
//!     trace SCENARIO.json [--out TRACE.jsonl]
//!
//! The scenario format (`conformance/README.md`) is read the same way by
//! `scripts/reference_trace.py`, which runs it on huc6280-python; the two
//! traces are compared with the reference's `first_trace_divergence`.

use std::collections::HashMap;
use std::io::{BufWriter, Write};

use huc6280::trace::{instruction_length, next_boundary, Access, BoundaryKind, StepRecord};
use huc6280::{Bus, DecimalFormula, HuC6280, IrqPollCycle, Pins, TmaMultiple, VanishedInterrupt};
use serde::Deserialize;

#[derive(Deserialize)]
struct Block {
    address: u32,
    hex: String,
}

#[derive(Deserialize, Default, Clone)]
struct Registers {
    a: Option<u8>,
    x: Option<u8>,
    y: Option<u8>,
    s: Option<u8>,
    pc: Option<u16>,
    p: Option<u8>,
    mpr: Option<[u8; 8]>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Switches {
    decimal_formula: Option<String>,
    sbc_honours_t: Option<bool>,
    irq_poll_cycle: Option<String>,
    vanished_interrupt: Option<String>,
    tma_multiple: Option<String>,
    tsb_z_from_test: Option<bool>,
    zero_page_pointer_wraps: Option<bool>,
    block_reads_hardware_page_as_zero: Option<bool>,
    nop_63_cycles: Option<u32>,
    interrupt_entry_cycles: Option<u32>,
}

#[derive(Deserialize, Clone)]
struct Event {
    #[serde(default)]
    step: Option<u64>,
    #[serde(default)]
    access: Option<u64>,
    action: String,
    #[serde(default)]
    value: u8,
    /// For the action `set`: the registers the host loads at that boundary.
    #[serde(default)]
    registers: Option<Registers>,
}

#[derive(Deserialize)]
struct Scenario {
    /// Seeded noise over the whole 2 MB, under the blocks.
    #[serde(default)]
    fill: Option<u64>,
    /// The host answers three hardware-page addresses as a board would.
    #[serde(default)]
    board: bool,
    memory: Vec<Block>,
    #[serde(default)]
    registers: Registers,
    #[serde(default)]
    switches: Switches,
    #[serde(default)]
    reset: bool,
    steps: u64,
    #[serde(default)]
    events: Vec<Event>,
    #[serde(default = "default_true")]
    accesses: bool,
}

fn default_true() -> bool {
    true
}

/// A host action a bus callback can make: it reaches the CPU through its pins.
fn act(pins: &mut Pins, event: &Event) {
    let value = &event.value;
    match event.action.as_str() {
        // The three lines in the $1402 layout: bit 0 IRQ2, bit 1 IRQ1, bit 2 TIQ.
        "lines" => pins.set_irq_lines(value & 2 != 0, value & 1 != 0, value & 4 != 0),
        "disable" => pins.set_irq_disable(*value),
        "nmi" => pins.frame.request_non_maskable_interrupt(),
        "nmi_clear" => pins.frame.clear_non_maskable_interrupt(),
        "reset" => pins.frame.request_reset(),
        other => panic!("unknown action {other}"),
    }
}

/// 2 MB of flat RAM, the scenario's in-cycle actions and the access log.
struct Host {
    memory: Vec<u8>,
    count: u64,
    log: Option<Vec<Access>>,
    during: HashMap<u64, Vec<Event>>,
    board: bool,
}

/// The sliver of a PC Engine the interrupt workout uses (`"board": true`).
const IRQ_DISABLE: u32 = 0x1FF402;
const TIMER_ACKNOWLEDGE: u32 = 0x1FF403;
const VDC_STATUS: u32 = 0x1FE000;

impl Host {
    fn tick(&mut self, pins: &mut Pins) {
        let index = self.count;
        self.count += 1;
        if let Some(actions) = self.during.get(&index) {
            for action in actions {
                act(pins, action);
            }
        }
    }
}

impl Bus for Host {
    fn read(&mut self, address: u32, pins: &mut Pins) -> u8 {
        self.tick(pins);
        if self.board && address == VDC_STATUS {
            let (_, irq2, tiq) = pins.irq_lines();
            pins.set_irq_lines(false, irq2, tiq);
        }
        let value = self.memory[address as usize];
        if let Some(log) = &mut self.log {
            log.push(Access::Read(address, value));
        }
        value
    }

    fn write(&mut self, address: u32, value: u8, pins: &mut Pins) {
        if let Some(log) = &mut self.log {
            log.push(Access::Write(address, value));
        }
        self.tick(pins);
        if self.board && address == IRQ_DISABLE {
            pins.set_irq_disable(value);
        } else if self.board && address == TIMER_ACKNOWLEDGE {
            let (irq1, irq2, _) = pins.irq_lines();
            pins.set_irq_lines(irq1, irq2, false);
        }
        self.memory[address as usize] = value;
    }

    fn idle(&mut self, address: Option<u32>, pins: &mut Pins) {
        if let Some(log) = &mut self.log {
            log.push(match address {
                Some(address) => Access::Dummy(address),
                None => Access::Idle,
            });
        }
        self.tick(pins);
    }
}

fn set_switches(cpu: &mut HuC6280<Host>, switches: &Switches) {
    if let Some(name) = &switches.decimal_formula {
        cpu.decimal_formula = match name.as_str() {
            "mednafen" => DecimalFormula::Mednafen,
            "mesen" => DecimalFormula::Mesen,
            other => panic!("unknown decimal_formula {other}"),
        };
    }
    if let Some(name) = &switches.irq_poll_cycle {
        cpu.irq_poll_cycle = match name.as_str() {
            "second_to_last" => IrqPollCycle::SecondToLast,
            "last" => IrqPollCycle::Last,
            other => panic!("unknown irq_poll_cycle {other}"),
        };
    }
    if let Some(name) = &switches.vanished_interrupt {
        cpu.vanished_interrupt = match name.as_str() {
            "mesen2" => VanishedInterrupt::Mesen2,
            "drop" => VanishedInterrupt::Drop,
            other => panic!("unknown vanished_interrupt {other}"),
        };
    }
    if let Some(name) = &switches.tma_multiple {
        cpu.tma_multiple = match name.as_str() {
            "or" => TmaMultiple::Or,
            "highest" => TmaMultiple::Highest,
            "lowest" => TmaMultiple::Lowest,
            other => panic!("unknown tma_multiple {other}"),
        };
    }
    if let Some(value) = switches.sbc_honours_t {
        cpu.sbc_honours_t = value;
    }
    if let Some(value) = switches.tsb_z_from_test {
        cpu.tsb_z_from_test = value;
    }
    if let Some(value) = switches.zero_page_pointer_wraps {
        cpu.zero_page_pointer_wraps = value;
    }
    if let Some(value) = switches.block_reads_hardware_page_as_zero {
        cpu.block_reads_hardware_page_as_zero = value;
    }
    if let Some(value) = switches.nop_63_cycles {
        cpu.nop_63_cycles = value;
    }
    if let Some(value) = switches.interrupt_entry_cycles {
        cpu.interrupt_entry_cycles = value;
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: trace SCENARIO.json [--out TRACE.jsonl]");
    let mut out: Box<dyn Write> = Box::new(std::io::stdout().lock());
    while let Some(arg) = args.next() {
        if arg == "--out" {
            let file = std::fs::File::create(args.next().expect("--out PATH")).expect("create");
            out = Box::new(file);
        }
    }
    let mut out = BufWriter::new(out);
    let parsed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    // A file holds one scenario or a list of them, run one after another.
    let scenarios: Vec<Scenario> = if parsed.is_array() {
        serde_json::from_value(parsed).expect("scenarios")
    } else {
        vec![serde_json::from_value(parsed).expect("scenario")]
    };
    for scenario in &scenarios {
        run(scenario, &mut out);
    }
}

fn set_registers(cpu: &mut HuC6280<Host>, r: &Registers) {
    cpu.a = r.a.unwrap_or(cpu.a);
    cpu.x = r.x.unwrap_or(cpu.x);
    cpu.y = r.y.unwrap_or(cpu.y);
    cpu.s = r.s.unwrap_or(cpu.s);
    cpu.pc = r.pc.unwrap_or(cpu.pc);
    cpu.p = r.p.unwrap_or(cpu.p);
    cpu.mpr = r.mpr.unwrap_or(cpu.mpr);
}

fn run(scenario: &Scenario, out: &mut impl Write) {
    let mut memory = vec![0u8; 0x200000];
    if let Some(seed) = scenario.fill {
        // The same generator as scripts/reference_trace.py's.
        let mut state = seed;
        for byte in memory.iter_mut() {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *byte = (state >> 56) as u8;
        }
    }
    for block in &scenario.memory {
        let hex = &block.hex;
        for (offset, i) in (0..hex.len()).step_by(2).enumerate() {
            let byte = u8::from_str_radix(&hex[i..i + 2], 16).expect("hex");
            memory[(block.address as usize + offset) & 0x1FFFFF] = byte;
        }
    }
    let mut during: HashMap<u64, Vec<Event>> = HashMap::new();
    let mut between: HashMap<u64, Vec<Event>> = HashMap::new();
    for event in &scenario.events {
        match (event.step, event.access) {
            (Some(step), None) => between.entry(step).or_default().push(event.clone()),
            (None, Some(access)) => during.entry(access).or_default().push(event.clone()),
            _ => panic!("an event has step or access"),
        }
    }
    let host = Host {
        memory,
        count: 0,
        log: scenario.accesses.then(Vec::new),
        during,
        board: scenario.board,
    };
    let mut cpu = HuC6280::new(host);
    set_registers(&mut cpu, &scenario.registers);
    set_switches(&mut cpu, &scenario.switches);
    if scenario.reset {
        cpu.request_reset();
    }

    for sequence in 0..scenario.steps {
        if let Some(actions) = between.get(&sequence) {
            for event in actions {
                match event.action.as_str() {
                    "restore" => {
                        // Capture and restore at this boundary, as the reference's script does.
                        let state = cpu.capture_state();
                        cpu.restore_state(&state).expect("restore");
                    }
                    "set" => set_registers(&mut cpu, event.registers.as_ref().expect("registers")),
                    _ => act(&mut cpu.pins, event),
                }
            }
        }
        let before = cpu.capture_state();
        let kind = next_boundary(&before);
        let instruction = if kind == BoundaryKind::Instruction {
            let pc = before.pc;
            let peek = |logical: u16| cpu.bus.memory[cpu.physical(logical) as usize];
            let length = instruction_length(peek(pc));
            let data = (0..length)
                .map(|n| peek(pc.wrapping_add(n as u16)))
                .collect();
            Some((pc, data))
        } else {
            None
        };
        if let Some(log) = &mut cpu.bus.log {
            log.clear();
        }
        let cycles = cpu.step();
        let after = cpu.capture_state();
        let record = StepRecord {
            sequence,
            kind,
            cycles,
            instruction,
            before,
            after,
            accesses: cpu.bus.log.clone(),
        };
        writeln!(out, "{}", record.to_line()).expect("write");
    }
}
