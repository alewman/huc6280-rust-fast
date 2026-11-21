// SPDX-License-Identifier: MIT
//! Rung 3: Klaus Dormann's 65C02 extended opcodes test, relocated, natively.
//!
//!     dormann TEST.bin START SUCCESS [--limit N]
//!
//! `TEST.bin` is the 64 KB image `scripts/reference_dormann.py` builds as the
//! reference's `scripts/run_dormann.py` builds it; `START` and `SUCCESS` are
//! its labels `start` and `success_address` (hex). The program is
//! self-checking: it traps (a jump or branch to itself) at the first wrong
//! result and reaches the success address when everything passed.
//!
//! The memory is the 65C02's flat 64 KB through the mapper as the reference's
//! `tests/dormann.py` maps it: MPR0 and MPR1 are one bank, so the absolute
//! `$00xx`/`$01xx` the program uses and the zero page and stack at
//! `$20xx`/`$21xx` the core uses are the same bytes; the code is at `$4000`
//! and up.

use huc6280::{Bus, HuC6280, Pins};

const BANK: [u8; 8] = [0x10, 0x10, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17];

struct Ram(Vec<u8>);

impl Bus for Ram {
    fn read(&mut self, address: u32, _: &mut Pins) -> u8 {
        self.0[address as usize]
    }
    fn write(&mut self, address: u32, value: u8, _: &mut Pins) {
        self.0[address as usize] = value;
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: dormann TEST.bin START SUCCESS [--limit N]");
        std::process::exit(2);
    }
    let image = std::fs::read(&args[0]).expect("read the image");
    let start = u16::from_str_radix(&args[1], 16).expect("START");
    let done = u16::from_str_radix(&args[2], 16).expect("SUCCESS");
    let limit: u64 = match args.get(3).map(String::as_str) {
        Some("--limit") => args[4].parse().expect("N"),
        _ => 100_000_000,
    };
    let mut memory = vec![0u8; 0x200000];
    for page in [0usize, 2, 3, 4, 5, 6, 7] {
        // page 1 is page 0 again (MPR1 = MPR0)
        let base = (BANK[page] as usize) << 13;
        memory[base..base + 0x2000].copy_from_slice(&image[page * 0x2000..(page + 1) * 0x2000]);
    }
    let mut cpu = HuC6280::new(Ram(memory));
    cpu.mpr = BANK;
    cpu.pc = start;
    cpu.s = 0xFF;
    cpu.p = 0x04;
    let test_case = |cpu: &HuC6280<Ram>| cpu.bus.0[((BANK[0] as usize) << 13) | 0x202];
    let began = std::time::Instant::now();
    let mut instructions = 0u64;
    let mut reason = "limit";
    while instructions < limit {
        let before = cpu.pc;
        cpu.step();
        instructions += 1;
        if cpu.pc == before {
            // a jump or branch to itself: a trap, or success
            reason = if before == done { "success" } else { "trap" };
            break;
        }
    }
    println!(
        "{reason} at ${:04X} after {instructions} instructions (test case {})",
        cpu.pc,
        test_case(&cpu)
    );
    eprintln!("{:.3} s", began.elapsed().as_secs_f64());
    std::process::exit(if reason == "success" { 0 } else { 1 });
}
