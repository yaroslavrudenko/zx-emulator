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
//! **The whole read-modify-write family, not one member of it.** `docs/STATUS.md` listed
//! *"the read-modify-write family is gated by one member"* among the properties nothing
//! covers: `INC (HL)` was exercised, while `INC (IX+d)`, the `CB` memory group and
//! `EX (SP),HL` took the corrected path *by construction* — an argument, not a verdict. All
//! four shapes are now driven through a real `Cpu<Ula>` against hand-derived totals, and the
//! indexed pair are separated from each other by
//! `the_index_computation_is_charged_on_the_displacement_address`, which moves the operand
//! out of the contended bank so that **where** the index-computation T-states are spent
//! becomes observable rather than merely asserted.
//!
//! Every expected figure here was derived from the hardware **before** the emulator was
//! measured, from a machine-cycle list recorded off a real `Cpu` rather than read off the
//! source, and cross-checked against a second implementation of the published delay rule
//! written with no sight of the crate. `docs/STATUS.md` records why that discipline is not
//! ceremony: two derivations of `INC (HL)`'s contended cost disagreed, 26 against 30, and the
//! wrong one came from adjusting an observed total instead of re-deriving it.
//!
//! # What is not graded here
//!
//! - **The phase** — where in the frame the pattern begins. That is `contention_phase.rs`,
//!   and it is a separate gate because every assertion in this file is relative to
//!   [`FIRST_CONTENDED_T_STATE`][spectrum::timing::FIRST_CONTENDED_T_STATE] and therefore
//!   survives that constant being wrong. **This bullet also said "because it is separately
//!   unverified", and that clause is now false.** `tests/timing_oracle.rs` establishes —
//!   narrowly — that **the first contended T-state falls exactly 14335 after `/INT`**, given
//!   that this machine asserts `/INT` at frame T-state 0. What stays open is the frame's
//!   **origin** (a convention: moving `/INT` and the window together leaves the oracle
//!   green), the interrupt window's **length** (32 → 24 leaves it green), and the
//!   `64 × 224` **factorisation**, whose *product* is measured while its factors are not.
//!   `contention_phase.rs` still opens with "the absolute phase remains unverified against
//!   any external oracle" and has **not** been corrected; `docs/STATUS.md` carries the three
//!   rows that remain.
//! - **I/O contention's four-case pattern**, which is `io_contention.rs` — a separate gate
//!   because a port cycle is priced by a different rule from a memory cycle and reads the
//!   clock rather than the cycle.
//! - Whether the published pattern is *right*. It is the emulator community's figure for an
//!   issue 3 48K, and nothing here **establishes** it. **The clause that used to follow —
//!   "`docs/MACHINE.md`'s timing-test program is the only one available and is not written" —
//!   is false: it is written.** It is `tests/timing_oracle.rs`, which is `MACHINE.md`'s
//!   verification item 2. That narrows this row without closing it. The oracle *constrains*
//!   the pattern — `docs/MACHINE.md`'s mutation table has `DELAY_PATTERN`'s last slot
//!   `0 → 1` red at 14 of 68 hardware rows and the pattern zeroed red at 38 — but it compares
//!   a whole frame's **integrated** cost over hundreds of loop iterations, not a single
//!   cycle's stall. So it cannot single `[6,5,4,3,2,1,0,0]` out from another pattern with the
//!   same integral over those loops. That last sentence is reasoning from what the suite
//!   measures, **not** a mutation anybody has run; whoever wants it as a measurement should
//!   construct such a pattern and run the oracle against it.
//! - **`DEC`, and the `IY` half of every indexed form.** They share their handlers with the
//!   forms above, which is the *by construction* argument this file exists to distrust — so
//!   it is recorded rather than relied on. (The *block* families' decrementing twins are no
//!   longer in this list: `block_contention.rs` drives all eight, and records both streams
//!   rather than arguing they must match.)
//!
//! # What was in that list and is now graded elsewhere
//!
//! - **Whether the machine's own Z80 cycle lengths match the core's** was listed here as a
//!   duplication no gate compared, whose divergence would move a figure in this file — *"a
//!   consequence, not the comparison itself."* **There is now nothing to compare.**
//!   `crates/z80` exports [`OPCODE_FETCH_T_STATES`][z80::OPCODE_FETCH_T_STATES] and its two
//!   neighbours as part of the [`Bus`][z80::Bus] contract, because they are the decoding key
//!   for the tick stream rather than an internal choice, and `crates/spectrum/src/ula.rs`
//!   consumes them. One definition, so a divergence is not a failure to detect — it is
//!   unrepresentable. What still grades the *value* is the `nominal` column below, written
//!   from the published Z80 figures and not read from either crate.
//! - **The `DD`/`FD` prefix chain** — `prefix_chain_contention.rs`.
//! - **The block instructions** — `block_contention.rs`, which separates the four repeat
//!   addresses from each other rather than only pricing the four totals.

mod common;

use common::{
    CONTENDED_CODE, NOP, NOP_T_STATES, UNCONTENDED_CODE, advance_to, cost_of_running, machine,
    with_cpu_state, write_program,
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
/// using [`OPCODE_FETCH_T_STATES`][z80::OPCODE_FETCH_T_STATES],
/// [`MEMORY_ACCESS_T_STATES`][z80::MEMORY_ACCESS_T_STATES] and
/// [`PORT_ACCESS_T_STATES`][z80::PORT_ACCESS_T_STATES] — the core's own constants, exported
/// as part of the [`Bus`][z80::Bus] contract, so the machine and the CPU cannot disagree
/// about a cycle's length.
///
/// That removes the *divergence* and leaves the *value*, which is what this column is for:
/// one wrong number now moves the core's accounting and the ULA's together, and only an
/// expectation written independently of both can see it. These are written from the
/// published figures.
struct Timing {
    name: &'static str,
    bytes: &'static [u8],
    /// Which register the instruction reaches its memory operand through.
    pointer: Pointer,
    nominal: u64,
    contended_at_phase_0: u64,
    contended_at_phase_7: u64,
}

/// The register an instruction's memory operand is addressed by.
///
/// The read-modify-write family reaches memory three different ways, and a gate that only
/// ever set `HL` could only ever exercise one of them.
#[derive(Clone, Copy)]
enum Pointer {
    /// `(HL)`, and the harmless default for an instruction with no memory operand.
    Hl,
    /// `(IX+d)`, with the displacement byte assembled as zero.
    Ix,
    /// `(SP)` — the two bytes `EX (SP),HL` exchanges.
    Sp,
}

/// Where the memory operand lives: the same bank as the code, so it contends too.
const CONTENDED_OPERAND: u16 = 0x4100;

/// The uncontended counterpart of [`CONTENDED_OPERAND`].
const UNCONTENDED_OPERAND: u16 = 0xC100;

/// Instructions chosen for the *shape* of their machine-cycle streams, not for coverage.
///
/// The streams are not assumed. Each was recorded from a real `Cpu` driven through a bus
/// that logs every `fetch`/`read`/`write`/`tick` with the address it drives, and the
/// arithmetic below walks the recorded list rather than a decomposition read off the source.
///
/// | | machine cycles | nominal |
/// |---|---|---|
/// | `NOP` | `pc:4` | 4 |
/// | `LD A,(HL)` | `pc:4, hl:3` | 7 |
/// | `ADD HL,BC` | `pc:4, ir:1 x7` | 11 |
/// | `INC (HL)` | `pc:4, hl:3, hl:1, hl:3` | 11 |
/// | `RLC (HL)` | `pc:4, pc+1:4, hl:3, hl:1, hl:3` | 15 |
/// | `INC (IX+d)` | `pc:4, pc+1:4, pc+2:3, pc+2:1 x5, ix:3, ix:1, ix:3` | 23 |
/// | `RLC (IX+d)` | `pc:4, pc+1:4, pc+2:3, pc+3:3, pc+3:1 x2, ix:3, ix:1, ix:3` | 23 |
/// | `EX (SP),HL` | `pc:4, sp:3, sp+1:3, sp+1:1, sp+1:3, sp:3, sp:1 x2` | 19 |
///
/// `NOP` is a bare fetch. `LD A,(HL)` adds an operand read at a second contended address.
/// `ADD HL,BC` adds seven internal cycles that ride the refresh address `IR` — which points
/// into the ROM, so they must be **free**, and that is the one thing a machine cannot
/// reconstruct from transfer addresses alone.
///
/// The last five are the **read-modify-write family**, the shape the retired deferral
/// heuristic mis-charged: a read, exactly one internal cycle at the address just read, then
/// the write-back. `docs/STATUS.md` recorded that family as *"gated by one member"* —
/// `INC (HL)` — with the other three taking the corrected path **by construction**, which is
/// an argument rather than a verdict. They are gated here.
///
/// `INC (IX+d)` is **not** a longer `INC (HL)`, and its cycle list is why. Five T-states are
/// owed between the displacement byte and the memory access, and they are spent as **five
/// separate one-T-state internal cycles on the displacement's own address** — `PC+2`, which
/// is in the *code*, not at `IX+d`. Each contends on its own account, so the instruction has
/// **eleven** contention points against `INC (HL)`'s four.
///
/// `RLC (IX+d)` is the `DDCB` shape, where the displacement byte comes *before* the opcode
/// byte and the opcode's own three-T-state read spends three of the five owed T-states — so
/// only **two** internal cycles remain, on `PC+3`. That is `Z80-REFERENCE.md`'s rule
/// *"any 3-T fetch in between spends three of them"*, and the recording confirms it.
const TIMINGS: [Timing; 8] = [
    Timing {
        name: "NOP",
        bytes: &[0x00],
        pointer: Pointer::Hl,
        nominal: 4,
        contended_at_phase_0: 10,
        contended_at_phase_7: 4,
    },
    Timing {
        name: "LD A,(HL)",
        bytes: &[0x7E],
        pointer: Pointer::Hl,
        nominal: 7,
        contended_at_phase_0: 17,
        contended_at_phase_7: 10,
    },
    Timing {
        name: "ADD HL,BC",
        bytes: &[0x09],
        pointer: Pointer::Hl,
        nominal: 11,
        contended_at_phase_0: 17,
        contended_at_phase_7: 11,
    },
    Timing {
        name: "INC (HL)",
        bytes: &[0x34],
        pointer: Pointer::Hl,
        nominal: 11,
        contended_at_phase_0: 26,
        contended_at_phase_7: 19,
    },
    Timing {
        name: "RLC (HL)",
        bytes: &[0xCB, 0x06],
        pointer: Pointer::Hl,
        nominal: 15,
        contended_at_phase_0: 34,
        contended_at_phase_7: 27,
    },
    Timing {
        name: "INC (IX+d)",
        bytes: &[0xDD, 0x34, 0x00],
        pointer: Pointer::Ix,
        nominal: 23,
        contended_at_phase_0: 58,
        contended_at_phase_7: 51,
    },
    Timing {
        name: "RLC (IX+d)",
        bytes: &[0xDD, 0xCB, 0x00, 0x06],
        pointer: Pointer::Ix,
        nominal: 23,
        contended_at_phase_0: 58,
        contended_at_phase_7: 51,
    },
    Timing {
        name: "EX (SP),HL",
        bytes: &[0xE3],
        pointer: Pointer::Sp,
        nominal: 19,
        contended_at_phase_0: 48,
        contended_at_phase_7: 41,
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
        point_at(&mut machine, timing.pointer, UNCONTENDED_OPERAND);

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
    //
    // The rest of the read-modify-write family, priced the same way at phase 0. Each cycle
    // is charged the stall for the column it *opens* at, and then its own T-states elapse.
    //
    //   RLC (HL)     fetch 4000 +0  d6 -> +6,  4 -> +10 | fetch 4001 +10 d4 -> +14, 4 -> +18
    //                read  4100 +18 d4 -> +22, 3 -> +25 | intl  4100 +25 d5 -> +30, 1 -> +31
    //                write 4100 +31 d0 -> +31, 3 -> +34                                   34
    //
    //   INC (IX+d)   fetch 4000 +0  d6 -> +6,  4 -> +10 | fetch 4001 +10 d4 -> +14, 4 -> +18
    //                read  4002 +18 d4 -> +22, 3 -> +25 | intl  4002 +25 d5 -> +30, 1 -> +31
    //                intl  4002 +31 d0 -> +31, 1 -> +32 | intl  4002 +32 d6 -> +38, 1 -> +39
    //                intl  4002 +39 d0 -> +39, 1 -> +40 | intl  4002 +40 d6 -> +46, 1 -> +47
    //                read  4100 +47 d0 -> +47, 3 -> +50 | intl  4100 +50 d4 -> +54, 1 -> +55
    //                write 4100 +55 d0 -> +55, 3 -> +58                                   58
    //
    //   RLC (IX+d)   fetch 4000 +0  d6 -> +6,  4 -> +10 | fetch 4001 +10 d4 -> +14, 4 -> +18
    //                read  4002 +18 d4 -> +22, 3 -> +25 | read  4003 +25 d5 -> +30, 3 -> +33
    //                intl  4003 +33 d5 -> +38, 1 -> +39 | intl  4003 +39 d0 -> +39, 1 -> +40
    //                read  4100 +40 d6 -> +46, 3 -> +49 | intl  4100 +49 d5 -> +54, 1 -> +55
    //                write 4100 +55 d0 -> +55, 3 -> +58                                   58
    //
    //   EX (SP),HL   fetch 4000 +0  d6 -> +6,  4 -> +10 | read  4100 +10 d4 -> +14, 3 -> +17
    //                read  4101 +17 d5 -> +22, 3 -> +25 | intl  4101 +25 d5 -> +30, 1 -> +31
    //                write 4101 +31 d0 -> +31, 3 -> +34 | write 4100 +34 d4 -> +38, 3 -> +41
    //                intl  4100 +41 d5 -> +46, 1 -> +47 | intl  4100 +47 d0 -> +47, 1 -> +48
    //                                                                                     48
    //
    // Phase 7 is uniformly `phase 0 - 7` across every row, and that is a property rather
    // than a coincidence: the first cycle absorbs the offset — at +7 the pattern stalls 0
    // and at +0 it stalls 6, so a four-T-state fetch ends at +11 against +10 — and the
    // second cycle's stall closes the gap. From then on the two runs are the same walk.
    //
    // The two indexed forms agree at 58, and that is also not a transcription slip: they
    // share their first two cycles, so both diverge from +18, and both are back at +55 when
    // they open the write. What differs is *how many* contention points they have — eleven
    // against nine. `the_index_computation_is_charged_on_the_displacement_address` separates
    // them by moving the operand out of the contended bank so the tails cannot coincide.
    for timing in &TIMINGS {
        for (phase, expected) in [
            (0, timing.contended_at_phase_0),
            (7, timing.contended_at_phase_7),
        ] {
            let mut machine = machine();
            advance_to(&mut machine, FIRST_CONTENDED_T_STATE + phase);
            write_program(&mut machine, CONTENDED_CODE, timing.bytes);
            point_at(&mut machine, timing.pointer, CONTENDED_OPERAND);

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

/// Point the instruction's operand register at `address`.
fn point_at(machine: &mut spectrum::Spectrum, pointer: Pointer, address: u16) {
    with_cpu_state(machine, |state| match pointer {
        Pointer::Hl => state.hl = address,
        Pointer::Ix => state.ix = address,
        Pointer::Sp => state.sp = address,
    });
}

/// The indexed forms, with their operand moved out of the contended bank.
///
/// Same two instructions as [`TIMINGS`], same contended code, operand at
/// [`UNCONTENDED_OPERAND`] instead. Derived in the test below.
const INDEXED_WITH_FREE_OPERAND: [(&str, &[u8], u64, u64); 2] = [
    ("INC (IX+d)", &[0xDD, 0x34, 0x00], 54, 47),
    ("RLC (IX+d)", &[0xDD, 0xCB, 0x00, 0x06], 47, 40),
];

#[test]
fn the_index_computation_is_charged_on_the_displacement_address() {
    // Five T-states are owed between an index instruction's displacement byte and its
    // memory access. **Where they are spent is a claim about the hardware, and it is not
    // the operand.** They ride `PC+2` — the displacement byte's own address, in the code —
    // which the recording bus shows directly: eight ticks at `PC+2` after a three-T-state
    // read there, and only then a transfer at `IX+d`.
    //
    // Nothing could see that while the code and the operand were in the same bank, because
    // both addresses contend identically and the cost is the same either way. Splitting them
    // is what makes the claim observable: code in the screen bank, operand in bank 0.
    //
    //   INC (IX+d), operand at 0xC100
    //     fetch 4000 +0  d6 -> +6,  4 -> +10 | fetch 4001 +10 d4 -> +14, 4 -> +18
    //     read  4002 +18 d4 -> +22, 3 -> +25 | intl  4002 +25 d5 -> +30, 1 -> +31
    //     intl  4002 +31 d0 -> +31, 1 -> +32 | intl  4002 +32 d6 -> +38, 1 -> +39
    //     intl  4002 +39 d0 -> +39, 1 -> +40 | intl  4002 +40 d6 -> +46, 1 -> +47
    //     read  C100 +47 free,      3 -> +50 | intl  C100 +50 free,      1 -> +51
    //     write C100 +51 free,      3 -> +54                                       54
    //
    //   RLC (IX+d), operand at 0xC100 — the DDCB shape, where the opcode byte's own
    //   three-T-state read spends three of the five owed T-states and two internals remain
    //     fetch 4000 +0  d6 -> +6,  4 -> +10 | fetch 4001 +10 d4 -> +14, 4 -> +18
    //     read  4002 +18 d4 -> +22, 3 -> +25 | read  4003 +25 d5 -> +30, 3 -> +33
    //     intl  4003 +33 d5 -> +38, 1 -> +39 | intl  4003 +39 d0 -> +39, 1 -> +40
    //     read  C100 +40 free,      3 -> +43 | intl  C100 +43 free,      1 -> +44
    //     write C100 +44 free,      3 -> +47                                       47
    //
    // The counterfactual is what gives this test its teeth. A machine that spent those five
    // T-states at `IX+d` rather than at `PC+2` would charge them **nothing at all** here,
    // because `0xC100` is uncontended — and `INC (IX+d)` would come out at **37**, seventeen
    // T-states short. Both machines agree exactly when the operand is contended, which is
    // why the row in `TIMINGS` cannot distinguish them and this one can.
    for (name, bytes, at_phase_0, at_phase_7) in INDEXED_WITH_FREE_OPERAND {
        for (phase, expected) in [(0, at_phase_0), (7, at_phase_7)] {
            let mut machine = machine();
            advance_to(&mut machine, FIRST_CONTENDED_T_STATE + phase);
            write_program(&mut machine, CONTENDED_CODE, bytes);
            point_at(&mut machine, Pointer::Ix, UNCONTENDED_OPERAND);

            assert_eq!(
                cost_of_running(&mut machine, CONTENDED_CODE, 1),
                expected,
                "{name} from the screen bank with its operand in bank 0, at phase +{phase}, \
                 must cost {expected} T-states: the index computation is charged on the \
                 displacement's address and only the operand access is free"
            );
        }
    }
}

/// `ADD HL,BC`'s cost, by phase, with its refresh address in the contended bank.
///
/// Derived in the test below. Uncontended code, so the fetch is free and the seven internal
/// cycles begin at group position `phase + 4`.
const CONTENDED_REFRESH_RUN: [u64; 8] = [31, 30, 29, 29, 35, 34, 33, 32];

#[test]
fn an_internal_run_on_a_contended_refresh_address_is_charged_at_every_t_state() {
    // **This test exists because a mutation survived.** Making a standalone internal cycle
    // leave one T-state marked as already paid for — so that only every *other* T-state of
    // an internal run contends — left the entire workspace green, and an independent
    // derivation then showed why: over all 120 start columns of a display line, for every
    // member of the read-modify-write family, the mutation is **observationally
    // identical**.
    //
    // That is a property of the pattern, not an accident. A contended one-T-state internal
    // cycle at group position `k <= 6` stalls `6 - k` and therefore ends at
    // `k + (6 - k) + 1 = 7` (mod 8), where the stall is **zero**; from 7 it ends at 0, where
    // the stall is 6. So a contended internal run strictly alternates charged, free, charged,
    // free — and skipping every second one skips exactly the free ones.
    //
    // The mutation can only bite where the run *begins* on position 7, and no instruction in
    // `TIMINGS` can start one there: a contended read or write always ends at position 2 and
    // a contended fetch always ends at 3, whatever position it began at. **The run has to
    // start from an uncontended cycle onto a contended address**, which needs the internal
    // cycles to ride an address the preceding transfer did not — and there is exactly one
    // such shape in the instruction set: the internal cycles that follow an opcode fetch sit
    // on the refresh address `IR`, not on `PC`.
    //
    // So: `ADD HL,BC` out of **uncontended** code, with `I = 0x40` putting `IR` in the
    // screen bank. The fetch is free and ends at `phase + 4`; the seven internals walk the
    // pattern from there, each charged at the column it opens at:
    //
    //   phase 3  fetch  C000  free, 4 T -> col 7
    //            intl   4001  col  7  d0 -> +1 -> col  8   (the run *begins* on a zero)
    //            intl   4001  col  8  d6 -> +7 -> col 15
    //            intl   4001  col 15  d0 -> +1 -> col 16
    //            intl   4001  col 16  d6 -> +7 -> col 23
    //            intl   4001  col 23  d0 -> +1 -> col 24
    //            intl   4001  col 24  d6 -> +7 -> col 31
    //            intl   4001  col 31  d0 -> +1 -> col 32     32 - 3 = 29
    //
    // and under the mutation the second internal — the one at column 8, worth **6** — is the
    // one skipped, giving 28. One T-state, at one phase, and it is the only place in the
    // whole model where the claim *"each internal cycle contends on its own account"* is
    // observable from an instruction's total at all.
    //
    // The other phases follow the same walk from a different entry column. The run costs 8
    // T-states per charged/free pair once it is in the alternation, so the totals fall into
    // two groups of four depending on whether the fetch lands the run before or after the
    // group boundary at column 8:
    //
    //   phase 0  run from col  4:  3 + 1 + 7 + 1 + 7 + 1 + 7 = 27, + 4 fetch = 31
    //   phase 1  run from col  5:  2 + 1 + 7 + 1 + 7 + 1 + 7 = 26, + 4       = 30
    //   phase 2  run from col  6:  1 + 1 + 7 + 1 + 7 + 1 + 7 = 25, + 4       = 29
    //   phase 3  run from col  7:  1 + 7 + 1 + 7 + 1 + 7 + 1 = 25, + 4       = 29
    //   phase 4  run from col  8:  7 + 1 + 7 + 1 + 7 + 1 + 7 = 31, + 4       = 35
    //   phase 5  run from col  9:  6 + 1 + 7 + 1 + 7 + 1 + 7 = 30, + 4       = 34
    //   phase 6  run from col 10:  5 + 1 + 7 + 1 + 7 + 1 + 7 = 29, + 4       = 33
    //   phase 7  run from col 11:  4 + 1 + 7 + 1 + 7 + 1 + 7 = 28, + 4       = 32
    //
    // `R` is pinned so `IR` is a fixed address rather than whatever the positioning prologue
    // left behind; every value it could take is in the same bank, so this is for legibility
    // rather than correctness.
    const INTERRUPT_VECTOR_IN_THE_SCREEN_BANK: u8 = 0x40;

    for (phase, expected) in CONTENDED_REFRESH_RUN.iter().enumerate() {
        let phase = u32::try_from(phase).expect("eight phases");
        let mut machine = machine();
        advance_to(&mut machine, FIRST_CONTENDED_T_STATE + phase);
        write_program(&mut machine, UNCONTENDED_CODE, &[0x09]);
        with_cpu_state(&mut machine, |state| {
            state.i = INTERRUPT_VECTOR_IN_THE_SCREEN_BANK;
            state.r = 0;
        });

        assert_eq!(
            cost_of_running(&mut machine, UNCONTENDED_CODE, 1),
            *expected,
            "ADD HL,BC from bank 0 with IR in the screen bank, at phase +{phase}, must cost \
             {expected} T-states: the fetch is free and all seven internal cycles are \
             charged, each at the column it opens at"
        );
    }
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
