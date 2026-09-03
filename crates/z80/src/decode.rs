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
use crate::registers::{PairBase, RegIndex, index, pair};

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
    ///
    /// **The list above used to be the whole list, and it was three-quarters of one.** The
    /// same field also names the register of `IN r,(C)` and `OUT (C),r` on the `ED` page —
    /// and those two are the reason encoding `110`'s [`None`] matters most. It is the
    /// omitted case that carries the meaning: `IN (C)` reads the port and keeps only the
    /// flags, `OUT (C),0` writes a zero, and both fall out of `register_index` returning
    /// nothing rather than out of a special case in either handler.
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

/// Which register pair stands in for `HL` in the instruction being decoded.
///
/// A `DD` or `FD` prefix substitutes `IX` or `IY` for `HL`, and that substitution reaches
/// three different places: the pair operations (`INC HL` becomes `INC IX`), the `H`/`L`
/// register halves (`H` becomes `IXh`), and the memory operand — where `(HL)` becomes
/// `(IX+d)` and acquires a displacement byte that must be fetched.
///
/// Carrying which index is in play, rather than a bare [`PairBase`], is what lets the same
/// decode table serve all three: the displacement only exists for the prefixed forms, and
/// [`Index::is_displaced`] is the one question that distinguishes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Index {
    /// Un-prefixed: `HL`, and `(HL)` addresses the pair with no displacement.
    Hl,
    /// `DD`-prefixed: `IX`, and `(HL)` becomes `(IX+d)`.
    Ix,
    /// `FD`-prefixed: `IY`, and `(HL)` becomes `(IY+d)`.
    Iy,
}

impl Index {
    /// The pair standing in for `HL`.
    pub(crate) const fn base(self) -> PairBase {
        match self {
            Self::Hl => pair::HL,
            Self::Ix => pair::IX,
            Self::Iy => pair::IY,
        }
    }

    /// Whether the memory operand carries a displacement byte.
    pub(crate) const fn is_displaced(self) -> bool {
        !matches!(self, Self::Hl)
    }

    /// The index that applies to the *register* halves of a two-operand `LD`.
    ///
    /// This is the asymmetry that makes the prefixes subtle. `DD 44` is `LD B,IXh` — the
    /// substitution reaches `H`. But `DD 66` is `LD H,(IX+d)`, writing real `H`, and
    /// `DD 74` is `LD (IX+d),H`, reading real `H`. Corpus vectors `dd66` and `dd74` prove
    /// it: `dd74` stores `0x01`, which is `H` of `HL = 0x0125`, not `IXh` of `IX = 0x5910`.
    ///
    /// So when either operand is the memory operand, the register half is **not**
    /// substituted — the prefix has already been spent on the address.
    pub(crate) const fn for_register_half(self, touches_memory: bool) -> Self {
        if touches_memory { Self::Hl } else { self }
    }
}

/// An [`Operand`] resolved to the place its value actually lives.
///
/// The distinction matters because resolving is not free and must not be repeated. For
/// `(HL)` the effective address is just the pair, so recomputing it is idempotent — but for
/// the `DD`/`FD` form `(IX+d)` resolving means **fetching the displacement byte and adding
/// it**, and both must happen exactly once per instruction. `INC (IX+d)` reads, waits and
/// writes back at one address; three independent recomputations would fetch three
/// displacements and charge the addition three times.
///
/// So the effective address is carried here as a *value*, computed once by
/// [`Cpu::resolve`] and handed to every site that needs it.
///
/// **"Computed once by [`Cpu::resolve`]" named the invariant correctly and the mechanism
/// incompletely.** `Cpu::execute_cb` builds a `Target::Memory` directly for the `DDCB`/`FDCB`
/// forms, and has to: those encodings put the displacement byte *before* the opcode, so the
/// address is known before there is an [`Operand`] to resolve. Two constructors, one
/// per-instruction computation — which is the property that matters, and which neither site
/// can break on its own.
///
/// [`Cpu::resolve`]: crate::Cpu::resolve
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    /// An 8-bit register.
    Register(RegIndex),
    /// A byte of memory at an already-computed effective address.
    Memory(u16),
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

/// The eight rotate and shift operations of the `CB` set, in field order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShiftOp {
    /// Rotate left circular.
    Rlc,
    /// Rotate right circular.
    Rrc,
    /// Rotate left through carry.
    Rl,
    /// Rotate right through carry.
    Rr,
    /// Shift left arithmetic.
    Sla,
    /// Shift right arithmetic, preserving bit 7.
    Sra,
    /// Shift left logical — undocumented, shifts a one into bit 0.
    Sll,
    /// Shift right logical.
    Srl,
}

/// The four groups a `CB` opcode's top two bits select.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CbOp {
    /// `00 ooo rrr` — rotate or shift.
    Shift(ShiftOp),
    /// `01 bbb rrr` — test bit `bbb`.
    Bit(u8),
    /// `10 bbb rrr` — clear bit `bbb`.
    Reset(u8),
    /// `11 bbb rrr` — set bit `bbb`.
    Set(u8),
}

/// The shift operations indexed by their 3-bit field.
const SHIFTS: [ShiftOp; 8] = [
    ShiftOp::Rlc,
    ShiftOp::Rrc,
    ShiftOp::Rl,
    ShiftOp::Rr,
    ShiftOp::Sla,
    ShiftOp::Sra,
    ShiftOp::Sll,
    ShiftOp::Srl,
];

/// Which group each value of the top two bits selects.
#[derive(Debug, Clone, Copy)]
enum CbGroup {
    Shift,
    Bit,
    Reset,
    Set,
}

/// The groups indexed by the opcode's top two bits.
const CB_GROUPS: [CbGroup; 4] = [CbGroup::Shift, CbGroup::Bit, CbGroup::Reset, CbGroup::Set];

impl CbOp {
    /// Decode a `CB` opcode. The whole set is `xx bbb rrr`: the top two bits pick the
    /// group, the middle three a bit number or a shift, and the low three the operand.
    pub(crate) fn from_opcode(opcode: u8) -> Self {
        // INVARIANT: a byte shifted right by six yields 0..=3, always within the table.
        let group = CB_GROUPS[usize::from(opcode >> 6)];
        // INVARIANT: masking with 0x07 yields 0..=7.
        let field = (opcode >> 3) & 0x07;
        match group {
            CbGroup::Shift => Self::Shift(SHIFTS[usize::from(field)]),
            CbGroup::Bit => Self::Bit(field),
            CbGroup::Reset => Self::Reset(field),
            CbGroup::Set => Self::Set(field),
        }
    }
}

/// The 16-bit pairs an `ED` opcode's bits 5–4 select.
const ED_PAIRS: [PairBase; 4] = [pair::BC, pair::DE, pair::HL, pair::SP];

/// The pair named by bits 5–4 of an `ED` opcode — `SBC HL,ss`, `LD (nn),dd` and friends.
pub(crate) fn ed_pair(opcode: u8) -> PairBase {
    // INVARIANT: shifting right by four then masking with 0x03 yields 0..=3.
    ED_PAIRS[usize::from((opcode >> 4) & 0x03)]
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
