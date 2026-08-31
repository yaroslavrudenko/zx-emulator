//! The Z80 `F` register bit layout.
//!
//! This is the SINGLE definition of the flag bits, shared by the independent ALU
//! reference model (`reference.rs`) and the vector-mismatch reporter (`report.rs`).
//! Duplicating the bit numbers in either of those would be two sources of truth for
//! one fact, and the undocumented bits are exactly where a silent divergence hides.

/// Bit 7 — Sign. Copy of bit 7 of the result.
pub const S: u8 = 1 << 7;
/// Bit 6 — Zero.
pub const Z: u8 = 1 << 6;
/// Bit 5 — **undocumented** ("Y" / `F5`). For most ALU ops it is a copy of bit 5 of
/// the result; `CP` copies it from the *operand*, and `SCF`/`CCF` from register Q.
pub const Y: u8 = 1 << 5;
/// Bit 4 — Half-carry (carry out of bit 3). Only `DAA` reads it back.
pub const H: u8 = 1 << 4;
/// Bit 3 — **undocumented** ("X" / `F3`). Same rules as [`Y`], one bit lower.
pub const X: u8 = 1 << 3;
/// Bit 2 — Parity/Overflow. Signed overflow for arithmetic, even parity for logic.
pub const PV: u8 = 1 << 2;
/// Bit 1 — Add/Subtract. Set by subtractive operations; read back by `DAA`.
pub const N: u8 = 1 << 1;
/// Bit 0 — Carry.
pub const C: u8 = 1 << 0;

/// The two undocumented bits as one mask — the pair that zexall exists to police.
pub const UNDOCUMENTED: u8 = X | Y;

/// Every flag, most-significant first, with the name used in failure reports.
/// The bit number is spelled out for the undocumented pair because "bit 3 was wrong"
/// is the sentence a reader of a failing test needs to see.
pub const BITS: [(&str, u8); 8] = [
    ("S", S),
    ("Z", Z),
    ("Y(bit5)", Y),
    ("H", H),
    ("X(bit3)", X),
    ("P/V", PV),
    ("N", N),
    ("C", C),
];

/// Render an `F` byte as `S=1 Z=0 Y(bit5)=1 H=0 X(bit3)=1 P/V=0 N=0 C=1`.
pub fn describe(f: u8) -> String {
    let mut out = String::with_capacity(BITS.len() * 10);
    for (i, (name, mask)) in BITS.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(name);
        out.push('=');
        out.push(if f & mask != 0 { '1' } else { '0' });
    }
    out
}

/// The names of the flags that differ between `expected` and `actual`, most
/// significant first. Empty when the two `F` bytes are equal.
pub fn differences(expected: u8, actual: u8) -> Vec<&'static str> {
    let changed = expected ^ actual;
    BITS.iter()
        .filter(|(_, mask)| changed & mask != 0)
        .map(|(name, _)| *name)
        .collect()
}

/// Z80 parity: the `P/V` flag is set when the number of set bits is **even**.
pub fn even_parity(value: u8) -> bool {
    value.count_ones().is_multiple_of(2)
}
