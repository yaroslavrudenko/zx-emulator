//! Opcode field decoding.
//!
//! An un-prefixed Z80 opcode is a bit pattern, not an arbitrary number. Most of the map
//! is three fields — `xx yyy zzz` — where `yyy` and `zzz` select a register, a branch
//! condition or an ALU operation. `LD r,r'` is literally `01 ddd sss`, and `ALU A,r` is
//! `10 ooo sss`, which is why those two blocks between them cover a quarter of the map
//! with two lines of dispatch.
//!
//! Decoding a field into an enum rather than passing a raw `u8` around means every
//! consumer matches exhaustively, so the compiler proves no encoding was forgotten. The
//! tables below are indexed by a masked field and the masks make the range self-evident,
//! which is noted at each site.

use crate::flags;
use crate::registers::{PairBase, RegIndex, index};

/// The eight encodings of an opcode's 3-bit operand field.
///
/// Seven name an 8-bit register; encoding 6 names the byte at `(HL)`. The memory operand
/// is not a register and has no index, which is why [`Operand::register_index`] returns
/// an `Option` instead of a `usize` — there is no value it could return for `MemHl` that
/// would not be silently wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Operand {
    B,
    C,
    D,
    E,
    H,
    L,
    MemHl,
    A,
}

/// The operand encodings in field order.
const OPERANDS: [Operand; 8] = [
    Operand::B,
    Operand::C,
    Operand::D,
    Operand::E,
    Operand::H,
    Operand::L,
    Operand::MemHl,
    Operand::A,
];

impl Operand {
    /// Bits 2–0: the source operand of `LD r,r'` and the operand of `ALU A,r`.
    pub(crate) fn source(opcode: u8) -> Self {
        // INVARIANT: masking with 0x07 yields 0..=7, always within the 8-entry table.
        OPERANDS[usize::from(opcode & 0x07)]
    }

    /// Bits 5–3: the destination of `LD r,r'` and `LD r,n`, and the target of `INC r`
    /// and `DEC r`.
    pub(crate) fn destination(opcode: u8) -> Self {
        // INVARIANT: shifting then masking with 0x07 yields 0..=7.
        OPERANDS[usize::from((opcode >> 3) & 0x07)]
    }

    /// The register this operand names, or `None` for the `(HL)` memory operand.
    ///
    /// `base` is the pair a `DD`/`FD` prefix has substituted for `HL` — [`pair::HL`] when
    /// un-prefixed. Note the asymmetry, which is the whole subtlety of the index prefixes:
    /// **`H` and `L` move with the base; `B`, `C`, `D`, `E` and `A` do not.** `DD 44` is
    /// `LD B,IXh`, not `LD IXh,IXh`.
    ///
    /// [`pair::HL`]: crate::registers::pair::HL
    pub(crate) const fn register_index(self, base: PairBase) -> Option<RegIndex> {
        match self {
            Self::B => Some(index::B),
            Self::C => Some(index::C),
            Self::D => Some(index::D),
            Self::E => Some(index::E),
            Self::H => Some(base.high()),
            Self::L => Some(base.low()),
            Self::A => Some(index::A),
            Self::MemHl => None,
        }
    }
}

/// The eight accumulator ALU operations, in the order the opcode field encodes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AluOp {
    /// Field 0 — `ADD A,s`.
    Add,
    /// Field 1 — `ADC A,s`.
    Adc,
    /// Field 2 — `SUB s`.
    Sub,
    /// Field 3 — `SBC A,s`.
    Sbc,
    /// Field 4 — `AND s`.
    And,
    /// Field 5 — `XOR s`.
    Xor,
    /// Field 6 — `OR s`.
    Or,
    /// Field 7 — `CP s`.
    Cp,
}

/// The ALU operations in field order.
///
/// `XOR` is field 5 and `OR` is field 6 — the order the three logical operations appear
/// in is `AND`, `XOR`, `OR`, which is not the order they are usually spoken in. Transpose
/// the middle two and the arithmetic still looks plausible on most inputs; the tell is
/// `XOR A`, which must clear the accumulator and instead leaves it untouched.
const ALU_OPS: [AluOp; 8] = [
    AluOp::Add,
    AluOp::Adc,
    AluOp::Sub,
    AluOp::Sbc,
    AluOp::And,
    AluOp::Xor,
    AluOp::Or,
    AluOp::Cp,
];

impl AluOp {
    /// Bits 5–3 select the operation, identically for the register form `10 ooo rrr` and
    /// the immediate form `11 ooo 110`. One decoder therefore serves both.
    pub(crate) fn from_opcode(opcode: u8) -> Self {
        // INVARIANT: shifting then masking with 0x07 yields 0..=7.
        ALU_OPS[usize::from((opcode >> 3) & 0x07)]
    }
}

/// The eight branch conditions, in the order the opcode field encodes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Condition {
    /// `NZ` — the zero flag is clear.
    NonZero,
    /// `Z` — the zero flag is set.
    Zero,
    /// `NC` — the carry flag is clear.
    NoCarry,
    /// `C` — the carry flag is set.
    Carry,
    /// `PO` — parity odd, or no signed overflow.
    ParityOdd,
    /// `PE` — parity even, or signed overflow.
    ParityEven,
    /// `P` — the sign flag is clear, so the result is positive.
    Positive,
    /// `M` — the sign flag is set, so the result is negative.
    Negative,
}

/// The conditions in field order.
const CONDITIONS: [Condition; 8] = [
    Condition::NonZero,
    Condition::Zero,
    Condition::NoCarry,
    Condition::Carry,
    Condition::ParityOdd,
    Condition::ParityEven,
    Condition::Positive,
    Condition::Negative,
];

impl Condition {
    /// Bits 5–3 of `RET cc`, `JP cc,nn` and `CALL cc,nn`, which have room for all eight.
    pub(crate) fn from_opcode(opcode: u8) -> Self {
        // INVARIANT: shifting then masking with 0x07 yields 0..=7.
        CONDITIONS[usize::from((opcode >> 3) & 0x07)]
    }

    /// Bits 4–3 of `JR cc,e`, which has room for only the first four conditions. The
    /// relative jump shares the `00 xxx 000` block with `NOP`, `EX AF,AF'`, `DJNZ` and
    /// the unconditional `JR`, so only two bits are left to encode a condition.
    pub(crate) fn from_relative_jump_opcode(opcode: u8) -> Self {
        // INVARIANT: shifting then masking with 0x03 yields 0..=3.
        CONDITIONS[usize::from((opcode >> 3) & 0x03)]
    }

    /// Whether this condition holds for the given `F` register value.
    pub(crate) const fn holds(self, f: u8) -> bool {
        match self {
            Self::NonZero => (f & flags::ZERO) == 0,
            Self::Zero => (f & flags::ZERO) != 0,
            Self::NoCarry => (f & flags::CARRY) == 0,
            Self::Carry => (f & flags::CARRY) != 0,
            Self::ParityOdd => (f & flags::PARITY_OVERFLOW) == 0,
            Self::ParityEven => (f & flags::PARITY_OVERFLOW) != 0,
            Self::Positive => (f & flags::SIGN) == 0,
            Self::Negative => (f & flags::SIGN) != 0,
        }
    }
}
