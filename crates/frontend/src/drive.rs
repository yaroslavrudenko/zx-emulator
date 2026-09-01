//! What the status bar says about the tape drive, asked of the machine rather than of the key.
//!
//! # The defect this exists to fix, which cost an evening
//!
//! `F3` reported `tape playing` **unconditionally**, because the shell wrote the message from the
//! command rather than from the drive:
//!
//! ```text
//! Hotkey::PlayTape => {
//!     machine.tape_mut().play();
//!     status.report("tape playing".to_owned());
//! }
//! ```
//!
//! [`spectrum::tape::Tape::play`] is `self.playing = self.index < self.pulses.len()`, so there are
//! two inputs on which it starts nothing at all — an **empty drive**, and a tape **already wound
//! to its end** — and in both the bar said `tape playing` anyway. A third silence follows from the
//! same place: when the train runs out the tape stops *itself*, and nothing reported that either,
//! so the message from the press stayed on the screen describing a drive that had not moved for
//! minutes.
//!
//! Those three are one defect with one shape, and it is the shape `crate::pacing`'s header calls
//! *"a readout answering a question nobody asked"*: **a message that reports the keystroke instead
//! of its effect cannot be wrong about the keystroke and cannot be right about anything else.**
//! On 2026-09-01 that cost the owner an evening — a cassette wound off unread while the bar read
//! `tape playing` throughout, and the one press that could have said so said nothing.
//!
//! So every one of the three tape keys is answered from [`spectrum::tape::Tape`] *after* the key
//! has acted, and the drive is watched between keys so that a tape ending is news. The strings are
//! here rather than in the shell for the reason `tests/on_screen_strings.rs` records in its own
//! header — it *"cannot cover `main.rs`'s own literals … because they are private to a binary that
//! needs a window"* — and these are messages a person reads at exactly the moment they are
//! confused, which is the worst possible moment for one to be unreachable by a test.
//!
//! # What this cannot say, and it is the interesting half
//!
//! [`Drive::ran_out`] reports that a tape **reached its end**. It does not report that the tape
//! reached its end **unread**, which is the sentence the owner actually needed, because nothing on
//! [`spectrum::Spectrum`]'s public surface says whether the machine was reading. The signal that
//! would — the rate at which the machine samples the `EAR` bit — is named in this crate's report
//! along with the measurement that picks it, and it is one field and one accessor away. Until it
//! exists this module says the true smaller thing rather than guessing the larger one.

use spectrum::tape::Tape;

/// The drive is turning and there is a signal on the line.
pub const PLAYING: &str = "tape playing";

/// `F3`, `F4` or `F5` on a machine with no cassette in it.
///
/// It names the gesture that fixes it, for the reason `main.rs`'s opening message names the
/// `SYMBOL SHIFT` alias: somebody who has just pressed a key and seen nothing happen is owed the
/// next step, not a diagnosis. Drag-and-drop is the only runtime load this frontend has —
/// `docs/M8.md` Decision 11 refuses a file picker and says what that costs — so it is what is
/// named.
pub const NO_TAPE: &str = "no tape in the drive - drop a .tap or .tzx on the window";

/// `F3` on a tape wound to its end: the press did nothing, and silently.
///
/// **The message a lost evening was spent not seeing.** A person whose tape has run off the end
/// presses PLAY again — it is the obvious thing to do, and the diagnostic harness recorded the
/// owner doing exactly that — and until now the emulator answered by claiming to play. Naming
/// `F5` in the same breath is the whole point: the state is recoverable in one keystroke, and the
/// only thing standing between a person and that keystroke was being told.
pub const AT_THE_END: &str = "tape is at its end and did not start - F5 rewinds it";

/// The train ran out while the drive was turning, so the tape stopped itself.
///
/// Reported on the tick it happens rather than left for somebody to infer from a stalled `frame`
/// counter. Under [`crate::pacing::Rung::Automatic`] a three-minute cassette can reach this in
/// about two seconds of wall clock, which is comfortably faster than a person can read the line
/// that is on the bar — so if this does not appear at the moment it happens, it may as well not
/// exist.
pub const RAN_OUT: &str = "tape reached its end - F5 rewinds it";

/// `F4` stopped a drive that had a cassette in it.
pub const STOPPED: &str = "tape stopped";

/// `F5` wound a cassette back to its start.
pub const REWOUND: &str = "tape rewound";

/// The drive, as the status bar last described it.
///
/// # Why this holds a flag when `crate::keymap` says not to
///
/// `crate::keymap` records that *"shadowing state that the owner can change behind your back is
/// how a frontend acquires a bug nothing can see"*, and [`spectrum::tape::Tape::is_playing`] exists
/// because of it. This is not that, and the difference is worth stating because it looks alike.
///
/// The flag here is **not a second copy of the drive's state**; it is a record of *what this
/// program last said out loud*, and it is rewritten from [`spectrum::tape::Tape`] at every point
/// where the frontend touches the drive. It can therefore never disagree with the machine about
/// the present — only about the past, which is the entire question being asked. A tape stopping
/// **on its own** is news; a tape stopping because somebody pressed `F4` is not, and no accessor
/// on the machine can tell the two apart, because the difference is not a property of the machine.
///
/// It is one `bool` and every method rewrites it, so there is no path that leaves it stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Drive {
    /// Whether the drive was turning when this program last looked.
    playing: bool,
}

impl Drive {
    /// A watch that has not seen the drive turn.
    #[must_use]
    pub const fn new() -> Self {
        Self { playing: false }
    }

    /// What to report once `F3` has pressed PLAY on `tape`.
    ///
    /// The three answers are the three things [`spectrum::tape::Tape::play`] can do, and they are
    /// distinguished the way the machine distinguishes them: it started, or there was nothing to
    /// start, or there was a cassette and it was spent.
    pub fn played(&mut self, tape: &Tape) -> &'static str {
        self.playing = tape.is_playing();
        if self.playing {
            PLAYING
        } else if tape.pulses().is_empty() {
            NO_TAPE
        } else {
            AT_THE_END
        }
    }

    /// What to report once `F4` has stopped `tape`.
    pub fn stopped(&mut self, tape: &Tape) -> &'static str {
        self.playing = tape.is_playing();
        if tape.pulses().is_empty() {
            NO_TAPE
        } else {
            STOPPED
        }
    }

    /// What to report once `F5` has rewound `tape`.
    ///
    /// `F4` and `F5` answer the empty drive too, and that is not scope creep — it is the same
    /// silence on the other two keys. A person who drops nothing and presses all three learns
    /// from the first one they try rather than from whichever they happened to try first.
    pub fn rewound(&mut self, tape: &Tape) -> &'static str {
        self.playing = tape.is_playing();
        if tape.pulses().is_empty() {
            NO_TAPE
        } else {
            REWOUND
        }
    }

    /// Follow the drive to wherever it now stands, reporting nothing.
    ///
    /// For the one path that changes the drive without being a tape key: a **dropped cassette**
    /// replaces the drive's contents, and a tape swapped out mid-play is not a tape that ran out.
    /// Without this the next tick would report [`RAN_OUT`] over the top of the message naming the
    /// file that had just been loaded — the same class of lie this module exists to remove,
    /// arriving from the one direction the keys do not cover.
    pub const fn follow(&mut self, tape: &Tape) {
        self.playing = tape.is_playing();
    }

    /// `Some` on the tick a tape ran out on its own; `None` otherwise.
    ///
    /// Called once per tick, after the frames have run. It answers a question about a *transition*,
    /// so it is the one method here that reads the flag before writing it.
    pub const fn ran_out(&mut self, tape: &Tape) -> Option<&'static str> {
        let was_playing = self.playing;
        self.playing = tape.is_playing();
        if was_playing && !self.playing {
            Some(RAN_OUT)
        } else {
            None
        }
    }
}
