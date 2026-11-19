// SPDX-License-Identifier: MIT
//! Rung 5 without MAME traces: a long synthetic program on a seeded bus.
//!
//!     long SEED STEPS [--alternate]
//!
//! Every read is answered from a 64-bit linear congruential generator, so the
//! core runs a random instruction stream with random operands, through every
//! opcode, T and D both ways, with the mapper changing under it. Every 1,024
//! cycles the bus takes one more value from the generator and drives the
//! interrupt inputs with it (the three lines, now and then the disable
//! register, an NMI, rarely a reset). Every cycle is folded into a digest
//! (address, value, and what the callback sees in the pins: the cycle's kind,
//! the clock switch, the opcode) and so is the whole `CpuState` after every
//! step;
//! the digest is printed every 250,000 steps and at the end.
//! `scripts/reference_long.py` does the same on the reference core, and the
//! two outputs are compared line for line.
//!
//! Block transfers would swallow the run (65,536 bytes each, on average half
//! that): a read that returns one of the five block opcodes is changed to the
//! opcode before it 63 times in 64, on both sides alike.

use huc6280::state::CpuState;
use huc6280::{
    Bus, DecimalFormula, HuC6280, IrqPollCycle, Kind, Pins, TmaMultiple, VanishedInterrupt,
};

const PRIME: u64 = 0x100000001B3;

struct Noise {
    state: u64,
    count: u64,
    digest: u64,
}

impl Noise {
    fn draw(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    fn fold(&mut self, value: u64) {
        self.digest = (self.digest ^ value).wrapping_mul(PRIME);
    }

    /// Every cycle: count it, and every 1,024th drive the interrupt inputs.
    fn tick(&mut self, pins: &mut Pins) {
        self.count += 1;
        if self.count & 0x3FF == 0 {
            let roll = self.draw() >> 32;
            pins.set_irq_lines(roll & 2 != 0, roll & 1 != 0, roll & 4 != 0);
            if (roll >> 3) & 7 == 0 {
                pins.set_irq_disable((roll >> 6) as u8 & 7);
            }
            if (roll >> 9) & 0x1F == 0 {
                pins.frame.request_non_maskable_interrupt();
            }
            if (roll >> 14) & 0x3FF == 0 {
                pins.frame.request_reset();
            }
        }
    }
}

/// What a callback can see of the CPU, for the digest: the cycle's kind as
/// `Pins::kind` gives it, the clock switch and the opcode.
fn seen(pins: &Pins) -> u64 {
    let kind = match pins.kind() {
        Kind::Read => 1,
        Kind::Write => 2,
        Kind::Dummy => 3,
        Kind::Idle => 4,
    };
    (kind << 32) | ((pins.high_speed as u64) << 36) | ((pins.opcode() as u64) << 40)
}

impl Bus for Noise {
    fn read(&mut self, address: u32, pins: &mut Pins) -> u8 {
        self.tick(pins);
        let state = self.draw();
        let mut value = (state >> 56) as u8;
        if matches!(value, 0x73 | 0xC3 | 0xD3 | 0xE3 | 0xF3) && (state >> 48) & 0x3F != 0 {
            value ^= 0x01;
        }
        self.fold(seen(pins) | ((address as u64) << 8) | value as u64);
        value
    }

    fn write(&mut self, address: u32, value: u8, pins: &mut Pins) {
        self.tick(pins);
        self.fold(seen(pins) | ((address as u64) << 8) | value as u64);
    }

    fn idle(&mut self, address: Option<u32>, pins: &mut Pins) {
        self.tick(pins);
        self.fold(seen(pins) | ((address.unwrap_or(0) as u64) << 8));
    }
}

fn tri(value: Option<bool>) -> u64 {
    match value {
        None => 2,
        Some(value) => value as u64,
    }
}

/// Every field of the state, in the order the reference's script folds them.
fn fold_state(bus: &mut Noise, state: &CpuState, cycles: u32) {
    bus.fold(cycles as u64);
    for value in [state.a, state.x, state.y, state.s] {
        bus.fold(value as u64);
    }
    bus.fold(state.pc as u64);
    bus.fold(state.p as u64);
    for value in state.mpr {
        bus.fold(value as u64);
    }
    for value in [
        state.high_speed as u64,
        state.mpr_latch as u64,
        state.irq1 as u64,
        state.irq2 as u64,
        state.tiq as u64,
        state.irq_disable as u64,
        state.halted as u64,
        state.reset_pending as u64,
        state.maskable_interrupt_pending as u64,
        state.non_maskable_interrupt_pending as u64,
        tri(state.polled_i),
        tri(state.polled_maskable_interrupt),
        tri(state.polled_non_maskable_interrupt),
        state.skipped_poll as u64,
        state.polled_irq_lines.map_or(8, u64::from),
    ] {
        bus.fold(value);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: long SEED STEPS [--alternate]");
        std::process::exit(2);
    }
    let seed: u64 = args[0].parse().expect("SEED");
    let steps: u64 = args[1].parse().expect("STEPS");
    let mut cpu = HuC6280::new(Noise {
        state: seed,
        count: 0,
        digest: 0xCBF29CE484222325,
    });
    if args.get(2).map(String::as_str) == Some("--alternate") {
        cpu.decimal_formula = DecimalFormula::Mesen;
        cpu.sbc_honours_t = true;
        cpu.tsb_z_from_test = true;
        cpu.tma_multiple = TmaMultiple::Highest;
        cpu.zero_page_pointer_wraps = false;
        cpu.block_reads_hardware_page_as_zero = false;
        cpu.nop_63_cycles = 4;
        cpu.interrupt_entry_cycles = 7;
        cpu.irq_poll_cycle = IrqPollCycle::Last;
        // Drop since the reference's clean rewrite withdrew "latched" (6ce348e). Under
        // poll Last the poll lines are the live lines, so "latched" and "drop" took the
        // same sources: the run is the one it was.
        cpu.vanished_interrupt = VanishedInterrupt::Drop;
    }
    cpu.request_reset();
    let began = std::time::Instant::now();
    let mut total = 0u64;
    for step in 1..=steps {
        let cycles = cpu.step();
        total += cycles as u64;
        let state = cpu.capture_state();
        fold_state(&mut cpu.bus, &state, cycles);
        if step % 250_000 == 0 || step == steps {
            println!("step {step} cycles {total} digest {:016x}", cpu.bus.digest);
        }
    }
    eprintln!("{:.3} s", began.elapsed().as_secs_f64());
}
