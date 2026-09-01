//! Gate: contention exists, and it costs the published number of T-states.
//!
//! # Why this exists
//!
//! Deleting contention outright left the boot gate **green**. It would: the ROM's start-up
//! is a sequence of instructions, not a sequence of deadlines, so running it 20 % fast
//! changes when the copyright message appears and not whether it does. The boot example even
//! prints the frame it appeared on — a real regression signal — but nothing asserts it.
//!
//! # What is graded here
//!
//! The **same instruction**, at the **same frame position**, differing only in which bank it
//! is fetched from. A 48K contends exactly one bank, so that difference is attributable to
//! contention and to nothing else — not to the instruction, not to the position, not to the
//! frame.
//!
//! The amount is asserted, not the direction: each of the eight positions within a ULA group
//! has its own published stall, and a run of sixteen fetches has a closed-form excess derived
//! in `a_run_of_fetches_costs_the_excess_the_pattern_predicts`.
//!
//! # What is not graded here
//!
//! - **The phase** — where in the frame the pattern begins. That is
//!   `contention_phase.rs`, and it is a separate gate because it is separately unverified:
//!   every assertion in this file is relative to
//!   [`FIRST_CONTENDED_T_STATE`][spectrum::timing::FIRST_CONTENDED_T_STATE] and therefore
//!   survives that constant being wrong.
//! - **I/O contention's four-case pattern**, which `crates/spectrum/src/ula.rs` unit-tests
//!   directly and nothing here reaches.
//! - **Whether the machine's own Z80 cycle lengths match the core's.** `crates/spectrum`
//!   holds its own `OPCODE_FETCH_CYCLE`/`MEMORY_CYCLE`/`PORT_CYCLE` because `crates/z80`
//!   keeps its copies private, and no gate compares the two sets directly. What this file
//!   does instead is assert hand-derived totals for real instructions, so a wrong length
//!   moves a figure here — which is a consequence, not the comparison itself.
//! - Whether the published pattern is *right*. It is the emulator community's figure for an
//!   issue 3 48K, and this project has no oracle for it — `docs/MACHINE.md`'s timing-test
//!   program is the only one available and is not written.

mod common;

use common::{
    CONTENDED_CODE, NOP, NOP_T_STATES, UNCONTENDED_CODE, advance_to, cost_of_running, machine,
    write_program,
};
use spectrum::timing::FIRST_CONTENDED_T_STATE;

/// The stall a contended access suffers, by its position within an eight-T-state ULA group.
///
/// The published 48K figures, written here as the **expectation**. Deliberately not read from
/// the crate: a table taken from the implementation agrees with any implementation.
const PUBLISHED_STALL: [u32; 8] = [6, 5, 4, 3, 2, 1, 0, 0];

/// Fetches in the run measured by `a_run_of_fetches_costs_the_excess_the_pattern_predicts`.
///
/// Sixteen is not round, it is the largest run that stays inside one line's fetch window —
/// see the derivation there.
const RUN_FETCHES: usize = 16;

/// T-states the run costs from contended memory, derived in that test.
const CONTENDED_RUN: u64 = 130;

/// T-states the same run costs from uncontended memory.
const UNCONTENDED_RUN: u64 = RUN_FETCHES as u64 * NOP_T_STATES as u64;

#[test]
fn a_contended_fetch_is_stalled_by_the_published_amount_for_its_phase() {
    for (phase, stall) in PUBLISHED_STALL.iter().enumerate() {
        let at = FIRST_CONTENDED_T_STATE + u32::try_from(phase).expect("eight phases");

        let mut contended = machine();
        advance_to(&mut contended, at);
        write_program(&mut contended, CONTENDED_CODE, &[NOP]);
        let contended_cost = cost_of_running(&mut contended, CONTENDED_CODE, 1);

        let mut uncontended = machine();
        advance_to(&mut uncontended, at);
        write_program(&mut uncontended, UNCONTENDED_CODE, &[NOP]);
        let uncontended_cost = cost_of_running(&mut uncontended, UNCONTENDED_CODE, 1);

        assert_eq!(
            uncontended_cost,
            u64::from(NOP_T_STATES),
            "phase {phase}: a NOP in bank 0 must cost its nominal length. If this fails, the \
             control is wrong and the comparison below means nothing"
        );
        assert_eq!(
            contended_cost,
            u64::from(NOP_T_STATES + stall),
            "phase {phase}: the identical NOP in the screen bank must be stalled {stall} \
             T-states. Same instruction, same frame position, one bank apart"
        );
        assert_eq!(
            contended_cost - uncontended_cost,
            u64::from(*stall),
            "phase {phase}: the whole difference between the two banks is the ULA's stall"
        );
    }
}

#[test]
fn a_run_of_fetches_costs_the_excess_the_pattern_predicts() {
    // The derivation, which is what makes 130 an assertion rather than a golden number.
    //
    // Each NOP is one opcode fetch: the ULA charges the stall for the position the fetch
    // *starts* at, then the cycle's four T-states elapse. Starting the run on the first
    // contended T-state, `column` is the offset into the line's 128 T-state fetch window:
    //
    //   fetch 0   column   0   stall 6   cost 10
    //   fetch 1   column  10   stall 4   cost  8      (10 mod 8 = 2)
    //   fetch k   column 8k+2  stall 4   cost  8      for every k >= 1
    //
    // The run self-synchronises after the first fetch: a cost of 8 is exactly one ULA group,
    // so every later fetch lands on the same position within it and stalls the same 4. That
    // holds while `column < 128`, i.e. `8k + 2 < 128`, i.e. `k <= 15` — so sixteen fetches,
    // and the seventeenth would fall in the border and stall nothing.
    //
    //   contended     10 + 15 * 8  = 130
    //   uncontended    16 * 4      =  64
    //   excess          6 + 15 * 4 =  66
    const EXCESS: u64 = 66;
    const _: () = assert!(CONTENDED_RUN - UNCONTENDED_RUN == EXCESS);

    let mut contended = machine();
    advance_to(&mut contended, FIRST_CONTENDED_T_STATE);
    write_program(&mut contended, CONTENDED_CODE, &[NOP; RUN_FETCHES]);
    let contended_cost = cost_of_running(&mut contended, CONTENDED_CODE, RUN_FETCHES);

    let mut uncontended = machine();
    advance_to(&mut uncontended, FIRST_CONTENDED_T_STATE);
    write_program(&mut uncontended, UNCONTENDED_CODE, &[NOP; RUN_FETCHES]);
    let uncontended_cost = cost_of_running(&mut uncontended, UNCONTENDED_CODE, RUN_FETCHES);

    assert_eq!(
        uncontended_cost, UNCONTENDED_RUN,
        "{RUN_FETCHES} NOPs out of bank 0 must cost their nominal length"
    );
    assert_eq!(
        contended_cost, CONTENDED_RUN,
        "{RUN_FETCHES} NOPs out of the screen bank must cost {CONTENDED_RUN} T-states: one \
         stall of 6 and fifteen of 4 on top of the nominal {UNCONTENDED_RUN}"
    );
    assert_eq!(
        contended_cost - uncontended_cost,
        EXCESS,
        "the same program in the screen bank must cost exactly {EXCESS} T-states more"
    );
}

/// One instruction, and what it costs out of each bank.
///
/// `nominal` is the **published Z80 figure**, which is the point of including it: nothing
/// else in this workspace checks that the machine's clock agrees with it. `crates/z80` grades
/// its own T-state accounting against FUSE, and `crates/spectrum/src/ula.rs` charges cycles
/// using its own copies of the same lengths — `OPCODE_FETCH_CYCLE = 4`, `MEMORY_CYCLE = 3`,
/// `PORT_CYCLE = 4`. Those two sets of constants are duplicates that no gate compares, so if
/// they diverged every contended access would be charged wrongly. The contended figures below
/// are what makes that visible.
struct Timing {
    name: &'static str,
    bytes: &'static [u8],
    nominal: u64,
    contended_at_phase_0: u64,
    contended_at_phase_7: u64,
}

/// Where `(HL)` points: the same bank as the code, so a memory operand contends too.
const CONTENDED_OPERAND: u16 = 0x4100;

/// The uncontended counterpart of [`CONTENDED_OPERAND`].
const UNCONTENDED_OPERAND: u16 = 0xC100;

/// Instructions chosen for the *shape* of their tick streams, not for coverage.
///
/// `NOP` is a bare fetch. `LD A,(HL)` adds an operand read at a second contended address.
/// `ADD HL,BC` adds seven internal cycles that ride the refresh address `IR` — which points
/// into the ROM, so they must be **free**, and that is the one thing a machine cannot
/// reconstruct from transfer addresses alone. `INC (HL)` is the read-modify-write shape —
/// fetch, read, one internal cycle at the address just read, write — which is the shape the
/// retired deferral heuristic mis-charged.
const TIMINGS: [Timing; 4] = [
    Timing {
        name: "NOP",
        bytes: &[0x00],
        nominal: 4,
        contended_at_phase_0: 10,
        contended_at_phase_7: 4,
    },
    Timing {
        name: "LD A,(HL)",
        bytes: &[0x7E],
        nominal: 7,
        contended_at_phase_0: 17,
        contended_at_phase_7: 10,
    },
    Timing {
        name: "ADD HL,BC",
        bytes: &[0x09],
        nominal: 11,
        contended_at_phase_0: 17,
        contended_at_phase_7: 11,
    },
    Timing {
        name: "INC (HL)",
        bytes: &[0x34],
        nominal: 11,
        contended_at_phase_0: 26,
        contended_at_phase_7: 19,
    },
];

#[test]
fn real_instructions_cost_their_published_length_out_of_uncontended_memory() {
    // The control, and the only check anywhere that the machine's clock and the Z80's
    // published machine cycles agree. Driven through a real `Cpu<Ula>`, which matters: every
    // other contention test in this crate synthesises the tick stream by hand, so the ULA's
    // cycle accounting is otherwise graded only against streams this crate wrote itself.
    for timing in &TIMINGS {
        let mut machine = machine();
        advance_to(&mut machine, FIRST_CONTENDED_T_STATE);
        write_program(&mut machine, UNCONTENDED_CODE, timing.bytes);
        point_hl_at(&mut machine, UNCONTENDED_OPERAND);

        assert_eq!(
            cost_of_running(&mut machine, UNCONTENDED_CODE, 1),
            timing.nominal,
            "{} out of bank 0 must cost its published {} T-states",
            timing.name,
            timing.nominal
        );
    }
}

#[test]
fn real_instructions_are_stalled_by_the_pattern_at_each_cycle_they_open() {
    // Hand-derived from the published pattern, one machine cycle at a time. `INC (HL)` is
    // the one worth writing out, at both phases, because it is the instruction whose figure
    // the machine used to get wrong. Its four machine cycles, as a recording bus observes
    // them from a real `Cpu`, are `pc:4, hl:3, hl:1, hl:3` — each contending once, at the
    // address it drives, at the moment it opens.
    //
    //   phase 0                                   | phase 7
    //   fetch 0x4000 at +0  delay 6 -> +6,  4 -> +10 | at +7  delay 0 -> +7,  4 -> +11
    //   read  0x4100 at +10 delay 4 -> +14, 3 -> +17 | at +11 delay 3 -> +14, 3 -> +17
    //   intl  0x4100 at +17 delay 5 -> +22, 1 -> +23 | at +17 delay 5 -> +22, 1 -> +23
    //   write 0x4100 at +23 delay 0 -> +23, 3 -> +26 | at +23 delay 0 -> +23, 3 -> +26
    //
    // 26 and 19 — the runs converge on +26 because a cost of 8 is one whole ULA group, so a
    // run re-synchronises after its first cycle.
    //
    // The retired deferral heuristic reached 25 and 18. It could not see the third cycle —
    // a read followed by exactly one internal cycle at the same address was byte-identical
    // to an opcode fetch — so it dropped that stall and opened the write four T-states early
    // at +18, where the pattern stalls 4 rather than 0. **The lost quantity was one
    // contention point; the visible error was one T-state, not the five the missing stall
    // suggests**, because every stall shifts the ones after it and four of the five came
    // straight back. A figure of 30 comes from adding delay(17)=5 to the observed 25 without
    // re-siting the write, and is wrong.
    for timing in &TIMINGS {
        for (phase, expected) in [
            (0, timing.contended_at_phase_0),
            (7, timing.contended_at_phase_7),
        ] {
            let mut machine = machine();
            advance_to(&mut machine, FIRST_CONTENDED_T_STATE + phase);
            write_program(&mut machine, CONTENDED_CODE, timing.bytes);
            point_hl_at(&mut machine, CONTENDED_OPERAND);

            assert_eq!(
                cost_of_running(&mut machine, CONTENDED_CODE, 1),
                expected,
                "{} from the screen bank at phase +{phase} must cost {expected} T-states \
                 against its nominal {}",
                timing.name,
                timing.nominal
            );
        }
    }
}

/// Point `HL` at `address`, so a memory operand lands where the caller intends.
fn point_hl_at(machine: &mut spectrum::Spectrum, address: u16) {
    let mut state = machine.cpu_state();
    state.hl = address;
    machine.set_cpu_state(state);
}

#[test]
fn a_run_outside_the_fetch_window_costs_the_same_in_either_bank() {
    // The control for the whole file. Contention is a property of *when*, not only of
    // *where*: the same contended bank, at a position the ULA is not fetching in, must cost
    // exactly what uncontended memory costs. A model that stalled on address alone would
    // pass every assertion above and fail this one.
    let before_display = FIRST_CONTENDED_T_STATE - NOP_T_STATES * 8;

    let mut contended = machine();
    advance_to(&mut contended, before_display);
    write_program(&mut contended, CONTENDED_CODE, &[NOP; 4]);
    let contended_cost = cost_of_running(&mut contended, CONTENDED_CODE, 4);

    assert_eq!(
        contended_cost,
        4 * u64::from(NOP_T_STATES),
        "the screen bank must be free while the ULA is in the top border"
    );
}
