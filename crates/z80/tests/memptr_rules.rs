//! `MEMPTR`/`WZ`, one rule at a time, asserted on all sixteen bits.
//!
//! # Why this exists alongside the exerciser
//!
//! `crates/spectrum/tests/memptr_oracle.rs` runs Patrik Rak's `z80memptr` on the whole machine
//! and reports 160 groups. It is the stronger evidence — an expectation written by somebody
//! else, against real silicon — and it is **not sufficient**, for two reasons it states about
//! itself:
//!
//! 1. **It is a CRC oracle, so it localises to a group and no further.** `LD (NN),A` failing
//!    says the folded result over that group is wrong. It cannot say whether the low byte
//!    carried when it should not have, or the high byte took the wrong register.
//! 2. **It observes `MEMPTR` through two bits.** The register reaches software only through bits
//!    3 and 5 of `BIT n,(HL)`, which are bits 11 and 13 of the latch. Every assertion in this
//!    file reads all sixteen.
//!
//! There is a third reason, and the exerciser demonstrated it rather than merely allowing it:
//! `113 JP (HL)` and `114 JP (XY)` were in its failing set while being two of the three
//! encodings that correctly take **no** rule at all. They failed on the rules of the
//! instructions used to set them up, and came green with no change to `jump_to_pair`. A group's
//! verdict is not a report on its own rule.
//!
//! So: that file proves the rules add up to what a real Z80 does; this one says which rule is
//! which. Neither replaces the other, and a rule nobody thought to write is invisible to both.
//!
//! # That division of labour is measured, not assumed
//!
//! One mutation makes it concrete. Replacing the accumulator-store quirk with the plain
//! `address + 1` rule — three instructions made wrong in one line — produces:
//!
//! | instrument | result |
//! |---|---|
//! | this file | **4 tests red**, naming `LD (nn),A`, `LD (DE),A`, `OUT (n),A` and the asymmetry |
//! | the exerciser | **2 groups red**: `141 LD ([BC,DE]),A` and `143 LD (NN),A` |
//!
//! **`104 OUT (N),A` passes the exerciser with its rule broken.** Not because the exerciser is
//! poorly built, but because it reads two bits of the latch and folds them: a wrong value whose
//! bits 11 and 13 survive the group's cases is a wrong value it cannot see. That is the caveat
//! its own `FAILING_GROUPS` comment states, demonstrated rather than argued — and it is the
//! whole reason this file asserts sixteen bits per rule instead of trusting a green run.
//!
//! # Where the rules come from
//!
//! Boo-boo and Vladimir Kladov, *MEMPTR, esoteric register of the ZiLOG Z80 CPU* (zx.pk.ru,
//! 2006) — measurements of real parts, not a specification; Zilog documents none of this. Each
//! test below quotes the formula it pins. One rule is **not** from that document and contradicts
//! it: see [`the_io_block_repeat_takes_the_instruction_address_against_the_2006_document`].
//!
//! # How each test is built to be able to fail
//!
//! Two habits, both because a `MEMPTR` assertion is unusually easy to pass by accident:
//!
//! - **The latch is seeded with [`POISON`] first.** Otherwise *"the handler wrote the right
//!   address"* and *"the handler wrote nothing"* are the same observation whenever the right
//!   answer happens to be zero.
//! - **Operands are chosen so the plausible wrong answers are all distinct.** Every address
//!   crosses a byte boundary, so an implementation that let a carry propagate where it must not
//!   — or truncated one where it must — lands somewhere this file names. Each assertion message
//!   states the value the specific wrong rule would have produced.

mod common;

use common::machine::Machine;
use common::vectors::{MemoryBlock, Registers, Setup, State};

/// Where every program below is assembled.
///
/// Not zero: several rules are *"the instruction's own address plus one"*, and at zero that is
/// `0x0001`, which is too easy a number to arrive at by accident.
const ORIGIN: u16 = 0x4321;

/// The value `MEMPTR` holds before each instruction runs.
///
/// No rule under test can produce it, so an assertion that sees it has caught a handler that
/// wrote nothing — which for most of these instructions is exactly the defect being guarded
/// against, since the whole register was unimplemented until recently.
const POISON: u16 = 0xA5C3;

/// Where the tests whose operand arrives from memory rather than from the instruction stream
/// put their stack, clear of [`ORIGIN`] and of every target below.
const STACK: u16 = 0x8000;

/// The program image every test starts from: `bytes` assembled at [`ORIGIN`], with `PC` there.
fn program(bytes: &[u8], registers: Registers) -> Setup {
    Setup {
        name: String::from("memptr_rules"),
        registers: Registers {
            pc: ORIGIN,
            ..registers
        },
        state: State::default(),
        memory: vec![MemoryBlock {
            start: ORIGIN,
            bytes: bytes.to_vec(),
        }],
    }
}

/// Load a setup and seed [`POISON`].
///
/// The one place in this file the latch is primed, so a test cannot be written that forgets
/// to — and against a zeroed latch half these assertions would pass on a handler that wrote
/// nothing at all.
fn poisoned(setup: Setup) -> Machine {
    let mut machine = Machine::load(&setup);
    machine.set_memptr(POISON);
    machine
}

/// Assemble `bytes` at [`ORIGIN`] with the given registers, and seed [`POISON`].
fn machine_with(bytes: &[u8], registers: Registers) -> Machine {
    poisoned(program(bytes, registers))
}

/// [`machine_with`], with `stacked` sitting at [`STACK`] and `SP` pointing at it.
///
/// The returns and `EX (SP),HL` take their operand off the stack, so the value they latch is
/// one no part of their encoding could have produced — which is the whole reason those rules
/// are worth asserting separately from the jumps'. `SP` is set here rather than by each caller
/// because a stacked word and a stack pointer that disagree would make every one of those
/// assertions vacuous.
fn machine_with_stack(bytes: &[u8], registers: Registers, stacked: u16) -> Machine {
    let mut setup = program(
        bytes,
        Registers {
            sp: STACK,
            ..registers
        },
    );
    setup.memory.push(MemoryBlock {
        start: STACK,
        bytes: stacked.to_le_bytes().to_vec(),
    });
    poisoned(setup)
}

/// Assemble `bytes`, run one instruction, and report `MEMPTR`.
fn memptr_after(bytes: &[u8], registers: Registers) -> u16 {
    let mut machine = machine_with(bytes, registers);
    machine.step();
    machine.memptr()
}

// ---------------------------------------------------------------------------------------
// The accumulator quirk: three instructions, one formula, and it is not `address + 1`
// ---------------------------------------------------------------------------------------

/// The address the three store rules use, chosen to sit on a byte boundary.
///
/// `0x40FF + 1` is `0x4100`, so a rule that carried into the high byte and a rule that truncated
/// land two `MEMPTR` bytes apart rather than one bit apart.
const STORE_ADDRESS: u16 = 0x40FF;

/// The accumulator, which the quirk puts in `MEMPTR`'s **high** byte.
///
/// Distinct from both halves of [`STORE_ADDRESS`], so "high byte took `A`" cannot be confused
/// with "high byte kept the address's own".
const STORE_A: u8 = 0x7E;

/// What all three accumulator stores must produce: `A` over the truncated low byte.
const STORE_MEMPTR: u16 = 0x7E00;

#[test]
fn ld_nn_a_puts_the_accumulator_in_the_high_byte_and_truncates_the_low() {
    // "LD (addr),A — MEMPTR_low = (addr + 1) & #FF, MEMPTR_hi = A"
    let mut program = vec![0x32]; // LD (nn),A
    program.extend(STORE_ADDRESS.to_le_bytes());

    let memptr = memptr_after(
        &program,
        Registers {
            af: u16::from(STORE_A) << 8,
            ..Registers::default()
        },
    );

    assert_eq!(
        STORE_MEMPTR,
        memptr,
        "LD (nn),A must leave A over the truncated low byte. {:#06X} would be the plain \
         `address + 1` rule the loads take, and {:#06X} would be that rule with the carry \
         suppressed but the address's own high byte kept",
        STORE_ADDRESS.wrapping_add(1),
        STORE_ADDRESS & 0xFF00,
    );
}

#[test]
fn ld_de_a_takes_the_same_quirk_through_a_register_pair() {
    // "LD (rp),A — MEMPTR_low = (rp + 1) & #FF, MEMPTR_hi = A"
    let memptr = memptr_after(
        &[0x12], // LD (DE),A
        Registers {
            af: u16::from(STORE_A) << 8,
            de: STORE_ADDRESS,
            ..Registers::default()
        },
    );

    assert_eq!(
        STORE_MEMPTR, memptr,
        "LD (DE),A shares its rule verbatim with LD (nn),A — same formula, different \
         addressing mode"
    );
}

#[test]
fn out_n_a_takes_the_same_quirk_through_a_port() {
    // "OUT (port),A — MEMPTR_low = (port + 1) & #FF, MEMPTR_hi = A". The port's high half is
    // already A, so only the low byte's truncation is under test here — which is why the two
    // tests above carry the high-byte half of the claim.
    let memptr = memptr_after(
        &[0xD3, 0xFF], // OUT (0xFF),A
        Registers {
            af: u16::from(STORE_A) << 8,
            ..Registers::default()
        },
    );

    assert_eq!(
        STORE_MEMPTR,
        memptr,
        "OUT (n),A must not carry out of the low byte; {:#06X} is what carrying would give",
        (u16::from(STORE_A) << 8 | 0xFF).wrapping_add(1),
    );
}

#[test]
fn the_loads_carry_where_the_stores_truncate() {
    // The asymmetry the three tests above exist to pin, stated as one comparison: identical
    // addressing, opposite direction, different rule. If a future refactor gave both directions
    // one helper, this is the assertion that would go red.
    let load = memptr_after(
        &[0x1A], // LD A,(DE)
        Registers {
            de: STORE_ADDRESS,
            ..Registers::default()
        },
    );
    let store = memptr_after(
        &[0x12], // LD (DE),A
        Registers {
            af: u16::from(STORE_A) << 8,
            de: STORE_ADDRESS,
            ..Registers::default()
        },
    );

    assert_eq!(
        STORE_ADDRESS.wrapping_add(1),
        load,
        "LD A,(DE) takes the ordinary sixteen-bit increment, carry and all"
    );
    assert_ne!(
        load, store,
        "the load and the store of one address must not agree: {load:#06X} against \
         {store:#06X} is the whole quirk"
    );
}

#[test]
fn in_a_n_carries_into_the_high_byte_unlike_its_mirror_image() {
    // "IN A,(port) — MEMPTR = (A_before_operation << 8) + port + 1", a full sixteen-bit add.
    // `OUT (n),A` with the identical operands truncates; this is the pair that makes the
    // asymmetry impossible to read as a transcription slip.
    let memptr = memptr_after(
        &[0xDB, 0xFF], // IN A,(0xFF)
        Registers {
            af: u16::from(STORE_A) << 8,
            ..Registers::default()
        },
    );

    assert_eq!(
        0x7F00,
        memptr,
        "IN A,(n) forms port {:#06X} and adds one across the byte boundary",
        u16::from(STORE_A) << 8 | 0xFF,
    );
}

// ---------------------------------------------------------------------------------------
// Reads and writes that take the plain `address + 1`
// ---------------------------------------------------------------------------------------

/// An address whose increment crosses into the next page, so a truncated rule is visible.
const PLAIN_ADDRESS: u16 = 0x50FF;

#[test]
fn ld_a_nn_takes_the_address_plus_one() {
    let mut program = vec![0x3A]; // LD A,(nn)
    program.extend(PLAIN_ADDRESS.to_le_bytes());
    assert_eq!(0x5100, memptr_after(&program, Registers::default()));
}

#[test]
fn the_pair_load_and_store_both_take_the_address_plus_one() {
    // "LD (addr),rp / LD rp,(addr) — MEMPTR = addr + 1". The pair *stores* take the plain rule
    // even though they are stores: the document gives both directions one line, and separates
    // `LD (addr),A` onto its own. This is the test that stops the accumulator quirk being
    // applied to the wrong half of the instruction set.
    let mut load = vec![0x2A]; // LD HL,(nn)
    load.extend(PLAIN_ADDRESS.to_le_bytes());
    let mut store = vec![0x22]; // LD (nn),HL
    store.extend(PLAIN_ADDRESS.to_le_bytes());

    let registers = Registers {
        af: 0x7E00,
        hl: 0x1234,
        ..Registers::default()
    };

    assert_eq!(
        0x5100,
        memptr_after(&load, registers),
        "LD HL,(nn) takes addr + 1"
    );
    assert_eq!(
        0x5100,
        memptr_after(&store, registers),
        "LD (nn),HL takes addr + 1 and NOT the accumulator quirk, which would give {:#06X}",
        0x7E00_u16,
    );
}

#[test]
fn rld_takes_hl_plus_one() {
    // "RLD/RRD — MEMPTR = HL + 1". Both share one handler, so one of them pins the rule.
    let memptr = memptr_after(
        &[0xED, 0x6F], // RLD
        Registers {
            hl: PLAIN_ADDRESS,
            ..Registers::default()
        },
    );
    assert_eq!(0x5100, memptr);
}

// ---------------------------------------------------------------------------------------
// 16-bit arithmetic: the destination *before* the operation
// ---------------------------------------------------------------------------------------

#[test]
fn add_hl_rr_takes_the_destination_before_the_addition() {
    // "ADD/ADC/SBC rp1,rp2 — MEMPTR = rp1_before_operation + 1". The operands are chosen so
    // that the *result* is a different number from the augend: HL is 0x1000 and becomes
    // 0x3000, so a handler reading the destination one line too late is caught.
    let memptr = memptr_after(
        &[0x09], // ADD HL,BC
        Registers {
            hl: 0x1000,
            bc: 0x2000,
            ..Registers::default()
        },
    );

    assert_eq!(
        0x1001, memptr,
        "the latch takes HL as it was, plus one. 0x3001 would be the sum plus one — the same \
         code with the write moved after the addition"
    );
}

#[test]
fn add_ix_rr_takes_ix_because_the_prefix_moves_the_destination() {
    // The rule names `rp1`, and under a DD prefix `rp1` is IX. HL is loaded with a different
    // value, so an implementation that hard-coded HL produces 0x1001 and is caught.
    let memptr = memptr_after(
        &[0xDD, 0x09], // ADD IX,BC
        Registers {
            hl: 0x1000,
            ix: 0x8000,
            bc: 0x2000,
            ..Registers::default()
        },
    );

    assert_eq!(
        0x8001, memptr,
        "ADD IX,BC latches IX + 1, not HL + 1 (0x1001)"
    );
}

/// `F` with the carry flag set, so `ADC` takes a third operand into its sum.
const CARRY_SET: u16 = 0x0001;

#[test]
fn adc_hl_rr_takes_the_destination_before_the_addition() {
    // The third encoding of the one arithmetic rule, and it had no assertion of its own while
    // `ADD` and `SBC` each had one: "ADD/ADC/SBC rp1,rp2 — MEMPTR = rp1_before_operation + 1".
    // `ED` ignores an index prefix, so `rp1` here is always HL — unlike `Cpu::add_pair`,
    // whose destination the prefix does move.
    //
    // The carry is set so the sum lands further from the augend than a plain `ADD` would put
    // it, and the operands are chosen so the three candidate answers are three numbers.
    let memptr = memptr_after(
        &[0xED, 0x4A], // ADC HL,BC
        Registers {
            af: CARRY_SET,
            hl: 0x2000,
            bc: 0x3010,
            ..Registers::default()
        },
    );

    assert_eq!(
        0x2001, memptr,
        "the latch takes HL as it was, plus one. 0x5011 would be the sum plus one — the same \
         code with the write moved after the addition — and 0x3011 the addend's"
    );
}

#[test]
fn sbc_hl_rr_takes_the_minuend_and_not_the_difference() {
    // Subtraction is no exception to the one arithmetic rule: the latch takes `rp1` before the
    // operation, so the difference never reaches it.
    let memptr = memptr_after(
        &[0xED, 0x42], // SBC HL,BC
        Registers {
            hl: 0x9000,
            bc: 0x1000,
            ..Registers::default()
        },
    );

    assert_eq!(
        0x9001, memptr,
        "0x8001 would be the difference plus one, which is what taking the result gives"
    );
}

// ---------------------------------------------------------------------------------------
// Branches: the conditional split that is the point of the whole group
// ---------------------------------------------------------------------------------------

/// Where the conditional branches point, clear of [`ORIGIN`].
const BRANCH_TARGET: u16 = 0x1234;

/// `F` with the zero flag set, so `Z` conditions hold and `NZ` conditions do not.
const ZERO_SET: u16 = 0x0040;

#[test]
fn jp_cc_loads_the_target_even_when_the_branch_is_not_taken() {
    // "JP (except JP rp)/CALL addr (even in case of conditional call/jp, independantly on
    // condition satisfied or not) — MEMPTR = addr". The latch follows the operand fetch, not
    // the branch, and this is the *not taken* case — the one that distinguishes the rule.
    let mut program = vec![0xC2]; // JP NZ,nn
    program.extend(BRANCH_TARGET.to_le_bytes());

    let mut machine = machine_with(
        &program,
        Registers {
            af: ZERO_SET, // Z is set, so NZ fails
            ..Registers::default()
        },
    );
    machine.step();

    assert_eq!(
        ORIGIN + 3,
        machine.snapshot().0.pc,
        "precondition failed: the branch was taken, so this proves nothing about the \
         not-taken path"
    );
    assert_eq!(
        BRANCH_TARGET,
        machine.memptr(),
        "a JP cc that does not jump still latches its operand"
    );
}

#[test]
fn call_cc_loads_the_target_even_when_the_branch_is_not_taken() {
    // The same rule as above and worth its own case: unlike `JP cc`, `CALL cc` genuinely does
    // lose machine cycles when not taken, so it is the encoding where a handler is most likely
    // to put the latch inside the `if` along with the push.
    let mut program = vec![0xC4]; // CALL NZ,nn
    program.extend(BRANCH_TARGET.to_le_bytes());

    let mut machine = machine_with(
        &program,
        Registers {
            af: ZERO_SET,
            sp: 0xFF00,
            ..Registers::default()
        },
    );
    machine.step();

    assert_eq!(
        ORIGIN + 3,
        machine.snapshot().0.pc,
        "precondition failed: the call was taken"
    );
    assert_eq!(BRANCH_TARGET, machine.memptr());
}

#[test]
fn jr_cc_leaves_the_latch_alone_when_the_branch_is_not_taken() {
    // The other half of the split, and the reason the document's two lines are worded
    // differently: "JR/DJNZ/RET/RETI/RST (**jumping to** addr)" against `JP`'s "independantly
    // on condition satisfied or not". A relative jump has no absolute operand — there is
    // nothing to latch until the addition happens, and it does not happen.
    let mut machine = machine_with(
        &[0x20, 0x10], // JR NZ,+0x10
        Registers {
            af: ZERO_SET,
            ..Registers::default()
        },
    );
    machine.step();

    assert_eq!(
        ORIGIN + 2,
        machine.snapshot().0.pc,
        "precondition failed: the branch was taken"
    );
    assert_eq!(
        POISON,
        machine.memptr(),
        "an untaken JR cc must leave MEMPTR untouched — this is the one case that \
         distinguishes the relative jumps from JP cc"
    );
}

#[test]
fn jr_cc_loads_the_destination_when_the_branch_is_taken() {
    let mut machine = machine_with(
        &[0x28, 0x10], // JR Z,+0x10
        Registers {
            af: ZERO_SET,
            ..Registers::default()
        },
    );
    machine.step();

    let destination = ORIGIN + 2 + 0x10;
    assert_eq!(destination, machine.snapshot().0.pc);
    assert_eq!(
        destination,
        machine.memptr(),
        "a taken JR latches where it lands, not its displacement byte"
    );
}

#[test]
fn ret_cc_leaves_the_latch_alone_when_the_branch_is_not_taken() {
    // Here the *reason* needs no appeal to wording: an untaken `RET cc` performs no stack read
    // at all, so the instruction contains no address for the latch to hold.
    let mut machine = machine_with(
        &[0xC0], // RET NZ
        Registers {
            af: ZERO_SET,
            sp: 0xFF00,
            ..Registers::default()
        },
    );
    machine.step();

    assert_eq!(POISON, machine.memptr());
}

/// The address the return family finds on the stack.
///
/// Unrelated to [`STACK`] in both bytes, so *"latched the address it popped from"* and
/// *"latched `SP` afterwards"* are each a different number from the rule — and unrelated to
/// [`ORIGIN`], so *"latched its own instruction address"* is too.
const RETURN_TARGET: u16 = 0x2F8D;

#[test]
fn ret_latches_the_address_it_returns_to() {
    // "JR/DJNZ/RET/RETI/RST (jumping to addr) — MEMPTR = addr". `RET`'s address comes off the
    // stack rather than out of its own encoding, which is what makes this assertion possible
    // at all: a one-byte instruction contains no operand a wrong rule could have latched
    // instead, so every wrong answer is a *different* number rather than a coincidence.
    let mut machine = machine_with_stack(&[0xC9], Registers::default(), RETURN_TARGET);
    machine.step();

    assert_eq!(
        RETURN_TARGET,
        machine.snapshot().0.pc,
        "precondition failed: the return did not happen, so this proves nothing"
    );
    assert_eq!(
        RETURN_TARGET,
        machine.memptr(),
        "RET latches where it returns to. {STACK:#06X} would be the address it popped from, \
         {:#06X} the stack pointer afterwards, and {POISON:#06X} a handler that wrote nothing",
        STACK.wrapping_add(2),
    );
}

#[test]
fn ret_cc_latches_its_target_when_the_branch_is_taken() {
    // The complement of `ret_cc_leaves_the_latch_alone_when_the_branch_is_not_taken`, and the
    // half that has to exist for either to mean anything: the untaken case reads `POISON` just
    // as happily when the taken path writes nothing at all.
    let mut machine = machine_with_stack(
        &[0xC8], // RET Z
        Registers {
            af: ZERO_SET,
            ..Registers::default()
        },
        RETURN_TARGET,
    );
    machine.step();

    assert_eq!(
        RETURN_TARGET,
        machine.snapshot().0.pc,
        "precondition failed: the return was not taken"
    );
    assert_eq!(
        RETURN_TARGET,
        machine.memptr(),
        "a taken RET cc latches like an unconditional one — the condition governs whether the \
         stack is read, and the latch follows the read"
    );
}

#[test]
fn reti_and_retn_latch_the_address_they_return_to() {
    // "MEMPTR = addr, as for RET" — the document lists `RETI` beside it. Both encodings share
    // one handler, so asserting the two together is what pins that they still do.
    let mut reti = machine_with_stack(&[0xED, 0x4D], Registers::default(), RETURN_TARGET);
    reti.step();
    let mut retn = machine_with_stack(&[0xED, 0x45], Registers::default(), RETURN_TARGET);
    retn.step();

    assert_eq!(
        RETURN_TARGET,
        reti.snapshot().0.pc,
        "precondition failed: RETI did not return"
    );
    assert_eq!(RETURN_TARGET, reti.memptr(), "RETI");
    assert_eq!(
        RETURN_TARGET,
        retn.memptr(),
        "RETN takes the same rule through the same handler"
    );
}

#[test]
fn rst_latches_its_page_zero_destination() {
    // "JR/DJNZ/RET/RETI/RST (jumping to addr) — MEMPTR = addr". `RST` has no operand fetch, so
    // the destination is all there is.
    let memptr = memptr_after(
        &[0xFF], // RST 38h
        Registers {
            sp: 0xFF00,
            ..Registers::default()
        },
    );
    assert_eq!(0x0038, memptr);
}

#[test]
fn jp_hl_is_the_documented_exception_and_latches_nothing() {
    // "JP (**except JP rp**)". The one encoding in the whole jump family that takes no rule,
    // and the exerciser had it in its failing set while it was already correct — see this
    // file's module documentation. Asserting the absence is the only way that stays true.
    let memptr = memptr_after(
        &[0xE9], // JP (HL)
        Registers {
            hl: BRANCH_TARGET,
            ..Registers::default()
        },
    );

    assert_eq!(
        POISON, memptr,
        "JP (HL) forms no address and must leave MEMPTR as it found it"
    );
}

#[test]
fn ex_sp_hl_latches_the_value_that_came_off_the_stack() {
    // "EX (SP),rp — MEMPTR = rp value after the operation". The one rule whose operand is a
    // *value* rather than an address. HL, SP and the stacked word are all different, so taking
    // any of the other two is caught.
    let mut machine = machine_with_stack(
        &[0xE3], // EX (SP),HL
        Registers {
            hl: 0x1111,
            ..Registers::default()
        },
        0xABCD,
    );
    machine.step();

    assert_eq!(
        0xABCD,
        machine.memptr(),
        "0x1111 would be the outgoing HL and {STACK:#06X} the stack address; the rule takes \
         the incoming value"
    );
    assert_eq!(
        0xABCD,
        machine.snapshot().0.hl,
        "precondition: HL exchanged"
    );
}

// ---------------------------------------------------------------------------------------
// The block families
// ---------------------------------------------------------------------------------------

#[test]
fn cpi_increments_the_latch_and_cpd_decrements_it() {
    // "CPI — MEMPTR = MEMPTR + 1", "CPD — MEMPTR = MEMPTR - 1". The only rule that reads the
    // latch in order to write it, and the mechanism the whole register was measured with: a
    // loop of CPD walks it downwards one at a time while `BIT n,(HL)` watches two of its bits.
    let registers = Registers {
        bc: 0x0002, // > 1, so a repeating form would not stop here either
        hl: 0x6000,
        ..Registers::default()
    };

    assert_eq!(
        POISON.wrapping_add(1),
        memptr_after(&[0xED, 0xA1], registers), // CPI
        "CPI must increment whatever the latch already held"
    );
    assert_eq!(
        POISON.wrapping_sub(1),
        memptr_after(&[0xED, 0xA9], registers), // CPD
        "CPD must decrement it"
    );
}

#[test]
fn ldi_leaves_the_latch_alone() {
    // Not an omission: `LDIR`'s rule is "when BC == 1: MEMPTR is not changed" — BC == 1 being
    // the last iteration — so a form that never repeats never changes it. This is the
    // assertion that keeps that reading honest, since the alternative (LDI takes some rule the
    // document forgot) would be invisible without it.
    let memptr = memptr_after(
        &[0xED, 0xA0], // LDI
        Registers {
            bc: 0x0002,
            de: 0x7000,
            hl: 0x6000,
            ..Registers::default()
        },
    );
    assert_eq!(POISON, memptr);
}

#[test]
fn a_repeating_ldir_takes_its_own_instruction_address_plus_one() {
    // "when BC <> 1: MEMPTR = PC + 1, where PC = instruction address". The instruction is at
    // ORIGIN, so the answer is ORIGIN + 1 — deliberately not `PC` as it stands after the two
    // opcode fetches, which would be ORIGIN + 2.
    let memptr = memptr_after(
        &[0xED, 0xB0], // LDIR
        Registers {
            bc: 0x0005, // several iterations left, so this one repeats
            de: 0x7000,
            hl: 0x6000,
            ..Registers::default()
        },
    );

    assert_eq!(
        ORIGIN + 1,
        memptr,
        "a repeating LDIR latches its own address plus one; {:#06X} would be PC after the \
         opcode fetches rather than the instruction's address",
        ORIGIN + 2,
    );
}

#[test]
fn the_last_iteration_of_an_ldir_leaves_the_latch_alone() {
    // The other half of the same rule, and the half that makes it a rule rather than "LDIR
    // always writes PC + 1". With BC == 1 the instruction does not repeat and must not write.
    let memptr = memptr_after(
        &[0xED, 0xB0], // LDIR
        Registers {
            bc: 0x0001,
            de: 0x7000,
            hl: 0x6000,
            ..Registers::default()
        },
    );
    assert_eq!(POISON, memptr);
}

#[test]
fn a_cpir_that_stops_takes_the_cpi_rule_rather_than_the_repeat_rule() {
    // "CPIR — when BC=1 or A=(HL): exactly as CPI." Both stopping conditions are worth
    // separating from the repeating case, because the two rules produce very different
    // numbers: ORIGIN + 1 against POISON + 1.
    let memptr = memptr_after(
        &[0xED, 0xB1], // CPIR
        Registers {
            bc: 0x0001, // stops on the counter
            hl: 0x6000,
            ..Registers::default()
        },
    );

    assert_eq!(
        POISON.wrapping_add(1),
        memptr,
        "the stopping iteration takes CPI's increment, not the repeat's {:#06X}",
        ORIGIN + 1,
    );
}

#[test]
fn ini_takes_the_port_before_b_is_decremented_and_outi_after() {
    // "INI — MEMPTR = BC_before_decrementing_B + 1" against "OUTI — MEMPTR =
    // BC_after_decrementing_B + 1". The one place the two I/O families diverge, and B is chosen
    // so the two answers differ in the high byte rather than by one.
    let registers = Registers {
        bc: 0x0234, // B = 0x02, so before/after are 0x0234 and 0x0134
        hl: 0x6000,
        ..Registers::default()
    };

    assert_eq!(
        0x0235,
        memptr_after(&[0xED, 0xA2], registers), // INI
        "INI addresses its port with B still at its old value"
    );
    assert_eq!(
        0x0135,
        memptr_after(&[0xED, 0xA3], registers), // OUTI
        "OUTI decrements B first, so its port — and its latch — carry the new value"
    );
}

#[test]
fn ind_and_outd_decrement_where_their_incrementing_twins_add() {
    let registers = Registers {
        bc: 0x0234,
        hl: 0x6000,
        ..Registers::default()
    };

    assert_eq!(0x0233, memptr_after(&[0xED, 0xAA], registers)); // IND
    assert_eq!(0x0133, memptr_after(&[0xED, 0xAB], registers)); // OUTD
}

#[test]
fn the_io_block_repeat_takes_the_instruction_address_against_the_2006_document() {
    // **The one rule here that its primary source denies.** Boo-boo and Kladov give "INIR —
    // exactly as INI on each execution", which would make this POISON-independent and equal to
    // INI's 0x0235. It is wrong: David Banks traced the repeat's extra five T-states on real
    // parts — one Zilog NMOS and two NEC NMOS — and found the latch loaded from the instruction
    // address, exactly as in the transfer and compare families. MAME took the same rule for
    // `inir`, `indr`, `otir` and `otdr`.
    //
    // Two independent things then agree with the correction and not with the document: this
    // assertion, and `102 INIR->NOP'` / `103 INDR->NOP'` in the exerciser, whose CRCs are met
    // only under the newer rule.
    let memptr = memptr_after(
        &[0xED, 0xB2], // INIR
        Registers {
            bc: 0x0234, // B > 1, so this iteration repeats
            hl: 0x6000,
            ..Registers::default()
        },
    );

    assert_eq!(
        ORIGIN + 1,
        memptr,
        "a repeating INIR latches its instruction address plus one; 0x0235 is what the 2006 \
         document's 'exactly as INI' would give"
    );
}

#[test]
fn the_last_iteration_of_an_inir_takes_the_ini_rule() {
    // The complement, and what keeps the correction above narrow: only an iteration that
    // actually repeats takes the instruction address. `100 INIR` in the exerciser passes under
    // both readings precisely because a completed INIR ends on this iteration.
    let memptr = memptr_after(
        &[0xED, 0xB2], // INIR
        Registers {
            bc: 0x0134, // B = 1, so it stops here
            hl: 0x6000,
            ..Registers::default()
        },
    );

    assert_eq!(
        0x0135, memptr,
        "the stopping iteration takes INI's port + 1"
    );
}

#[test]
fn a_repeating_otir_takes_its_own_instruction_address_plus_one() {
    // **The half of the repeat rule that no oracle in this project reaches.** The exerciser
    // grades the input forms through a self-overwriting `->NOP'` trick that has no output
    // counterpart, so `OTIR` and `OTDR` have no group — they are covered by
    // `Cpu::repeat_block`'s single write and graded, until this test, by nothing at all.
    //
    // That matters more here than it would for a rule with a document behind it. The uniform
    // repeat rule contradicts its own primary source (see
    // `the_io_block_repeat_takes_the_instruction_address_against_the_2006_document`), and the
    // handler *did* once carry the per-family exception the 2006 document asks for. If it were
    // reintroduced for the outputs alone, every other instrument in this repository would stay
    // green.
    let memptr = memptr_after(
        &[0xED, 0xB3], // OTIR
        Registers {
            bc: 0x0234, // B = 2, so this iteration repeats
            hl: 0x6000,
            ..Registers::default()
        },
    );

    assert_eq!(
        ORIGIN + 1,
        memptr,
        "a repeating OTIR latches its instruction address plus one; 0x0135 is what OUTI's own \
         port rule would leave if the repeat wrote nothing over it"
    );
}

#[test]
fn the_last_iteration_of_an_otir_takes_the_outi_rule_and_otdr_the_outd_one() {
    // The complement, and what keeps the correction above narrow at the one end of the block
    // families no oracle can see: an iteration that stops takes the plain port rule, and the
    // two directions differ by two.
    let registers = Registers {
        bc: 0x0134, // B = 1, so both stop here; the port is 0x0034 once B has fallen
        hl: 0x6000,
        ..Registers::default()
    };

    assert_eq!(
        0x0035,
        memptr_after(&[0xED, 0xB3], registers), // OTIR
        "the stopping iteration takes OUTI's port + 1"
    );
    assert_eq!(
        0x0033,
        memptr_after(&[0xED, 0xBB], registers), // OTDR
        "and OTDR's takes OUTD's port - 1"
    );
}

// ---------------------------------------------------------------------------------------
// The port group addressed by BC
// ---------------------------------------------------------------------------------------

#[test]
fn the_bc_addressed_port_instructions_take_bc_plus_one() {
    // "IN A,(C) — MEMPTR = BC + 1", "OUT (C),A — MEMPTR = BC + 1". The document names the
    // accumulator forms; the rule is stated in terms of the port, and the port is the whole of
    // BC for all eight encodings — the destination register is selected after the cycle and
    // cannot reach back into it. The exerciser grades `IN R,(C)` and `OUT (C),0` as their own
    // groups, which is the independent check on that reading.
    let registers = Registers {
        bc: 0x30FF, // crosses a byte boundary, so a truncating rule is visible
        ..Registers::default()
    };

    assert_eq!(
        0x3100,
        memptr_after(&[0xED, 0x78], registers), // IN A,(C)
        "IN A,(C)"
    );
    assert_eq!(
        0x3100,
        memptr_after(&[0xED, 0x50], registers), // IN D,(C) — not the accumulator form
        "IN D,(C) takes the same rule; the destination register is not part of it"
    );
    assert_eq!(
        0x3100,
        memptr_after(&[0xED, 0x71], registers), // OUT (C),0
        "OUT (C),0 names no source register and still latches its port"
    );
}

// ---------------------------------------------------------------------------------------
// The indexed rule, which was the only one implemented before this work
// ---------------------------------------------------------------------------------------

#[test]
fn an_indexed_access_latches_its_effective_address() {
    // "Any instruction with (INDEX+d): MEMPTR = INDEX+d" — one line covering the whole DD/FD
    // set, which is why the core writes it at a single site. The displacement is negative so
    // that a handler ignoring its sign is caught.
    let memptr = memptr_after(
        &[0xDD, 0x7E, 0xFE], // LD A,(IX-2)
        Registers {
            ix: 0x6000,
            ..Registers::default()
        },
    );
    assert_eq!(0x5FFE, memptr);
}

// ---------------------------------------------------------------------------------------
// Interrupt acceptance, which is a `CALL` the device asked for
// ---------------------------------------------------------------------------------------

/// Where mode 2's vector-table entry sits.
///
/// `I` and the byte the device supplies are taken from its two halves rather than written
/// separately, so the pointer the test *builds* and the pointer the test *names as a wrong
/// answer* cannot drift apart.
const VECTOR_POINTER: u16 = 0x3040;

/// The address mode 2's table entry holds.
///
/// No arithmetic in this file — and none in the interrupt path — can produce it from anything
/// else in the setup, so a latch holding it can only have been loaded from the table.
const VECTOR_TARGET: u16 = 0x9ABC;

/// A machine holding `state`, stopped at [`ORIGIN`] with a stack, mode 2's vector table
/// loaded, and [`POISON`] in the latch.
///
/// The table is loaded whichever mode is asked for. The modes that ignore it are unaffected by
/// its presence, and one shape for all of these tests keeps the difference between them down to
/// the one thing under test. The `NOP` at [`ORIGIN`] never executes: an interrupt is offered to
/// a stopped CPU, and `PC` is there only to be the return address.
fn machine_for_interrupt(state: State) -> Machine {
    let mut setup = program(
        &[0x00],
        Registers {
            sp: STACK,
            ..Registers::default()
        },
    );
    setup.state = state;
    setup.memory.push(MemoryBlock {
        start: VECTOR_POINTER,
        bytes: VECTOR_TARGET.to_le_bytes().to_vec(),
    });
    poisoned(setup)
}

#[test]
fn an_accepted_interrupt_latches_the_address_it_vectors_to() {
    // Boo-boo and Kladov give the whole rule in three words — "as usual CALL" — and an accepted
    // interrupt is exactly that: the device asks for a call and the CPU performs one, so the
    // latch takes the destination as `CALL nn`'s does. Mode 1 vectors to 0x0038 whatever byte
    // the device puts on the bus, which is why the byte here is deliberately not 0xFF's
    // `RST 38h`: under mode 1 it cannot matter, and a passing test must not depend on it.
    let mut machine = machine_for_interrupt(State {
        iff1: true,
        im: 1,
        ..State::default()
    });
    let t_states = machine.interrupt(0x00);

    assert_ne!(
        0, t_states,
        "precondition failed: the offer was declined, so nothing was accepted to observe"
    );
    assert_eq!(
        0x0038,
        machine.snapshot().0.pc,
        "precondition: mode 1 vectors to 0x0038"
    );
    assert_eq!(
        0x0038,
        machine.memptr(),
        "an accepted interrupt latches its vector. {POISON:#06X} would be an acceptance that \
         wrote nothing and {ORIGIN:#06X} the return address it stacked instead"
    );
}

#[test]
fn a_mode_2_interrupt_latches_the_address_it_read_and_not_the_table_pointer() {
    // The discriminating case, and the reason the core reads the value back from `PC` rather
    // than from the dispatch it resolved: in mode 2 the destination is not known until the
    // vector table has been read, so a rule written to latch what the *device* named would
    // latch the pointer. Mode 1 cannot tell those two apart — its pointer and its destination
    // are the same fixed number — and this is the mode where they are different numbers.
    let [page, low] = VECTOR_POINTER.to_be_bytes();
    let mut machine = machine_for_interrupt(State {
        iff1: true,
        im: 2,
        i: page,
        ..State::default()
    });
    machine.interrupt(low);

    assert_eq!(
        VECTOR_TARGET,
        machine.snapshot().0.pc,
        "precondition: the vector table was read"
    );
    assert_eq!(
        VECTOR_TARGET,
        machine.memptr(),
        "mode 2 latches the address its table named; {VECTOR_POINTER:#06X} is the table entry's \
         own address, which is what the device actually supplied"
    );
}

#[test]
fn a_declined_interrupt_leaves_the_latch_alone() {
    // The complement, and it guards a property the handler is built around rather than a
    // MEMPTR rule of its own: an interrupt is accepted whole or not at all. A latch written
    // before the accept/decline test would be a half-accepted interrupt leaving a trace, and
    // this is the assertion that would notice.
    let mut machine = machine_for_interrupt(State {
        iff1: false,
        im: 1,
        ..State::default()
    });
    let t_states = machine.interrupt(0x00);

    assert_eq!(
        0, t_states,
        "precondition failed: the offer was accepted, so this proves nothing about declining"
    );
    assert_eq!(
        ORIGIN,
        machine.snapshot().0.pc,
        "precondition: a declined interrupt does not move PC"
    );
    assert_eq!(
        POISON,
        machine.memptr(),
        "a declined offer must leave MEMPTR exactly as it found it"
    );
}

#[test]
fn an_nmi_latches_its_fixed_vector() {
    // An NMI is always accepted and always vectors to 0x0066, so unlike the maskable path it
    // has no mode to vary — which makes the whole of its rule the one write. `IFF1` is left
    // clear on purpose: an NMI does not consult it, and a test that set it would leave the
    // reader unable to tell whether that mattered.
    let mut machine = machine_for_interrupt(State::default());
    machine.nmi();

    assert_eq!(
        0x0066,
        machine.snapshot().0.pc,
        "precondition: the NMI vectored to 0x0066"
    );
    assert_eq!(
        0x0066,
        machine.memptr(),
        "an NMI latches its vector as an accepted interrupt does; {POISON:#06X} would be a \
         handler that wrote nothing"
    );
}
