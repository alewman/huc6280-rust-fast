// SPDX-License-Identifier: MIT AND BSD-3-Clause
// bank_offsets is ported from MAME's src/devices/bus/pce/pce_slot.cpp rom_map_setup (BSD-3-Clause,
// copyright Fabio Priuli), through huc6280-python's harness.py: notice in NOTICE.md.
//! Rung 5: replay MAME 0.285's trace of a real game through the core, as the
//! reference's `tests/mame_replay.py` does, and print the same report.
//!
//!     replay TRACE_DIR [--limit N]
//!
//! `TRACE_DIR` holds `cart.pce` and `error.log` (one line per instruction
//! before it runs, `PC A X Y S P totalcycles`, then `M MPR0 ... MPR7`, then
//! the hardware-page reads that instruction made, `[luaengine] R address
//! value`). The core runs in the reference's minimal harness (`harness.py`,
//! the part a replay uses: a HuCard, work RAM, a stubbed hardware page,
//! master-clock accounting) from the same reset and is answered from the log,
//! so it sees what MAME's board gave MAME's CPU. MAME is a detector here, not
//! a judge: the rung passes when this binary prints what
//! `scripts/reference_replay.py` prints for the reference core, the report
//! and a digest of the core after every step.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::Path;

use huc6280::trace::{next_boundary, BoundaryKind};
use huc6280::{Bus, CpuState, HuC6280, Pins};

const HARDWARE_PAGE: u32 = 0x1FE000; // physical; offsets $0000-$1FFF within it
const VDC_VCE_MASK: u32 = 0x1FF800; // (address & mask) == $1FE000: VDC and VCE
const B_FLAG: u8 = 0x10;
const VECTORS: [(&str, u16); 4] = [
    ("irq2", 0xFFF6),
    ("irq1", 0xFFF8),
    ("tiq", 0xFFFA),
    ("nmi", 0xFFFC),
];
const BLOCK_OPCODES: [u8; 5] = [0x73, 0xC3, 0xD3, 0xE3, 0xF3];
const REGISTERS: [&str; 6] = ["pc", "a", "x", "y", "s", "p"];

/// One instruction of MAME's trace: the registers before it, the 7.16 MHz
/// cycle count before it, and the hardware-page reads it made.
struct Line {
    registers: [u32; 14], // pc a x y s p, then MPR0-7
    cycles: i64,
    reads: Vec<(u32, u8)>,
}

fn hex(field: &[u8]) -> Option<u32> {
    let text = std::str::from_utf8(field).ok()?;
    u32::from_str_radix(text, 16).ok()
}

/// MAME's trace lines with their reads, lazily, from an `error.log`.
struct Log<R: BufRead> {
    input: R,
    raw: Vec<u8>,
    registers: Option<[u32; 14]>,
    cycles: i64,
    reads: Vec<(u32, u8)>,
    head: Option<([u32; 6], i64)>,
    done: bool,
}

impl<R: BufRead> Log<R> {
    fn new(input: R) -> Self {
        Log {
            input,
            raw: Vec::new(),
            registers: None,
            cycles: 0,
            reads: Vec::new(),
            head: None,
            done: false,
        }
    }
}

impl<R: BufRead> Iterator for Log<R> {
    type Item = Line;

    fn next(&mut self) -> Option<Line> {
        if self.done {
            return None;
        }
        loop {
            self.raw.clear();
            if self
                .input
                .read_until(b'\n', &mut self.raw)
                .expect("read the log")
                == 0
            {
                self.done = true;
                let registers = self.registers.take()?;
                return Some(Line {
                    registers,
                    cycles: self.cycles,
                    reads: std::mem::take(&mut self.reads),
                });
            }
            let fields: Vec<&[u8]> = self
                .raw
                .split(|byte| byte.is_ascii_whitespace())
                .filter(|field| !field.is_empty())
                .collect();
            if fields.len() == 4 && fields[0] == b"[luaengine]" && fields[1] == b"R" {
                let address = hex(fields[2]).expect("a read's address");
                let value = hex(fields[3]).expect("a read's value");
                self.reads.push((address, value as u8));
            } else if fields.len() == 9 && fields[0] == b"M" && self.head.is_some() {
                let mut mpr = [0u32; 8];
                let mut valid = true;
                for (slot, field) in mpr.iter_mut().zip(&fields[1..]) {
                    match hex(field) {
                        Some(value) => *slot = value,
                        None => valid = false,
                    }
                }
                if !valid {
                    continue;
                }
                let (head, head_cycles) = self.head.take().unwrap();
                let mut registers = [0u32; 14];
                registers[..6].copy_from_slice(&head);
                registers[6..].copy_from_slice(&mpr);
                let finished = self.registers.replace(registers).map(|registers| Line {
                    registers,
                    cycles: self.cycles,
                    reads: std::mem::take(&mut self.reads),
                });
                self.cycles = head_cycles;
                self.reads.clear();
                if finished.is_some() {
                    return finished;
                }
            } else if fields.len() == 7 && !self.raw.starts_with(b"[") {
                let mut head = [0u32; 6];
                let mut valid = true;
                for (slot, field) in head.iter_mut().zip(&fields[..6]) {
                    match hex(field) {
                        Some(value) => *slot = value,
                        None => valid = false,
                    }
                }
                let cycles = std::str::from_utf8(fields[6])
                    .ok()
                    .and_then(|text| text.parse::<i64>().ok());
                self.head = match (valid, cycles) {
                    (true, Some(cycles)) => Some((head, cycles)),
                    _ => None,
                };
            }
        }
    }
}

/// Where in the ROM (in 8 KB banks) each sixteenth of the 128 ROM pages
/// starts (the reference's `harness.bank_offsets`, 6ce348e).
///
/// pcetech 3 says only that cards smaller than 1 MB "may be mirrored". The
/// shapes are MAME's (`bus/pce/pce_slot.cpp` `rom_map_setup`, BSD-3-Clause,
/// copyright Fabio Priuli; ported by the reference, see NOTICE.md; emulator
/// tier): in 128 KB blocks, a 384 KB card is 0 1 0 1 2 2 2 2 (two chips, the
/// conventional split), a 192 KB image 0 1 2 2 0 1 2 2, and anything else
/// maps its blocks in order and fills the rest by repeating the run before (so
/// 768 KB is 0-5 4 5 and a power of two repeats whole). Cards under 128 KB
/// repeat from the start (the harness's modulo; a choice).
fn bank_offsets(banks: usize) -> [usize; 8] {
    let blocks: Vec<usize> = if banks == 0x30 {
        vec![0, 1, 0, 1, 2, 2, 2, 2]
    } else if banks == 0x18 {
        vec![0, 1, 2, 2, 0, 1, 2, 2]
    } else if banks < 0x10 {
        (0..8).collect()
    } else {
        let mut blocks: Vec<usize> = (0..(banks / 0x10).min(8)).collect();
        let mut filled = blocks.len();
        while filled % 8 != 0 {
            let mut shift = 0;
            while filled % (8 >> shift) != 0 && shift < 3 {
                shift += 1;
            }
            let repeat = filled % (8 >> (shift - 1));
            blocks.extend_from_within(filled - repeat..filled);
            filled += repeat;
        }
        blocks
    };
    let mut offsets = [0; 8];
    for (offset, block) in offsets.iter_mut().zip(blocks) {
        *offset = block * 0x10;
    }
    offsets
}

/// The reference's minimal harness with MAME's board behind its
/// hardware-page reads (`Harness` and `Replay`).
struct Board {
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_banks: Vec<usize>,
    open_bus: u8,
    master_clocks: u64,
    reads: VecDeque<(u32, u8)>,
    /// A hardware-page read nothing logged, this step.
    guessed: Option<u32>,
}

impl Board {
    fn new(rom: Vec<u8>) -> Self {
        let banks = rom.len() / 0x2000;
        let offsets = bank_offsets(banks);
        let rom_banks = if banks != 0 {
            (0..0x80)
                .map(|page| (offsets[page >> 4] + (page & 0x0F)) % banks)
                .collect()
        } else {
            Vec::new()
        };
        Board {
            rom,
            ram: vec![0; 0x2000],
            rom_banks,
            open_bus: 0xFF,
            master_clocks: 0,
            reads: VecDeque::new(),
            guessed: None,
        }
    }

    fn tick(&mut self, address: Option<u32>, pins: &Pins) {
        // 12 master clocks a CPU cycle at 1.79 MHz, 3 at 7.16 MHz.
        let mut per_cycle = if pins.high_speed { 3 } else { 12 };
        // The VDC/VCE stretch: one extra cycle on any access there (U18), the board's job.
        if matches!(address, Some(address) if address & VDC_VCE_MASK == HARDWARE_PAGE) {
            per_cycle *= 2;
        }
        self.master_clocks += per_cycle;
    }

    /// The byte at a physical address as the bus would give it;
    /// `side_effects = false` (a debugger's view) answers the hardware page
    /// with $FF.
    fn peek(&mut self, address: u32, side_effects: bool) -> u8 {
        let page = address >> 13;
        if page < 0x80 {
            if self.rom_banks.is_empty() {
                return 0xFF;
            }
            let bank = self.rom_banks[page as usize];
            return self.rom[(bank << 13) | (address & 0x1FFF) as usize];
        }
        if (0xF8..=0xFB).contains(&page) {
            return self.ram[address as usize & 0x1FFF];
        }
        if (page == 0xF7 || page == 0xFF) && side_effects {
            return self.answer(address);
        }
        0xFF
    }

    fn answer(&mut self, address: u32) -> u8 {
        let offset = address & 0x1FFF;
        if matches!(self.reads.front(), Some(&(first, _)) if first == address) {
            return self.reads.pop_front().unwrap().1;
        }
        if let Some(position) = self.reads.iter().position(|&(logged, _)| logged == address) {
            // MAME logged other reads first (a block transfer's, say): skip them.
            self.reads.drain(..position);
            return self.reads.pop_front().unwrap().1;
        }
        self.guessed = Some(address);
        if address >= HARDWARE_PAGE {
            self.default_io_read(offset)
        } else {
            0xFF
        }
    }

    fn default_io_read(&self, offset: u32) -> u8 {
        if (0x1000..0x1400).contains(&offset) {
            return 0xBF; // port K: no button pressed, bit 6 set
        }
        if offset < 0x0800 {
            return 0; // VDC, VCE: nothing there
        }
        self.open_bus // PSG, timer, interrupt registers: the I/O buffer (U13), roughly
    }
}

impl Bus for Board {
    fn read(&mut self, address: u32, pins: &mut Pins) -> u8 {
        self.tick(Some(address), pins);
        let value = self.peek(address, true);
        self.open_bus = value;
        value
    }

    fn write(&mut self, address: u32, value: u8, pins: &mut Pins) {
        self.tick(Some(address), pins);
        self.open_bus = value;
        if (0xF8..=0xFB).contains(&(address >> 13)) {
            self.ram[address as usize & 0x1FFF] = value;
        }
    }

    fn idle(&mut self, address: Option<u32>, pins: &mut Pins) {
        self.tick(address, pins);
    }
}

/// A counter that remembers the order its keys arrived in, as Python's does.
struct Counter<K: PartialEq + Clone>(Vec<(K, u64)>);

impl<K: PartialEq + Clone> Counter<K> {
    fn new() -> Self {
        Counter(Vec::new())
    }

    fn add(&mut self, key: K, count: u64) {
        match self.0.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 += count,
            None => self.0.push((key, count)),
        }
    }

    /// The `n` most common, ties in arrival order (`Counter.most_common`).
    fn most_common(&self, n: usize) -> Vec<(K, u64)> {
        let mut items = self.0.clone();
        items.sort_by_key(|item| std::cmp::Reverse(item.1)); // stable: ties keep arrival order
        items.truncate(n);
        items
    }
}

struct Divergence {
    index: u64,
    address: u32,
    opcode: u8,
    differences: Vec<(String, u32, u32)>, // register: (core, MAME)
}

struct Report {
    instructions: u64,
    interrupts: Counter<&'static str>,
    forced: Counter<&'static str>,
    forced_after: Counter<(&'static str, u8)>,
    adopted: u64,
    block_registers: u64,
    adopted_addresses: Counter<u32>,
    unread: u64,
    unread_addresses: Counter<u32>,
    divergences: Vec<Divergence>,
    cycles_equal: u64,
    cycles_skipped: u64,
    cycle_differences: Counter<(u8, i64)>,
    entry_differences: Counter<i64>,
}

/// An integer with thousands separators (Python's `{:,}`).
fn commas(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index != 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

fn or_else(text: String, fallback: &str) -> String {
    if text.is_empty() {
        fallback.to_string()
    } else {
        text
    }
}

fn named(counter: &Counter<&'static str>) -> String {
    let mut items = counter.0.clone();
    items.sort();
    let parts: Vec<String> = items
        .iter()
        .map(|(k, v)| format!("{k} {}", commas(*v)))
        .collect();
    or_else(parts.join(", "), "none")
}

impl Report {
    fn summary(&self) -> String {
        let unexplained: u64 = self.cycle_differences.0.iter().map(|(_, n)| n).sum();
        let compared = self.cycles_equal + unexplained;
        let mut entries = self.entry_differences.0.clone();
        entries.sort();
        let entries: Vec<String> = entries
            .iter()
            .map(|(d, n)| format!("{d:+} x{}", commas(*n)))
            .collect();
        let mut lines = vec![
            format!(
                "instructions {}; interrupts taken {}; taken a boundary earlier than the core's poll would: {}",
                commas(self.instructions),
                named(&self.interrupts),
                named(&self.forced)
            ),
            format!(
                "register divergences {}; block transfers' A, X, Y adopted {}; adopted MAME's registers after {} unlogged reads; {} logged reads not made",
                commas(self.divergences.len() as u64),
                commas(self.block_registers),
                commas(self.adopted),
                commas(self.unread)
            ),
            format!(
                "cycles equal {} of {}, differ {} ({} steps with an interrupt entry compared apart: {})",
                commas(self.cycles_equal),
                commas(compared),
                commas(unexplained),
                commas(self.cycles_skipped),
                or_else(entries.join(", "), "-")
            ),
        ];
        let mut by_opcode: Counter<u8> = Counter::new();
        for ((opcode, _), count) in &self.cycle_differences.0 {
            by_opcode.add(*opcode, *count);
        }
        for (opcode, count) in by_opcode.most_common(25) {
            let mut deltas: Vec<(i64, u64)> = self
                .cycle_differences
                .0
                .iter()
                .filter(|((o, _), _)| *o == opcode)
                .map(|((_, d), n)| (*d, *n))
                .collect();
            deltas.sort();
            let text: Vec<String> = deltas
                .iter()
                .map(|(d, n)| format!("{d:+} x{}", commas(*n)))
                .collect();
            lines.push(format!(
                "  opcode {opcode:02X}: {} differ (MAME - core: {})",
                commas(count),
                text.join(", ")
            ));
        }
        for (label, counter) in [
            ("logged reads not made", &self.unread_addresses),
            ("unlogged reads", &self.adopted_addresses),
        ] {
            if !counter.0.is_empty() {
                let common: Vec<String> = counter
                    .most_common(8)
                    .iter()
                    .map(|(a, n)| format!("{a:06X} x{}", commas(*n)))
                    .collect();
                lines.push(format!("  {label}: {}", common.join(", ")));
            }
        }
        if !self.forced_after.0.is_empty() {
            let common: Vec<String> = self
                .forced_after
                .most_common(8)
                .iter()
                .map(|((n, o), c)| format!("{n} after {o:02X} x{}", commas(*c)))
                .collect();
            lines.push(format!("  taken early: {}", common.join(", ")));
        }
        for divergence in self.divergences.iter().take(20) {
            let diffs: Vec<String> = divergence
                .differences
                .iter()
                .map(|(name, core, mame)| format!("{name} core {core:X} MAME {mame:X}"))
                .collect();
            lines.push(format!(
                "  line {} at {:04X} (opcode {:02X}): {}",
                commas(divergence.index),
                divergence.address,
                divergence.opcode,
                diffs.join(", ")
            ));
        }
        lines.join("\n")
    }
}

/// The core and the digest of it after every step the replay makes.
struct Replay {
    cpu: HuC6280<Board>,
    digest: u64,
    steps: u64,
}

impl Replay {
    fn step(&mut self) -> u32 {
        let cycles = self.cpu.step();
        let cpu = &self.cpu;
        let mut values = [0u64; 17];
        values[..7].copy_from_slice(&[
            cycles as u64,
            cpu.pc as u64,
            cpu.a as u64,
            cpu.x as u64,
            cpu.y as u64,
            cpu.s as u64,
            cpu.p as u64,
        ]);
        for (slot, register) in values[7..15].iter_mut().zip(cpu.mpr) {
            *slot = register as u64;
        }
        values[15] = cpu.high_speed() as u64;
        values[16] = cpu.bus.master_clocks & 0xFFFFFFFF;
        for value in values {
            self.digest = (self.digest ^ value).wrapping_mul(0x100000001B3);
        }
        self.steps += 1;
        cycles
    }

    fn registers(&self) -> [u32; 14] {
        let cpu = &self.cpu;
        let mut registers = [0u32; 14];
        registers[..6].copy_from_slice(&[
            cpu.pc as u32,
            cpu.a as u32,
            cpu.x as u32,
            cpu.y as u32,
            cpu.s as u32,
            (cpu.p & !B_FLAG) as u32,
        ]);
        for (slot, register) in registers[6..].iter_mut().zip(cpu.mpr) {
            *slot = register as u32;
        }
        registers
    }

    fn peek_logical(&mut self, logical: u16) -> u8 {
        let physical = self.cpu.physical(logical);
        self.cpu.bus.peek(physical, false)
    }

    /// If MAME's next line is an interrupt handler's first instruction, step the entry.
    fn take_interrupt(
        &mut self,
        line: &Line,
        report: &mut Report,
        previous: u8,
    ) -> Option<&'static str> {
        if line.registers[4] != self.cpu.s.wrapping_sub(3) as u32 {
            return None;
        }
        let mut found = None;
        for (name, vector) in VECTORS {
            let target = self.peek_logical(vector) as u32
                | ((self.peek_logical(vector.wrapping_add(1)) as u32) << 8);
            if target == line.registers[0] {
                found = Some(name);
                break;
            }
        }
        let name = found?;
        if self.cpu.pc as u32 == line.registers[0] {
            // Already at the handler: no entry to make, and no line left
            // asserted (issue 0005).
            return None;
        }
        if name == "nmi" {
            self.cpu.request_non_maskable_interrupt();
        } else {
            self.cpu
                .set_irq_lines(name == "irq1", name == "irq2", name == "tiq");
        }
        let state = self.cpu.capture_state();
        if next_boundary(&state) == BoundaryKind::Instruction {
            // The core's poll would wait an instruction (CLI, SEI or PLP
            // before it, or an entry): MAME took the interrupt now, so make
            // the poll see the current I and lines and count it.
            self.cpu.pins.frame.polled_i = None;
            self.cpu.pins.frame.polled = None;
            // The reference also clears _polled_lines; Pins keeps it private,
            // so it goes through a state with the poll agreeing (the frame's
            // fields restored as they now stand, polled_irq_lines None).
            let cleared = CpuState {
                polled_irq_lines: None,
                ..self.cpu.capture_state()
            };
            if state.polled_irq_lines.is_some() {
                self.cpu.restore_state(&cleared).expect("a captured state");
            }
            report.forced.add(name, 1);
            report.forced_after.add((name, previous), 1);
        }
        self.step();
        if name != "nmi" {
            self.cpu.set_irq_lines(false, false, false);
        }
        Some(name)
    }
}

fn replay(
    rom: Vec<u8>,
    log: &Path,
    limit: Option<u64>,
    max_divergences: usize,
) -> (Report, Replay) {
    let mut report = Report {
        instructions: 0,
        interrupts: Counter::new(),
        forced: Counter::new(),
        forced_after: Counter::new(),
        adopted: 0,
        block_registers: 0,
        adopted_addresses: Counter::new(),
        unread: 0,
        unread_addresses: Counter::new(),
        divergences: Vec::new(),
        cycles_equal: 0,
        cycles_skipped: 0,
        cycle_differences: Counter::new(),
        entry_differences: Counter::new(),
    };
    let mut cpu = HuC6280::new(Board::new(rom));
    cpu.request_reset();
    let mut harness = Replay {
        cpu,
        digest: 0xCBF29CE484222325,
        steps: 0,
    };
    harness.step(); // the reset sequence
    let file = std::fs::File::open(log).unwrap_or_else(|e| panic!("{}: {e}", log.display()));
    let mut iterator = Log::new(BufReader::with_capacity(1 << 20, file));
    let mut line = iterator.next();
    let mut index = 0u64;
    // The previous instruction's cycles are settled once we know whether an
    // interrupt entry followed it (MAME's delta to the next line holds the
    // entry): (opcode, MAME's cycles, the core's).
    let mut pending: Option<(u8, i64, i64)> = None;
    while let Some(current) = line.take() {
        if matches!(limit, Some(limit) if index >= limit) {
            break;
        }
        let following = iterator.next();
        let mut entered = None;
        let mut entry_clocks = 0;
        let mut want = current.registers;
        want[5] &= !(B_FLAG as u32);
        if harness.registers()[..6] != want[..6] {
            let mark = harness.cpu.bus.master_clocks;
            entered = harness.take_interrupt(&current, &mut report, pending.map_or(0, |p| p.0));
            entry_clocks = harness.cpu.bus.master_clocks - mark;
            if let Some(name) = entered {
                report.interrupts.add(name, 1);
            }
        }
        if let Some((last_opcode, mame, core)) = pending.take() {
            if entered.is_some() {
                report.cycles_skipped += 1;
                report
                    .entry_differences
                    .add(mame - core - (entry_clocks / 3) as i64, 1);
            } else if mame == core {
                report.cycles_equal += 1;
            } else {
                report.cycle_differences.add((last_opcode, mame - core), 1);
            }
        }
        let mine = harness.registers();
        if mine != want {
            let names = REGISTERS
                .iter()
                .map(|name| name.to_string())
                .chain((0..8).map(|n| format!("mpr{n}")));
            let differences = names
                .zip(mine.iter().zip(want.iter()))
                .filter(|(_, (a, b))| a != b)
                .map(|(name, (a, b))| (name, *a, *b))
                .collect();
            let opcode = harness.peek_logical(want[0] as u16);
            report.divergences.push(Divergence {
                index,
                address: want[0],
                opcode,
                differences,
            });
            if report.divergences.len() >= max_divergences {
                break;
            }
            // Take MAME's registers so one divergence does not spread.
            let cpu = &mut harness.cpu;
            cpu.pc = want[0] as u16;
            cpu.a = want[1] as u8;
            cpu.x = want[2] as u8;
            cpu.y = want[3] as u8;
            cpu.s = want[4] as u8;
            cpu.p = want[5] as u8;
            for (register, value) in cpu.mpr.iter_mut().zip(&want[6..]) {
                *register = *value as u8;
            }
        }
        harness.cpu.bus.reads = current.reads.iter().copied().collect();
        harness.cpu.bus.guessed = None;
        let opcode = harness.peek_logical(harness.cpu.pc);
        let start = harness.cpu.bus.master_clocks;
        harness.step();
        report.instructions += 1;
        report.unread += harness.cpu.bus.reads.len() as u64;
        for &(address, _) in &harness.cpu.bus.reads {
            report.unread_addresses.add(address, 1);
        }
        if let Some(next) = &following {
            let cpu = &mut harness.cpu;
            if BLOCK_OPCODES.contains(&opcode) {
                // C14: MAME's block transfers keep A, X and Y in the CPU and
                // never push them; the core pushes
                // and pops them (HW HB-6), so a transfer over the stack gives
                // them back overwritten. MAME's are adopted.
                report.block_registers += 1;
                cpu.a = next.registers[1] as u8;
                cpu.x = next.registers[2] as u8;
                cpu.y = next.registers[3] as u8;
            }
            if let Some(guessed) = cpu.bus.guessed {
                report.adopted += 1;
                report.adopted_addresses.add(guessed, 1);
                cpu.a = next.registers[1] as u8;
                cpu.x = next.registers[2] as u8;
                cpu.y = next.registers[3] as u8;
                cpu.s = next.registers[4] as u8;
                cpu.pc = next.registers[0] as u16;
                cpu.p = next.registers[5] as u8 & !B_FLAG;
            }
            pending = Some((
                opcode,
                next.cycles - current.cycles,
                ((cpu.bus.master_clocks - start) / 3) as i64,
            ));
        }
        line = following;
        index += 1;
    }
    (report, harness)
}

fn main() {
    let mut directory: Option<String> = None;
    let mut limit: Option<u64> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--limit" {
            limit = Some(args.next().expect("--limit N").parse().expect("N"));
        } else {
            directory = Some(arg);
        }
    }
    let directory = directory.expect("usage: replay TRACE_DIR [--limit N]");
    let directory = Path::new(&directory);
    let rom = std::fs::read(directory.join("cart.pce")).expect("cart.pce");
    let began = std::time::Instant::now();
    let (report, harness) = replay(rom, &directory.join("error.log"), limit, 50);
    println!("{}", report.summary());
    println!(
        "digest {:016x} over {} steps",
        harness.digest,
        commas(harness.steps)
    );
    eprintln!("{:.3} s", began.elapsed().as_secs_f64());
}

#[cfg(test)]
mod tests {
    use super::bank_offsets;

    #[test]
    fn rom_mirroring_is_mames_cartridge_map() {
        // The reference's test_harness_rom_mirroring_is_mames_cartridge_map.
        let blocks = |banks| bank_offsets(banks).map(|offset| offset / 0x10);
        assert_eq!(blocks(0x30), [0, 1, 0, 1, 2, 2, 2, 2]); // 384 KB: two chips
        assert_eq!(blocks(0x18), [0, 1, 2, 2, 0, 1, 2, 2]); // 192 KB
        assert_eq!(blocks(0x40), [0, 1, 2, 3, 0, 1, 2, 3]); // 512 KB repeats whole
        assert_eq!(blocks(0x60), [0, 1, 2, 3, 4, 5, 4, 5]); // 768 KB
        assert_eq!(blocks(0x50), [0, 1, 2, 3, 4, 4, 4, 4]); // 640 KB
        assert_eq!(blocks(0x80), [0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(blocks(0x10), [0; 8]);
        assert_eq!(blocks(0x20), [0, 1, 0, 1, 0, 1, 0, 1]);
        assert_eq!(blocks(0x04), [0, 1, 2, 3, 4, 5, 6, 7]); // under 128 KB: the modulo repeats it
    }
}
