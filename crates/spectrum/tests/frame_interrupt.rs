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
//! - The line is asserted at frame position zero, and stays asserted for exactly
//!   [`INTERRUPT_T_STATES`] — checked at each position it reaches, and at the last T-state
//!   inside the window and the first outside it.
//! - Acceptance as a function of `IFF1` **and** position, as one table rather than as
//!   separate cases: the window and the flip-flop are two independent reasons to decline and
//!   a gate that varies one at a time cannot see them interact.
//! - `HALT` is escaped by it, with the return address the hardware pushes.
//! - **The real ROM's own `FRAMES` counter advances once per frame.** That is the strongest
//!   evidence available here: it is not this crate asserting that it raised an interrupt, it
//!   is Sinclair's interrupt handler being reached, executing, and leaving a number behind.
//!
//! # What is not graded here
//!
//! - **The 32 T-state window length itself.** It is asserted to be what the crate says it is,
//!   which pins it against drift; no oracle in this project measures it against hardware. A
//!   machine holding the line for 24 or 48 T-states would pass every assertion here after
//!   the constant was changed to match.
//! - **Interrupt modes 0 and 2.** The Spectrum's bus floats to `RST 38h`, so mode 0 lands
//!   where mode 1 does and mode 2 is never used by the 48K ROM. `crates/spectrum/src/lib.rs`
//!   covers the mode 0 case; nothing here or anywhere else grades mode 2 on a machine.
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
    let mut machine = machine();
    write_program(&mut machine, UNCONTENDED_CODE, &[NOP; 16]);
    set_pc(&mut machine, UNCONTENDED_CODE);

    while machine.frame_t_state() < INTERRUPT_T_STATES {
        assert!(
            machine.ula().interrupt_asserted(),
            "/INT dropped at frame position {}, inside the {INTERRUPT_T_STATES} T-state window",
            machine.frame_t_state()
        );
        machine.step();
    }

    assert_eq!(
        machine.frame_t_state(),
        INTERRUPT_T_STATES,
        "uncontended NOPs should land exactly on the end of the window"
    );
    assert!(
        !machine.ula().interrupt_asserted(),
        "/INT must drop once the window has elapsed"
    );
}

#[test]
fn the_last_t_state_inside_the_window_still_asserts_and_the_first_outside_does_not() {
    // The boundary itself, positioned exactly rather than stepped over — the sampling above
    // can only land on multiples of a NOP.
    let inside = machine_at(INTERRUPT_T_STATES - 1);
    assert_eq!(inside.frame_t_state(), INTERRUPT_T_STATES - 1);
    assert!(
        inside.ula().interrupt_asserted(),
        "the window includes its last T-state"
    );

    let outside = machine_at(INTERRUPT_T_STATES);
    assert!(
        !outside.ula().interrupt_asserted(),
        "the window excludes the T-state after its last"
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
    let cases = [
        ("open window, interrupts enabled", 0, true, true),
        (
            "last T-state of the window, interrupts enabled",
            INTERRUPT_T_STATES - 1,
            true,
            true,
        ),
        (
            "one T-state past the window, interrupts enabled",
            INTERRUPT_T_STATES,
            true,
            false,
        ),
        ("open window, interrupts disabled", 0, false, false),
        (
            "last T-state of the window, interrupts disabled",
            INTERRUPT_T_STATES - 1,
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
