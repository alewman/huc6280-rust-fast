// SPDX-License-Identifier: MIT
//! Rung 2: the SingleStepTests huc6280 corpus (Mesen-generated; emulator
//! tier), every case run and judged as the reference's `tests/single_step.py`
//! judges it, mask for mask.
//!
//!     sst CORPUS_DIR [--limit N] [--formula mesen|mednafen] [--jobs N]
//!         [--verdicts DIR] [--opcode XX]
//!
//! `CORPUS_DIR` holds `XX.json.bin` (the corpus's `v1/`). The binary layout
//! is the one the reference's `scripts/sst_reader.py` reads: little-endian,
//! `u32 count`, then per test `i32 size, 50-byte name, u32 opcode, initial
//! state, final state, u32 num_cycles, u32 recorded, recorded x u32`; a state
//! is `i32 size, A X Y S P, u16 PC, 8 MPR bytes, i32 n, n x (u32 address, u8
//! value)`; a cycle word is `RD | WR << 1 | dummy << 2 | address << 3 (21
//! bits) | data << 24`.
//!
//! The readings, each counted as a mask (the reference's module docstring
//! gives the reasons): `st_write` (C1: the corpus records ST0-ST2's write
//! cycle as an internal one), `truncated` (a block transfer of which the
//! corpus kept 500 cycles and 500 RAM pairs), `block_unknown` (a transfer
//! that read an address the case does not list: A, X, Y are not compared and
//! what a tainted byte feeds is not), `block_self_overlap` (a transfer over
//! its own bytes or stack cells), `block_final_unexplained`, `tma_latch`
//! (TMA #$00 returns a register the corpus does not initialise), `tsb_z`
//! (C12) and `decimal_formula` (C13: a decimal ADC or SBC that differs under
//! the core's default formula and passes under Mesen's).
//!
//! `--verdicts DIR` writes one line per case, `index verdict masks`, into
//! `DIR/XX.txt`: what `scripts/reference_sst.py` writes for the reference,
//! so the two cores are compared case for case (`scripts/compare_sst.py`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use huc6280::{Bus, DecimalFormula, HuC6280, Pins};

const ST_OPCODES: [u32; 3] = [0x03, 0x13, 0x23];
const TSB_TRB: [u32; 4] = [0x04, 0x0C, 0x14, 0x1C];
const BLOCK_OPCODES: [u32; 5] = [0x73, 0xC3, 0xD3, 0xE3, 0xF3];
const TMA: u32 = 0x43;
const DECIMAL_ARITHMETIC: [u32; 18] = [
    0x61, 0x65, 0x69, 0x6D, 0x71, 0x72, 0x75, 0x79, 0x7D, 0xE1, 0xE5, 0xE9, 0xED, 0xF1, 0xF2, 0xF5,
    0xF9, 0xFD,
];

struct State {
    a: u8,
    x: u8,
    y: u8,
    s: u8,
    p: u8,
    pc: u16,
    mpr: [u8; 8],
    ram: Vec<(u32, u8)>,
}

struct Case {
    name: String,
    opcode: u32,
    initial: State,
    end: State,
    num_cycles: u32,
    /// (address, data, RD | WR << 1 | dummy << 2)
    cycles: Vec<(u32, u8, u8)>,
}

fn u32_at(buf: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(buf[at..at + 4].try_into().unwrap())
}

fn state(buf: &[u8], mut ptr: usize) -> (usize, State) {
    let size = u32_at(buf, ptr) as usize;
    ptr += 4;
    let (a, x, y, s, p) = (
        buf[ptr],
        buf[ptr + 1],
        buf[ptr + 2],
        buf[ptr + 3],
        buf[ptr + 4],
    );
    let pc = u16::from_le_bytes([buf[ptr + 5], buf[ptr + 6]]);
    ptr += 7;
    let mpr: [u8; 8] = buf[ptr..ptr + 8].try_into().unwrap();
    ptr += 8;
    let count = u32_at(buf, ptr) as usize;
    ptr += 4;
    let mut ram = Vec::with_capacity(count);
    for _ in 0..count {
        ram.push((u32_at(buf, ptr), buf[ptr + 4]));
        ptr += 5;
    }
    (
        size,
        State {
            a,
            x,
            y,
            s,
            p,
            pc,
            mpr,
            ram,
        },
    )
}

/// Every test in one `XX.json.bin` file.
fn load_cases(path: &Path, limit: Option<usize>) -> Vec<Case> {
    let buf = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let count = u32_at(&buf, 0) as usize;
    let mut ptr = 4;
    let mut cases = Vec::new();
    for _ in 0..count.min(limit.unwrap_or(usize::MAX)) {
        let size = u32_at(&buf, ptr) as usize;
        let mut at = ptr + 4;
        let name = String::from_utf8_lossy(&buf[at..at + 50])
            .trim_matches(|c| c == '\0' || c == ' ')
            .to_string();
        at += 50;
        let opcode = u32_at(&buf, at);
        at += 4;
        let (used, initial) = state(&buf, at);
        at += used;
        let (used, end) = state(&buf, at);
        at += used;
        let num_cycles = u32_at(&buf, at);
        let recorded = u32_at(&buf, at + 4) as usize;
        at += 8;
        let mut cycles = Vec::with_capacity(recorded);
        for n in 0..recorded {
            let word = u32_at(&buf, at + 4 * n);
            let pins = (word & 7) as u8;
            let transfer = pins & 3 != 0;
            cycles.push((
                if transfer { (word >> 3) & 0x1FFFFF } else { 0 },
                if transfer { (word >> 24) as u8 } else { 0 },
                pins,
            ));
        }
        cases.push(Case {
            name,
            opcode,
            initial,
            end,
            num_cycles,
            cycles,
        });
        ptr += size;
    }
    cases
}

const SIZE: usize = 0x200000;

/// The case's sparse RAM over the flat 2 MB, and the bookkeeping the
/// reference keeps in dicts and sets.
#[derive(Default)]
struct Ram {
    memory: Vec<u8>,
    /// The address is a key of the reference's `memory` dict.
    listed: Vec<bool>,
    /// Block transfers read far more than the 500 RAM pairs the generator
    /// kept: a read of an address the case does not list returns 0 and
    /// taints the byte written next, and anything that depends on a tainted
    /// byte is not compared.
    tainted: Vec<bool>,
    touched: Vec<u32>,
    seen: Vec<(u8, u32, u8)>,
    unlisted: Option<u32>,
    pending_taint: bool,
    own_cells: Vec<u32>,
    overlapped: bool,
}

impl Ram {
    fn new() -> Self {
        Ram {
            memory: vec![0; SIZE],
            listed: vec![false; SIZE],
            tainted: vec![false; SIZE],
            touched: Vec::new(),
            seen: Vec::new(),
            unlisted: None,
            pending_taint: false,
            own_cells: Vec::new(),
            overlapped: false,
        }
    }

    fn clear(&mut self) {
        for &address in &self.touched {
            let address = address as usize;
            self.memory[address] = 0;
            self.listed[address] = false;
            self.tainted[address] = false;
        }
        self.touched.clear();
        self.seen.clear();
        self.unlisted = None;
        self.pending_taint = false;
        self.own_cells.clear();
        self.overlapped = false;
    }

    fn set(&mut self, address: u32, value: u8) {
        self.memory[address as usize] = value;
        self.listed[address as usize] = true;
        self.touched.push(address);
    }
}

impl Bus for Ram {
    fn read(&mut self, address: u32, _: &mut Pins) -> u8 {
        let index = address as usize;
        if self.tainted[index] || !self.listed[index] {
            self.unlisted.get_or_insert(address);
            self.pending_taint = true;
        }
        let value = self.memory[index];
        self.seen.push((b'r', address, value));
        value
    }

    fn write(&mut self, address: u32, value: u8, _: &mut Pins) {
        self.set(address, value);
        // The transfer's own writes start after its three pushes (seen[3..6]).
        let own = self.own_cells.contains(&address) && self.seen.len() >= 6;
        if self.pending_taint || own {
            self.tainted[address as usize] = true;
            self.pending_taint = false;
            if own {
                self.overlapped = true;
            }
        } else {
            self.tainted[address as usize] = false;
        }
        self.seen.push((b'w', address, value));
    }

    fn idle(&mut self, address: Option<u32>, _: &mut Pins) {
        self.seen.push(match address {
            None => (b'i', 0, 0),
            Some(address) => (b'd', address, 0),
        });
    }
}

type Masks = BTreeMap<&'static str, u32>;

/// A difference, and whether it is one of the final-state ones (A, X, Y or a
/// RAM cell) that `block_final_unexplained` covers.
struct Problem {
    final_state: bool,
    text: String,
}

fn physical(mpr: &[u8; 8], logical: u16) -> u32 {
    ((mpr[(logical >> 13) as usize] as u32) << 13) | (logical & 0x1FFF) as u32
}

/// TMA #$00 returns the hidden latch: the operand byte is the case's second code byte.
fn tma_latch(case: &Case) -> bool {
    let initial = &case.initial;
    let operand = physical(&initial.mpr, initial.pc.wrapping_add(1));
    // The reference builds a dict from the pairs: the last pair for an address wins.
    initial
        .ram
        .iter()
        .rev()
        .find(|(address, _)| *address == operand)
        .map_or(0, |(_, value)| *value)
        == 0
}

/// The physical cells of a block transfer's own opcode and operand bytes and
/// the three stack bytes it pushes.
fn own_cells(initial: &State) -> Vec<u32> {
    let mut cells: Vec<u32> = (0..7)
        .map(|offset| physical(&initial.mpr, initial.pc.wrapping_add(offset)))
        .collect();
    cells.extend(
        (0..3).map(|offset| physical(&initial.mpr, 0x2100 | initial.s.wrapping_sub(offset) as u16)),
    );
    cells
}

fn run_once(
    case: &Case,
    cpu: &mut HuC6280<Ram>,
    formula: Option<DecimalFormula>,
) -> (Vec<Problem>, Masks) {
    let opcode = case.opcode;
    let initial = &case.initial;
    let block = BLOCK_OPCODES.contains(&opcode);
    cpu.bus.clear();
    for &(address, value) in &initial.ram {
        cpu.bus.set(address, value);
    }
    if block {
        cpu.bus.own_cells = own_cells(initial);
    }
    // A new CPU for every case, as the reference makes one; the buffers are kept.
    let bus = std::mem::take(&mut cpu.bus);
    *cpu = HuC6280::new(bus);
    cpu.block_reads_hardware_page_as_zero = false; // no hardware page in the corpus
    if let Some(formula) = formula {
        cpu.decimal_formula = formula;
    }
    cpu.a = initial.a;
    cpu.x = initial.x;
    cpu.y = initial.y;
    cpu.s = initial.s;
    cpu.p = initial.p;
    cpu.pc = initial.pc;
    cpu.mpr = initial.mpr;
    cpu.step();

    let mut masks = Masks::new();
    let mut problems: Vec<Problem> = Vec::new();
    let mut problem =
        |final_state: bool, text: String| problems.push(Problem { final_state, text });
    let end = &case.end;
    if cpu.bus.overlapped {
        *masks.entry("block_self_overlap").or_default() += 1;
    }
    let block_unknown = block && cpu.bus.unlisted.is_some();
    if block_unknown {
        *masks.entry("block_unknown").or_default() += 1;
    }
    let latch = opcode == TMA && tma_latch(case);
    for (name, got, want) in [
        ("a", cpu.a as u16, end.a as u16),
        ("x", cpu.x as u16, end.x as u16),
        ("y", cpu.y as u16, end.y as u16),
        ("s", cpu.s as u16, end.s as u16),
        ("p", cpu.p as u16, end.p as u16),
        ("pc", cpu.pc, end.pc),
    ] {
        let axy = matches!(name, "a" | "x" | "y");
        if block_unknown && axy {
            continue;
        }
        if name == "p" && TSB_TRB.contains(&opcode) && got ^ want == 0x02 {
            *masks.entry("tsb_z").or_default() += 1;
            continue;
        }
        if got != want && !(latch && name == "a") {
            problem(axy, format!("{name}: got ${got:X}, want ${want:X}"));
        }
    }
    if latch {
        *masks.entry("tma_latch").or_default() += 1;
    }
    if cpu.mpr != end.mpr {
        problem(false, format!("mpr: got {:?}, want {:?}", cpu.mpr, end.mpr));
    }
    for &(address, value) in &end.ram {
        if cpu.bus.tainted[address as usize] {
            continue;
        }
        let got = cpu.bus.memory[address as usize];
        if got != value
            && !(ST_OPCODES.contains(&opcode) && (0x1FE000..=0x1FE003).contains(&address))
        {
            problem(
                true,
                format!("ram ${address:06X}: got ${got:02X}, want ${value:02X}"),
            );
        }
    }
    let expected = &case.cycles;
    let seen = &cpu.bus.seen;
    let truncated = case.num_cycles as usize != expected.len();
    if truncated {
        *masks.entry("truncated").or_default() += 1;
    }
    if seen.len() != case.num_cycles as usize {
        problem(
            false,
            format!("cycles: got {}, want {}", seen.len(), case.num_cycles),
        );
    }
    if let (Some(address), false) = (cpu.bus.unlisted, block) {
        problem(
            false,
            format!("read of an address the case does not list: ${address:06X}"),
        );
    }
    for (index, (&(address, value, pins), &(got_kind, got_address, got_value))) in
        expected.iter().zip(seen.iter()).enumerate()
    {
        let want_kind = match pins {
            0b001 => b'r',
            0b010 => b'w',
            0b101 => b'd',
            0b000 => b'i',
            other => panic!("{}: a cycle with pins {other:03b}", case.name),
        };
        if ST_OPCODES.contains(&opcode) && index == 3 {
            *masks.entry("st_write").or_default() += 1;
            continue;
        }
        if got_kind != want_kind {
            problem(
                false,
                format!(
                    "cycle {index}: {}, want {}",
                    got_kind as char, want_kind as char
                ),
            );
            break;
        }
        if want_kind != b'i' && got_address != address {
            problem(
                false,
                format!("cycle {index}: address ${got_address:06X}, want ${address:06X}"),
            );
        }
        if (want_kind == b'r' || want_kind == b'w') && got_value != value {
            problem(
                false,
                format!("cycle {index}: value ${got_value:02X}, want ${value:02X}"),
            );
        }
    }
    // Nothing but the final state of a truncated transfer disagrees (a, x, y
    // or listed RAM), and that state depends on the 99% of the transfer the
    // corpus did not record. Counted, not hidden.
    if truncated && block && !problems.is_empty() && problems.iter().all(|p| p.final_state) {
        *masks.entry("block_final_unexplained").or_default() += 1;
        problems.clear();
    }
    (problems, masks)
}

/// Run one case; return the differences (empty: it passes) and the masks it used.
fn run_case(
    case: &Case,
    cpu: &mut HuC6280<Ram>,
    formula: Option<DecimalFormula>,
) -> (Vec<Problem>, Masks) {
    let (problems, mut masks) = run_once(case, cpu, formula);
    if !problems.is_empty() && formula.is_none() {
        // C13: the corpus is Mesen's. A case Mesen's formula fixes, and only
        // that, is counted.
        let decimal = DECIMAL_ARITHMETIC.contains(&case.opcode) && case.initial.p & 0x08 != 0;
        if decimal
            && run_once(case, cpu, Some(DecimalFormula::Mesen))
                .0
                .is_empty()
        {
            *masks.entry("decimal_formula").or_default() += 1;
            return (Vec::new(), masks);
        }
    }
    (problems, masks)
}

struct Totals {
    passed: usize,
    failed: usize,
    masks: Masks,
}

fn main() {
    let mut corpus: Option<PathBuf> = None;
    let mut limit: Option<usize> = None;
    let mut formula: Option<DecimalFormula> = None;
    let mut jobs = 8usize;
    let mut verdicts: Option<PathBuf> = None;
    let mut only: Option<usize> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--limit" => limit = Some(args.next().expect("--limit N").parse().expect("N")),
            "--jobs" => jobs = args.next().expect("--jobs N").parse().expect("N"),
            "--verdicts" => verdicts = Some(args.next().expect("--verdicts DIR").into()),
            "--opcode" => {
                only =
                    Some(usize::from_str_radix(&args.next().expect("--opcode XX"), 16).expect("XX"))
            }
            "--formula" => {
                formula = Some(match args.next().expect("--formula NAME").as_str() {
                    "mesen" => DecimalFormula::Mesen,
                    "mednafen" => DecimalFormula::Mednafen,
                    other => panic!("unknown formula {other}"),
                })
            }
            _ => corpus = Some(arg.into()),
        }
    }
    let corpus = corpus
        .expect("usage: sst CORPUS_DIR [--limit N] [--formula NAME] [--jobs N] [--verdicts DIR]");
    if let Some(directory) = &verdicts {
        std::fs::create_dir_all(directory).expect("verdicts directory");
    }
    let opcodes: Vec<usize> = match only {
        Some(opcode) => vec![opcode],
        None => (0..256).collect(),
    };
    let next = AtomicUsize::new(0);
    let totals = Mutex::new(Totals {
        passed: 0,
        failed: 0,
        masks: Masks::new(),
    });
    std::thread::scope(|scope| {
        for _ in 0..jobs.max(1) {
            scope.spawn(|| {
                let mut cpu = HuC6280::new(Ram::new());
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&opcode) = opcodes.get(index) else {
                        break;
                    };
                    let cases = load_cases(&corpus.join(format!("{opcode:02x}.json.bin")), limit);
                    let (mut passed, mut failed) = (0, 0);
                    let mut masks = Masks::new();
                    let mut lines = String::new();
                    let mut first: Option<String> = None;
                    for (number, case) in cases.iter().enumerate() {
                        let (problems, used) = run_case(case, &mut cpu, formula);
                        for (name, count) in &used {
                            *masks.entry(name).or_default() += count;
                        }
                        if problems.is_empty() {
                            passed += 1;
                        } else {
                            failed += 1;
                            if first.is_none() {
                                let texts: Vec<&str> =
                                    problems.iter().take(3).map(|p| p.text.as_str()).collect();
                                first = Some(format!("{}: {}", case.name, texts.join("; ")));
                            }
                        }
                        if verdicts.is_some() {
                            let used: Vec<String> = used
                                .iter()
                                .map(|(name, count)| format!("{name}={count}"))
                                .collect();
                            let used = if used.is_empty() {
                                "-".to_string()
                            } else {
                                used.join(",")
                            };
                            let verdict = if problems.is_empty() { 'P' } else { 'F' };
                            lines.push_str(&format!("{number} {verdict} {used}\n"));
                        }
                    }
                    if let Some(directory) = &verdicts {
                        std::fs::write(directory.join(format!("{opcode:02x}.txt")), lines)
                            .expect("write verdicts");
                    }
                    if let Some(first) = first {
                        println!("{opcode:02X}: {failed} failing; {first}");
                    }
                    let mut totals = totals.lock().unwrap();
                    totals.passed += passed;
                    totals.failed += failed;
                    for (name, count) in masks {
                        *totals.masks.entry(name).or_default() += count;
                    }
                }
            });
        }
    });
    let totals = totals.into_inner().unwrap();
    let masks: Vec<String> = totals
        .masks
        .iter()
        .map(|(name, count)| format!("{name} {count}"))
        .collect();
    println!(
        "cases {}, passed {}, failed {}; masks: {}",
        totals.passed + totals.failed,
        totals.passed,
        totals.failed,
        masks.join(", ")
    );
    std::process::exit(if totals.failed != 0 { 1 } else { 0 });
}
