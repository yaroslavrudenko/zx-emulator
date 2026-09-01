//! Gate **T3**: a program that only works on a 128, run on both machines, with **both**
//! outcomes asserted.
//!
//! # Why the negative half is the whole point
//!
//! *"This passes on the 128"* is a much weaker claim than *"this passes on the 128 **and fails
//! in a specific, asserted way on a 48K**"*. Only the second says that paging is what made the
//! difference. Without it the program could be passing for a reason unrelated to paging — a
//! stray write, an aliasing accident, a default that happens to match — and nothing would show
//! it. `docs/STATUS.md` has the general form: *"a gate can pass vacuously, and only a positive
//! control catches it."*
//!
//! The 48K arm is therefore not a courtesy. It is the control, and its expected value is
//! derived from what a 48K does with the program rather than observed from running it.
//!
//! # The program
//!
//! ```text
//!   LD BC,0x7FFD
//!   LD A,0        OUT (C),A     ; bank 0 into 0xC000
//!   LD HL,0xC000
//!   LD (HL),0xAA                ; a signature into bank 0
//!   LD A,1        OUT (C),A     ; bank 1 into 0xC000
//!   LD (HL),0x55                ; a different signature into bank 1
//!   LD A,0        OUT (C),A     ; bank 0 back
//!   LD A,(HL)                   ; read bank 0's signature back
//!   LD (0x9000),A               ; leave the answer somewhere neither bank can reach
//!   HALT
//! ```
//!
//! On a **128** the three `OUT`s page, the two signatures land in different banks, and the
//! read-back finds `0xAA`.
//!
//! On a **48K** every `OUT` is absorbed — the machine powers on with the paging lock set — so
//! both `LD (HL),n` write to the *same* bank at `0xC000` and the second overwrites the first.
//! The read-back finds `0x55`. That is not a failure to run; it is a different, definite
//! answer, which is what makes it assertable.
//!
//! `0x9000` is in bank 2, which is wired to `0x8000` on both machines and which no paging value
//! moves — so the answer is somewhere the experiment cannot disturb.

mod common;
mod m7_common;

use common::{set_pc, write_program};
use m7_common::machine_128;
use spectrum::memory::BankIndex;
use spectrum::{Model, Spectrum};

/// Where the program is assembled: bank 2, untouched by any paging value.
const PROGRAM: u16 = 0x8000;

/// Where it leaves its answer: bank 2 as well, clear of the program.
const ANSWER: u16 = 0x9000;

/// The address the two signatures are written through.
const PAGED: u16 = 0xC000;

/// The signature written while bank 0 is paged in, and the one written while bank 1 is.
const FIRST: u8 = 0xAA;
const SECOND: u8 = 0x55;

/// Instructions in [`program`].
const STEPS: usize = 13;

/// The 128-only program, assembled by hand.
fn program() -> Vec<u8> {
    let [port_low, port_high] = 0x7FFD_u16.to_le_bytes();
    let [paged_low, paged_high] = PAGED.to_le_bytes();
    let [answer_low, answer_high] = ANSWER.to_le_bytes();
    vec![
        0x01,
        port_low,
        port_high, // LD BC,0x7FFD
        0x3E,
        0x00, // LD A,0
        0xED,
        0x79, // OUT (C),A
        0x21,
        paged_low,
        paged_high, // LD HL,0xC000
        0x36,
        FIRST, // LD (HL),0xAA
        0x3E,
        0x01, // LD A,1
        0xED,
        0x79, // OUT (C),A
        0x36,
        SECOND, // LD (HL),0x55
        0x3E,
        0x00, // LD A,0
        0xED,
        0x79, // OUT (C),A
        0x7E, // LD A,(HL)
        0x32,
        answer_low,
        answer_high, // LD (0x9000),A
        0x76,        // HALT
    ]
}

/// Run the program on `machine` and return the byte it left at [`ANSWER`].
fn run(machine: &mut Spectrum) -> u8 {
    write_program(machine, PROGRAM, &program());
    set_pc(machine, PROGRAM);
    for _ in 0..STEPS {
        machine.step();
    }
    assert!(
        machine.cpu_state().halted,
        "the program must reach its HALT, not run off the end"
    );
    assert_eq!(machine.fault(), None);
    machine.memory().read(ANSWER)
}

#[test]
fn the_program_reads_back_its_first_signature_on_a_128() {
    let mut machine = machine_128();
    assert_eq!(
        run(&mut machine),
        FIRST,
        "the two signatures went to different banks, so the first survived"
    );
}

#[test]
fn the_same_program_reads_back_the_second_signature_on_a_48k() {
    // The control. A 48K absorbs the paging writes, so both `LD (HL),n` hit one bank and the
    // second wins — a definite wrong answer rather than a crash, which is what makes it worth
    // asserting.
    let mut machine = common::machine();
    assert_eq!(
        run(&mut machine),
        SECOND,
        "with paging absorbed, the second write overwrote the first"
    );
}

#[test]
fn the_two_machines_disagree_and_that_is_the_gate() {
    // Stated as one assertion rather than left for a reader to infer from the two above. If
    // both ever produced the same answer, both tests could still be individually green while
    // the thing they exist to demonstrate had stopped being true.
    let mut one_two_eight = machine_128();
    let mut forty_eight = common::machine();
    assert_ne!(run(&mut one_two_eight), run(&mut forty_eight));
}

#[test]
fn the_128_really_put_the_signatures_in_two_different_banks() {
    // The read-back proves *an* answer; this proves the mechanism. Bank 1 has no address at the
    // end of the run — bank 0 is at 0xC000 — so its contents are only reachable through
    // `Memory::bank`, which is the method M7 added for exactly this situation.
    let mut machine = machine_128();
    assert_eq!(run(&mut machine), FIRST);
    assert_eq!(machine.memory().bank(BankIndex::new(0))[0], FIRST);
    assert_eq!(machine.memory().bank(BankIndex::new(1))[0], SECOND);
    assert_eq!(
        machine.memory().slot_at(PAGED),
        spectrum::memory::Slot::Bank(BankIndex::new(0)),
        "bank 1 is paged out, so the assertion above went through no address"
    );
}

#[test]
fn the_48k_put_both_signatures_in_one_bank_and_left_the_others_alone() {
    // The negative half of the mechanism, and the reason the 48K's answer is `0x55` rather
    // than merely "not 0xAA": both writes went to bank 0, and bank 1 was never touched.
    let mut machine = common::machine();
    assert_eq!(run(&mut machine), SECOND);
    assert_eq!(machine.memory().bank(BankIndex::new(0))[0], SECOND);
    assert_eq!(
        machine.memory().bank(BankIndex::new(1))[0],
        0x00,
        "a 48K's bank 1 is unreachable, so it must still be as it powered on"
    );
}

#[test]
fn a_snapshot_of_one_machine_is_refused_by_the_other() {
    // The same asymmetry at the state level, and the reason `Spectrum::restore` became
    // fallible. A 128 image has five banks a 48K cannot hold; a 48K image carries no paging
    // byte, so restoring one into a 128 would leave 48K code running against the 128 editor
    // ROM. Both directions are refused and the error names both machines.
    let mut one_two_eight = machine_128();
    let mut forty_eight = common::machine();
    run(&mut one_two_eight);
    run(&mut forty_eight);

    let from_128 = one_two_eight.snapshot();
    let from_48 = forty_eight.snapshot();
    assert_eq!(from_128.model(), Model::Spectrum128);
    assert_eq!(from_48.model(), Model::Spectrum48K);

    let refused = forty_eight
        .restore(&from_128)
        .expect_err("a 128 image has five banks a 48K cannot hold");
    assert_eq!(refused.snapshot, Model::Spectrum128);
    assert_eq!(refused.machine, Model::Spectrum48K);

    let refused = one_two_eight
        .restore(&from_48)
        .expect_err("a 48K image would run against the 128 editor ROM");
    assert_eq!(refused.snapshot, Model::Spectrum48K);
    assert_eq!(refused.machine, Model::Spectrum128);

    // A refusal changes nothing: the check runs before anything is written, so neither machine
    // is half-loaded. Measured rather than asserted in prose, because "returns an error" and
    // "returns an error having already written five banks" look identical from the call site.
    assert_eq!(forty_eight.memory().read(ANSWER), SECOND);
    assert_eq!(one_two_eight.memory().read(ANSWER), FIRST);
}

#[test]
fn a_128_snapshot_carries_all_eight_banks_and_the_map_that_arranges_them() {
    // What the writer had to change: it used to walk the slot map and read through addresses,
    // which captures three banks of eight on a 128. And eight banks without the paging byte
    // would not describe a machine that could be restored.
    let mut machine = machine_128();
    run(&mut machine);
    let snapshot = machine.snapshot();

    let carried: Vec<u8> = (0..8)
        .filter(|&bank| snapshot.bank(BankIndex::new(bank)).is_some())
        .collect();
    assert_eq!(carried, (0..8).collect::<Vec<u8>>(), "all eight");
    assert_eq!(
        snapshot.bank(BankIndex::new(1)).map(|page| page[0]),
        Some(SECOND),
        "including the one with no address"
    );

    // And it restores into a machine that has been left somewhere else entirely.
    let mut other = machine_128();
    write_program(&mut other, PROGRAM, &common::pattern_rom()[..16]);
    other.restore(&snapshot).expect("both are 128s");
    assert_eq!(other.memory().bank(BankIndex::new(1))[0], SECOND);
    assert_eq!(other.memory().read(ANSWER), FIRST);
    assert_eq!(other.memory().slots(), machine.memory().slots());
}

#[test]
fn a_restore_reaches_a_machine_that_has_locked_its_own_paging() {
    // The trap `Memory::set_paging_port` exists for, and the one `docs/M6.md` names as the
    // template: a restore that silently did nothing would leave every field a round trip
    // compares still matching.
    let mut machine = machine_128();
    run(&mut machine);
    let snapshot = machine.snapshot();

    let mut locked = machine_128();
    write_program(&mut locked, PROGRAM, &m7_common::out_c_a(0x7FFD, 0x20 | 3));
    set_pc(&mut locked, PROGRAM);
    for _ in 0..m7_common::OUT_C_A_STEPS {
        locked.step();
    }
    assert_eq!(
        locked.memory().slot_at(PAGED),
        spectrum::memory::Slot::Bank(BankIndex::new(3)),
        "the machine locked itself with bank 3 paged in"
    );

    locked.restore(&snapshot).expect("both are 128s");
    assert_eq!(
        locked.memory().slot_at(PAGED),
        machine.memory().slot_at(PAGED),
        "the restore must get past the lock the running machine set"
    );
}
