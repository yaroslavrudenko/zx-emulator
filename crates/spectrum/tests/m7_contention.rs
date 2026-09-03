//! Gate: contention is a property of the **bank**, and a 128 can put a contended one anywhere.
//!
//! # The experiment a 48K cannot run
//!
//! Every 48K contention gate compares two *addresses* — `0x4000` against `0xC000` — and so
//! measures "contention" and "which slot" together, because a 48K's one contended bank is nailed
//! to `0x4000`. A model that keyed contention off the address range instead of the bank passes
//! all of them.
//!
//! A 128 separates the two. Here every measurement runs **the same instruction at the same
//! address at the same frame position**, and changes only which bank is paged into `0xC000`.
//! The single remaining variable is the bank, so a difference is attributable to the bank and to
//! nothing else.
//!
//! # Every expected number is derived from the pattern before it is measured
//!
//! Not one figure here was read off a run and promoted. Each is worked out from
//! `6,5,4,3,2,1,0,0` at the machine's own phase, one stall at a time, with the arithmetic written
//! out beside the assertion — because a stall shifts every stall after it, so a missing one
//! cannot be added to a total and an observed total cannot be reasoned backwards into a model.
//!
//! # What is **not** graded here
//!
//! - **Whether the 128's offset is right.** These gates position themselves at whatever the
//!   machine's own `first_contended_t_state` is, so they measure the *mechanism* and are silent
//!   about the *number* — which remains true and is the point of the row. *(It read **"Whether
//!   14361 is right … nothing in this repository can grade the 128's number"**. Both halves are
//!   now false: the number is **14362**, and `tests/timing_oracle.rs`'s 128 edition graded it
//!   against hardware on 2026-09-02. What survives is the scoping claim — that *these* gates do
//!   not.)* See [`spectrum::timing::Timing::SPECTRUM_128`].
//! - **Whether banks 1, 3, 5 and 7 are the right set.** Transcribed, and graded against the
//!   transcription.

mod common;
mod m7_common;

use common::{NOP, advance_to, set_pc, with_cpu_state, write_program};
use m7_common::{OUT_C_A_STEPS, cost_of_running, machine_128, out_c_a};
use spectrum::Spectrum;
use spectrum::timing::Timing;

/// The paging port, and where test programs live: bank 2, uncontended on both machines.
const PAGING_PORT: u16 = 0x7FFD;
const PROGRAM: u16 = common::PROLOGUE;

/// The slot a 128 can page any bank into, and where the measured instructions run.
const PAGEABLE: u16 = 0xC000;

/// A bank a 128 contends, and one it does not. Neighbours, so nothing else distinguishes them.
const CONTENDED_BANK: u8 = 1;
const UNCONTENDED_BANK: u8 = 0;

/// `ADD HL,BC` — one M1 fetch and **seven internal cycles on the refresh address**.
const ADD_HL_BC: u8 = 0x09;

/// A 128 with `bank` at `0xC000`, positioned at `target`, paged **through the bus**.
///
/// `advance_to` asserts it is handed a machine that has not run, so the paging write cannot
/// precede it. It goes afterwards instead, out of bank 2 — which no paging value moves, and
/// which is uncontended on both machines, so the write costs the same in every arm.
fn paged_at(bank: u8, target: u32) -> Spectrum {
    let mut machine = machine_128();
    advance_to(&mut machine, target);
    write_program(&mut machine, PROGRAM, &out_c_a(PAGING_PORT, bank));
    set_pc(&mut machine, PROGRAM);
    for _ in 0..OUT_C_A_STEPS {
        machine.step();
    }
    machine
}

/// Where a 128 begins contending.
fn first_contended() -> u32 {
    Timing::SPECTRUM_128.first_contended_t_state()
}

/// T-states the `OUT` prologue costs out of uncontended bank 2: `LD BC,nn` `LD A,n` `OUT (C),A`.
///
/// 10 + 7 + 12. Derived rather than measured, so that a change in the fixture's own cost shows
/// up as a failure here rather than as a silent shift in every figure below.
const PAGING_WRITE_T_STATES: u32 = 10 + 7 + 12;

#[test]
fn the_paging_prologue_costs_what_it_is_derived_to_cost() {
    // The fixture's own cost, asserted before anything is measured through it. Every position
    // below is `first_contended() - PAGING_WRITE_T_STATES` so that the measured instruction
    // starts exactly on the first contended T-state; if this figure were wrong, every
    // derivation in this file would be measuring a different phase and would still be
    // self-consistent.
    let mut machine = machine_128();
    let before = machine.frame_t_state();
    write_program(
        &mut machine,
        PROGRAM,
        &out_c_a(PAGING_PORT, UNCONTENDED_BANK),
    );
    set_pc(&mut machine, PROGRAM);
    for _ in 0..OUT_C_A_STEPS {
        machine.step();
    }
    assert_eq!(machine.frame_t_state() - before, PAGING_WRITE_T_STATES);
}

/// A machine paged to `bank` and standing exactly on its first contended T-state.
fn armed(bank: u8) -> Spectrum {
    let machine = paged_at(bank, first_contended() - PAGING_WRITE_T_STATES);
    assert_eq!(
        machine.frame_t_state(),
        first_contended(),
        "the fixture must land on the first contended T-state, not near it"
    );
    machine
}

#[test]
fn the_same_nops_at_the_same_address_cost_more_from_a_contended_bank() {
    // Derived from `6,5,4,3,2,1,0,0` at the machine's own phase, one stall at a time:
    //
    //   NOP 1  fetch at +0   -> delay 6, then 4 T-states  -> +10
    //   NOP 2  fetch at +10  -> (10 & 7) = 2 -> delay 4, +4 -> +18
    //   NOP 3  fetch at +18  -> (18 & 7) = 2 -> delay 4, +4 -> +26
    //   NOP 4  fetch at +26  -> (26 & 7) = 2 -> delay 4, +4 -> +34
    //
    // Every NOP after the first costs 8, because 8 T-states lands on the same phase — which is
    // why the first is the only one that differs and why a four-NOP run is a sharper test than
    // a one-NOP one.
    const NOPS: usize = 4;
    const CONTENDED_COST: u64 = 34;
    const UNCONTENDED_COST: u64 = 4 * NOPS as u64;

    let mut contended = armed(CONTENDED_BANK);
    write_program(&mut contended, PAGEABLE, &[NOP; NOPS]);
    assert_eq!(
        cost_of_running(&mut contended, PAGEABLE, NOPS),
        CONTENDED_COST,
        "bank {CONTENDED_BANK} at 0xC000 must be contended"
    );

    let mut uncontended = armed(UNCONTENDED_BANK);
    write_program(&mut uncontended, PAGEABLE, &[NOP; NOPS]);
    assert_eq!(
        cost_of_running(&mut uncontended, PAGEABLE, NOPS),
        UNCONTENDED_COST,
        "bank {UNCONTENDED_BANK} at 0xC000 must not be"
    );
}

#[test]
fn all_four_odd_banks_are_contended_at_c000_and_the_four_even_ones_are_not() {
    // The set, through the machine, at one address. A model that contended "the screen bank" —
    // or the `0x4000` range — is right about bank 5 and wrong about the other three, and would
    // pass every gate that only ever reaches bank 5.
    const NOPS: usize = 4;
    for bank in 0..8_u8 {
        let mut machine = armed(bank);
        write_program(&mut machine, PAGEABLE, &[NOP; NOPS]);
        let cost = cost_of_running(&mut machine, PAGEABLE, NOPS);
        let expected = if bank % 2 == 1 { 34 } else { 16 };
        assert_eq!(cost, expected, "bank {bank} paged into 0xC000");
    }
}

#[test]
fn the_contended_set_does_not_follow_the_screen_select() {
    // Bit 3 chooses which bank the ULA *draws*; it chooses nothing about which banks it
    // *contends*. Both are true at once and a model that conflated them would pass the test
    // above, because bank 5 is contended either way.
    const NOPS: usize = 4;
    for screen in [0x00_u8, 0x08] {
        for bank in [UNCONTENDED_BANK, CONTENDED_BANK] {
            let mut machine = armed(screen | bank);
            write_program(&mut machine, PAGEABLE, &[NOP; NOPS]);
            let cost = cost_of_running(&mut machine, PAGEABLE, NOPS);
            let expected = if bank % 2 == 1 { 34 } else { 16 };
            assert_eq!(
                cost, expected,
                "bank {bank} with the screen select at {screen:#04X}"
            );
        }
    }
}

#[test]
fn internal_cycles_on_a_contended_refresh_address_are_each_charged() {
    // **The case `docs/STATUS.md` says M7 makes routine.** A 48K reaches it only by an
    // arrangement it cannot page into; a 128 reaches it whenever a contended bank sits under
    // the refresh address. `I` is set to 0xC0, so `IR` is somewhere in `0xC000..0xC0FF` — slot
    // 3, whichever bank is paged there — for every value of `R`.
    //
    // `ADD HL,BC` is one M1 fetch **out of uncontended bank 2** and seven internal cycles on
    // `IR`. So the fetch is free in both arms and the seven cycles are the whole difference:
    //
    //   fetch  at +0   uncontended, 4 T-states               -> +4
    //   tick 1 at +4   (4 & 7) = 4 -> delay 2, then 1        -> +7
    //   tick 2 at +7   (7 & 7) = 7 -> delay 0, then 1        -> +8
    //   tick 3 at +8   (8 & 7) = 0 -> delay 6, then 1        -> +15
    //   tick 4 at +15  -> 0, then 1                          -> +16
    //   tick 5 at +16  -> 6, then 1                          -> +23
    //   tick 6 at +23  -> 0, then 1                          -> +24
    //   tick 7 at +24  -> 6, then 1                          -> +31
    const CONTENDED_COST: u64 = 31;
    const UNCONTENDED_COST: u64 = 11;

    for (bank, expected) in [
        (CONTENDED_BANK, CONTENDED_COST),
        (UNCONTENDED_BANK, UNCONTENDED_COST),
    ] {
        let mut machine = armed(bank);
        with_cpu_state(&mut machine, |state| state.i = 0xC0);
        write_program(&mut machine, PROGRAM, &[ADD_HL_BC]);
        assert_eq!(
            cost_of_running(&mut machine, PROGRAM, 1),
            expected,
            "ADD HL,BC with IR in bank {bank}"
        );
    }
}

#[test]
fn a_contended_bank_is_contended_in_every_slot_it_can_reach() {
    // Bank 5 is contended at `0x4000` on both machines. On a 128 it can also be paged into
    // `0xC000`, and it must be contended there too — same bank, different address. This is the
    // property `memory.rs` has asserted since M5 in the small; here it is through the machine,
    // measured on the clock.
    const NOPS: usize = 4;

    let mut at_c000 = armed(5);
    write_program(&mut at_c000, PAGEABLE, &[NOP; NOPS]);
    assert_eq!(cost_of_running(&mut at_c000, PAGEABLE, NOPS), 34);

    let mut at_4000 = armed(UNCONTENDED_BANK);
    write_program(&mut at_4000, 0x4000, &[NOP; NOPS]);
    assert_eq!(
        cost_of_running(&mut at_4000, 0x4000, NOPS),
        34,
        "bank 5 at its own address costs exactly the same"
    );
}

#[test]
fn nothing_is_contended_one_t_state_before_the_window_opens() {
    // The edge, on the machine's own phase. A model that started one T-state early would show
    // a stall here and would otherwise be indistinguishable from a correct one.
    let mut machine = paged_at(
        CONTENDED_BANK,
        first_contended() - PAGING_WRITE_T_STATES - 1,
    );
    assert_eq!(machine.frame_t_state(), first_contended() - 1);
    write_program(&mut machine, PAGEABLE, &[NOP]);
    assert_eq!(
        cost_of_running(&mut machine, PAGEABLE, 1),
        4,
        "a fetch starting one T-state early must not stall"
    );
}

#[test]
fn a_128_contends_at_its_own_phase_and_not_at_the_48ks() {
    // The two machines' phases differ by 26 T-states, so a 128 running the 48K's constant
    // would stall here and must not. This is the one assertion in the file that is about the
    // *number* rather than the mechanism — and it grades only that the two differ, which is
    // exactly what the 48K oracle's 14361 mutation established and no more.
    let forty_eight = Timing::SPECTRUM_48K.first_contended_t_state();
    assert!(forty_eight < first_contended());

    let mut machine = paged_at(CONTENDED_BANK, forty_eight - PAGING_WRITE_T_STATES);
    assert_eq!(machine.frame_t_state(), forty_eight);
    write_program(&mut machine, PAGEABLE, &[NOP]);
    assert_eq!(
        cost_of_running(&mut machine, PAGEABLE, 1),
        4,
        "a 128 must not contend at the 48K's first contended T-state"
    );
}
