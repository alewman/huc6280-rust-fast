// SPDX-License-Identifier: MIT
//! Rung 1, the arithmetic tables: ADC's and SBC's result and flags for every
//! pair of operands, binary and decimal, under both decimal formulas, as one
//! digest a line. `scripts/reference_alu.py` prints the same from the
//! reference's `_alu.add` and `_alu.subtract`; the two outputs are compared.
//!
//!     alu

use huc6280::{add, subtract, DecimalFormula};

/// P values that cover C and D both ways, with the other flags clear and set.
const P_VALUES: [u8; 8] = [0x00, 0x01, 0x08, 0x09, 0xC2, 0xC3, 0xEA, 0xEB];

fn main() {
    type Operation = fn(u8, u8, u8, DecimalFormula) -> (u8, u8);
    let operations: [(&str, Operation); 2] = [("add", add), ("subtract", subtract)];
    for (name, operation) in operations {
        for (formula_name, formula) in [
            ("mednafen", DecimalFormula::Mednafen),
            ("mesen", DecimalFormula::Mesen),
        ] {
            for p in P_VALUES {
                // FNV-1a, 64 bits, over (result, P) for every A and operand.
                let mut digest: u64 = 0xCBF29CE484222325;
                for a in 0..=255u8 {
                    for operand in 0..=255u8 {
                        let (result, flags) = operation(a, operand, p, formula);
                        for byte in [result, flags] {
                            digest = (digest ^ byte as u64).wrapping_mul(0x100000001B3);
                        }
                    }
                }
                println!("{name} {formula_name} p={p:02X} {digest:016x}");
            }
        }
    }
}
