//! Which address the CPU drives during an internal cycle — pinned **without the corpus**.
//!
//! # Why this exists when `fuse_vectors.rs` already checks it
//!
//! `testdata/fuse/` is gitignored. On a fresh clone the conformance gate skips with a
//! notice, and every property it alone protects silently loses its guard. The internal-cycle
//! address is exactly such a property: it leaves T-states, registers, flags and memory
//! completely unchanged, so *nothing else in the suite would notice it breaking*. A
//! mutation at `instructions.rs:429` proved that — it turned only `bus address` red, with
//! transfers byte-identical.
//!
//! The general shape, worth keeping past M1: **a corpus-dependent gate needs a
//! corpus-independent floor for the properties that would otherwise vanish on a clean
//! checkout.** These tests use only [`Machine`] and its `TestBus`, so they run everywhere,
//! unconditionally.
//!
//! # These are stronger than the vectors they back up
//!
//! The corpus runs with `I = 0x00` and `R = 0x01`, so `IR` is `0x0001` — which collides with
//! the program addresses at `0x0000`/`0x0001`/`0x0002`. In vector `10` the refresh address
//! and the displacement address are *both* `0x0002`, so that vector cannot tell them apart
//! at all. Here `I` and `R` are chosen so `IR` is `0x4006` and cannot be confused with any
//! program, operand or stack address — a core that used the wrong source fails these tests
//! even though it would pass the corpus.

mod common;

use common::flags;
use common::machine::Machine;
use common::vectors::{MemoryBlock, Registers, Setup, State};

/// Deliberately not `0x0000`, so a core that defaults an address to zero is caught.
const PROGRAM_START: u16 = 0x0100;
/// The address of the byte after the opcode: operands and displacements live here.
const FIRST_OPERAND: u16 = 0x0101;
/// The second operand of a three-byte instruction — `CALL`'s "last operand address".
const SECOND_OPERAND: u16 = 0x0102;

const STACK_TOP: u16 = 0x8000;
/// The stack grows down: the high byte of the return address lands here first.
const STACK_HIGH: u16 = 0x7FFF;
const STACK_LOW: u16 = 0x7FFE;

/// `I` and `R` chosen so `IR` shares no bits with any program or stack address above.
const INTERRUPT_VECTOR: u8 = 0x40;
const REFRESH: u8 = 0x05;
/// `IR` as seen *during* the instruction — the opcode fetch has already bumped `R` to 6.
const REFRESH_ADDRESS: u16 = 0x4006;

const OPCODE_FETCH: usize = 4;
const MEMORY_CYCLE: usize = 3;

#[test]
fn add_hl_ss_spends_its_seven_internal_t_states_on_ir() {
    // `ADD HL,BC` — 11 T: a 4-T fetch, then a 16-bit add occupying two internal machine
    // cycles, all seven T-states with the refresh address on the bus.
    assert_eq!(
        cycles(&[(PROGRAM_START, OPCODE_FETCH), (REFRESH_ADDRESS, 7)]),
        run(&[0x09], registers()),
        "ADD HL,BC must drive IR through all seven internal T-states, not the last address \
         it fetched from",
    );
}

#[test]
fn rst_spends_its_internal_t_state_on_ir() {
    // `RST 38` — 11 T: fetch, one internal T-state, then two stack writes.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (REFRESH_ADDRESS, 1),
            (STACK_HIGH, MEMORY_CYCLE),
            (STACK_LOW, MEMORY_CYCLE),
        ]),
        run(&[0xFF], registers()),
        "RST's internal cycle sits on IR",
    );
}

/// The counterpart to the test above, and the reason both are needed.
///
/// `RST` and `CALL` share their push-and-jump implementation, yet the corpus insists their
/// internal cycles sit on *different* addresses. Nothing but the trace would have predicted
/// that; with `IR` at `0x4006` and the last operand at `0x0102`, giving either instruction
/// the other's rule fails here immediately.
#[test]
fn call_spends_its_internal_t_state_on_the_last_operand_address() {
    // `CALL 0x1234` — 17 T: fetch, two operand reads, one internal T-state, two writes.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (FIRST_OPERAND, MEMORY_CYCLE),
            (SECOND_OPERAND, MEMORY_CYCLE),
            (SECOND_OPERAND, 1),
            (STACK_HIGH, MEMORY_CYCLE),
            (STACK_LOW, MEMORY_CYCLE),
        ]),
        run(&[0xCD, 0x34, 0x12], registers()),
        "CALL's internal cycle sits on the last operand address, not on IR",
    );
}

/// `DJNZ` drives two different addresses inside one instruction.
///
/// The corpus cannot demonstrate this: in vector `10` the refresh address and the
/// displacement address are both `0x0002`, so the two are indistinguishable there. With
/// `IR` at `0x4006` they are not.
#[test]
fn djnz_taken_uses_ir_for_the_m1_extra_t_state_and_the_displacement_address_for_the_add() {
    // `DJNZ -3` with B = 2, so the branch is taken — 13 T: a 5-T M1 (4 + 1 extra), a 3-T
    // displacement read, then a 5-T internal add.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (REFRESH_ADDRESS, 1),
            (FIRST_OPERAND, MEMORY_CYCLE),
            (FIRST_OPERAND, 5),
        ]),
        run(&[0x10, 0xFD], counting_down_from(2)),
        "DJNZ's extra M1 T-state belongs to IR; its five internal T-states belong to the \
         displacement byte's own address",
    );
}

#[test]
fn djnz_not_taken_still_reads_the_displacement() {
    // B = 1 decrements to zero, so the branch is not taken — 8 T, and the five internal
    // T-states of the add disappear. The displacement read does not: this is the behaviour
    // the corpus declines to record and `CORPUS_OMISSIONS` documents.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (REFRESH_ADDRESS, 1),
            (FIRST_OPERAND, MEMORY_CYCLE),
        ]),
        run(&[0x10, 0xFD], counting_down_from(1)),
        "a not-taken DJNZ still fetches its displacement — only the internal add is skipped",
    );
}

#[test]
fn push_spends_its_internal_t_state_on_ir() {
    // `PUSH BC` — 11 T: a 5-T M1 (4 + 1 extra), then two stack writes.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (REFRESH_ADDRESS, 1),
            (STACK_HIGH, MEMORY_CYCLE),
            (STACK_LOW, MEMORY_CYCLE),
        ]),
        run(&[0xC5], registers()),
        "PUSH's extra M1 T-state sits on IR",
    );
}

#[test]
fn inc_ss_spends_its_two_internal_t_states_on_ir() {
    // `INC BC` — 6 T: a 4-T fetch, then a 2-T internal increment of the pair.
    assert_eq!(
        cycles(&[(PROGRAM_START, OPCODE_FETCH), (REFRESH_ADDRESS, 2)]),
        run(&[0x03], registers()),
        "the 16-bit increment's two internal T-states sit on IR",
    );
}

#[test]
fn jr_spends_its_five_internal_t_states_on_the_displacement_address() {
    // `JR +0` — 12 T: fetch, a 3-T displacement read, then a 5-T internal add. Like DJNZ
    // and unlike everything above, the add is charged to the displacement byte's address.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (FIRST_OPERAND, MEMORY_CYCLE),
            (FIRST_OPERAND, 5),
        ]),
        run(&[0x18, 0x00], registers()),
        "JR's internal add is charged to the displacement address, not to IR",
    );
}

/// `RET cc` in both directions — the taken path pops, the not-taken path stops after the
/// M1 extra T-state, and both charge that extra T-state to `IR`.
#[test]
fn ret_cc_spends_its_internal_t_state_on_ir_whether_or_not_it_returns() {
    // `RET NZ` taken (Z clear) — 11 T: 5-T M1, then two stack reads.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (REFRESH_ADDRESS, 1),
            (STACK_TOP, MEMORY_CYCLE),
            (STACK_TOP + 1, MEMORY_CYCLE),
        ]),
        run(&[0xC0], with_flags(0x00)),
        "RET NZ taken: the extra M1 T-state is on IR, then it pops",
    );

    // `RET NZ` not taken (Z set) — 5 T, and nothing is popped.
    assert_eq!(
        cycles(&[(PROGRAM_START, OPCODE_FETCH), (REFRESH_ADDRESS, 1)]),
        run(&[0xC0], with_flags(flags::Z)),
        "RET NZ not taken: 5 T-states and no stack access at all",
    );
}

/// `EX (SP),HL` — the only instruction that drives **stack** addresses during its internal
/// cycles, and it drives two different ones.
#[test]
fn ex_sp_hl_drives_the_stack_addresses_through_its_internal_cycles() {
    // 19 T: fetch, two stack reads, 1 internal on SP+1, two stack writes, 2 internal on SP.
    // Corpus vector `e3` with SP = 0x0373 shows the shape: the single internal T-state
    // belongs to the high half just read, the trailing pair to the low half just written.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (STACK_TOP, MEMORY_CYCLE),
            (STACK_TOP + 1, MEMORY_CYCLE),
            (STACK_TOP + 1, 1),
            (STACK_TOP + 1, MEMORY_CYCLE),
            (STACK_TOP, MEMORY_CYCLE),
            (STACK_TOP, 2),
        ]),
        run(&[0xE3], registers()),
        "EX (SP),HL: one internal T-state on SP+1, two on SP",
    );
}

/// `LDIR` mid-run — the repeat mechanism, and the newest code in the core.
///
/// M2 added six block-instruction internal-cycle sites and the corpus is the only thing
/// that touched any of them. These two tests are the corpus-independent floor for the most
/// distinctive of them: the five extra T-states that make the instruction repeat.
#[test]
fn ldir_charges_its_repeat_cycles_to_the_destination_address() {
    // 21 T while BC != 0 after the copy: ED fetch, B0 fetch, read (HL), write (DE), then
    // seven internal T-states — two for the copy, five for the repeat — all on DE.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (PROGRAM_START + 1, OPCODE_FETCH),
            (BLOCK_SOURCE, MEMORY_CYCLE),
            (BLOCK_DESTINATION, MEMORY_CYCLE),
            (BLOCK_DESTINATION, 7),
        ]),
        run(&[0xED, 0xB0], block_copy(2)),
        "a repeating LDIR spends all seven internal T-states on DE",
    );
}

#[test]
fn ldir_on_its_final_iteration_drops_the_five_repeat_cycles() {
    // 16 T when BC reaches zero: the same shape with only the copy's two internal T-states.
    // This is the difference the repeat mechanism has to get right.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (PROGRAM_START + 1, OPCODE_FETCH),
            (BLOCK_SOURCE, MEMORY_CYCLE),
            (BLOCK_DESTINATION, MEMORY_CYCLE),
            (BLOCK_DESTINATION, 2),
        ]),
        run(&[0xED, 0xB0], block_copy(1)),
        "the last LDIR iteration is 16 T, not 21 — no repeat cycles",
    );
}

#[test]
fn lddr_matches_ldir_cycle_for_cycle() {
    // LDDR differs from LDIR only in which way HL and DE move afterwards; the machine
    // cycles of a single iteration are identical, so the floor is too.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (PROGRAM_START + 1, OPCODE_FETCH),
            (BLOCK_SOURCE, MEMORY_CYCLE),
            (BLOCK_DESTINATION, MEMORY_CYCLE),
            (BLOCK_DESTINATION, 7),
        ]),
        run(&[0xED, 0xB8], block_copy(2)),
        "a repeating LDDR has the same cycle shape as LDIR",
    );
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Expand `(address, t_states)` runs into the flat per-T-state address log the bus records.
///
/// Written as runs because that is how the hardware behaves — one address held for a whole
/// machine cycle — and because it keeps each expectation readable as a cycle breakdown
/// rather than a wall of repeated literals.
fn cycles(runs: &[(u16, usize)]) -> Vec<u16> {
    runs.iter()
        .flat_map(|(address, t_states)| std::iter::repeat_n(*address, *t_states))
        .collect()
}

fn registers() -> Registers {
    Registers {
        pc: PROGRAM_START,
        sp: STACK_TOP,
        ..Registers::default()
    }
}

/// `B` is the high byte of `BC`, and it is what `DJNZ` counts down.
fn counting_down_from(b: u8) -> Registers {
    Registers {
        bc: u16::from(b) << 8,
        ..registers()
    }
}

/// Source and destination for the block-copy tests, well away from the program and the
/// stack so a wrong address cannot coincide with a right one.
const BLOCK_SOURCE: u16 = 0x5000;
const BLOCK_DESTINATION: u16 = 0x6000;

/// `HL` -> `DE`, `BC` bytes to go. `BC == 1` makes the next copy the last one.
fn block_copy(remaining: u16) -> Registers {
    Registers {
        hl: BLOCK_SOURCE,
        de: BLOCK_DESTINATION,
        bc: remaining,
        ..registers()
    }
}

/// `F` is the low byte of `AF`, and it is what the conditional instructions branch on.
fn with_flags(f: u8) -> Registers {
    Registers {
        af: u16::from(f),
        ..registers()
    }
}

/// Execute exactly one instruction and return the address the bus saw at each T-state.
fn run(bytes: &[u8], registers: Registers) -> Vec<u16> {
    let setup = Setup {
        name: String::from("bus_timing"),
        registers,
        state: State {
            i: INTERRUPT_VECTOR,
            r: REFRESH,
            ..State::default()
        },
        memory: vec![MemoryBlock {
            start: PROGRAM_START,
            bytes: bytes.to_vec(),
        }],
    };
    let mut machine = Machine::load(&setup);
    machine.step();
    // Every test in this file rests on the instruction actually having executed. A core
    // that recorded an unimplemented-opcode fault would still produce a tick log, and a
    // short one could coincidentally match a short expectation — so the fault is checked
    // here, once, rather than assumed nine times.
    assert_eq!(
        None,
        machine.fault(),
        "the core faulted instead of executing {bytes:02x?}",
    );
    machine.tick_addresses().to_vec()
}
