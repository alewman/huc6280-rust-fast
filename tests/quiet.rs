//! The fork's quiet window (`Bus::QUIET`): a bus that lends windows must see
//! exactly what the same bus sees without lending them, as a board would: the
//! same calls at the same master clocks with the same pins, the same memory,
//! the same clock and the same CPU state after every step.
//!
//! The model board keeps a master clock and an event horizon (`wake`) as
//! pcengine-rust-fast does: a call whose cycle ends at or past the horizon is
//! an event (it moves the horizon by a random amount, may change the
//! interrupt lines, the disable register or request an NMI, may answer a read
//! from the generator), and every call to its device page ($FF) is one too. Below the
//! horizon a cycle only moves the clock and, in RAM, the byte. Memory is
//! random code with the mapper pointed at it; the block opcodes are rare (as in
//! rung 5's synthetic program), so the run goes through every other opcode,
//! CSL and CSH, T and D, and the interrupt entries.

use huc6280::{Bus, CpuState, HuC6280, Kind, Pins, QuietWindow};

const LOW: i64 = 12;
const HIGH: i64 = 3;
const DEVICE_BANK: u32 = 0xFF;

/// One call as the board saw it: kind, address, value, the clock after the
/// cycle, the pins' kind, opcode and speed.
type Call = (u8, u32, u8, i64, Kind, u8, bool);

struct Model<const LEND: bool> {
    memory: Vec<u8>,
    mclk: i64,
    wake: i64,
    state: u64,
    calls: Vec<Call>,
}

impl<const LEND: bool> Model<LEND> {
    fn new(seed: u64) -> Self {
        let mut model = Model {
            memory: vec![0; 0x200000],
            mclk: 0,
            wake: 0,
            state: seed,
            calls: Vec::new(),
        };
        for i in 0..model.memory.len() {
            let mut value = (model.draw() >> 56) as u8;
            if matches!(value, 0x73 | 0xC3 | 0xD3 | 0xE3 | 0xF3) && model.draw() & 0x3F != 0 {
                value ^= 0x01; // block transfers would swallow the run
            }
            model.memory[i] = value;
        }
        model
    }

    fn draw(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    /// A call: the clock moves; at the horizon, or on the device page, something happens.
    fn call(&mut self, kind: u8, address: u32, value: u8, pins: &mut Pins) -> Option<u8> {
        self.mclk += if pins.high_speed { HIGH } else { LOW };
        let device = kind != 2 && address >> 13 == DEVICE_BANK;
        let mut answer = None;
        if self.mclk >= self.wake || device {
            let roll = self.draw();
            if self.mclk >= self.wake {
                self.wake = self.mclk + (roll >> 40) as i64 % 400;
            }
            if roll & 0x0F == 0 {
                pins.set_irq_lines(roll & 0x10 != 0, roll & 0x20 != 0, roll & 0x40 != 0);
            }
            if roll & 0x3F0 == 0 {
                pins.set_irq_disable((roll >> 10) as u8);
            }
            if roll & 0xFFF000 == 0 {
                pins.frame.request_non_maskable_interrupt();
            }
            if device {
                answer = Some((roll >> 56) as u8);
            }
            self.calls.push((
                kind,
                address,
                value,
                self.mclk,
                pins.kind(),
                pins.opcode(),
                pins.high_speed,
            ));
        }
        answer
    }
}

impl<const LEND: bool> Bus for Model<LEND> {
    fn read(&mut self, address: u32, pins: &mut Pins) -> u8 {
        let answer = self.call(0, address, 0, pins);
        answer.unwrap_or(self.memory[address as usize])
    }
    fn write(&mut self, address: u32, value: u8, pins: &mut Pins) {
        if self.call(1, address, value, pins).is_none() {
            self.memory[address as usize] = value;
        }
    }
    fn idle(&mut self, address: Option<u32>, pins: &mut Pins) {
        self.call(2, address.unwrap_or(0), 0, pins);
    }

    const QUIET: bool = LEND;

    fn quiet_window(&mut self, _pins: &Pins) -> QuietWindow {
        QuietWindow {
            budget: self.wake.saturating_sub(self.mclk),
            clocks: [LOW, HIGH],
        }
    }
    fn quiet_read(&mut self, address: u32) -> Option<u8> {
        (address >> 13 != DEVICE_BANK).then(|| self.memory[address as usize])
    }
    fn quiet_write(&mut self, address: u32, value: u8) -> bool {
        if address >> 13 == DEVICE_BANK {
            return false;
        }
        self.memory[address as usize] = value;
        true
    }
    fn quiet_spent(&mut self, clocks: i64, _pins: &Pins) {
        self.mclk += clocks;
    }
}

fn cpu<const LEND: bool>(seed: u64) -> HuC6280<Model<LEND>> {
    let mut cpu = HuC6280::new(Model::<LEND>::new(seed));
    cpu.request_reset();
    cpu
}

/// Run both for `steps` steps and compare after every one.
fn lockstep(seed: u64, steps: usize, setup: impl Fn(&mut CpuState)) {
    let mut plain = cpu::<false>(seed);
    let mut lent = cpu::<true>(seed);
    let mut state = plain.capture_state();
    setup(&mut state);
    plain.restore_state(&state).unwrap();
    lent.restore_state(&state).unwrap();
    let mut quiet_steps = 0;
    for step in 0..steps {
        if step % 5000 == 0 {
            // move the mapper about, as a program would, with the device page in it
            let roll = plain.bus.draw();
            let mut mpr = [0u8; 8];
            for (i, register) in mpr.iter_mut().enumerate() {
                *register = (roll >> (8 * i)) as u8 | 0x80;
            }
            mpr[1] = 0xF8;
            mpr[(roll % 7) as usize + 1] = DEVICE_BANK as u8;
            lent.bus.draw();
            plain.mpr = mpr;
            lent.mpr = mpr;
        }
        let calls = plain.bus.calls.len();
        let a = plain.step();
        let b = lent.step();
        quiet_steps += (plain.bus.calls.len() - calls < a as usize) as usize;
        assert_eq!(a, b, "step {step}: cycles");
        assert_eq!(plain.bus.mclk, lent.bus.mclk, "step {step}: master clock");
        assert_eq!(plain.capture_state(), lent.capture_state(), "step {step}");
        assert_eq!(plain.kind(), lent.kind(), "step {step}: kind");
        assert_eq!(
            plain.bus.calls.len(),
            lent.bus.calls.len(),
            "step {step}: calls"
        );
        assert_eq!(plain.bus.calls.last(), lent.bus.calls.last(), "step {step}");
    }
    assert_eq!(plain.bus.calls, lent.bus.calls);
    assert!(plain.bus.memory == lent.bus.memory, "memory");
    assert!(
        quiet_steps > steps / 2,
        "the window was used ({quiet_steps} of {steps} steps)"
    );
}

#[test]
fn a_lending_bus_sees_what_a_plain_one_sees() {
    for seed in 1..=4 {
        lockstep(seed, 200_000, |_| {});
    }
}

#[test]
fn a_lending_bus_sees_what_a_plain_one_sees_at_high_speed() {
    for seed in 11..=14 {
        lockstep(seed, 200_000, |state| state.high_speed = true);
    }
}
