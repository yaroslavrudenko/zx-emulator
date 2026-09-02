//! The three things the tape drive can do silently, and the assertion that it no longer does.
//!
//! # What this grades
//!
//! [`spectrum::tape::Tape::play`] is `self.playing = self.index < self.pulses.len()`, so pressing
//! PLAY has three outcomes and the shell used to report one of them: it started, or the drive was
//! **empty**, or the cassette was already **wound to its end**. A fourth silence follows from the
//! same field — playback clears it when the train runs out, so a tape stops **itself** and nothing
//! said so. All four are graded here, against a real cassette in a real machine driven to each
//! state, rather than by handing [`drive::Drive`] a constructed answer.
//!
//! # The negative controls are the half that discriminates
//!
//! Every positive assertion below passes on a `Drive` that reported the right string for the
//! wrong reason — one that always said [`drive::RAN_OUT`], say, would satisfy the first half of
//! `a_tape_that_runs_out_says_so_exactly_once` perfectly. So the tests that decide whether this
//! module works are the ones asserting it stays **quiet**: `F4` is not a tape running out, a
//! **dropped**
//! cassette is not a tape running out, and a tape that has already been reported must not be
//! reported again on every tick for the rest of the session. That last one is the difference
//! between a message and a stuck message, and it is the failure `crate::pacing`'s `LossMeter`
//! exists to remove one layer up.
//!
//! # Why the ROM is sixteen kilobytes of nothing
//!
//! What has to happen is that **time passes** — [`spectrum::Ula`]'s `advance` moves the clock and
//! the tape together, and every instruction ticks the clock — so what the CPU executes is
//! irrelevant and a Sinclair ROM would only make the test need `testdata/`. A NOP sled is the
//! smallest machine that advances a cassette, and `crates/testsupport` exists because a gate
//! backed by a corpus is a gate that might not run. This file runs on a clean checkout.
//!
//! # What it does not grade
//!
//! Whether anybody **sees** the message. `Status::draw` needs a GPU and never runs under
//! `cargo test`, which is the standing limit `crates/frontend/src/lib.rs` records for this whole
//! crate. What is reachable is the decision — which string, and when — and that is what is here.
//! The width of these strings is gated in `src/main.rs`'s own `mod tests`, next to the window
//! measurement it needs, and their drawability in `tests/on_screen_strings.rs`.

use frontend::drive::{self, Drive};
use spectrum::Spectrum;
use spectrum::tape::Tape;

/// A ROM of nothing but `NOP`, so the clock advances and the cassette with it.
const NOTHING: [u8; 16 * 1024] = [0x00; 16 * 1024];

/// Half-periods totalling 60 T-states: a cassette that runs out inside one frame.
const A_SHORT_TAPE: [u32; 3] = [10, 20, 30];

/// Half-periods totalling 200,000 T-states, which is about three frames of a 48K.
const A_LONGER_TAPE: [u32; 2] = [100_000, 100_000];

fn machine_holding(pulses: &[u32]) -> Spectrum {
    let mut machine = Spectrum::new(&NOTHING).expect("16 KB is a 48K ROM");
    machine.insert_tape(Tape::new(pulses.to_vec()));
    machine
}

/// A machine whose cassette has been played to its end.
fn wound_off() -> Spectrum {
    let mut machine = machine_holding(&A_SHORT_TAPE);
    machine.tape_mut().play();
    machine.run_frame();
    assert!(
        !machine.tape().is_playing(),
        "the fixture is wrong: 60 T-states must not survive a 69,888 T-state frame",
    );
    machine
}

#[test]
fn play_on_an_empty_drive_says_the_drive_is_empty() {
    // The first of the three silences. `Tape::default()` is the no-cassette state, and it is the
    // state a person is in when they open the emulator with no arguments — so this is the press
    // most likely to be somebody's first.
    let mut machine = Spectrum::new(&NOTHING).expect("16 KB is a 48K ROM");
    let mut watch = Drive::new();

    machine.tape_mut().play();
    assert_eq!(watch.played(machine.tape()), drive::NO_TAPE);
    assert!(!machine.tape().is_playing(), "and nothing started");
}

#[test]
fn play_on_a_tape_wound_to_its_end_says_so_and_names_the_key_that_fixes_it() {
    // The second silence, and the expensive one: this is the press the owner made when his
    // cassette had run off the end under `Rung::Automatic`, and the bar answered `tape playing`.
    let mut machine = wound_off();
    let mut watch = Drive::new();

    machine.tape_mut().play();
    assert!(
        !machine.tape().is_playing(),
        "a wound-off tape stays stopped"
    );
    let message = watch.played(machine.tape());
    assert_eq!(message, drive::AT_THE_END);
    assert!(
        message.contains("F5"),
        "the state is recoverable in one keystroke, and the message has to name it: {message}",
    );
}

#[test]
fn play_on_a_cassette_with_signal_left_says_it_is_playing() {
    let mut machine = machine_holding(&A_LONGER_TAPE);
    let mut watch = Drive::new();

    machine.tape_mut().play();
    assert_eq!(watch.played(machine.tape()), drive::PLAYING);
}

#[test]
fn a_tape_that_runs_out_says_so_exactly_once() {
    // The third silence, plus the control that makes it worth having. A report that fired on the
    // transition and then again on every tick afterwards would pin the message to the bar for
    // ever, which is the latching failure `pacing::LossMeter` was written to remove — the same
    // defect one layer up, and it would look identical from the outside on the first frame.
    let mut machine = machine_holding(&A_SHORT_TAPE);
    let mut watch = Drive::new();

    machine.tape_mut().play();
    assert_eq!(watch.played(machine.tape()), drive::PLAYING);
    assert_eq!(
        watch.ran_out(machine.tape()),
        None,
        "nothing has happened yet"
    );

    machine.run_frame();
    assert_eq!(watch.ran_out(machine.tape()), Some(drive::RAN_OUT));

    for tick in 0..10 {
        machine.run_frame();
        assert_eq!(
            watch.ran_out(machine.tape()),
            None,
            "reported again on tick {tick} after the one that mattered",
        );
    }
}

#[test]
fn stopping_a_tape_is_not_a_tape_running_out() {
    // The discriminating negative. `F4` takes the drive from turning to stopped, which is the
    // *same transition* a cassette ending produces — and no accessor on the machine can tell them
    // apart, because the difference is not a property of the machine. It is a property of who did
    // it, which is why `Drive` is told by the key rather than left to infer.
    let mut machine = machine_holding(&A_LONGER_TAPE);
    let mut watch = Drive::new();

    machine.tape_mut().play();
    assert_eq!(watch.played(machine.tape()), drive::PLAYING);

    machine.tape_mut().stop();
    assert_eq!(watch.stopped(machine.tape()), drive::STOPPED);
    assert_eq!(
        watch.ran_out(machine.tape()),
        None,
        "a tape somebody stopped has not reached its end",
    );
}

#[test]
fn swapping_the_cassette_mid_play_is_not_a_tape_running_out() {
    // The other direction the same lie can arrive from, and the one no tape key covers: a file
    // dropped on the window replaces the drive's contents, so a tape that was turning is simply
    // gone. Without `Drive::follow` the next tick would report `RAN_OUT` over the top of the
    // message naming the file that had just loaded.
    let mut machine = machine_holding(&A_LONGER_TAPE);
    let mut watch = Drive::new();

    machine.tape_mut().play();
    assert_eq!(watch.played(machine.tape()), drive::PLAYING);

    machine.insert_tape(Tape::new(A_LONGER_TAPE.to_vec()));
    watch.follow(machine.tape());
    assert_eq!(
        watch.ran_out(machine.tape()),
        None,
        "a cassette that was taken out did not reach its end",
    );
}

#[test]
fn stop_and_rewind_answer_an_empty_drive_too() {
    // The same silence on the other two keys. A person who has dropped nothing and is pressing
    // keys to find out what they do learns from whichever one they try, not from `F3` alone.
    let mut machine = Spectrum::new(&NOTHING).expect("16 KB is a 48K ROM");
    let mut watch = Drive::new();

    machine.tape_mut().stop();
    assert_eq!(watch.stopped(machine.tape()), drive::NO_TAPE);
    machine.tape_mut().rewind();
    assert_eq!(watch.rewound(machine.tape()), drive::NO_TAPE);
}

#[test]
fn rewinding_a_wound_off_cassette_makes_it_playable_again() {
    // The recovery `AT_THE_END` promises, asserted rather than left as a claim in a string. A
    // message naming a key that does not fix the state would be worse than saying nothing.
    let mut machine = wound_off();
    let mut watch = Drive::new();

    machine.tape_mut().rewind();
    assert_eq!(watch.rewound(machine.tape()), drive::REWOUND);
    machine.tape_mut().play();
    assert_eq!(watch.played(machine.tape()), drive::PLAYING);
    assert!(
        machine.tape().is_playing(),
        "and the drive really is turning"
    );
}

#[test]
fn the_reports_can_disagree() {
    // The positive control. Every assertion above compares a string to a constant, and a module
    // that returned the same constant for all four states would satisfy any one of them read
    // alone. This is the assertion that the four states are actually distinguishable, and it is
    // the one that fails first if `Drive::played` ever loses a branch.
    let mut watch = Drive::new();
    let mut empty = Spectrum::new(&NOTHING).expect("16 KB is a 48K ROM");
    empty.tape_mut().play();
    let on_empty = watch.played(empty.tape());

    let mut spent = wound_off();
    spent.tape_mut().play();
    let on_spent = watch.played(spent.tape());

    let mut fresh = machine_holding(&A_LONGER_TAPE);
    fresh.tape_mut().play();
    let on_fresh = watch.played(fresh.tape());

    assert_ne!(on_empty, on_spent);
    assert_ne!(on_spent, on_fresh);
    assert_ne!(on_fresh, on_empty);
    assert_ne!(
        on_spent,
        drive::RAN_OUT,
        "a press is not the drive stopping"
    );
}
