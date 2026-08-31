//! Flag rules, one helper per operation class.
//!
//! Every instruction that touches `F` delegates to a function here. Nothing inlines flag
//! logic into an opcode handler, because the Z80 has roughly two hundred flag-setting
//! opcodes falling into about twenty classes: written per-opcode, that is two hundred
//! chances to get the same rule subtly wrong, and the undocumented bits are exactly where
//! such a mistake hides until `zexall` finds it.
//!
//! # The undocumented bits
//!
//! Bits 3 and 5 of `F` have no documented meaning, but they are not random: the Z80
//! copies them from the result of almost every operation, and real software — plus the
//! `zexall` conformance suite — depends on it. They are handled here as a first-class
//! part of every rule, not as an afterthought. Two classes break the "copy from the
//! result" pattern and are commented at the site:
//!
//! - `CP` copies them from the **operand**, because it throws its result away.
//! - `SCF` and `CCF` take them from the **accumulator**.
//!
//! # Milestone split
//!
//! The rules the un-prefixed instruction set uses live at the top level. The rules that
//! belong to the `CB`- and `ED`-prefixed classes live in [`prefixed`], which M2 wires up.
//! They are written now rather than later because a class's rule belongs beside its
//! siblings — that adjacency is what keeps the shared pieces (`sign_and_zero`, `parity`,
//! the rotate bit-movements) honest across both milestones.

/// Bit 7 — set from bit 7 of the result. A signed value's sign.
pub(crate) const SIGN: u8 = 0b1000_0000;
/// Bit 6 — set when the result is zero.
pub(crate) const ZERO: u8 = 0b0100_0000;
/// Bit 5 — undocumented. Copied from bit 5 of the result by most operations.
pub(crate) const BIT5: u8 = 0b0010_0000;
/// Bit 4 — carry out of bit 3. Only `DAA` reads it back.
pub(crate) const HALF_CARRY: u8 = 0b0001_0000;
/// Bit 3 — undocumented. Copied from bit 3 of the result by most operations.
pub(crate) const BIT3: u8 = 0b0000_1000;
/// Bit 2 — signed overflow for arithmetic, even parity for logic and shifts.
pub(crate) const PARITY_OVERFLOW: u8 = 0b0000_0100;
/// Bit 1 — the Z80's "N" flag: set by subtractive operations. Read back by `DAA`.
pub(crate) const ADD_SUBTRACT: u8 = 0b0000_0010;
/// Bit 0 — carry out of bit 7.
pub(crate) const CARRY: u8 = 0b0000_0001;

/// The two undocumented bits as one mask.
const UNDOCUMENTED: u8 = BIT3 | BIT5;

/// Carry out of bit 11, the half-carry position for 16-bit arithmetic.
const HALF_CARRY_16: u16 = 0x1000;

/// Carry out of bit 3, the half-carry position for 8-bit arithmetic.
const HALF_CARRY_8: u8 = 0x10;

/// The low nibble of a byte — one BCD digit.
const LOW_NIBBLE: u8 = 0x0F;

/// The low 12 bits of a 16-bit value, below the 16-bit half-carry position.
const LOW_12_BITS: u16 = 0x0FFF;

/// The largest value a byte can hold, as the 16-bit intermediate sees it.
const BYTE_MAX: u16 = 0xFF;

/// Carry out of bit 7, seen in the 16-bit intermediate used by 8-bit subtraction.
const BORROW_OUT_8: u16 = 0x0100;

/// The largest value a 16-bit register can hold, as the 32-bit intermediate sees it.
const WORD_MAX: u32 = 0xFFFF;

// ---------------------------------------------------------------------------
// Shared pieces of the rules
// ---------------------------------------------------------------------------

/// `mask` when `set` holds, nothing otherwise.
const fn flag(mask: u8, set: bool) -> u8 {
    if set { mask } else { 0 }
}

/// The undocumented pair, copied from a result byte.
const fn undocumented(result: u8) -> u8 {
    result & UNDOCUMENTED
}

/// Sign and zero, which almost every rule derives the same way.
const fn sign_and_zero(result: u8) -> u8 {
    (result & SIGN) | flag(ZERO, result == 0)
}

/// Even parity, which the logical and shift classes report in the P/V bit.
const fn parity(result: u8) -> u8 {
    flag(PARITY_OVERFLOW, result.count_ones().is_multiple_of(2))
}

/// The flags shared by `AND`, `OR` and `XOR`. Carry and the add/subtract flag are
/// cleared; `AND` adds the half-carry on top, which is the only thing separating them.
const fn logic_flags(result: u8) -> u8 {
    sign_and_zero(result) | undocumented(result) | parity(result)
}

// ---------------------------------------------------------------------------
// 8-bit arithmetic
// ---------------------------------------------------------------------------

/// `ADD A,s`.
pub(crate) fn add8(a: u8, operand: u8) -> (u8, u8) {
    adc8(a, operand, false)
}

/// `ADC A,s`. Addition with `ADD` as the special case of a clear incoming carry.
pub(crate) fn adc8(a: u8, operand: u8, carry_in: bool) -> (u8, u8) {
    let carry = u8::from(carry_in);
    let sum = u16::from(a) + u16::from(operand) + u16::from(carry);
    let [result, _] = sum.to_le_bytes();

    let half_carry = (((a & LOW_NIBBLE) + (operand & LOW_NIBBLE) + carry) & HALF_CARRY_8) != 0;
    // Signed overflow: both operands agreed on their sign and the result disagrees.
    let overflow = ((a ^ result) & (operand ^ result) & SIGN) != 0;

    let flags = sign_and_zero(result)
        | undocumented(result)
        | flag(HALF_CARRY, half_carry)
        | flag(PARITY_OVERFLOW, overflow)
        | flag(CARRY, sum > BYTE_MAX);
    (result, flags)
}

/// `SUB s`.
pub(crate) fn sub8(a: u8, operand: u8) -> (u8, u8) {
    sbc8(a, operand, false)
}

/// `SBC A,s`. Subtraction with `SUB` as the special case of a clear incoming borrow.
pub(crate) fn sbc8(a: u8, operand: u8, carry_in: bool) -> (u8, u8) {
    let carry = u8::from(carry_in);
    let difference = u16::from(a)
        .wrapping_sub(u16::from(operand))
        .wrapping_sub(u16::from(carry));
    let [result, _] = difference.to_le_bytes();

    // A nibble subtraction that borrows lands in 0xF0..=0xFF, so bit 4 is the borrow.
    let half_borrow = ((a & LOW_NIBBLE)
        .wrapping_sub(operand & LOW_NIBBLE)
        .wrapping_sub(carry)
        & HALF_CARRY_8)
        != 0;
    // Signed overflow: the operands disagreed on their sign and the result took the
    // subtrahend's side.
    let overflow = ((a ^ operand) & (a ^ result) & SIGN) != 0;

    let flags = sign_and_zero(result)
        | undocumented(result)
        | flag(HALF_CARRY, half_borrow)
        | flag(PARITY_OVERFLOW, overflow)
        | ADD_SUBTRACT
        | flag(CARRY, (difference & BORROW_OUT_8) != 0);
    (result, flags)
}

/// `CP s` — subtract without keeping the result, and return only the flags.
///
/// This is the one ALU class whose undocumented bits do **not** come from the result:
/// the result is discarded, and bits 3 and 5 arrive from the operand being compared.
/// Getting this wrong is invisible until `zexall` reaches its `CP` block.
pub(crate) fn cp8(a: u8, operand: u8) -> u8 {
    let (_, flags) = sub8(a, operand);
    (flags & !UNDOCUMENTED) | undocumented(operand)
}

/// `AND s`. The only logical operation that sets the half-carry.
pub(crate) fn and8(a: u8, operand: u8) -> (u8, u8) {
    let result = a & operand;
    (result, logic_flags(result) | HALF_CARRY)
}

/// `OR s`.
pub(crate) fn or8(a: u8, operand: u8) -> (u8, u8) {
    let result = a | operand;
    (result, logic_flags(result))
}

/// `XOR s`.
pub(crate) fn xor8(a: u8, operand: u8) -> (u8, u8) {
    let result = a ^ operand;
    (result, logic_flags(result))
}

/// `INC r` / `INC (HL)`.
///
/// The increment and decrement classes are deliberately not `ADD A,1` and `SUB 1`: they
/// leave the carry flag alone, which is what lets a loop counter be incremented in the
/// middle of a multi-byte addition.
pub(crate) fn inc8(value: u8, f: u8) -> (u8, u8) {
    let result = value.wrapping_add(1);
    let flags = (f & CARRY)
        | sign_and_zero(result)
        | undocumented(result)
        | flag(HALF_CARRY, (value & LOW_NIBBLE) == LOW_NIBBLE)
        | flag(PARITY_OVERFLOW, value == 0x7F);
    (result, flags)
}

/// `DEC r` / `DEC (HL)`. Leaves the carry flag alone, as [`inc8`] does.
pub(crate) fn dec8(value: u8, f: u8) -> (u8, u8) {
    let result = value.wrapping_sub(1);
    let flags = (f & CARRY)
        | sign_and_zero(result)
        | undocumented(result)
        | flag(HALF_CARRY, (value & LOW_NIBBLE) == 0)
        | flag(PARITY_OVERFLOW, value == 0x80)
        | ADD_SUBTRACT;
    (result, flags)
}

/// `CPL` — complement the accumulator.
///
/// Defines only the half-carry and add/subtract flags; sign, zero, parity and carry
/// survive untouched.
pub(crate) fn cpl(a: u8, f: u8) -> (u8, u8) {
    let result = !a;
    let flags = (f & (SIGN | ZERO | PARITY_OVERFLOW | CARRY))
        | undocumented(result)
        | HALF_CARRY
        | ADD_SUBTRACT;
    (result, flags)
}

/// `DAA` — re-normalise the accumulator to packed BCD after an add or subtract.
///
/// `DAA` is the only instruction that reads the half-carry and add/subtract flags back,
/// which is why those two bits exist at all. The correction it applies is +6 to a nibble
/// that overflowed its BCD range, and +0x60 to the byte when the high nibble did — or the
/// same values subtracted, when the flags say the preceding operation was a subtraction.
pub(crate) fn daa(a: u8, f: u8) -> (u8, u8) {
    /// A nibble above this needs re-normalising; BCD digits run 0..=9.
    const MAX_BCD_DIGIT: u8 = 9;
    /// A byte above this has a high nibble outside BCD range.
    const MAX_BCD_BYTE: u8 = 0x99;
    /// The correction that re-normalises one overflowed nibble.
    const NIBBLE_CORRECTION: u8 = 0x06;
    /// The same correction scaled to the high nibble.
    const BYTE_CORRECTION: u8 = 0x60;

    let subtracting = (f & ADD_SUBTRACT) != 0;
    let low_nibble = a & LOW_NIBBLE;

    // The magnitude conditions are the same on both paths — a nibble outside BCD range
    // needs the same 0x06 whether it got there by adding or by subtracting. Only the
    // direction below is chosen by N. Gating these tests on `!subtracting` leaves a
    // subtraction that lands on, say, 0x9A uncorrected, which is what FUSE vector `27_1`
    // catches.
    let correct_low = (f & HALF_CARRY) != 0 || low_nibble > MAX_BCD_DIGIT;
    let carry_out = (f & CARRY) != 0 || a > MAX_BCD_BYTE;

    let low_correction = if correct_low { NIBBLE_CORRECTION } else { 0 };
    let high_correction = if carry_out { BYTE_CORRECTION } else { 0 };
    let correction = low_correction | high_correction;
    let result = if subtracting {
        a.wrapping_sub(correction)
    } else {
        a.wrapping_add(correction)
    };

    // The half-carry reported is the one the correction itself produced, computed by the
    // same rule the add and subtract classes use.
    let correction_nibble = correction & LOW_NIBBLE;
    let half_carry = if subtracting {
        (low_nibble.wrapping_sub(correction_nibble) & HALF_CARRY_8) != 0
    } else {
        ((low_nibble + correction_nibble) & HALF_CARRY_8) != 0
    };

    let flags = sign_and_zero(result)
        | undocumented(result)
        | parity(result)
        | flag(HALF_CARRY, half_carry)
        | (f & ADD_SUBTRACT)
        | flag(CARRY, carry_out);
    (result, flags)
}

/// `SCF` — set the carry flag.
///
/// Bits 3 and 5 come from the accumulator rather than from any result, since there is no
/// result. A real NMOS Z80 additionally ORs in the internal flag latch left by the
/// preceding instruction, so the two bits can be set by a value this rule never sees;
/// that refinement is not modelled here. See the crate-level documentation.
pub(crate) fn scf(a: u8, f: u8) -> u8 {
    (f & (SIGN | ZERO | PARITY_OVERFLOW)) | undocumented(a) | CARRY
}

/// `CCF` — complement the carry flag, moving its old value into the half-carry.
///
/// Bits 3 and 5 follow the same accumulator rule as [`scf`], with the same caveat.
pub(crate) fn ccf(a: u8, f: u8) -> u8 {
    let carry_was_set = (f & CARRY) != 0;
    (f & (SIGN | ZERO | PARITY_OVERFLOW))
        | undocumented(a)
        | flag(HALF_CARRY, carry_was_set)
        | flag(CARRY, !carry_was_set)
}

// ---------------------------------------------------------------------------
// 16-bit arithmetic
// ---------------------------------------------------------------------------

/// `ADD HL,ss`.
///
/// The 16-bit add is unusual in leaving sign, zero and overflow untouched — only the
/// half-carry, add/subtract and carry flags are defined. The undocumented bits come from
/// the **high byte** of the result, which is the byte the ALU handled last.
pub(crate) fn add16(target: u16, operand: u16, f: u8) -> (u16, u8) {
    let sum = u32::from(target) + u32::from(operand);
    let result = target.wrapping_add(operand);
    let half_carry = (((target & LOW_12_BITS) + (operand & LOW_12_BITS)) & HALF_CARRY_16) != 0;
    let [high, _] = result.to_be_bytes();

    let flags = (f & (SIGN | ZERO | PARITY_OVERFLOW))
        | undocumented(high)
        | flag(HALF_CARRY, half_carry)
        | flag(CARRY, sum > WORD_MAX);
    (result, flags)
}

// ---------------------------------------------------------------------------
// Rotates and shifts — the bit movements shared by both milestones
//
// The bit movement is shared; the flag rule is not. The four accumulator forms below
// (`RLCA`, `RRCA`, `RLA`, `RRA`) leave sign, zero and parity alone, while their
// `CB`-prefixed counterparts in `prefixed` report a full set. Conflating the two classes
// costs an emulator dearly, because the accumulator forms sit inside tight arithmetic
// loops where a clobbered zero flag changes control flow.
// ---------------------------------------------------------------------------

/// The bit movement of a rotate or shift: the new byte, and the bit that fell out.
type Shifted = (u8, bool);

fn rotate_left_circular(value: u8) -> Shifted {
    (value.rotate_left(1), (value & 0x80) != 0)
}

fn rotate_right_circular(value: u8) -> Shifted {
    (value.rotate_right(1), (value & 0x01) != 0)
}

fn rotate_left_through_carry(value: u8, carry_in: bool) -> Shifted {
    ((value << 1) | u8::from(carry_in), (value & 0x80) != 0)
}

fn rotate_right_through_carry(value: u8, carry_in: bool) -> Shifted {
    (
        (value >> 1) | (u8::from(carry_in) << 7),
        (value & 0x01) != 0,
    )
}

/// Flags for the accumulator rotate class. Sign, zero and parity survive; the half-carry
/// and add/subtract flags are cleared; the undocumented bits follow the rotated
/// accumulator.
const fn rotate_accumulator_flags(result: u8, carry_out: bool, f: u8) -> u8 {
    (f & (SIGN | ZERO | PARITY_OVERFLOW)) | undocumented(result) | flag(CARRY, carry_out)
}

/// `RLCA` — the accumulator form of [`prefixed::rlc`], with the accumulator flag rule.
pub(crate) fn rlca(a: u8, f: u8) -> (u8, u8) {
    let (result, carry) = rotate_left_circular(a);
    (result, rotate_accumulator_flags(result, carry, f))
}

/// `RRCA` — the accumulator form of [`prefixed::rrc`].
pub(crate) fn rrca(a: u8, f: u8) -> (u8, u8) {
    let (result, carry) = rotate_right_circular(a);
    (result, rotate_accumulator_flags(result, carry, f))
}

/// `RLA` — the accumulator form of [`prefixed::rl`].
pub(crate) fn rla(a: u8, f: u8) -> (u8, u8) {
    let (result, carry) = rotate_left_through_carry(a, (f & CARRY) != 0);
    (result, rotate_accumulator_flags(result, carry, f))
}

/// `RRA` — the accumulator form of [`prefixed::rr`].
pub(crate) fn rra(a: u8, f: u8) -> (u8, u8) {
    let (result, carry) = rotate_right_through_carry(a, (f & CARRY) != 0);
    (result, rotate_accumulator_flags(result, carry, f))
}

/// Flag rules belonging to the `CB`- and `ED`-prefixed instruction sets.
///
/// They live beside the rules they share arithmetic with, rather than beside their callers,
/// because that adjacency is what stops the two drifting: `sbc16` and [`add16`] must agree
/// on where the 16-bit half-carry lives, and `rlc` and [`rlca`] must agree on how a byte
/// rotates. All of them are now wired up — the `CB` rotates, shifts, `SLL`, `BIT`, `RES`
/// and `SET` to both the plain and the `DD`/`FD`-indexed forms, and [`neg`], [`adc16`] and
/// [`sbc16`] to the `ED` decoder.
pub(crate) mod prefixed {
    use super::{
        ADD_SUBTRACT, BIT3, BIT5, BYTE_MAX, CARRY, HALF_CARRY, HALF_CARRY_16, LOW_12_BITS,
        PARITY_OVERFLOW, SIGN, Shifted, WORD_MAX, ZERO, flag, logic_flags, parity,
        rotate_left_circular, rotate_left_through_carry, rotate_right_circular,
        rotate_right_through_carry, sign_and_zero, sub8, undocumented,
    };

    /// Bit 15 of a 16-bit value — the sign bit for the 16-bit arithmetic classes.
    const SIGN_16: u16 = 0x8000;

    /// Carry out of bit 15, seen in the 32-bit intermediate used by 16-bit arithmetic.
    const CARRY_OUT_16: u32 = 0x0001_0000;

    /// Sign, zero and the undocumented pair for a 16-bit result.
    ///
    /// Sign and the undocumented bits come from the high byte — the one the ALU handled
    /// last — while zero considers all sixteen. Written once because `adc16` and `sbc16`
    /// must agree on it; two copies of one rule is how the `DAA` subtract path went wrong.
    const fn sign_zero_undocumented_16(result: u16) -> u8 {
        let [high, _] = result.to_be_bytes();
        (high & SIGN) | flag(ZERO, result == 0) | undocumented(high)
    }

    /// `NEG` — `A := 0 - A`.
    ///
    /// Every flag follows the ordinary subtraction rules, which is why this delegates
    /// rather than restating them: subtracting from zero borrows for any non-zero operand
    /// (so carry ends up set exactly then) and overflows only for `0x80`, whose negation
    /// cannot be represented. Stating those two facts as their own rule would be a second
    /// source of truth that could drift from [`super::sub8`].
    pub(crate) fn neg(a: u8) -> (u8, u8) {
        sub8(0, a)
    }

    /// `ADC HL,ss`. Unlike [`super::add16`], this one defines every flag.
    pub(crate) fn adc16(target: u16, operand: u16, carry_in: bool) -> (u16, u8) {
        let carry = u16::from(u8::from(carry_in));
        let sum = u32::from(target) + u32::from(operand) + u32::from(carry);
        let result = target.wrapping_add(operand).wrapping_add(carry);

        let half_carry =
            (((target & LOW_12_BITS) + (operand & LOW_12_BITS) + carry) & HALF_CARRY_16) != 0;
        let overflow = ((target ^ result) & (operand ^ result) & SIGN_16) != 0;

        let flags = sign_zero_undocumented_16(result)
            | flag(super::HALF_CARRY, half_carry)
            | flag(PARITY_OVERFLOW, overflow)
            | flag(CARRY, sum > WORD_MAX);
        (result, flags)
    }

    /// `SBC HL,ss`.
    pub(crate) fn sbc16(target: u16, operand: u16, carry_in: bool) -> (u16, u8) {
        let carry = u16::from(u8::from(carry_in));
        let difference = u32::from(target)
            .wrapping_sub(u32::from(operand))
            .wrapping_sub(u32::from(carry));
        let result = target.wrapping_sub(operand).wrapping_sub(carry);

        let half_borrow = ((target & LOW_12_BITS)
            .wrapping_sub(operand & LOW_12_BITS)
            .wrapping_sub(carry)
            & HALF_CARRY_16)
            != 0;
        let overflow = ((target ^ operand) & (target ^ result) & SIGN_16) != 0;

        let flags = sign_zero_undocumented_16(result)
            | flag(super::HALF_CARRY, half_borrow)
            | flag(PARITY_OVERFLOW, overflow)
            | ADD_SUBTRACT
            | flag(CARRY, (difference & CARRY_OUT_16) != 0);
        (result, flags)
    }

    fn shift_left(value: u8) -> Shifted {
        (value << 1, (value & 0x80) != 0)
    }

    /// Arithmetic right shift: bit 7 is duplicated, preserving a signed value's sign.
    fn shift_right_arithmetic(value: u8) -> Shifted {
        ((value >> 1) | (value & 0x80), (value & 0x01) != 0)
    }

    fn shift_right_logical(value: u8) -> Shifted {
        (value >> 1, (value & 0x01) != 0)
    }

    /// Flags for the `CB`-prefixed rotate and shift class: a full set, with parity in P/V.
    const fn shift_flags(result: u8, carry_out: bool) -> u8 {
        sign_and_zero(result) | undocumented(result) | parity(result) | flag(CARRY, carry_out)
    }

    /// `RLC r` — rotate left, bit 7 wrapping into bit 0 and into the carry.
    pub(crate) fn rlc(value: u8) -> (u8, u8) {
        let (result, carry) = rotate_left_circular(value);
        (result, shift_flags(result, carry))
    }

    /// `RRC r` — rotate right, bit 0 wrapping into bit 7 and into the carry.
    pub(crate) fn rrc(value: u8) -> (u8, u8) {
        let (result, carry) = rotate_right_circular(value);
        (result, shift_flags(result, carry))
    }

    /// `RL r` — rotate left through the carry.
    pub(crate) fn rl(value: u8, carry_in: bool) -> (u8, u8) {
        let (result, carry) = rotate_left_through_carry(value, carry_in);
        (result, shift_flags(result, carry))
    }

    /// `RR r` — rotate right through the carry.
    pub(crate) fn rr(value: u8, carry_in: bool) -> (u8, u8) {
        let (result, carry) = rotate_right_through_carry(value, carry_in);
        (result, shift_flags(result, carry))
    }

    /// `SLA r` — shift left, zero into bit 0.
    pub(crate) fn sla(value: u8) -> (u8, u8) {
        let (result, carry) = shift_left(value);
        (result, shift_flags(result, carry))
    }

    /// `SRA r` — shift right, preserving bit 7.
    pub(crate) fn sra(value: u8) -> (u8, u8) {
        let (result, carry) = shift_right_arithmetic(value);
        (result, shift_flags(result, carry))
    }

    /// Flags for the operations that simply report a byte: `IN r,(C)`, `RRD` and `RLD`.
    ///
    /// Sign, zero, parity and the undocumented bits come from the byte; the half-carry and
    /// add/subtract flags are cleared; the carry is left alone.
    pub(crate) const fn reported_byte(result: u8, f: u8) -> u8 {
        logic_flags(result) | (f & CARRY)
    }

    /// Flags for `LD A,I` and `LD A,R`.
    ///
    /// P/V reports `IFF2` rather than parity — the one place the interrupt state is
    /// readable by software, and the reason these two instructions are used to sample it.
    pub(crate) const fn load_a_from_interrupt_register(value: u8, iff2: bool, f: u8) -> u8 {
        sign_and_zero(value) | undocumented(value) | flag(PARITY_OVERFLOW, iff2) | (f & CARRY)
    }

    /// Flags for `LDI`/`LDD` and their repeating forms.
    ///
    /// Neither the sign, the zero flag nor the carry is touched: P/V reports whether `BC`
    /// is still non-zero, which is what makes `JP PE` the idiomatic loop back-edge. The
    /// undocumented bits come from `A + transferred`, and **bit 5 of `F` comes from bit 1**
    /// of that sum, not bit 5 — a genuine quirk, not a typo.
    pub(crate) const fn block_transfer(a: u8, transferred: u8, more: bool, f: u8) -> u8 {
        let sum = a.wrapping_add(transferred);
        (f & (SIGN | ZERO | CARRY))
            | flag(PARITY_OVERFLOW, more)
            | flag(BIT3, (sum & 0x08) != 0)
            | flag(BIT5, (sum & 0x02) != 0)
    }

    /// Flags for `CPI`/`CPD` and their repeating forms.
    ///
    /// The comparison sets sign, zero and half-carry; P/V again reports `BC != 0`; the
    /// carry is untouched. The undocumented bits come from the difference *minus the
    /// half-borrow*, with the same bit-1-into-bit-5 quirk as [`block_transfer`].
    pub(crate) fn block_compare(a: u8, value: u8, more: bool, f: u8) -> u8 {
        let (difference, compared) = sub8(a, value);
        let adjusted = difference.wrapping_sub(u8::from((compared & HALF_CARRY) != 0));
        (f & CARRY)
            | (compared & (SIGN | ZERO | HALF_CARRY))
            | ADD_SUBTRACT
            | flag(PARITY_OVERFLOW, more)
            | flag(BIT3, (adjusted & 0x08) != 0)
            | flag(BIT5, (adjusted & 0x02) != 0)
    }

    /// Flags for `INI`/`IND`/`OUTI`/`OUTD` and their repeating forms.
    ///
    /// `counter` is `B` after its decrement — sign, zero and the undocumented bits all come
    /// from it. `index` is the value added to the transferred byte to derive the carry and
    /// half-carry: `C + 1` for `INI`, `C - 1` for `IND`, and `L` for the output forms.
    /// P/V is the parity of the low three bits of that sum exclusive-or'd with the counter.
    /// Every flag is defined, so no incoming `F` is needed.
    pub(crate) fn block_io(counter: u8, value: u8, index: u8) -> u8 {
        let sum = u16::from(value) + u16::from(index);
        let carried = sum > BYTE_MAX;
        let [low, _] = sum.to_le_bytes();
        sign_and_zero(counter)
            | undocumented(counter)
            | flag(ADD_SUBTRACT, (value & SIGN) != 0)
            | flag(HALF_CARRY, carried)
            | flag(CARRY, carried)
            | parity((low & 0x07) ^ counter)
    }

    /// `SLL r` — undocumented: shift left with a **one** into bit 0.
    ///
    /// The Z80's designers left this encoding in the gap where a "shift left logical" would
    /// mirror [`srl`]; it shifts in a one rather than a zero, which is almost certainly not
    /// what anyone intended. Real software uses it, so it is not optional.
    pub(crate) fn sll(value: u8) -> (u8, u8) {
        let result = (value << 1) | 1;
        (result, shift_flags(result, (value & 0x80) != 0))
    }

    /// `BIT n,s` — test one bit, discarding everything but the flags.
    ///
    /// `undocumented_source` supplies bits 3 and 5, and is **not** always the tested value:
    /// for `BIT n,(IX+d)` it is the high byte of the effective address, because that is what
    /// was last on the bus. Corpus vector `ddcb46` separates the two — it expects `F=0x30`,
    /// which is bit 5 of the address `0xa381`, while the tested value `0xd5` has neither
    /// undocumented bit set.
    ///
    /// P/V mirrors Z, and only bit 7 can set the sign flag.
    pub(crate) fn bit(value: u8, bit_index: u8, f: u8, undocumented_source: u8) -> u8 {
        let index = bit_index & 0x07;
        let tested = value & (1 << index);
        (f & CARRY)
            | flag(SIGN, index == 7 && tested != 0)
            | flag(ZERO, tested == 0)
            | flag(PARITY_OVERFLOW, tested == 0)
            | HALF_CARRY
            | undocumented(undocumented_source)
    }

    /// `SRL r` — shift right, zero into bit 7.
    pub(crate) fn srl(value: u8) -> (u8, u8) {
        let (result, carry) = shift_right_logical(value);
        (result, shift_flags(result, carry))
    }
}

#[cfg(test)]
mod tests {
    use super::prefixed::{adc16, neg, rl, rlc, rr, rrc, sbc16, sla, sra, srl};
    use super::*;

    // The un-prefixed rules are exercised through their opcodes in `lib.rs`, which proves
    // both the rule and its wiring. These are exercised directly as well, because a direct
    // test names the rule when it fails, where an opcode-level failure only reports that
    // some instruction came out wrong.
    //
    // Every expectation below is derived from the Z80's documented rule for that class
    // and written out by hand. A test that obtained its expectation by running the
    // implementation would agree with any bug the implementation contains.

    #[test]
    fn adc16_defines_every_flag_unlike_add16() {
        let cases: &[(&str, (u16, u8), u16, u8)] = &[
            ("0 + 0", adc16(0x0000, 0x0000, false), 0x0000, ZERO),
            (
                "carry out of bit 11",
                adc16(0x0FFF, 0x0001, false),
                0x1000,
                HALF_CARRY,
            ),
            (
                "signed overflow",
                adc16(0x7FFF, 0x0001, false),
                0x8000,
                SIGN | HALF_CARRY | PARITY_OVERFLOW,
            ),
            ("incoming carry", adc16(0x0000, 0x0000, true), 0x0001, 0),
            (
                "wrap to zero",
                adc16(0xFFFF, 0x0000, true),
                0x0000,
                ZERO | HALF_CARRY | CARRY,
            ),
        ];

        for &(name, (result, flags), expected_result, expected_flags) in cases {
            assert_eq!(result, expected_result, "{name} result");
            assert_eq!(flags, expected_flags, "{name} flags");
        }
    }

    #[test]
    fn sbc16_borrows_and_reports_the_subtract_flag() {
        let cases: &[(&str, (u16, u8), u16, u8)] = &[
            (
                "0 - 1 borrows",
                sbc16(0x0000, 0x0001, false),
                0xFFFF,
                SIGN | BIT5 | HALF_CARRY | BIT3 | ADD_SUBTRACT | CARRY,
            ),
            (
                "1 - 1 is zero",
                sbc16(0x0001, 0x0001, false),
                0x0000,
                ZERO | ADD_SUBTRACT,
            ),
            (
                "signed overflow crossing the boundary",
                sbc16(0x8000, 0x0001, false),
                0x7FFF,
                BIT5 | HALF_CARRY | BIT3 | PARITY_OVERFLOW | ADD_SUBTRACT,
            ),
        ];

        for &(name, (result, flags), expected_result, expected_flags) in cases {
            assert_eq!(result, expected_result, "{name} result");
            assert_eq!(flags, expected_flags, "{name} flags");
        }
    }

    #[test]
    fn neg_borrows_for_every_non_zero_operand() {
        // Carry set exactly when the operand is non-zero, and overflow only for 0x80 —
        // the one value whose negation cannot be represented in eight signed bits.
        assert_eq!(neg(0x00), (0x00, ZERO | ADD_SUBTRACT));
        assert_eq!(
            neg(0x01),
            (0xFF, SIGN | BIT5 | HALF_CARRY | BIT3 | ADD_SUBTRACT | CARRY)
        );
        assert_eq!(
            neg(0x80),
            (0x80, SIGN | PARITY_OVERFLOW | ADD_SUBTRACT | CARRY)
        );
    }

    #[test]
    fn prefixed_rotates_and_shifts_move_bits_and_report_full_flags() {
        let cases: &[(&str, (u8, u8), u8, u8)] = &[
            ("RLC 0x80", rlc(0x80), 0x01, CARRY),
            ("RLC 0x00", rlc(0x00), 0x00, ZERO | PARITY_OVERFLOW),
            // 0x14 rotates into 0x28, which is exactly the two undocumented bits.
            (
                "RLC 0x14 undocumented bits",
                rlc(0x14),
                0x28,
                BIT5 | BIT3 | PARITY_OVERFLOW,
            ),
            ("RRC 0x01", rrc(0x01), 0x80, SIGN | CARRY),
            (
                "RL 0x80 carry in clear",
                rl(0x80, false),
                0x00,
                ZERO | PARITY_OVERFLOW | CARRY,
            ),
            ("RL 0x80 carry in set", rl(0x80, true), 0x01, CARRY),
            (
                "RR 0x01 carry in clear",
                rr(0x01, false),
                0x00,
                ZERO | PARITY_OVERFLOW | CARRY,
            ),
            ("RR 0x01 carry in set", rr(0x01, true), 0x80, SIGN | CARRY),
            ("SLA 0x80", sla(0x80), 0x00, ZERO | PARITY_OVERFLOW | CARRY),
            ("SRA 0x81", sra(0x81), 0xC0, SIGN | PARITY_OVERFLOW | CARRY),
            ("SRL 0x81", srl(0x81), 0x40, CARRY),
        ];

        for &(name, (result, flags), expected_result, expected_flags) in cases {
            assert_eq!(result, expected_result, "{name} result");
            assert_eq!(flags, expected_flags, "{name} flags");
        }
    }

    #[test]
    fn sra_preserves_the_sign_bit_where_srl_clears_it() {
        // The single difference between the two right shifts, on the one input that shows
        // it: an arithmetic shift keeps a negative number negative.
        assert_eq!(sra(0x80).0, 0xC0);
        assert_eq!(srl(0x80).0, 0x40);
    }

    #[test]
    fn daa_applies_the_same_magnitude_correction_on_both_paths() {
        // Only the direction is chosen by N; the two magnitude conditions are identical.
        // A = 0x9A trips both of them — low nibble 0xA > 9 asks for 0x06, and the byte
        // itself > 0x99 asks for 0x60 and sets the carry.
        //
        // The subtract case is FUSE vector `27_1`, which is what caught the earlier
        // version that gated both conditions on the add path and so corrected nothing.
        assert_eq!(
            daa(0x9A, ADD_SUBTRACT),
            (0x34, BIT5 | ADD_SUBTRACT | CARRY),
            "0x9A - 0x66 = 0x34, no half-borrow out of 0xA - 0x6"
        );

        assert_eq!(
            daa(0x9A, 0),
            (0x00, ZERO | PARITY_OVERFLOW | HALF_CARRY | CARRY),
            "0x9A + 0x66 = 0x100, wrapping to zero and carrying"
        );
    }

    #[test]
    fn prefixed_rotates_set_zero_where_the_accumulator_forms_do_not() {
        // The distinguishing property of the two rotate classes. `RLC A` reports that its
        // result is zero; `RLCA` leaves whatever the zero flag already held, which is what
        // makes it safe inside a loop that is testing something else.
        assert_eq!(rlc(0x00).1 & ZERO, ZERO);
        assert_eq!(rlca(0x00, 0).1 & ZERO, 0);
        assert_eq!(
            rlca(0x00, ZERO).1 & ZERO,
            ZERO,
            "and it preserves a set one"
        );
    }
}
