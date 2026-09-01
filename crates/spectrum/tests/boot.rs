//! Gate: the real 48K ROM boots to `© 1982 Sinclair Research Ltd`.
//!
//! # Why this file exists rather than the example
//!
//! `examples/boot.rs` has printed this verdict since M5 landed, and `cargo test` **builds an
//! example without ever calling its `main`**. So the M5 gate ran nowhere: with
//! `testdata/roms/48.rom` deleted the suite stayed green and the test count did not change.
//! That is this project's own recorded failure — *"an `#[ignore]`d gate that no pipeline
//! executes is not a gate"* — in a new shape, and the shape is worse, because an example
//! looks like it is being run by the build.
//!
//! # What is graded here
//!
//! Two facts, and the second is the one that does the work.
//!
//! - **The message appears.** Binary, and it exercises the memory map, the screen address
//!   layout, the interrupt and the keyboard scan in one go.
//! - **The frame it first appears on.** Measured by mutation: deleting contention entirely
//!   leaves the message appearing — the ROM's start-up is a sequence of instructions, not of
//!   deadlines — but it appears on frame **85** instead of 87, because the machine now
//!   executes more instructions per frame. The example printed that number as a "regression
//!   signal" and asserted nothing, so it caught nothing.
//!
//!   This is a **change detector, not a correctness claim.** Nothing establishes that 87 is
//!   the hardware's figure. A legitimate change to the machine's timing will move it, and the
//!   right response is to re-measure and update the constant — deliberately, having seen it
//!   move, which is exactly what an unasserted printout never forces.
//!
//! # What is not graded here
//!
//! Everything the other gates in this directory exist for. The mutation table above is why:
//! four of the five properties they cover leave this gate green. Reaching the copyright
//! message says the memory map and the screen are right and says almost nothing else.

mod common;

use common::sinclair_rom;
use spectrum::Spectrum;
use spectrum::screen::read_text;

/// What the ROM prints once it has finished starting up.
const COPYRIGHT: &str = "\u{a9} 1982 Sinclair Research Ltd";

/// Frames to give the ROM — four seconds of emulated time, more than twice what it needs.
const FRAMES: u64 = 200;

/// The frame the message first appears on today.
///
/// A change detector. See the module documentation for what it does and does not claim.
const EXPECTED_FRAME: u64 = 87;

#[test]
fn the_rom_boots_to_the_copyright_message() {
    let Some(rom) = sinclair_rom() else {
        return;
    };
    let mut machine = Spectrum::new(&rom).expect("the 48K ROM is one page");

    let mut appeared = None;
    for _ in 0..FRAMES {
        machine.run_frame();
        if appeared.is_none() && shows_copyright(&machine) {
            appeared = Some(machine.frames());
        }
    }

    assert_eq!(
        appeared,
        Some(EXPECTED_FRAME),
        "the ROM must reach {COPYRIGHT:?}, and it must take the same number of frames to get \
         there. The screen as it stands:\n{}",
        read_text(machine.memory()).join("\n")
    );
    assert_eq!(
        machine.fault(),
        None,
        "a Spectrum cannot fault: its bus floats to 0xFF, which is RST 38h"
    );
}

#[test]
fn the_machine_is_still_running_its_interrupt_loop_at_the_end() {
    // The message being on screen is a snapshot; this is the assertion that the machine did
    // not reach it and then wedge. `HALT` with interrupts off, or interrupts never accepted
    // again, would both leave the screen looking perfect.
    let Some(rom) = sinclair_rom() else {
        return;
    };
    let mut machine = Spectrum::new(&rom).expect("the 48K ROM is one page");
    machine.run_frames(FRAMES);

    let state = machine.cpu_state();
    assert!(
        state.iff1,
        "the ROM's editor loop runs with interrupts enabled"
    );
    assert!(!state.halted, "the machine must not be wedged in HALT");
    assert_eq!(machine.frames(), FRAMES);
}

/// Whether the copyright message is on the screen now.
fn shows_copyright(machine: &Spectrum) -> bool {
    read_text(machine.memory())
        .iter()
        .any(|line| line.contains(COPYRIGHT))
}
