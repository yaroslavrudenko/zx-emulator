//! Gate: where in the frame contention begins — pinned to the frame's structure.
//!
//! # Read this before trusting anything below
//!
//! **The absolute phase remains unverified against any external oracle.** Nothing in this
//! file, and nothing anywhere in this project, measures
//! [`FIRST_CONTENDED_T_STATE`][spectrum::timing::FIRST_CONTENDED_T_STATE] against a real
//! machine or against a program that reports a T-state count. `docs/MACHINE.md` names such a
//! program as verification item 2 and it is not written. An issue 2 Spectrum is one T-state
//! earlier than an issue 3, and this crate models an issue 3 because the community reports
//! that figure — which is a citation, not a measurement.
//!
//! That mattered concretely: moving the constant from 14335 to 14334 produced **byte-identical
//! output** from the boot gate, and left every existing test green, because every one of them
//! is written relative to the constant. This file is what makes that mutation cost something.
//!
//! # What is graded here, and what kind of evidence it is
//!
//! A **derivation from documented structure**, which is stronger than nothing and weaker than
//! an oracle. The 48K frame is 312 lines of 224 T-states. The display begins after 64 lines
//! of vertical blanking and top border, so the first display byte is fetched at
//! `64 x 224 = 14336`, and contention begins one T-state before it. That derivation is
//! asserted **as an equation over the crate's own structural constants**, so:
//!
//! - changing [`FIRST_CONTENDED_T_STATE`][spectrum::timing::FIRST_CONTENDED_T_STATE] alone
//!   breaks this file;
//! - changing the structure — [`T_STATES_PER_LINE`], [`LINES_PER_FRAME`], the display height
//!   — changes what the equation expects, coherently, which is what a 128 at 70908 T-states
//!   will need.
//!
//! It does **not** establish that 14335 is the hardware's figure. It establishes that 14335
//! is the figure this frame layout implies, and that the two cannot drift apart silently.
//!
//! # What is not graded here
//!
//! - The stall *amounts* — that is `contention_magnitude.rs`.
//! - The 64-line pre-display count, which is itself a documented figure this file writes
//!   down rather than derives. It is the one input the derivation takes on trust, and it is a
//!   single named constant so that a future measurement has one place to correct.

mod common;

use common::{CONTENDED_CODE, NOP, NOP_T_STATES, advance_to, cost_of_running, machine};
use spectrum::screen::DISPLAY_HEIGHT;
use spectrum::timing::{
    FIRST_CONTENDED_T_STATE, LINES_PER_FRAME, T_STATES_PER_FRAME, T_STATES_PER_LINE,
};

/// Lines of vertical blanking and top border before the first display line, on a 48K.
///
/// The one figure the derivation takes on trust. Named rather than inlined so that the
/// timing-test program `docs/MACHINE.md` asks for has a single place to correct if it ever
/// measures something else.
const LINES_BEFORE_DISPLAY: u32 = 64;

/// The stall the pattern opens with — the ULA's worst case.
const FIRST_STALL: u32 = 6;

#[test]
fn the_first_contended_t_state_follows_from_the_frame_structure() {
    // A derivation, not a measurement. See the module documentation.
    let first_display_fetch = LINES_BEFORE_DISPLAY * T_STATES_PER_LINE;
    let derived = first_display_fetch - 1;

    assert_eq!(
        FIRST_CONTENDED_T_STATE, derived,
        "contention begins one T-state before the first display byte is fetched. With \
         {LINES_BEFORE_DISPLAY} lines before the display at {T_STATES_PER_LINE} T-states \
         each, that fetch is at {first_display_fetch} and contention begins at {derived}"
    );
    assert_eq!(
        derived, 14335,
        "the derivation must still produce the value this crate has always used; if the \
         frame structure legitimately changed, this figure changes with it"
    );
}

#[test]
fn the_frame_decomposes_into_the_three_vertical_regions() {
    // The structural constants the derivation rests on, asserted to be mutually consistent.
    // Without this, `LINES_BEFORE_DISPLAY` could be any number at all and the equation above
    // would still balance by construction.
    assert_eq!(
        T_STATES_PER_FRAME,
        LINES_PER_FRAME * T_STATES_PER_LINE,
        "the frame is exactly its lines"
    );

    let display = u32::try_from(DISPLAY_HEIGHT).expect("192 lines");
    assert!(
        LINES_BEFORE_DISPLAY + display < LINES_PER_FRAME,
        "there must be room below the display as well as above it"
    );

    let after_display = LINES_PER_FRAME - LINES_BEFORE_DISPLAY - display;
    assert_eq!(
        LINES_BEFORE_DISPLAY + display + after_display,
        LINES_PER_FRAME,
        "vertical blanking and top border, the display, and the bottom border are the whole \
         frame: {LINES_BEFORE_DISPLAY} + {display} + {after_display}"
    );

    let last_contended = FIRST_CONTENDED_T_STATE + display * T_STATES_PER_LINE;
    assert!(
        last_contended < T_STATES_PER_FRAME,
        "the contended span must end inside the frame: it runs to {last_contended} of \
         {T_STATES_PER_FRAME}"
    );
}

#[test]
fn the_machine_starts_contending_at_the_t_state_the_constant_names() {
    // The behavioural half: the named constant is where the machine's behaviour actually
    // changes, rather than a number that happens to sit in the module.
    //
    // This is written relative to the constant, so it survives the constant being wrong —
    // deliberately. Pinning the *phase* is the job of the derivation above; pinning the
    // *boundary* to the constant is this test's, and keeping them apart is what lets each
    // failure name one cause.
    let free = cost_of_one_fetch_at(FIRST_CONTENDED_T_STATE - 1);
    assert_eq!(
        free,
        u64::from(NOP_T_STATES),
        "one T-state before the first contended T-state, the screen bank must still be free"
    );

    let stalled = cost_of_one_fetch_at(FIRST_CONTENDED_T_STATE);
    assert_eq!(
        stalled,
        u64::from(NOP_T_STATES + FIRST_STALL),
        "the first contended T-state must open the pattern at its worst case"
    );
}

/// What one opcode fetch from the screen bank costs, starting at frame position `at`.
fn cost_of_one_fetch_at(at: u32) -> u64 {
    let mut machine = machine();
    advance_to(&mut machine, at);
    common::write_program(&mut machine, CONTENDED_CODE, &[NOP]);
    cost_of_running(&mut machine, CONTENDED_CODE, 1)
}
