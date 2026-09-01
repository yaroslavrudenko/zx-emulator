//! Gate: the ULA raises `/INT` at the top of every frame, and the machine acts on it.
//!
//! # Why this exists
//!
//! Never asserting `/INT` at all left the boot gate **green**. A positive probe confirmed the
//! interrupt path is executed, so the acceptance is not dead code — it is simply not graded,
//! because the 48K ROM's start-up reaches the copyright message without needing a single
//! frame interrupt. What the interrupt actually drives is everything *after* that: the
//! keyboard repeat, the `FRAMES` clock, and every game's timing.
//!
//! # What is graded here
//!
//! - The line is asserted at frame position zero, and both edges of the window are pinned by
//!   **literals** — the line is low at 31 and released at 32 — so the file has a failing case
//!   whichever way the window moves.
//! - Acceptance as a function of `IFF1` **and** position, as one table rather than as
//!   separate cases: the window and the flip-flop are two independent reasons to decline and
//!   a gate that varies one at a time cannot see them interact.
//! - `HALT` is escaped by it, with the return address the hardware pushes.
//! - **The real ROM's own `FRAMES` counter advances once per frame.** That is the strongest
//!   evidence available here: it is not this crate asserting that it raised an interrupt, it
//!   is Sinclair's interrupt handler being reached, executing, and leaving a number behind.
//!
//! # This file used to grade the window against its own constant
//!
//! Every position it sampled **and** every value it expected came from [`INTERRUPT_T_STATES`],
//! the constant it appears to pin. That is the keyboard-matrix tautology again — the gate that
//! derived both the port it scanned and the byte it expected from `Key::position()`, and under
//! which 38 of 40 keys could be rewired with the whole suite green.
//!
//! It was **measured**, not inferred. `INTERRUPT_T_STATES` moved 32 → 24 reddened exactly one
//! test in the workspace and it was not in this file; it was
//! `frame_boundary.rs`'s `an_overshoot_past_the_interrupt_window_misses_that_frames_interrupt`.
//! Moving it 32 → **33** did redden this file — but only by an accident of divisibility: a
//! `NOP` is four T-states, 33 is not a multiple of four, and the sampling loop could no longer
//! land on it. A one-sided pin held up by an accident reads in a run log exactly like a
//! two-sided one.
//!
//! The fix is the one the keyboard got: **expectations that owe nothing to the constant under
//! test**. [`LAST_ASSERTED_POSITION`] and [`FIRST_RELEASED_POSITION`] are literals, written out
//! separately rather than one derived from the other, and `frame_boundary.rs`'s overshoot case
//! is the model — it uses 31 and 32 and names the constant nowhere.
//!
//! ## A derived *position* is not a derived *expectation*
//!
//! The distinction is the reusable half of this, and it is why [`INTERRUPT_T_STATES`] still
//! appears once below. [`halt_is_escaped_by_the_frame_interrupt`] advances to it as a way of
//! saying *"wherever the window ends, get past it"* — a **precondition**, which stays true and
//! stays legible whatever the constant becomes. A test that computes what it expects to
//! observe from the subject has no failing case; a test that computes where to stand has one,
//! and it is the assertion it then makes.
//!
//! # What is not graded here
//!
//! - **Whether 32 is the right number.** Both edges are now pinned by literals, which catches
//!   drift in either direction; nothing in this project compares 32 to hardware.
//!   `docs/STATUS.md` carries that as an open row and `timing_oracle.rs` is demonstrated
//!   unable to close it — the oracle is green at 24.
//! - **The line's state deep inside a frame.** Nothing here samples past the window's far
//!   edge. `frame_boundary.rs` is the witness: its overshoot case spins a whole frame waiting
//!   for the next offer, and would accept immediately if the line were held frame-wide.
//! - **Interrupt modes 0 and 2.** The Spectrum's bus floats to `RST 38h`, so mode 0 lands
//!   where mode 1 does and mode 2 is never used by the 48K ROM. `crates/spectrum/src/lib.rs`
//!   covers the mode 0 case; `block_interrupt.rs` and `interrupt_block_families.rs` drive
//!   mode 2 through the machine.
//! - **`docs/STATUS.md` still records that interrupts have no external oracle at all** — no
//!   FUSE vector injects one and no exerciser generates one. Everything here grades the
//!   *machine's* side of the wire against this crate's own model of it.

mod common;

use common::{
    HALT, NOP, NOP_T_STATES, UNCONTENDED_CODE, advance_to, elapsed, enable_interrupts, machine,
    set_pc, sinclair_rom, write_program,
};
use spectrum::Spectrum;
use spectrum::timing::INTERRUPT_T_STATES;
use z80::InterruptMode;

/// The last frame position at which the ULA is still holding `/INT` low.
///
/// **A literal, and that is the entire point of it.** Every window position in this file used
/// to be `INTERRUPT_T_STATES` or `INTERRUPT_T_STATES - 1`, so the constant supplied both sides
/// of every comparison and the file had no failing case for the property it appeared to test.
/// Written out here as a number a reader can check against the published 48K figure, and
/// **not** as `FIRST_RELEASED_POSITION - 1`: two literals cannot slide together, one literal
/// and an offset from it can.
const LAST_ASSERTED_POSITION: u32 = 31;

/// The first frame position at which the line has been released.
///
/// The other literal. See [`LAST_ASSERTED_POSITION`].
const FIRST_RELEASED_POSITION: u32 = 32;

/// Where a mode 1 interrupt vectors.
const MODE_1_VECTOR: u16 = 0x0038;

/// A stack far from anything else these tests write.
const STACK_TOP: u16 = 0xFF00;

/// The 48K ROM's `FRAMES` system variable: a three-byte counter its interrupt handler bumps.
const FRAMES_COUNTER: u16 = 0x5C78;

/// Bytes of [`FRAMES_COUNTER`].
const FRAMES_COUNTER_LEN: u16 = 3;

/// Frames given to the real ROM before its interrupt handler is sampled.
///
/// **Measured, not chosen.** Sampling `FRAMES` every ten frames across a 300-frame run:
///
/// ```text
///   frame  10   FRAMES = 0
///   frame  20   FRAMES = 131586   (0x020202 — the ROM's RAM test writing a pattern
///   frame  30   FRAMES = 131586    straight through this address, not a count)
///   frame  40   FRAMES = 0        (start-up clears the system variables)
///   frame  80   FRAMES = 0
///   frame  90   FRAMES = 7        (start-up finished; the handler now runs each frame)
///   frame 100+  FRAMES = +10 per 10 frames, indefinitely
/// ```
///
/// So the counter is meaningless until start-up completes — which is around frame 87, where
/// the boot gate reports the copyright message appearing. A settle of 20 frames samples the
/// RAM test's leftovers and was the first thing this test did wrong.
const SETTLE_FRAMES: u64 = 100;

/// Frames over which the ROM's own counter is compared with the machine's.
const SAMPLE_FRAMES: u64 = 10;

/// A machine positioned at `at`, with `IFF1` as given and a `NOP` under `PC`.
///
/// The order matters and is the reason this is a fixture rather than four inline blocks:
/// positioning runs the machine through the interrupt window at the top of the frame, so the
/// flip-flops are set *afterwards* or the prologue would vector out of itself.
fn armed_at(at: u32, iff1: bool) -> Spectrum {
    let mut machine = machine();
    advance_to(&mut machine, at);

    let mut state = machine.cpu_state();
    state.iff1 = iff1;
    state.iff2 = iff1;
    state.im = InterruptMode::Mode1;
    state.sp = STACK_TOP;
    machine.set_cpu_state(state);

    write_program(&mut machine, UNCONTENDED_CODE, &[NOP; 4]);
    set_pc(&mut machine, UNCONTENDED_CODE);
    machine
}

#[test]
fn the_line_is_asserted_at_the_top_of_the_frame() {
    let machine = machine();
    assert_eq!(machine.frame_t_state(), 0);
    assert!(
        machine.ula().interrupt_asserted(),
        "the ULA must hold /INT low at frame position zero"
    );
}

#[test]
fn the_line_is_held_across_the_whole_window_and_drops_at_its_end() {
    // Sampled at every position the machine actually reaches, rather than at the boundary
    // alone: a line that flickered inside the window would pass a boundary-only check.
    //
    // The loop's bound and its landing are both literals now. That is what makes this a gate
    // rather than a restatement: at a 24 T-state window the sample taken at position 24 finds
    // the line already released and fails inside the loop; at 33 the landing at 32 finds it
    // still low and fails after it. One test, a failing case in each direction.
    let mut machine = machine();
    write_program(&mut machine, UNCONTENDED_CODE, &[NOP; 16]);
    set_pc(&mut machine, UNCONTENDED_CODE);

    while machine.frame_t_state() <= LAST_ASSERTED_POSITION {
        assert!(
            machine.ula().interrupt_asserted(),
            "/INT dropped at frame position {}, which is at or before {LAST_ASSERTED_POSITION} \
             and so inside the window",
            machine.frame_t_state()
        );
        machine.step();
    }

    assert_eq!(
        machine.frame_t_state(),
        FIRST_RELEASED_POSITION,
        "a NOP is four T-states and {FIRST_RELEASED_POSITION} is a multiple of four, so \
         uncontended NOPs from the top of the frame land exactly on it"
    );
    assert!(
        !machine.ula().interrupt_asserted(),
        "/INT must be released by frame position {FIRST_RELEASED_POSITION}"
    );
}

#[test]
fn the_last_t_state_inside_the_window_still_asserts_and_the_first_outside_does_not() {
    // The boundary itself, positioned exactly rather than stepped over — the sampling above
    // can only land on multiples of a NOP, so 31 is a position no other test in this file
    // reaches.
    //
    // The two positions are one T-state apart and are the two literals. Everything else about
    // the two machines is identical.
    let inside = machine_at(LAST_ASSERTED_POSITION);
    assert_eq!(inside.frame_t_state(), LAST_ASSERTED_POSITION);
    assert!(
        inside.ula().interrupt_asserted(),
        "the line must still be low at frame position {LAST_ASSERTED_POSITION}: a window \
         shorter than that has already let go"
    );

    let outside = machine_at(FIRST_RELEASED_POSITION);
    assert_eq!(outside.frame_t_state(), FIRST_RELEASED_POSITION);
    assert!(
        !outside.ula().interrupt_asserted(),
        "and it must be released by {FIRST_RELEASED_POSITION}: a window longer than that is \
         still holding it"
    );
}

/// A machine run to exactly `at`, with nothing else set up.
fn machine_at(at: u32) -> Spectrum {
    let mut machine = machine();
    advance_to(&mut machine, at);
    machine
}

#[test]
fn acceptance_depends_on_iff1_and_on_the_position_in_the_window() {
    // One table over both reasons to decline. The two are independent, and separate tests
    // that each hold one fixed cannot show that either alone is sufficient.
    //
    // The positions are the two literals, so the rows carry their own claim about where the
    // window ends rather than restating the constant back at itself.
    let cases = [
        ("open window, interrupts enabled", 0, true, true),
        (
            "last T-state of the window, interrupts enabled",
            LAST_ASSERTED_POSITION,
            true,
            true,
        ),
        (
            "one T-state past the window, interrupts enabled",
            FIRST_RELEASED_POSITION,
            true,
            false,
        ),
        ("open window, interrupts disabled", 0, false, false),
        (
            "last T-state of the window, interrupts disabled",
            LAST_ASSERTED_POSITION,
            false,
            false,
        ),
    ];

    for (name, at, iff1, vectors) in cases {
        let mut machine = armed_at(at, iff1);
        let before = machine.cpu_state();

        machine.step();

        let after = machine.cpu_state();
        if vectors {
            assert_eq!(
                after.pc, MODE_1_VECTOR,
                "{name}: mode 1 must vector to {MODE_1_VECTOR:#06X}"
            );
            assert_eq!(
                after.sp,
                STACK_TOP - 2,
                "{name}: the return address must be pushed"
            );
            assert!(!after.iff1, "{name}: acceptance clears both flip-flops");
            assert!(!after.iff2, "{name}: acceptance clears both flip-flops");
        } else {
            assert_eq!(
                after.pc,
                UNCONTENDED_CODE + 1,
                "{name}: the machine must have run its NOP instead of vectoring"
            );
            assert_eq!(after.sp, STACK_TOP, "{name}: nothing may be pushed");
            assert_eq!(
                after.iff1, before.iff1,
                "{name}: a declined offer changes nothing"
            );
        }
        assert_eq!(machine.fault(), None, "{name}");
    }
}

#[test]
fn halt_is_escaped_by_the_frame_interrupt() {
    // Positioned past the window first, so the `HALT` genuinely executes rather than being
    // pre-empted by the offer waiting at the top of the frame.
    //
    // **The one place [`INTERRUPT_T_STATES`] is still read, and deliberately.** It is a
    // position, not an expectation: the sentence it encodes is "wherever the window ends, get
    // past it", which stays both true and legible whatever the constant becomes. Substituting
    // the literal 32 here would make this test red under a 33 T-state window for a reason that
    // has nothing to do with `HALT` — the offer would pre-empt the instruction — which is a
    // worse failure message than the ones the window's own gates already produce.
    let mut machine = machine();
    advance_to(&mut machine, INTERRUPT_T_STATES);
    write_program(&mut machine, UNCONTENDED_CODE, &[HALT]);
    enable_interrupts(&mut machine, InterruptMode::Mode1);
    set_pc(&mut machine, UNCONTENDED_CODE);

    machine.step();
    assert!(machine.cpu_state().halted, "HALT must stop the CPU");
    assert_eq!(
        machine.cpu_state().pc,
        UNCONTENDED_CODE,
        "PC must stay on the HALT opcode, which is what makes resumption land correctly"
    );

    let halted_at = elapsed(&machine);
    machine.run_frame();
    assert_eq!(machine.frames(), 1);
    assert!(
        machine.cpu_state().halted,
        "nothing but an interrupt may leave HALT, and the line is low only at a boundary"
    );
    assert!(
        elapsed(&machine) - halted_at > u64::from(NOP_T_STATES),
        "the machine must have spent real time halted"
    );

    machine.step();
    assert!(
        !machine.cpu_state().halted,
        "the interrupt must resume the CPU"
    );
    assert_eq!(machine.cpu_state().pc, MODE_1_VECTOR);

    let sp = machine.cpu_state().sp;
    let return_address = u16::from_le_bytes([
        machine.memory().read(sp),
        machine.memory().read(sp.wrapping_add(1)),
    ]);
    assert_eq!(
        return_address,
        UNCONTENDED_CODE + 1,
        "a resumed HALT returns to the instruction after it, never to the HALT itself"
    );
}

#[test]
fn the_real_rom_counts_one_frame_interrupt_per_frame() {
    // The strongest evidence available: Sinclair's own interrupt handler, reached and
    // executed, leaving a number behind. Nothing here asserts that this crate raised an
    // interrupt — it asserts that the ROM observed one, once per frame, for ten frames.
    let Some(rom) = sinclair_rom() else {
        return;
    };
    let mut machine = Spectrum::new(&rom).expect("the 48K ROM is one page");

    machine.run_frames(SETTLE_FRAMES);
    let settled = rom_frames(&machine);
    assert!(
        settled > 0,
        "the ROM's FRAMES counter never moved in {SETTLE_FRAMES} frames: its handler is not \
         being reached at all"
    );

    machine.run_frames(SAMPLE_FRAMES);

    assert_eq!(
        rom_frames(&machine) - settled,
        SAMPLE_FRAMES,
        "the ROM's own frame counter must advance once per frame of emulated time"
    );
    assert_eq!(machine.frames(), SETTLE_FRAMES + SAMPLE_FRAMES);
    assert_eq!(machine.fault(), None);
}

/// The 48K ROM's `FRAMES` system variable, as the ROM's handler maintains it.
fn rom_frames(machine: &Spectrum) -> u64 {
    (0..FRAMES_COUNTER_LEN)
        .map(|byte| u64::from(machine.memory().read(FRAMES_COUNTER + byte)) << (8 * byte))
        .sum()
}
