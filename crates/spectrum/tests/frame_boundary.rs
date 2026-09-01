//! Gate: the frame clock rolls over **inside** an instruction, and everything downstream of
//! that — the frame counter, contention, and the next frame's interrupt window — is priced
//! from the new frame's position rather than from time since power-on.
//!
//! # Why this exists
//!
//! `docs/MACHINE.md` Decision 2 is a design decision with no gate behind it:
//!
//! > There is no small maximum for a single instruction. A run of `DD`/`FD` prefix bytes is
//! > **one instruction**, four T-states per prefix, and guest memory decides how long the run
//! > is. The frame loop must handle a step that carries it past the interrupt point rather
//! > than assuming it can stop exactly on 69888.
//!
//! `Spectrum::run_frame` is built on it — it watches the frame *counter* rather than trying
//! to land on 69888 — and `timing.rs` unit-tests [`Clock::advance`][spectrum::timing::Clock]
//! in isolation. **Nothing drives an instruction across the boundary through the machine.**
//! Every gate in this directory before this one measures inside frame zero, for a mechanical
//! reason: [`advance_to`] assembles one straight-line prologue and a frame is seventeen
//! thousand instructions away, so the far side of the boundary was not merely ungraded but
//! unreachable. [`advance_to_absolute`] is what this file needed first.
//!
//! # The wrap can never fall inside the fetch window, and that shapes the whole file
//!
//! A 48K contends from [`FIRST_CONTENDED_T_STATE`] for 192 lines of 224 T-states — position
//! 14335 to 57342 inclusive — and the frame wraps at 0. **So both sides of the wrap are
//! always border, and no instruction can straddle it with contention live on either side.**
//! That is a fact about the machine rather than a gap in the file, and it is why the pricing
//! claim here is not "contended, then contended differently" but the two that are actually
//! available:
//!
//! - contention **resumes at the right frame-relative position** after a wrap that happened
//!   mid-instruction — the long chain in
//!   [`an_instruction_is_priced_by_the_region_on_each_side_of_the_wrap`] runs free through the
//!   bottom border, across the wrap, on through the top border, and into the *next* frame's
//!   display window, all inside one `step()`;
//! - the same instruction at the same frame position costs the same in frame 0, frame 1 and
//!   frame 2 — [`contention_is_a_position_within_a_frame_not_a_position_since_power_on`].
//!
//! A machine that priced contention from time since power-on passes every *contention* gate in
//! this directory — `contention_magnitude.rs`, `io_contention.rs`, `block_contention.rs`,
//! `prefix_chain_contention.rs` and `contention_phase.rs`, all of which measure inside frame
//! zero — and fails both of those, because `delay` would put every frame after the first past
//! the end of the display and charge nothing at all.
//!
//! # Why the frame counter and the offset are asserted as a pair
//!
//! [`elapsed`] is `frames * 69888 + frame_t_state`, so **a wrong split between the two is
//! invisible to the sum**. `a_step_that_lands_exactly_on_the_boundary_has_wrapped` is the case
//! that separates them: an instruction ending on precisely 69888 must report `(1, 0)`, and a
//! rollover testing `>` rather than `>=` reports `(0, 69888)` — the same elapsed total, one
//! frame behind, with contention and the interrupt window both consulting a position that does
//! not exist. Every case here therefore asserts the pair.
//!
//! # The claims above were measured, not argued
//!
//! Eight mutations, each proven to have landed before its verdict was trusted — occurrence
//! count asserted before the write, file re-read after — and each restored from bytes held by
//! the driver rather than with `git checkout --`. The counts are failing tests across the whole
//! workspace under `--no-fail-fast`, against a baseline of 398 passed:
//!
//! | Mutation | Workspace | Caught here by |
//! |---|---|---|
//! | Rollover tests `>` rather than `>=` | RED 8 | the exact landing, the NOP equivalence, the `HALT` |
//! | Contention priced from time since power-on | RED 4 | both pricing rows |
//! | The interrupt offered *after* the instruction | RED 6 | the overshoot, the `HALT` |
//! | `INTERRUPT_T_STATES` 32 → 33 | RED 4 | the overshoot |
//! | **`INTERRUPT_T_STATES` 32 → 24** | **RED 1** | **the overshoot — sole witness in the workspace** |
//! | **A 16-bit T-state accumulator** | **RED 1** | **the two-wrap chain — sole witness** |
//! | Rollover discards the overshoot | RED 3 | **nothing here** — see below |
//! | Rollover `if` rather than `while` | RED 3 | **nothing here** — see below |
//!
//! Three more were run for the phase, and they are why the *what is not graded here* list below
//! is narrower than it first read: [`FIRST_CONTENDED_T_STATE`] moved to 14334 or to 14336 is
//! RED 3 each, and [`an_instruction_is_priced_by_the_region_on_each_side_of_the_wrap`] is one
//! of the three both times.
//!
//! Two of those are the interesting rows, and both are single witnesses. `INTERRUPT_T_STATES`
//! at 24 reddens **only** this file: `frame_interrupt.rs`'s window test derives both the
//! positions it samples *and* the value it expects from the constant under test, so it is a
//! consistency check that cannot see the window move — it fails at 33 only because 33 is not a
//! multiple of four and its `NOP`s can no longer land on it exactly. The overshoot case here
//! uses the literals 31 and 32 and mentions the constant nowhere, which is what makes it a
//! two-sided pin. A 16-bit accumulator is the next size up from the `u8` that once aborted this
//! process on a legal instruction stream, and 131,072 T-states in one `step()` is the only
//! measurement in the workspace large enough to see it.
//!
//! **And the last two rows are a fact about the machine rather than a hole here.** Every
//! multi-T-state `Clock::advance` on a 48K is a contention stall, contention exists only
//! between 14335 and 57342, and the largest stall is six — so **`advance` can never cross a
//! frame boundary by more than one T-state through the machine**, and `= 0` is
//! indistinguishable from `-= 69888`, and `if` from `while`. Both are graded by `timing.rs`'s
//! own unit tests, which call `advance` directly with a whole frame's worth. Nothing here can
//! reach them, and nothing should try: the reason is the same one that shapes this entire file
//! — around a frame boundary a 48K is always in the border.
//!
//! # How the expected values were obtained
//!
//! Every figure was derived **before** the emulator was measured, and never by adjusting an
//! observed one — `docs/STATUS.md` records what that costs. A recording bus was attached to a
//! real `Cpu`, the machine-cycle stream printed rather than assumed, the published delay rule
//! applied to that list by hand, and the result cross-checked against a second implementation
//! of the rule written with no sight of `crates/spectrum`. The recorder was validated against
//! a known answer first: `INC (HL)` must decompose as `pc:4, hl:3, hl:1, hl:3` and cost 26 at
//! phase 0 and 19 at phase 7, and it does.
//!
//! # What is not graded here
//!
//! - **Whether the published pattern is right.** No oracle for the pattern itself; see
//!   `contention_magnitude.rs`. `tests/timing_oracle.rs` is what grades the model as a whole
//!   against measured hardware.
//! - **Where the pattern begins — but only for five of the seven cases here.** An earlier
//!   draft of this list claimed the whole file survives
//!   [`FIRST_CONTENDED_T_STATE`] being wrong, on the grounds that every position is expressed
//!   relative to it. **Measured, that is false for
//!   [`an_instruction_is_priced_by_the_region_on_each_side_of_the_wrap`]**, which is positioned
//!   relative to [`T_STATES_PER_FRAME`] instead and then runs *into* the display window — so
//!   moving 14335 by one T-state in either direction reddens it, and it is a second witness for
//!   the phase alongside `contention_phase.rs` and the oracle. The claim is corrected rather
//!   than deleted because "positions are relative to the constant" is the right instinct and
//!   was simply not true of the one case that crosses a region boundary.
//! - **The 128's geometry.** Its frame is 70908 T-states rather than 69888, and 70908 is not a
//!   multiple of 8 — so at M7 the ULA group's phase does *not* line up across a frame boundary
//!   the way it does here, and every figure in this file must be re-derived rather than
//!   scaled.
//! - **A chain that runs *through* the top of memory into the ROM.**
//!   [`one_instruction_can_advance_the_frame_counter_more_than_once`] ends exactly on `0xFFFF`
//!   and asserts `PC` wraps to zero, which closes half of the row
//!   `prefix_chain_contention.rs` lists as ungraded; a chain whose *prefixes* continue past
//!   the wrap is still not driven, because guest RAM ends there and the ROM is not ours to
//!   fill.
//! - **An interrupt accepted inside an instruction.** The Z80 accepts only between them, and
//!   `block_interrupt.rs` is where that is graded.
//! - **A `HALT` straddling the boundary.** `frame_interrupt.rs` grades the `HALT` escape;
//!   nothing positions one across a wrap.

mod common;

use common::{
    CONTENDED_CODE, HALT, NOP, NOP_T_STATES, UNCONTENDED_CODE, advance_to_absolute,
    cost_of_running, elapsed, enable_interrupts, machine, set_pc, with_cpu_state, write_program,
};
use spectrum::Spectrum;
use spectrum::timing::{FIRST_CONTENDED_T_STATE, T_STATES_PER_FRAME};
use z80::InterruptMode;

/// The prefix that substitutes `IX` for `HL` in the following instruction.
const PREFIX_DD: u8 = 0xDD;

/// The two positions within the ULA's eight-T-state group every phase-sensitive case uses.
///
/// Phase 0 is where the pattern stalls most and phase 7 where it stalls nothing, which is the
/// convention `contention_magnitude.rs`, `io_contention.rs`, `block_contention.rs` and
/// `prefix_chain_contention.rs` all already follow.
const PHASES: [u32; 2] = [0, 7];

/// Frames the cross-frame cases are measured in.
///
/// Three rather than two: one boundary crossed proves the counter moves, and two proves the
/// pricing does not drift with each crossing — which is the shape a model accumulating an
/// error per frame would have.
const FRAMES: [u64; 3] = [0, 1, 2];

// ---------------------------------------------------------------------------
// The instruments
// ---------------------------------------------------------------------------

/// `NOP` out of the screen bank, at [`PHASES`]: four nominal T-states plus the pattern's
/// stall of 6 and 0.
///
/// Written here as the **expectation**, from the published stall figures rather than from the
/// crate — a table taken from the implementation agrees with any implementation. It is the
/// same pair `contention_magnitude.rs` derives at those two phases; this file's claim is about
/// the *frame* axis, and these are the fixed points it moves along it.
const CONTENDED_NOP: [u64; 2] = [10, 4];

/// `INC (HL)` with both the opcode and the operand in the screen bank, at [`PHASES`].
///
/// The canonical figure of this project: `docs/MACHINE.md` derives it cycle by cycle from
/// `pc:4, hl:3, hl:1, hl:3` and `docs/STATUS.md` records the session in which a second
/// derivation reached 30 by adjusting an observed total instead of re-deriving it.
const CONTENDED_INC_HL: [u64; 2] = [26, 19];

/// `INC (HL)`'s nominal cost, and the bytes that encode it.
const INC_HL: [u8; 1] = [0x34];
const INC_HL_T_STATES: u64 = 11;

/// Where `INC (HL)` points when the operand is to share the screen bank with the opcode.
const OPERAND_IN_THE_SCREEN_BANK: u16 = 0x4100;

/// And where it points when the operand is to be free.
const OPERAND_FREE: u16 = 0xC800;

// ---------------------------------------------------------------------------
// Chains
// ---------------------------------------------------------------------------

/// `length` `DD` bytes followed by the terminal `NOP` that ends the instruction.
fn chain(length: usize) -> Vec<u8> {
    let mut bytes = vec![PREFIX_DD; length];
    bytes.push(NOP);
    bytes
}

/// T-states a chain of `length` prefixes costs out of uncontended memory.
const fn chain_nominal(length: usize) -> u64 {
    (length as u64 + 1) * NOP_T_STATES as u64
}

/// A machine positioned at `at` T-states after power-on with `program` at `code_at`.
fn positioned(at: u64, code_at: u16, program: &[u8]) -> Spectrum {
    let mut machine = machine();
    advance_to_absolute(&mut machine, at);
    write_program(&mut machine, code_at, program);
    machine
}

/// Where a machine stands: the frame it is in and how far into it.
fn position(machine: &Spectrum) -> (u64, u32) {
    (machine.frames(), machine.frame_t_state())
}

// ---------------------------------------------------------------------------
// One instruction, one wrap
// ---------------------------------------------------------------------------

/// A chain of four prefixes and a `NOP`: five M1 cycles, twenty T-states.
const SHORT_CHAIN: usize = 4;

/// Positions to start it from, and where each must leave the machine.
///
/// All three are in the bottom border, and twenty T-states later all three are in the top
/// border of the next frame, so the cost is the nominal twenty in **either** bank — the wrap
/// itself is free, on a machine where it can never be anything else. What the three separate
/// is the landing:
///
/// | start | ends on | wrap falls |
/// |---|---|---|
/// | 69868 | exactly 69888 | on the last cycle's final T-state |
/// | 69869 | 69889 | inside the last cycle |
/// | 69878 | 69898 | inside the third cycle |
///
/// The first row is the one that separates the counter from the offset: a rollover written
/// `>` rather than `>=` leaves it at `(0, 69888)`, whose [`elapsed`] is identical.
static SHORT_CHAIN_CASES: &[(u64, (u64, u32))] = &[
    (69868, (1, 0)),
    (69869, (1, 1)),
    (69878, (1, 10)),
    // A control from the same border, far enough back that no wrap happens at all.
    (69800, (0, 69820)),
];

#[test]
fn a_step_that_lands_exactly_on_the_boundary_has_wrapped() {
    for &(start, expected) in SHORT_CHAIN_CASES {
        for code_at in [UNCONTENDED_CODE, CONTENDED_CODE] {
            let mut machine = positioned(start, code_at, &chain(SHORT_CHAIN));
            let cost = cost_of_running(&mut machine, code_at, 1);

            assert_eq!(
                cost,
                chain_nominal(SHORT_CHAIN),
                "a chain crossing the boundary from {start} must cost its nominal {} T-states \
                 out of {code_at:#06X}: both sides of the wrap are border, so nothing here is \
                 contended in either bank",
                chain_nominal(SHORT_CHAIN)
            );
            assert_eq!(
                position(&machine),
                expected,
                "from {start} the machine must end at frame {} offset {}. The frame count and \
                 the offset are asserted as a pair because their *sum* is the same whatever \
                 the split: {} T-states have elapsed either way",
                expected.0,
                expected.1,
                elapsed(&machine)
            );
        }
    }
}

#[test]
fn a_chain_across_the_wrap_costs_what_the_same_bytes_cost_as_separate_instructions() {
    // The independent construction, and it uses no frame arithmetic at all. A chain of N
    // prefixes and a run of N+1 `NOP`s occupy the same addresses and emit the same N+1
    // four-T-state opcode fetches, so from one starting position they must cost the same and
    // land in the same place — even though the chain crosses the boundary **inside one
    // `step()`** while the `NOP`s cross it between two of them.
    //
    // That is the assertion this file most needs a second opinion on: `Spectrum::run_frame`
    // watches the frame counter precisely because a step can overrun the budget, and a machine
    // that clamped a step at the boundary, or restarted the clock rather than carrying the
    // overshoot, would agree with every published nominal length and disagree here.
    for &(start, expected) in SHORT_CHAIN_CASES {
        let mut as_chain = positioned(start, CONTENDED_CODE, &chain(SHORT_CHAIN));
        let chain_cost = cost_of_running(&mut as_chain, CONTENDED_CODE, 1);

        let filler = vec![NOP; SHORT_CHAIN + 1];
        let mut as_nops = positioned(start, CONTENDED_CODE, &filler);
        let nop_cost = cost_of_running(&mut as_nops, CONTENDED_CODE, filler.len());

        assert_eq!(
            chain_cost,
            nop_cost,
            "from {start} the chain and {} separate NOPs over the same addresses must cost \
             the same: the bus sees one stream of {} opcode fetches either way, and only the \
             CPU knows the boundary was crossed inside a single instruction",
            filler.len(),
            filler.len()
        );
        assert_eq!(
            position(&as_nops),
            expected,
            "and they must leave the machine in the same frame at the same offset"
        );
    }
}

// ---------------------------------------------------------------------------
// Two wraps in one instruction
// ---------------------------------------------------------------------------

/// The longest chain uncontended RAM can hold: `0x8000` through `0xFFFF`.
///
/// 32,767 prefixes and the terminal `NOP` — 32,768 M1 cycles, 131,072 nominal T-states, which
/// is 1.876 frames. Started at [`TWO_WRAP_START`] it crosses **two** boundaries, which is the
/// claim: `docs/MACHINE.md` says there is no small maximum for one instruction, and a machine
/// whose rollover handled one crossing but not two would pass every other case in this file.
const LONG_CHAIN: usize = 32767;
const LONG_CHAIN_AT: u16 = 0x8000;

/// Where to start it, and where it must land.
///
/// `8708 + 131072 = 139780`, which is four T-states past two whole frames. Deliberately not
/// the exact multiple: that case is already covered by
/// [`a_step_that_lands_exactly_on_the_boundary_has_wrapped`], and a non-zero remainder is what
/// shows the second crossing carried its overshoot rather than resetting.
const TWO_WRAP_START: u64 = 8708;
const TWO_WRAP_END: (u64, u32) = (2, 4);

#[test]
fn one_instruction_can_advance_the_frame_counter_more_than_once() {
    // The chain is assembled over the positioning prologue and the sled, which have both
    // already run by the time it is written — `0x8000` upward is the only 32 KB of contiguous
    // uncontended RAM there is, and a chain this long needs all of it.
    let mut machine = positioned(TWO_WRAP_START, LONG_CHAIN_AT, &chain(LONG_CHAIN));

    set_pc(&mut machine, LONG_CHAIN_AT);
    let before = elapsed(&machine);
    let nominal = u64::from(machine.step());
    let cost = elapsed(&machine) - before;

    assert_eq!(
        nominal,
        chain_nominal(LONG_CHAIN),
        "one step, {} M1 cycles, {} nominal T-states",
        LONG_CHAIN + 1,
        chain_nominal(LONG_CHAIN)
    );
    assert_eq!(
        cost,
        chain_nominal(LONG_CHAIN),
        "and out of uncontended RAM the bus must charge exactly that"
    );
    assert_eq!(
        position(&machine),
        TWO_WRAP_END,
        "one instruction of {} T-states started {TWO_WRAP_START} into frame zero must leave \
         the machine two frames on, four T-states in. A rollover that fired once would report \
         frame one with the same elapsed total",
        chain_nominal(LONG_CHAIN)
    );
    assert_eq!(
        machine.cpu_state().pc,
        0,
        "the chain ends on the last byte of the address space, so consuming its terminal \
         opcode wraps PC to zero"
    );
}

// ---------------------------------------------------------------------------
// Contention on the far side
// ---------------------------------------------------------------------------

#[test]
fn contention_is_a_position_within_a_frame_not_a_position_since_power_on() {
    // The row that catches a contention model keyed on time since power-on. Nothing else in
    // this directory can: every other gate measures inside frame zero, where the two models
    // are indistinguishable. From frame one onwards they are not — `delay` subtracts
    // `FIRST_CONTENDED_T_STATE` and compares against the display's span, so an absolute
    // position lands past the end of the display and charges **nothing**, for every frame
    // after the first, for ever.
    //
    // The uncontended run at each position is the control: if it ever costs more than nominal
    // the comparison means nothing, because the difference would no longer be attributable to
    // the one bank a 48K contends.
    for frame in FRAMES {
        for (index, phase) in PHASES.into_iter().enumerate() {
            let at = frame * u64::from(T_STATES_PER_FRAME)
                + u64::from(FIRST_CONTENDED_T_STATE)
                + u64::from(phase);

            let mut free = positioned(at, UNCONTENDED_CODE, &[NOP]);
            assert_eq!(
                cost_of_running(&mut free, UNCONTENDED_CODE, 1),
                u64::from(NOP_T_STATES),
                "frame {frame}, phase {phase}: a NOP in bank 0 must cost its nominal length"
            );

            let mut held = positioned(at, CONTENDED_CODE, &[NOP]);
            assert_eq!(
                cost_of_running(&mut held, CONTENDED_CODE, 1),
                CONTENDED_NOP[index],
                "frame {frame}, phase {phase}: a NOP in the screen bank must cost {} T-states, \
                 exactly as it does in frame zero. Contention is a position within a frame",
                CONTENDED_NOP[index]
            );

            // The read-modify-write shape as well as the bare fetch: four machine cycles
            // rather than one, so a model that got the frame right for the first cycle and
            // wrong for the rest has somewhere to fail.
            let mut free_rmw = positioned(at, UNCONTENDED_CODE, &INC_HL);
            with_cpu_state(&mut free_rmw, |state| state.hl = OPERAND_FREE);
            assert_eq!(
                cost_of_running(&mut free_rmw, UNCONTENDED_CODE, 1),
                INC_HL_T_STATES,
                "frame {frame}, phase {phase}: INC (HL) in bank 0 must cost its nominal length"
            );

            let mut held_rmw = positioned(at, CONTENDED_CODE, &INC_HL);
            with_cpu_state(&mut held_rmw, |state| {
                state.hl = OPERAND_IN_THE_SCREEN_BANK;
            });
            assert_eq!(
                cost_of_running(&mut held_rmw, CONTENDED_CODE, 1),
                CONTENDED_INC_HL[index],
                "frame {frame}, phase {phase}: INC (HL) wholly in the screen bank must cost \
                 {} T-states, the figure docs/MACHINE.md derives cycle by cycle",
                CONTENDED_INC_HL[index]
            );
        }
    }
}

// ---------------------------------------------------------------------------
// One instruction, priced by the region on each side of the wrap
// ---------------------------------------------------------------------------

/// Prefixes in the chain that runs from one frame's bottom border into the next frame's
/// display.
///
/// 3,600 of them is 14,404 nominal T-states, which is a little more than the 14,335 that
/// separate the top of a frame from the first contended position — so a chain started a few
/// T-states before the wrap spends almost all of itself free and its last handful of fetches
/// inside the next frame's fetch window.
const STRADDLING_CHAIN: usize = 3600;

/// Where to start it, what it must cost, and where it must land.
///
/// The derivation, from a recorded cycle list of 3,601 four-T-state opcode fetches at
/// consecutive addresses and nothing else. `d(k)` is the published pattern
/// `[6, 5, 4, 3, 2, 1, 0, 0]` indexed by the column `k mod 8`, and a fetch's column is its
/// frame position minus [`FIRST_CONTENDED_T_STATE`].
///
/// Starting at 69882, fetch `k` opens at `69882 + 4k`; the wrap at 69888 falls **inside fetch
/// 1**, and from fetch 2 on the frame position is `4k - 6`. Everything is free until that
/// reaches 14335:
///
/// ```text
///   4k - 6 >= 14335  ->  k >= 3585.25  ->  k = 3586, at 14338, column 3
///     M1 at 14338   d(3) = 3  -> 14341   4 -> 14345
///     M1 at 14345   column 10, 10 mod 8 = 2, d = 4 -> 14349   4 -> 14353
///     ...
/// ```
///
/// From the second stalled fetch the run **self-synchronises**: a stall of 4 plus four
/// T-states is eight, exactly one ULA group, so every later fetch opens on column 2 again and
/// stalls 4 until the column passes 128 and the line's border begins. Columns
/// `10, 18, ..., 122` is fifteen fetches — but only fourteen remain, because `k = 3600` is the
/// terminal `NOP` and the chain ends. So the stall is `3 + 14 * 4 = 59`, and the cost is
/// `14404 + 59 = 14463`.
///
/// Starting at 69885 the wrap falls inside fetch **0**, the frame position from fetch 1 on is
/// `4k - 3`, contention begins at `k = 3585` on column 2 rather than column 3 — `d = 4` — and
/// fifteen fetches remain rather than fourteen: `4 + 15 * 4 = 64`, and `14404 + 64 = 14468`.
///
/// Two starts three T-states apart, entering the window on two different columns, with the
/// wrap inside two different machine cycles.
static STRADDLING_CASES: &[(u64, u64, (u64, u32))] =
    &[(69882, 14463, (1, 14457)), (69885, 14468, (1, 14465))];

#[test]
fn an_instruction_is_priced_by_the_region_on_each_side_of_the_wrap() {
    for &(start, expected, ends) in STRADDLING_CASES {
        let program = chain(STRADDLING_CHAIN);

        let mut free = positioned(start, UNCONTENDED_CODE, &program);
        assert_eq!(
            cost_of_running(&mut free, UNCONTENDED_CODE, 1),
            chain_nominal(STRADDLING_CHAIN),
            "from {start}, out of bank 0, the chain must cost its nominal {} T-states however \
             many frames it crosses. If this fails the control is wrong and the comparison \
             below means nothing",
            chain_nominal(STRADDLING_CHAIN)
        );

        let mut held = positioned(start, CONTENDED_CODE, &program);
        let cost = cost_of_running(&mut held, CONTENDED_CODE, 1);
        assert_eq!(
            cost,
            expected,
            "from {start}, out of the screen bank, one instruction runs free through the \
             bottom border, across the wrap, on through the next frame's top border and into \
             its display — {expected} T-states against a nominal {}. A machine pricing \
             contention from time since power-on charges nothing after the wrap and reaches \
             the nominal figure",
            chain_nominal(STRADDLING_CHAIN)
        );
        assert_eq!(
            position(&held),
            ends,
            "and it must land in frame {} at offset {}",
            ends.0,
            ends.1
        );

        // The same second opinion as for the short chain, and it matters more here: this is
        // the one case in the file where a stall is charged on the far side of a wrap, so a
        // construction that reaches the figure without walking the pattern is worth having.
        let filler = vec![NOP; STRADDLING_CHAIN + 1];
        let mut as_nops = positioned(start, CONTENDED_CODE, &filler);
        assert_eq!(
            cost_of_running(&mut as_nops, CONTENDED_CODE, filler.len()),
            cost,
            "from {start}, {} separate NOPs over the same addresses must cost what the chain \
             costs: one long instruction and many short ones are the same stream to the bus",
            filler.len()
        );
    }
}

// ---------------------------------------------------------------------------
// The interrupt window on the far side of the wrap
// ---------------------------------------------------------------------------

/// A chain of sixteen prefixes and a `NOP`: 68 nominal T-states, long enough to overshoot the
/// top of a frame by more than the interrupt window is wide.
const WINDOW_CHAIN: usize = 16;

/// Where the handler lives, and the two bytes of it.
const HANDLER: u16 = 0xA000;
const HANDLER_CODE: [u8; 2] = [0xFB, 0xC9];

/// `I`, and therefore where mode 2 looks up the handler's address.
///
/// A 48K's bus floats to `0xFF`, so the vector pointer is `(I << 8) | 0xFF`. Mode 2 rather
/// than mode 1 because mode 1 vectors into the ROM, where this fixture's `pattern_rom` holds
/// arbitrary bytes; a handler in RAM makes the acceptance unambiguous and keeps the machine
/// running afterwards. It also exercises the vector-table read, which nothing else here does.
const VECTOR_HIGH: u8 = 0xA1;
const VECTOR_POINTER: u16 = 0xA1FF;

/// `JR -2`, an unconditional two-byte loop onto itself.
///
/// Twelve T-states a pass, in uncontended RAM, so the machine can be left spinning for a whole
/// frame without running off into memory it was never given.
const SPIN: [u8; 2] = [0x18, 0xFE];

/// Steps to allow the spin before concluding no interrupt is coming.
///
/// A frame is 69888 T-states and the loop costs twelve, so 5824 passes cover one whole frame;
/// the budget is generous rather than tight because its only job is to fail loudly instead of
/// hanging.
const SPIN_BUDGET: usize = 20_000;

/// Assemble the machine used by both halves of the window case.
fn machine_with_a_handler(start: u64) -> Spectrum {
    let mut machine = positioned(start, UNCONTENDED_CODE, &chain(WINDOW_CHAIN));
    write_program(
        &mut machine,
        UNCONTENDED_CODE + WINDOW_CHAIN as u16 + 1,
        &SPIN,
    );
    write_program(&mut machine, HANDLER, &HANDLER_CODE);
    write_program(&mut machine, VECTOR_POINTER, &HANDLER.to_le_bytes());
    enable_interrupts(&mut machine, InterruptMode::Mode2);
    with_cpu_state(&mut machine, |state| state.i = VECTOR_HIGH);
    machine
}

/// Step until the CPU vectors to [`HANDLER`], and report the frame it happened in.
fn run_to_acceptance(machine: &mut Spectrum) -> u64 {
    for _ in 0..SPIN_BUDGET {
        machine.step();
        if machine.cpu_state().pc == HANDLER {
            return machine.frames();
        }
    }
    panic!("no interrupt was accepted within {SPIN_BUDGET} steps");
}

#[test]
fn an_overshoot_past_the_interrupt_window_misses_that_frames_interrupt() {
    // `Spectrum::run_frame`'s documentation claims this in as many words — *"including the
    // case where the overshoot is long enough to miss the following interrupt"* — and nothing
    // grades it. The window is the first 32 T-states of a frame, so a step that ends on 31 is
    // still inside it and a step that ends on 32 has stepped over the whole frame's interrupt.
    //
    // The two starts are one T-state apart. Everything else about them is identical: same
    // instruction, same bank, same registers, same handler.
    const ACCEPTED: u64 = 69_888 + 31 - 68;
    const MISSED: u64 = 69_888 + 32 - 68;

    let mut just_inside = machine_with_a_handler(ACCEPTED);
    let cost = cost_of_running(&mut just_inside, UNCONTENDED_CODE, 1);
    assert_eq!(cost, chain_nominal(WINDOW_CHAIN));
    assert_eq!(
        position(&just_inside),
        (1, 31),
        "the chain must overshoot the top of frame one by 31 T-states"
    );
    assert_eq!(
        run_to_acceptance(&mut just_inside),
        1,
        "31 is inside the 32-T-state window, so the very next step must accept frame one's \
         interrupt"
    );
    assert_eq!(
        just_inside.cpu_state().sp,
        0xFF00 - 2,
        "and the return address must be on the stack"
    );

    let mut just_outside = machine_with_a_handler(MISSED);
    assert_eq!(
        cost_of_running(&mut just_outside, UNCONTENDED_CODE, 1),
        chain_nominal(WINDOW_CHAIN)
    );
    assert_eq!(
        position(&just_outside),
        (1, 32),
        "one T-state later, the chain must overshoot by 32"
    );
    assert!(
        just_outside.cpu_state().iff1,
        "nothing has been accepted yet"
    );
    assert_eq!(
        run_to_acceptance(&mut just_outside),
        2,
        "32 is one past the window: frame one's interrupt is gone entirely, and the machine \
         spins through the whole of that frame before frame two offers the next one. An \
         interrupt held for the frame rather than for its first 32 T-states would be accepted \
         in frame one and this would read 1"
    );
}

#[test]
fn a_halt_at_the_boundary_resumes_on_the_next_frames_interrupt() {
    // The complementary shape, and the reason it belongs in this file rather than in
    // `frame_interrupt.rs`: a halted CPU issues one M1 cycle per step, so it crosses the
    // boundary the way any other instruction does — and the frame it resumes in is decided by
    // the counter this file exists to grade. Positioned in the bottom border, four T-states
    // short of the wrap, so the `HALT` cycle that carries the machine into frame one is the
    // one whose four T-states straddle it.
    const AT: u64 = 69_888 - 4;

    let mut machine = machine_with_a_handler(AT);
    write_program(&mut machine, UNCONTENDED_CODE, &[HALT]);
    set_pc(&mut machine, UNCONTENDED_CODE);

    machine.step();
    assert_eq!(
        position(&machine),
        (1, 0),
        "the HALT cycle spans the wrap and lands exactly on the new frame"
    );
    assert!(machine.cpu_state().halted, "and the CPU is still halted");

    assert_eq!(
        run_to_acceptance(&mut machine),
        1,
        "offset zero is inside the window, so frame one's interrupt must be accepted at once"
    );
    assert!(
        !machine.cpu_state().halted,
        "acceptance leaves HALT: the handler must not return into it"
    );
    assert_eq!(
        u16::from_le_bytes([
            machine.memory().read(0xFF00 - 2),
            machine.memory().read(0xFF00 - 1)
        ]),
        UNCONTENDED_CODE + 1,
        "and the return address is the byte *after* the HALT, not the HALT itself"
    );
}
