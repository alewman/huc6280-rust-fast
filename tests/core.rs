// SPDX-License-Identifier: MIT
//! Directed tests of the crate's public surface that need no reference: the
//! bus contract (one callback a cycle, physical addresses, what the pins
//! show), the three interrupt lines and their disable register, the named
//! switches, and state capture and restore. The proof of equivalence with
//! huc6280-python is the ladder (`scripts/rung*.sh`), not these.

use huc6280::trace::{next_boundary, BoundaryKind};
use huc6280::{
    Bus, CpuState, DecimalFormula, HuC6280, IrqPollCycle, Kind, Pins, VanishedInterrupt, OPCODES,
    T, VDC_ADDRESS,
};

/// 2 MB of RAM that records every cycle with what the pins showed.
struct Ram {
    memory: Vec<u8>,
    cycles: Vec<Cycle>,
    /// A line change to make during the cycle with this index.
    during: Option<(usize, Lines)>,
}

/// The three maskable inputs: (IRQ1, IRQ2, TIQ).
type Lines = (bool, bool, bool);

/// One cycle as a callback saw it: the kind, the address, the clock switch.
type Cycle = (Kind, Option<u32>, bool);

impl Bus for Ram {
    fn read(&mut self, address: u32, pins: &mut Pins) -> u8 {
        self.cycle(Some(address), pins);
        self.memory[address as usize]
    }
    fn write(&mut self, address: u32, value: u8, pins: &mut Pins) {
        self.cycle(Some(address), pins);
        self.memory[address as usize] = value;
    }
    fn idle(&mut self, address: Option<u32>, pins: &mut Pins) {
        self.cycle(address, pins);
    }
}

impl Ram {
    fn cycle(&mut self, address: Option<u32>, pins: &mut Pins) {
        if let Some((at, (irq1, irq2, tiq))) = self.during {
            if at == self.cycles.len() {
                pins.set_irq_lines(irq1, irq2, tiq);
            }
        }
        self.cycles.push((pins.kind(), address, pins.high_speed));
    }
}

const CODE: u32 = 0x1F0000; // work RAM, page $F8

/// A CPU with page $F8 at logical $4000 (code) and $2000 (zero page, stack),
/// bank 0 at $E000 (vectors), running `code` from $4000.
fn cpu(code: &[u8]) -> HuC6280<Ram> {
    let mut memory = vec![0; 0x200000];
    memory[CODE as usize..CODE as usize + code.len()].copy_from_slice(code);
    let mut cpu = HuC6280::new(Ram {
        memory,
        cycles: Vec::new(),
        during: None,
    });
    cpu.mpr = [0xFF, 0xF8, 0xF8, 0, 0, 0, 0, 0];
    cpu.pc = 0x4000;
    cpu
}

fn vector(cpu: &mut HuC6280<Ram>, vector: u16, target: u16) {
    let at = cpu.physical(vector) as usize;
    cpu.bus.memory[at] = target as u8;
    cpu.bus.memory[at + 1] = (target >> 8) as u8;
}

#[test]
fn every_cycle_is_one_callback_with_its_kind() {
    // LDA $10: fetch, operand, internal, read of the zero page at physical $1F0010.
    let mut cpu = cpu(&[0xA5, 0x10]);
    cpu.bus.memory[0x1F0010] = 0x80;
    assert_eq!(cpu.step(), 4);
    assert_eq!(
        cpu.bus.cycles,
        [
            (Kind::Read, Some(CODE), false),
            (Kind::Read, Some(CODE + 1), false),
            (Kind::Idle, None, false),
            (Kind::Read, Some(0x1F0010), false),
        ]
    );
    assert_eq!((cpu.a, cpu.p & 0x82), (0x80, 0x80));
    assert_eq!(cpu.opcode(), 0xA5);
}

#[test]
fn the_table_has_every_opcode_and_its_base_cycles_are_what_runs() {
    // Every opcode but the block transfers and BRK, from the same state: T
    // and D clear, branches not taken where the flags say so.
    let mut measured = 0;
    for (opcode, row) in OPCODES.iter().enumerate() {
        let (_, _, mnemonic, form, cycles) = *row;
        let taken = matches!(mnemonic, "BPL" | "BVC" | "BCC" | "BNE");
        if form == huc6280::Form::Block || taken || mnemonic.starts_with("BBR") {
            continue;
        }
        let mut cpu = cpu(&[opcode as u8, 0x10, 0x30, 0x00]);
        assert_eq!(cpu.step(), cycles as u32, "opcode {opcode:02X} {mnemonic}");
        measured += 1;
    }
    assert_eq!(measured, 256 - 5 - 4 - 8);
}

#[test]
fn st0_writes_the_vdc_whatever_the_mapper_holds() {
    let mut cpu = cpu(&[0x03, 0x5A]);
    cpu.mpr = [0xF8; 8]; // no register holds $FF, the hardware page
    cpu.step();
    assert_eq!(cpu.bus.memory[VDC_ADDRESS as usize], 0x5A);
    assert_eq!(cpu.bus.cycles[3], (Kind::Write, Some(VDC_ADDRESS), false));
}

#[test]
fn a_block_transfer_is_one_step_of_17_plus_6n_cycles() {
    // TII $2200,$2240,3
    let mut cpu = cpu(&[0x73, 0x00, 0x22, 0x40, 0x22, 0x03, 0x00]);
    cpu.bus.memory[0x1F0200..0x1F0203].copy_from_slice(&[1, 2, 3]);
    assert_eq!(cpu.step(), 17 + 6 * 3);
    assert_eq!(cpu.bus.memory[0x1F0240..0x1F0243], [1, 2, 3]);
    assert_eq!(cpu.pc, 0x4007);
}

#[test]
fn csh_holds_for_its_own_last_cycle_and_reset_selects_low() {
    let mut cpu = cpu(&[0xD4]);
    cpu.step();
    assert!(cpu.high_speed());
    let speeds: Vec<bool> = cpu.bus.cycles.iter().map(|cycle| cycle.2).collect();
    assert_eq!(speeds, [false, false, true]); // C7
    cpu.request_reset();
    cpu.step();
    assert!(!cpu.high_speed());
    assert_eq!(cpu.mpr[7], 0);
}

#[test]
fn set_makes_adc_work_on_the_zero_page_and_the_fetch_clears_t() {
    // SET; ADC #$05 with X = $10: M($2010) += 5, A untouched, three more cycles.
    let mut cpu = cpu(&[0xF4, 0x69, 0x05]);
    cpu.x = 0x10;
    cpu.a = 0x77;
    cpu.bus.memory[0x1F0010] = 0x20;
    cpu.step();
    assert_eq!(cpu.p & T, T);
    assert_eq!(cpu.step(), 2 + 3);
    assert_eq!(
        (cpu.bus.memory[0x1F0010], cpu.a, cpu.p & T),
        (0x25, 0x77, 0)
    );
}

#[test]
fn the_decimal_formulas_differ_only_on_invalid_digits() {
    let p = 0x08; // D
    for (a, operand) in [(0x19, 0x28), (0x99, 0x01), (0x50, 0x50)] {
        assert_eq!(
            huc6280::add(a, operand, p, DecimalFormula::Mednafen),
            huc6280::add(a, operand, p, DecimalFormula::Mesen)
        );
    }
    assert_eq!(
        huc6280::add(0x19, 0x28, p, DecimalFormula::Mednafen).0,
        0x47
    );
    assert_ne!(
        huc6280::add(0x0F, 0x0F, p, DecimalFormula::Mednafen),
        huc6280::add(0x0F, 0x0F, p, DecimalFormula::Mesen)
    );
}

#[test]
fn the_three_lines_have_their_vectors_priority_and_disable_bits() {
    for (lines, disable, target) in [
        ((true, true, true), 0, 0x5000),   // TIQ first
        ((true, true, false), 0, 0x5100),  // then IRQ1
        ((false, true, false), 0, 0x5200), // then IRQ2
        ((true, true, true), 4, 0x5100),   // the timer masked
        ((true, true, true), 6, 0x5200),   // the timer and IRQ1 masked
    ] {
        let mut cpu = cpu(&[0xEA]);
        vector(&mut cpu, 0xFFFA, 0x5000);
        vector(&mut cpu, 0xFFF8, 0x5100);
        vector(&mut cpu, 0xFFF6, 0x5200);
        cpu.p = 0;
        cpu.set_irq_disable(disable);
        cpu.set_irq_lines(lines.0, lines.1, lines.2);
        assert_eq!(cpu.step(), 8); // C3
        assert_eq!(cpu.pc, target);
        assert_eq!(cpu.p & 0x0C, 0x04); // I set, D clear
        assert_eq!(cpu.irq_lines(), lines); // levels: taking one does not clear it
    }
    // All three masked: nothing is taken, the lines are remembered.
    let mut cpu = cpu(&[0xEA]);
    cpu.p = 0;
    cpu.set_irq_disable(7);
    cpu.set_irq_lines(true, true, true);
    assert_eq!(cpu.step(), 2);
    assert_eq!(cpu.pc, 0x4001);
}

#[test]
fn a_line_raised_in_the_last_cycle_waits_one_instruction() {
    // NOP NOP NOP; IRQ1 rises during the first NOP's last cycle (cycle 1).
    let mut cpu = cpu(&[0xEA, 0xEA, 0xEA]);
    vector(&mut cpu, 0xFFF8, 0x5100);
    cpu.p = 0;
    cpu.bus.during = Some((1, (true, false, false)));
    cpu.step();
    cpu.step(); // the second NOP still runs
    assert_eq!(cpu.pc, 0x4002);
    cpu.step();
    assert_eq!(cpu.pc, 0x5100);
}

#[test]
fn a_timer_interrupt_whose_source_has_gone_is_dropped() {
    // OBSERVED (Mesen2 2.1.1 and Mednafen 1.32.1 as closed boxes, the
    // reference's validation/vanish_probe.py): a timer acknowledged in the last
    // cycle is not entered. The withdrawn "latched" has no variant (the
    // reference refuses it with ValueError).
    for (mode, entered) in [
        (VanishedInterrupt::Mesen2, false),
        (VanishedInterrupt::Drop, false),
    ] {
        // CLI; NOP; NOP with the timer's line up: CLI's poll still sees I
        // set, the NOP's poll sees the line, and the line drops in the NOP's
        // last cycle (C18).
        let mut cpu = cpu(&[0x58, 0xEA, 0xEA]);
        vector(&mut cpu, 0xFFFA, 0x5000);
        cpu.vanished_interrupt = mode;
        cpu.set_irq_lines(false, false, true);
        cpu.bus.during = Some((3, (false, false, false)));
        cpu.step();
        cpu.step();
        assert_eq!(cpu.pc, 0x4002);
        cpu.step();
        assert_eq!(cpu.pc == 0x5000, entered, "{mode:?}");
    }
}

#[test]
fn a_source_still_asserted_is_taken_in_every_mode() {
    for (mode, poll) in [
        (VanishedInterrupt::Mesen2, IrqPollCycle::SecondToLast),
        (VanishedInterrupt::Drop, IrqPollCycle::SecondToLast),
        (VanishedInterrupt::Mesen2, IrqPollCycle::Last),
        (VanishedInterrupt::Drop, IrqPollCycle::Last),
    ] {
        let mut cpu = cpu(&[0xEA]);
        vector(&mut cpu, 0xFFFA, 0x6200);
        cpu.vanished_interrupt = mode;
        cpu.irq_poll_cycle = poll;
        cpu.p = 0;
        cpu.set_irq_lines(false, false, true);
        assert_eq!(cpu.step(), 8, "{mode:?} {poll:?}");
        assert_eq!(cpu.pc, 0x6200, "{mode:?} {poll:?}");
    }
}

#[test]
fn a_state_is_captured_and_restored_whole() {
    let mut cpu = cpu(&[0x58, 0xD4, 0xA9, 0x12, 0x53, 0x04]); // CLI; CSH; LDA #$12; TAM #$04
    cpu.set_irq_disable(5);
    cpu.set_irq_lines(false, true, true);
    for _ in 0..4 {
        cpu.step();
    }
    let state = cpu.capture_state();
    assert_eq!(
        (state.a, state.mpr[2], state.mpr_latch, state.high_speed),
        (0x12, 0x12, 0x12, true)
    );
    assert_eq!(
        (state.irq1, state.irq2, state.tiq, state.irq_disable),
        (false, true, true, 5)
    );
    assert!(!state.maskable_interrupt_pending); // IRQ2 and TIQ are both masked

    let mut other = HuC6280::new(Ram {
        memory: Vec::new(),
        cycles: Vec::new(),
        during: None,
    });
    other.restore_state(&state).unwrap();
    assert_eq!(other.capture_state(), state);

    let bad = CpuState {
        p: 0x10,
        ..CpuState::default()
    };
    assert!(other.restore_state(&bad).is_err()); // B is not stored
}

// -- issue 0005: the poll rule (U21, U25) at the edges this port found --------
// The rule: the poll sees the lines as they stood at the end of an
// instruction's second-to-last cycle; a change in the last cycle waits for the
// next instruction's poll. BRK, entries and reset do not poll. A capture and
// restore at any boundary changes nothing. These mirror the reference's
// tests/test_directed.py, "issue 0005", case for case.

const CLI: u8 = 0x58;
const NOP: u8 = 0xEA;
const BRK: u8 = 0x00;
const TIQ_ONLY: Lines = (false, false, true);
const IRQ1_ONLY: Lines = (true, false, false);
const NONE: Lines = (false, false, false);

/// 2 MB of RAM, a schedule of line changes made from inside the callback of
/// the given bus cycle (counted from 0 over the whole run), and an NMI
/// requested in the given read.
struct Scheduled {
    memory: Vec<u8>,
    count: usize,
    reads: usize,
    schedule: Vec<(usize, Lines)>,
    nmi_at_read: Option<usize>,
}

impl Scheduled {
    fn tick(&mut self, pins: &mut Pins) {
        let index = self.count;
        self.count += 1;
        if let Some(&(_, (irq1, irq2, tiq))) = self.schedule.iter().find(|(at, _)| *at == index) {
            pins.set_irq_lines(irq1, irq2, tiq);
        }
    }
}

impl Bus for Scheduled {
    fn read(&mut self, address: u32, pins: &mut Pins) -> u8 {
        self.reads += 1;
        if self.nmi_at_read == Some(self.reads) {
            pins.frame.request_non_maskable_interrupt();
        }
        self.tick(pins);
        self.memory[address as usize]
    }
    fn write(&mut self, address: u32, value: u8, pins: &mut Pins) {
        self.tick(pins);
        self.memory[address as usize] = value;
    }
    fn idle(&mut self, _address: Option<u32>, pins: &mut Pins) {
        self.tick(pins);
    }
}

/// `program` at logical $4000 with `start` asserted, the reference's test
/// host's mapping (MPR = $00 $F8 $01 $02 $03 $04 $05 $00). Handlers are NOPs:
/// TIQ -> $6200, IRQ1 -> $6100, IRQ2 and BRK -> $6000, NMI -> $6300.
fn scheduled(
    program: &[u8],
    start: Lines,
    schedule: &[(usize, Lines)],
    mode: Option<VanishedInterrupt>,
) -> HuC6280<Scheduled> {
    let mut memory = vec![0; 0x200000];
    memory[0x2000..0x2000 + program.len()].copy_from_slice(program); // logical $4000
    memory[0x4000..0x4400].fill(NOP); // logical $6000-$63FF (MPR3 = $02)
    for (vector, target) in [
        (0xFFF6u16, 0x6000u16),
        (0xFFF8, 0x6100),
        (0xFFFA, 0x6200),
        (0xFFFC, 0x6300),
    ] {
        let at = (vector - 0xE000) as usize;
        memory[at] = target as u8;
        memory[at + 1] = (target >> 8) as u8;
    }
    let mut cpu = HuC6280::new(Scheduled {
        memory,
        count: 0,
        reads: 0,
        schedule: schedule.to_vec(),
        nmi_at_read: None,
    });
    cpu.mpr = [0x00, 0xF8, 0x01, 0x02, 0x03, 0x04, 0x05, 0x00];
    cpu.pc = 0x4000;
    cpu.s = 0xFF;
    cpu.p = 0x04;
    if let Some(mode) = mode {
        cpu.vanished_interrupt = mode;
    }
    cpu.set_irq_lines(start.0, start.1, start.2);
    cpu
}

/// PC after each step; with `restore_at`, a capture and restore before that step.
fn trail(cpu: &mut HuC6280<Scheduled>, steps: usize, restore_at: Option<usize>) -> Vec<u16> {
    let mut trail = Vec::new();
    for index in 0..steps {
        if restore_at == Some(index) {
            let state = cpu.capture_state();
            cpu.restore_state(&state).unwrap();
        }
        cpu.step();
        trail.push(cpu.pc);
    }
    trail
}

/// CLI; NOP x4. TIQ drops in the first NOP's last cycle (cycle 3): the entry
/// after it finds nothing (C18, the default) and runs the second NOP; IRQ1
/// rises in that NOP's last cycle (5).
fn vanished_then_rising() -> HuC6280<Scheduled> {
    scheduled(
        &[CLI, NOP, NOP, NOP, NOP],
        TIQ_ONLY,
        &[(3, NONE), (5, IRQ1_ONLY)],
        None,
    )
}

/// CLI; NOP x3 with TIQ up; in the first NOP's last cycle TIQ drops and IRQ1 rises.
fn swapped() -> HuC6280<Scheduled> {
    scheduled(&[CLI, NOP, NOP, NOP], TIQ_ONLY, &[(3, IRQ1_ONLY)], None)
}

/// CLI; NOP; NOP with IRQ1 up, dropped in the first NOP's last cycle: taken (C18, default).
fn dropped() -> HuC6280<Scheduled> {
    scheduled(&[CLI, NOP, NOP], IRQ1_ONLY, &[(3, NONE)], None)
}

#[test]
fn issue_0005_a_line_rising_in_a_vanished_entrys_instruction_waits_for_the_next_poll() {
    // Item 1: as after any other instruction (U21, U25 default), one more instruction runs.
    let expected = vec![0x4001, 0x4002, 0x4003, 0x4004, 0x6100];
    assert_eq!(trail(&mut vanished_then_rising(), 5, None), expected);
    let mut plain = scheduled(&[CLI, NOP, NOP, NOP, NOP], NONE, &[(5, IRQ1_ONLY)], None);
    assert_eq!(trail(&mut plain, 5, None), expected);
}

#[test]
fn issue_0005_a_source_swapped_in_the_last_cycle_is_not_taken_at_that_boundary() {
    // Item 2: the poll saw TIQ only. By default TIQ has gone (dropped, C18)
    // and IRQ1, which rose after the poll, waits for the next instruction's poll.
    assert_eq!(
        trail(&mut swapped(), 4, None),
        vec![0x4001, 0x4002, 0x4003, 0x6100]
    );
    // "drop" takes nothing either: TIQ has gone and IRQ1 was not polled.
    let mut dropped = scheduled(
        &[CLI, NOP, NOP, NOP],
        TIQ_ONLY,
        &[(3, IRQ1_ONLY)],
        Some(VanishedInterrupt::Drop),
    );
    assert_eq!(
        trail(&mut dropped, 4, None),
        vec![0x4001, 0x4002, 0x4003, 0x6100]
    );
}

#[test]
fn c16_an_entry_between_set_and_its_target_clears_t_for_the_handler() {
    // OBSERVED, provisional: Mesen2 and Mednafen as closed boxes (the
    // reference's validation/vanish_probe.py --only t-entry) run the handler's
    // first ORA on A, and push P with T set (pcetech 2.3); srg320's VHDL under
    // GHDL dissents (validation/vanish_vhdl.py --t-entry).
    let mut cpu = scheduled(&[CLI, 0xF4, NOP, NOP], TIQ_ONLY, &[], None); // CLI; SET; NOP; NOP
    cpu.bus.memory[0x4200..0x4202].copy_from_slice(&[0x09, 0x0F]); // the TIQ handler: ORA #$0F
    cpu.a = 0;
    cpu.x = 0;
    assert_eq!(
        trail(&mut cpu, 4, None),
        vec![0x4001, 0x4002, 0x6200, 0x6202]
    ); // entered after SET
    assert_eq!((cpu.a, cpu.bus.memory[0x1F0000]), (0x0F, 0)); // ORA worked on A
    assert_ne!(cpu.bus.memory[0x1F01FD] & T, 0); // the pushed P keeps T
}

#[test]
fn the_lines_the_poll_saw_hold_until_the_next_boundary_only() {
    // The reference's clean rewrite (6ce348e): the boundary clears them, so a
    // capture right after an interrupt entry reports None.
    let mut cpu = dropped();
    cpu.step();
    cpu.step();
    assert_eq!(cpu.capture_state().polled_irq_lines, Some(0x02));
    cpu.step();
    assert_eq!(cpu.pc, 0x6100);
    assert_eq!(cpu.capture_state().polled_irq_lines, None);
}

#[test]
fn issue_0005_capture_and_restore_keep_the_lines_the_poll_saw() {
    // Item 3: the lines the poll saw are state (CpuState::polled_irq_lines).
    let mut cpu = dropped();
    cpu.step();
    cpu.step();
    let state = cpu.capture_state();
    assert_eq!(state.polled_irq_lines, Some(0x02));
    assert!(!state.irq1);
    assert_eq!(next_boundary(&state), BoundaryKind::Irq1);
    cpu.restore_state(&state).unwrap();
    cpu.step();
    assert_eq!(cpu.pc, 0x6100);

    let mut swapped = swapped();
    swapped.step();
    swapped.step();
    let state = swapped.capture_state();
    assert_eq!(state.polled_irq_lines, Some(0x04)); // TIQ, $1403 layout
                                                    // The timer the poll saw has gone and IRQ1 rose after it: an instruction runs.
    assert_eq!(next_boundary(&state), BoundaryKind::Instruction);
    swapped.step();
    assert_eq!(swapped.capture_state().polled_irq_lines, None); // until the next boundary only
}

#[test]
fn issue_0005_a_round_trip_at_any_boundary_changes_nothing() {
    let steps = 6;
    for scenario in [vanished_then_rising, swapped, dropped] {
        let expected = trail(&mut scenario(), steps, None);
        for boundary in 0..steps {
            assert_eq!(
                trail(&mut scenario(), steps, Some(boundary)),
                expected,
                "restored before step {boundary}"
            );
        }
    }
}

#[test]
fn issue_0005_a_state_from_before_the_field_loads_as_the_poll_agreeing() {
    assert_eq!(CpuState::default().polled_irq_lines, None);
    let bad = CpuState {
        polled_irq_lines: Some(8),
        ..CpuState::default()
    };
    let error = dropped().restore_state(&bad).unwrap_err();
    assert!(error.contains("polled_irq_lines"), "{error}");
    let good = CpuState {
        polled_irq_lines: Some(7),
        ..CpuState::default()
    };
    let mut cpu = dropped();
    cpu.restore_state(&good).unwrap();
    assert_eq!(cpu.capture_state(), good);
}

#[test]
fn issue_0005_brk_does_not_poll() {
    // Item 4: BRK makes an entry's pushes and vector reads (C3, C8) and, like
    // an entry and as on the 6502 (the frame's rule), does
    // not poll: an NMI latched during it waits for the handler's first
    // instruction. Only NMI shows it (BRK sets I); NMI is not connected on a
    // PC Engine.
    let mut cpu = scheduled(&[BRK, 0x00], NONE, &[], None);
    cpu.bus.nmi_at_read = Some(2); // the signature byte: well before BRK's last cycle
    cpu.step();
    assert_eq!(cpu.pc, 0x6000);
    assert!(cpu.capture_state().skipped_poll);
    assert_eq!(trail(&mut cpu, 2, None), vec![0x6001, 0x6300]);
}
