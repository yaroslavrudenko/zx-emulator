//! Property tests for the ALU flag helpers.
//!
//! Two design decisions matter here.
//!
//! **The expectation is computed independently.** `common::reference` derives every flag
//! from the Zilog rules using wide arithmetic and explicit bit tests. It never calls the
//! core. Checking `core::add8` against `core::add8` would agree on any pair of matching
//! bugs, which is the failure mode this test exists to rule out.
//!
//! **The core is driven through real opcodes, not through its internals.** `ADD A,B` is
//! executed as the byte `0x80`, so the test needs no access to private helpers, adds
//! nothing to the crate's frozen public API, and additionally proves each opcode is wired
//! to the flag helper it is supposed to use — a class of bug an internal unit test on
//! `add8` alone cannot see.

mod common;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

use common::flags;
use common::machine::{Completion, Machine};
use common::reference::{Binary, Unary};
use common::vectors::{MemoryBlock, Registers, Setup, State};

/// Values chosen to sit on every boundary the Z80 flag rules care about: zero, one, the
/// nibble edge, the signed edge, and saturation.
const BOUNDARY_OPERANDS: [u8; 8] = [0x00, 0x01, 0x0f, 0x10, 0x7f, 0x80, 0x81, 0xff];

/// All flags clear and all flags set — so every test also proves the instruction
/// overwrites the bits it owns and preserves the ones it does not.
const BOUNDARY_FLAGS: [u8; 2] = [0x00, 0xff];

/// The address the one-instruction programs are assembled at.
const PROGRAM_START: u16 = 0x0000;

// ---------------------------------------------------------------------------
// Deterministic boundary coverage
// ---------------------------------------------------------------------------

#[test]
fn binary_alu_flags_match_the_reference_on_every_boundary() {
    for op in Binary::ALL {
        for a in BOUNDARY_OPERANDS {
            for operand in BOUNDARY_OPERANDS {
                for entry_flags in BOUNDARY_FLAGS {
                    if let Err(message) = compare_binary(op, a, operand, entry_flags) {
                        panic!("{message}");
                    }
                }
            }
        }
    }
}

#[test]
fn unary_alu_flags_match_the_reference_on_every_boundary() {
    for op in Unary::ALL {
        for value in BOUNDARY_OPERANDS {
            for entry_flags in BOUNDARY_FLAGS {
                if let Err(message) = compare_unary(op, value, entry_flags) {
                    panic!("{message}");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Random coverage
// ---------------------------------------------------------------------------

proptest! {
    /// `ADD` / `ADC` / `SUB` / `SBC` / `AND` / `XOR` / `OR` / `CP` over random operand
    /// pairs and a random entry `F`, covering carry, half-carry, overflow, sign, zero and
    /// the undocumented bits 3 and 5 in one sweep.
    #[test]
    fn binary_alu_flags_match_the_reference(
        a in any::<u8>(),
        operand in any::<u8>(),
        entry_flags in any::<u8>(),
    ) {
        for op in Binary::ALL {
            compare_binary(op, a, operand, entry_flags).map_err(TestCaseError::fail)?;
        }
    }

    /// `INC` / `DEC` — the operations whose defining trap is that they must leave `C`
    /// exactly as they found it while recomputing everything else.
    #[test]
    fn unary_alu_flags_match_the_reference(
        value in any::<u8>(),
        entry_flags in any::<u8>(),
    ) {
        for op in Unary::ALL {
            compare_unary(op, value, entry_flags).map_err(TestCaseError::fail)?;
        }
    }
}

// ---------------------------------------------------------------------------
// Execution + comparison
// ---------------------------------------------------------------------------

fn compare_binary(op: Binary, a: u8, operand: u8, entry_flags: u8) -> Result<(), String> {
    let carry_in = entry_flags & flags::C != 0;
    let expected = op.apply(a, operand, carry_in);
    let setup = single_instruction(op.mnemonic(), op.opcode_with_b(), a, entry_flags, operand);
    let actual = execute(&setup)?;

    verdict(
        &format!(
            "{} with A={a:02x} B={operand:02x} entry F={entry_flags:02x}",
            op.mnemonic()
        ),
        expected.result,
        expected.flags,
        actual,
    )
}

fn compare_unary(op: Unary, value: u8, entry_flags: u8) -> Result<(), String> {
    let carry_in = entry_flags & flags::C != 0;
    let expected = op.apply(value, carry_in);
    // The accumulator is untouched by `INC B` / `DEC B`, so the result to check is the
    // B register — read back below out of `BC`.
    let setup = single_instruction(op.mnemonic(), op.opcode_on_b(), 0x00, entry_flags, value);
    let mut machine = Machine::load(&setup);
    if machine.run(setup.state.t_states) == Completion::StepLimit {
        return Err(step_limit_message(op.mnemonic()));
    }
    let (registers, _) = machine.snapshot();

    verdict(
        &format!(
            "{} with B={value:02x} entry F={entry_flags:02x}",
            op.mnemonic()
        ),
        expected.result,
        expected.flags,
        ((registers.bc >> 8) as u8, registers.f()),
    )
}

/// Assemble a one-instruction program at [`PROGRAM_START`] with `A`, `F` and `B` preset.
fn single_instruction(name: &str, opcode: u8, a: u8, entry_flags: u8, b: u8) -> Setup {
    Setup {
        name: name.to_owned(),
        registers: Registers {
            af: (u16::from(a) << 8) | u16::from(entry_flags),
            bc: u16::from(b) << 8,
            pc: PROGRAM_START,
            ..Registers::default()
        },
        // A budget of one T-state means "run a single instruction": every real
        // instruction costs at least four, so the loop stops after exactly one step.
        state: State {
            t_states: 1,
            ..State::default()
        },
        memory: vec![MemoryBlock {
            start: PROGRAM_START,
            bytes: vec![opcode],
        }],
    }
}

/// Run the program and read back `(A, F)`.
fn execute(setup: &Setup) -> Result<(u8, u8), String> {
    let mut machine = Machine::load(setup);
    if machine.run(setup.state.t_states) == Completion::StepLimit {
        return Err(step_limit_message(&setup.name));
    }
    let (registers, _) = machine.snapshot();
    Ok((registers.a(), registers.f()))
}

fn step_limit_message(what: &str) -> String {
    format!(
        "{what}: the run loop hit its step limit without the core reporting any T-states — \
         Bus::tick is probably never called"
    )
}

/// Compare a result byte and an `F` byte, naming the individual flags that moved.
fn verdict(
    context: &str,
    expected_result: u8,
    expected_flags: u8,
    actual: (u8, u8),
) -> Result<(), String> {
    let (actual_result, actual_flags) = actual;
    if expected_result == actual_result && expected_flags == actual_flags {
        return Ok(());
    }
    Err(format!(
        "{context}\n  \
         result: expected {expected_result:02x}, got {actual_result:02x}\n  \
         F:      expected {expected_flags:02x}, got {actual_flags:02x}\n    \
         expected {}\n    \
         actual   {}\n    \
         differing flags: {}",
        flags::describe(expected_flags),
        flags::describe(actual_flags),
        {
            let differing = flags::differences(expected_flags, actual_flags);
            if differing.is_empty() {
                String::from("none (only the result byte differs)")
            } else {
                differing.join(", ")
            }
        },
    ))
}
