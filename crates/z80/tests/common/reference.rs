//! An **independent** model of the Z80 ALU flag rules.
//!
//! Independence is the whole point: every value here is computed from the Zilog
//! documented behaviour plus the known undocumented rules, using wide arithmetic and
//! explicit bit tests. Nothing in this file calls the CPU core. A property test that
//! checked `core::add8` against `core::add8` would pass for any pair of matching bugs.
//!
//! The module also owns the opcode for each operation, so the property test can drive
//! the real instruction through the real decoder instead of reaching into a private
//! helper — which keeps the core's public API frozen and additionally proves the
//! opcode is wired to the right flag helper.

use super::flags;

/// The value written back to the accumulator, and the resulting `F` byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AluOutcome {
    pub result: u8,
    pub flags: u8,
}

const fn flag_if(mask: u8, condition: bool) -> u8 {
    if condition { mask } else { 0 }
}

/// `S`, `Z`, and the undocumented bits 3 and 5 — which for most operations are simply
/// copied out of the result, so masking the result *is* the rule.
fn sign_zero_undocumented(result: u8) -> u8 {
    flag_if(flags::S, result & 0x80 != 0)
        | flag_if(flags::Z, result == 0)
        | (result & flags::UNDOCUMENTED)
}

/// `A + operand (+ carry)`.
pub fn add(a: u8, operand: u8, carry_in: bool) -> AluOutcome {
    let carry = u16::from(carry_in);
    let wide = u16::from(a) + u16::from(operand) + carry;
    let result = wide as u8;
    let half = (a & 0x0f) as u16 + (operand & 0x0f) as u16 + carry;
    AluOutcome {
        result,
        flags: sign_zero_undocumented(result)
            | flag_if(flags::H, half > 0x0f)
            // Signed overflow: the operands agreed in sign and the result disagrees.
            | flag_if(
                flags::PV,
                (a ^ operand) & 0x80 == 0 && (a ^ result) & 0x80 != 0,
            )
            | flag_if(flags::C, wide > 0xff),
    }
}

/// `A - operand (- borrow)`. Shared by `SUB`, `SBC` and `CP`.
fn subtract(a: u8, operand: u8, borrow_in: bool) -> (u8, u8) {
    let borrow = i16::from(borrow_in);
    let wide = i16::from(a) - i16::from(operand) - borrow;
    let result = wide as u8;
    let half = i16::from(a & 0x0f) - i16::from(operand & 0x0f) - borrow;
    let flags = flags::N
        | flag_if(flags::H, half < 0)
        // Signed overflow: the operands disagreed in sign and the result disagrees
        // with the minuend.
        | flag_if(
            flags::PV,
            (a ^ operand) & 0x80 != 0 && (a ^ result) & 0x80 != 0,
        )
        | flag_if(flags::C, wide < 0);
    (result, flags)
}

/// `A - operand (- borrow)`, written back to `A`.
pub fn sub(a: u8, operand: u8, borrow_in: bool) -> AluOutcome {
    let (result, flags) = subtract(a, operand, borrow_in);
    AluOutcome {
        result,
        flags: flags | sign_zero_undocumented(result),
    }
}

/// `CP` — a subtraction whose result is discarded, so `A` survives unchanged.
///
/// The trap: bits 3 and 5 come from the **operand**, not from the result. Every other
/// arithmetic instruction takes them from the result, which is precisely why an
/// emulator that shares one flag helper between `SUB` and `CP` fails zexall here.
pub fn cp(a: u8, operand: u8) -> AluOutcome {
    let (result, flags) = subtract(a, operand, false);
    AluOutcome {
        result: a,
        flags: flags
            | flag_if(flags::S, result & 0x80 != 0)
            | flag_if(flags::Z, result == 0)
            | (operand & flags::UNDOCUMENTED),
    }
}

/// `AND` — the only logic operation that sets `H`.
pub fn and(a: u8, operand: u8) -> AluOutcome {
    let result = a & operand;
    AluOutcome {
        result,
        flags: sign_zero_undocumented(result)
            | flags::H
            | flag_if(flags::PV, flags::even_parity(result)),
    }
}

/// `OR` / `XOR` — `H`, `N` and `C` all cleared, `P/V` is parity.
fn logic_without_half_carry(result: u8) -> AluOutcome {
    AluOutcome {
        result,
        flags: sign_zero_undocumented(result) | flag_if(flags::PV, flags::even_parity(result)),
    }
}

pub fn or(a: u8, operand: u8) -> AluOutcome {
    logic_without_half_carry(a | operand)
}

pub fn xor(a: u8, operand: u8) -> AluOutcome {
    logic_without_half_carry(a ^ operand)
}

/// `INC r` — note `C` is **preserved**, not computed.
pub fn inc(value: u8, carry_in: bool) -> AluOutcome {
    let result = value.wrapping_add(1);
    AluOutcome {
        result,
        flags: sign_zero_undocumented(result)
            | flag_if(flags::H, value & 0x0f == 0x0f)
            | flag_if(flags::PV, value == 0x7f)
            | flag_if(flags::C, carry_in),
    }
}

/// `DEC r` — `C` preserved, `N` set.
pub fn dec(value: u8, carry_in: bool) -> AluOutcome {
    let result = value.wrapping_sub(1);
    AluOutcome {
        result,
        flags: sign_zero_undocumented(result)
            | flags::N
            | flag_if(flags::H, value & 0x0f == 0)
            | flag_if(flags::PV, value == 0x80)
            | flag_if(flags::C, carry_in),
    }
}

/// The eight accumulator/operand ALU operations, in opcode order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binary {
    Add,
    Adc,
    Sub,
    Sbc,
    And,
    Xor,
    Or,
    Cp,
}

impl Binary {
    pub const ALL: [Self; 8] = [
        Self::Add,
        Self::Adc,
        Self::Sub,
        Self::Sbc,
        Self::And,
        Self::Xor,
        Self::Or,
        Self::Cp,
    ];

    /// The opcode of this operation's `B`-register form (`ADD A,B` = `0x80`, and so on
    /// in the regular `10 ooo rrr` encoding with `rrr = 000` for `B`).
    pub fn opcode_with_b(self) -> u8 {
        let operation = match self {
            Self::Add => 0,
            Self::Adc => 1,
            Self::Sub => 2,
            Self::Sbc => 3,
            Self::And => 4,
            Self::Xor => 5,
            Self::Or => 6,
            Self::Cp => 7,
        };
        0x80 | (operation << 3)
    }

    /// The independently-derived outcome. `carry_in` is the `C` flag on entry, which
    /// only `ADC` and `SBC` consume.
    pub fn apply(self, a: u8, operand: u8, carry_in: bool) -> AluOutcome {
        match self {
            Self::Add => add(a, operand, false),
            Self::Adc => add(a, operand, carry_in),
            Self::Sub => sub(a, operand, false),
            Self::Sbc => sub(a, operand, carry_in),
            Self::And => and(a, operand),
            Self::Xor => xor(a, operand),
            Self::Or => or(a, operand),
            Self::Cp => cp(a, operand),
        }
    }

    pub fn mnemonic(self) -> &'static str {
        match self {
            Self::Add => "ADD A,B",
            Self::Adc => "ADC A,B",
            Self::Sub => "SUB B",
            Self::Sbc => "SBC A,B",
            Self::And => "AND B",
            Self::Xor => "XOR B",
            Self::Or => "OR B",
            Self::Cp => "CP B",
        }
    }
}

/// The two single-operand ALU operations that touch flags without touching `C`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unary {
    Inc,
    Dec,
}

impl Unary {
    pub const ALL: [Self; 2] = [Self::Inc, Self::Dec];

    /// The opcode of this operation's `B`-register form.
    pub fn opcode_on_b(self) -> u8 {
        match self {
            Self::Inc => 0x04,
            Self::Dec => 0x05,
        }
    }

    pub fn apply(self, value: u8, carry_in: bool) -> AluOutcome {
        match self {
            Self::Inc => inc(value, carry_in),
            Self::Dec => dec(value, carry_in),
        }
    }

    pub fn mnemonic(self) -> &'static str {
        match self {
            Self::Inc => "INC B",
            Self::Dec => "DEC B",
        }
    }
}
