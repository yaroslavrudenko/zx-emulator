//! The tape: a signal on the `EAR` line, not a shortcut past the machine.
//!
//! ```
//! use spectrum::tape::{Tape, tap};
//!
//! # fn main() -> Result<(), spectrum::tape::Error> {
//! // A one-block .tap: a two-byte length, a flag byte, the data, and a checksum.
//! let file = [0x03, 0x00, 0xFF, 0x2A, 0xD5];
//! let tape = tap::parse(&file)?;
//! assert!(tape.pulses().len() > 3000, "a data block opens with 3223 pilot pulses");
//! # Ok(())
//! # }
//! ```
//!
//! # Why this is a signal and not a ROM trap
//!
//! `docs/M6.md` Decision 4, and it is the largest decision in the milestone. The cheap
//! alternative is to watch for `PC` reaching the ROM's `LD-BYTES` entry at `0x0556`, write the
//! block straight into the buffer `IX`/`DE` describe, set the flags the routine would have
//! set, and return — fifty lines, and it works today.
//!
//! It is rejected because **it makes the milestone's gate grade the trap.** A trap bypasses
//! the ULA, the contention model, the frame clock, the interrupt window and the port
//! decoding — every part of the machine M5 could not grade — so *"a real game runs"* would
//! mean *"the injection works"*. `docs/STATUS.md` records three occasions where this project
//! shipped evidence that graded less than it appeared to and calls the third *"the worst form
//! so far"*; choosing the trap would be the fourth **with the reason known in advance**.
//! Secondarily, it does not even cover the software it is for: most commercial loaders read
//! port `0xFE` themselves precisely to be faster and harder to copy.
//!
//! **A trap is not forbidden forever.** As a debugging convenience it is legitimate, and two
//! rules apply if one ever lands: it is **off by default**, and the tape gates **assert that
//! it is off**. Nothing here sniffs `PC`, and nothing here supplies data to the CPU by any
//! route other than [`crate::Ula`]'s own read of port `0xFE`, whose bit 6 is [`Tape::level`].
//!
//! # Why the internal form is a pulse train and not a block list
//!
//! `docs/M6.md` Decision 5. A `.tap` file **cannot represent a custom loader's tape at all**:
//! it is block data with the ROM's standard timings implied, and nothing in it can say *"this
//! loader uses 700-T-state bits"*. `.tzx` exists for exactly that, and it is what most
//! commercial games are distributed as. A block-list internal form would make `.tzx` a
//! rewrite of the tape subsystem; a pulse train makes it a second converter with the ULA side
//! untouched — which is the same argument `crate::snapshot` makes for keeping the machine's
//! state rather than a file format as its canonical type.
//!
//! Materialising every pulse costs about 16 `u32`s per data byte plus the pilot tone, so a
//! 48 KB tape is a few megabytes. `docs/ARCHITECTURE.md` makes performance a non-goal and
//! nothing here is on a hot path; a lazy generator would save the memory and cost a state
//! machine, and the flat vector is simple enough to **assert exactly** — a one-byte block's
//! pulse train is written out by hand in `crates/spectrum/tests/tape_signal.rs`.
//!
//! # Where time comes from
//!
//! Not from here. Contention means the clock does not advance one T-state at a time, so the
//! tape cannot be driven from `Bus::tick` alone. [`crate::Ula`] has one private `advance` that
//! moves the clock **and** the tape, and every `Clock::advance` call site routes through it —
//! one place, so the two cannot drift.

pub mod tap;

use std::fmt;

/// Why a tape file could not be read.
///
/// `Copy` and allocation-free, matching [`Error`](crate::snapshot::Error),
/// [`RomSizeError`](crate::RomSizeError) and [`StepError`](::z80::StepError). Two error types
/// rather than one shared with `snapshot`: a malformed tape is not a malformed snapshot, and
/// the siblings in this workspace are all small and specific.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The file ended in the middle of a block.
    #[error("truncated at offset {offset}: {needed} bytes needed, {available} available")]
    Truncated {
        /// Where the field that did not fit begins.
        offset: usize,
        /// How many bytes it needed.
        needed: usize,
        /// How many were left.
        available: usize,
    },

    /// A block declared a length of zero, so it carries not even a flag byte.
    ///
    /// The flag byte is what decides the pilot tone's length, so a block without one cannot
    /// be converted into a signal at all. Refused rather than skipped: a file this malformed
    /// is one we have misparsed, and saying so beats playing a tape that is missing a block.
    #[error("the block at offset {offset} declares a length of zero")]
    EmptyBlock {
        /// Where the two-byte length word begins.
        offset: usize,
    },
}

/// A cassette, as the succession of `EAR` levels it drives.
///
/// The `Vec` holds **half-period lengths in T-states**: during `pulses[i]` the signal holds
/// one level, and at the end of it the signal flips. So a "pulse" here is one edge-to-edge
/// interval, which is the quantity the ROM's loader actually measures — it counts edges and
/// times the gaps, and never observes an absolute level.
///
/// [`Default`] is a tape drive with nothing in it: no pulses, not playing, and the level low.
/// That is the same state the machine has always been in, which is why inserting no tape
/// leaves `IN A,(0xFE)` returning the `0xBF` that `crates/spectrum/tests/keyboard_matrix.rs`
/// has pinned since M5.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Tape {
    /// Half-period lengths in T-states, in playback order.
    pulses: Vec<u32>,
    /// Which half-period is playing now.
    index: usize,
    /// T-states left in it.
    remaining: u32,
    /// The level the `EAR` line is being driven to.
    level: bool,
    /// Whether time moves the head.
    playing: bool,
}

impl Tape {
    /// A stopped tape holding `pulses`, wound to the start.
    ///
    /// A zero-length half-period is legal and is consumed instantly, taking its edge with it.
    /// That keeps this total — there is no input a caller can hand over that has no meaning —
    /// and it cannot spin: every iteration of the playback loop either consumes T-states or
    /// moves the index forward, and the index is bounded by the vector's length.
    #[must_use]
    pub fn new(pulses: Vec<u32>) -> Self {
        let remaining = pulses.first().copied().unwrap_or(0);
        Self {
            pulses,
            index: 0,
            remaining,
            level: false,
            playing: false,
        }
    }

    /// Start the motor. A tape already at its end stays stopped.
    pub fn play(&mut self) {
        self.playing = self.index < self.pulses.len();
    }

    /// Stop the motor, holding the signal where it stands.
    pub fn stop(&mut self) {
        self.playing = false;
    }

    /// Wind back to the start, leaving the motor as it is.
    ///
    /// The level goes back to low with it: a rewound tape has to present the same signal it
    /// did the first time, or a second load of the same tape is a different experiment.
    pub fn rewind(&mut self) {
        self.index = 0;
        self.remaining = self.pulses.first().copied().unwrap_or(0);
        self.level = false;
    }

    /// The level the `EAR` line is being driven to right now.
    ///
    /// Low with no tape, which is the state an issue 3 machine idles in and the state
    /// [`crate::Ula`] has always reported.
    #[must_use]
    pub fn level(&self) -> bool {
        self.level
    }

    /// The half-period lengths, in T-states and in playback order.
    ///
    /// Public because the pulse train **is** the tape in this design rather than an
    /// implementation detail of one — `docs/M6.md` Decision 5 — and because the gate that
    /// grades the `.tap` converter decodes this back into bytes with a decoder written in the
    /// test. An expectation computed by the subject is a tautology; reading the train is what
    /// lets the test own its own decoder.
    #[must_use]
    pub fn pulses(&self) -> &[u32] {
        &self.pulses
    }

    /// Move the head `t_states` forward, flipping the signal at every half-period boundary.
    ///
    /// Called only from [`crate::Ula`]'s own `advance`, alongside the clock, so elapsed time
    /// reaches both or neither.
    pub(crate) fn advance(&mut self, mut t_states: u32) {
        while self.playing && t_states > 0 {
            if self.remaining > t_states {
                self.remaining -= t_states;
                return;
            }
            t_states -= self.remaining;
            self.finish_pulse();
        }
    }

    /// The current half-period has elapsed: flip the signal and take the next one.
    fn finish_pulse(&mut self) {
        self.level = !self.level;
        self.index += 1;
        match self.pulses.get(self.index) {
            Some(&length) => self.remaining = length,
            None => {
                self.playing = false;
                self.remaining = 0;
            }
        }
    }
}

impl fmt::Debug for Tape {
    /// Deliberately not derived, for the reason [`Memory`](crate::Memory) already had:
    /// *"a derived `Debug` prints 160 KB of page contents, which makes every failing assertion
    /// involving a machine unreadable."* A 48 KB tape is close to a million half-periods, and
    /// [`Tape`] is reachable from `Spectrum`'s `Debug` through the [`crate::Ula`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tape")
            .field("pulses", &self.pulses.len())
            .field("index", &self.index)
            .field("remaining", &self.remaining)
            .field("level", &self.level)
            .field("playing", &self.playing)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every production source file of this module.
    ///
    /// Listed rather than globbed, because a file that quietly stopped being scanned would be
    /// indistinguishable from a file with nothing to find.
    const SOURCES: [(&str, &str); 2] = [
        ("tape/mod.rs", include_str!("mod.rs")),
        ("tape/tap.rs", include_str!("tap.rs")),
    ];

    /// The production half of `source` — everything above its `#[cfg(test)]` module.
    fn production(source: &str) -> &str {
        source
            .split_once("\n#[cfg(test)]")
            .map_or(source, |(head, _)| head)
    }

    #[test]
    fn nothing_in_the_tape_module_can_panic_on_purpose() {
        // `docs/M6.md` Decision 6 lists `tape/` alongside `snapshot/`: this crate builds with
        // `panic = "abort"` in release, so a panic is not a recoverable error — it kills the
        // process, and `catch_unwind` is not available as a backstop. `tap::parse` reads
        // attacker-controlled block lengths, so the requirement is not "do not panic on the
        // inputs we tested"; it is that the constructs are absent.
        //
        // The sibling gate in `snapshot/mod.rs` also scans for slice **indexing**, with a
        // scanner that has its own failing cases. That scanner is inside another module's
        // test tree and is not reachable from here; lifting it into a shared test helper is
        // the right fix and is left for whoever next owns both modules. Until then this file
        // asserts the half a substring search can decide, and `tap.rs`'s exhaustive
        // truncation sweep asserts the behaviour — the two are not substitutes.
        const FORBIDDEN: [&str; 6] = [
            ".unwrap()",
            ".expect(",
            "panic!(",
            "todo!(",
            "unimplemented!(",
            "unreachable!(",
        ];
        for (name, source) in SOURCES {
            for (number, line) in production(source).lines().enumerate() {
                let statement = line.split("//").next().unwrap_or(line);
                for forbidden in FORBIDDEN {
                    assert!(
                        !statement.contains(forbidden),
                        "{name}:{} uses {forbidden}: {}",
                        number + 1,
                        statement.trim()
                    );
                }
            }
        }
    }

    #[test]
    fn the_production_split_finds_the_test_module() {
        // The gate above is only worth running if it is looking at the production half, and a
        // split that silently matched nothing would scan the tests too — which contain every
        // forbidden construct and would turn the gate red for the wrong reason. So: the split
        // must actually cut, and what it keeps must be shorter than the file.
        for (name, source) in SOURCES {
            assert!(
                production(source).len() < source.len(),
                "{name} has no `#[cfg(test)]` module, so the scanner is reading all of it"
            );
        }
    }

    /// Three half-periods with distinct lengths, so an off-by-one in the index is visible.
    fn tape() -> Tape {
        Tape::new(vec![10, 20, 30])
    }

    /// The level *during* each of the next `t_states` T-states.
    ///
    /// Sampled before advancing, not after. An edge at the end of a half-period belongs to
    /// the T-state that follows it, so reading after the advance would report a train shifted
    /// one T-state early — which is a plausible-looking expectation and would have made this
    /// test agree with a decoder that was off by one in the same direction.
    fn levels(tape: &mut Tape, t_states: u32) -> Vec<bool> {
        (0..t_states)
            .map(|_| {
                let level = tape.level();
                tape.advance(1);
                level
            })
            .collect()
    }

    #[test]
    fn a_tape_that_is_not_playing_holds_its_level() {
        let mut tape = tape();
        assert!(!tape.level());
        tape.advance(1_000_000);
        assert!(!tape.level(), "time does not move a stopped tape");
    }

    #[test]
    fn the_signal_flips_at_the_end_of_every_half_period() {
        // Written out by hand from the lengths above: low for 10, high for 20, low for 30.
        let mut tape = tape();
        tape.play();
        let observed = levels(&mut tape, 60);
        let expected: Vec<bool> = std::iter::repeat_n(false, 10)
            .chain(std::iter::repeat_n(true, 20))
            .chain(std::iter::repeat_n(false, 30))
            .collect();
        assert_eq!(observed, expected);
    }

    #[test]
    fn the_last_half_period_still_ends_with_its_edge() {
        // The edge at the end of the final pulse is a real edge — a loader reading the last
        // bit of a block needs it — so playback ends *after* the flip, not instead of it.
        let mut tape = tape();
        tape.play();
        tape.advance(59);
        assert!(!tape.level());
        tape.advance(1);
        assert!(tape.level(), "the final edge must happen");
        tape.advance(1_000_000);
        assert!(tape.level(), "and the level holds afterwards");
    }

    #[test]
    fn one_long_advance_lands_where_many_short_ones_do() {
        // The property that makes contention safe: the ULA advances by a stall of 0-6
        // T-states and then by single ticks, and the tape must not care which.
        for step in [1, 2, 3, 7, 60] {
            let mut stepped = tape();
            stepped.play();
            let mut done = 0;
            while done < 45 {
                stepped.advance(step.min(45 - done));
                done += step.min(45 - done);
            }
            let mut once = tape();
            once.play();
            once.advance(45);
            assert_eq!(stepped, once, "advancing in steps of {step}");
        }
    }

    #[test]
    fn a_zero_length_half_period_is_an_instant_edge_and_cannot_spin() {
        // Total by construction: every iteration either consumes T-states or moves the index,
        // and the index is bounded. A tape of nothing but zeros therefore terminates.
        let mut tape = Tape::new(vec![0, 0, 0, 5]);
        tape.play();
        tape.advance(1);
        assert!(
            tape.level(),
            "three instant edges leave the level flipped an odd number"
        );
        let mut all_zero = Tape::new(vec![0; 1000]);
        all_zero.play();
        all_zero.advance(1);
        assert!(
            !all_zero.playing,
            "an even number of instant edges, and the tape is spent"
        );
    }

    #[test]
    fn an_empty_tape_never_plays_and_never_drives_the_line() {
        // This is the "no tape inserted" state, and it is the one `keyboard_matrix.rs` has
        // pinned since M5: bit 6 of a `0xFE` read stays clear.
        let mut tape = Tape::default();
        tape.play();
        tape.advance(1_000_000);
        assert!(!tape.level());
        assert_eq!(tape.pulses(), &[] as &[u32]);
    }

    #[test]
    fn rewinding_presents_the_same_signal_a_second_time() {
        let mut tape = tape();
        tape.play();
        let first = levels(&mut tape, 60);
        tape.rewind();
        tape.play();
        let second = levels(&mut tape, 60);
        assert_eq!(first, second);
    }

    #[test]
    fn stopping_and_restarting_resumes_rather_than_restarting() {
        let mut tape = tape();
        tape.play();
        tape.advance(15);
        assert!(tape.level());
        tape.stop();
        tape.advance(1_000_000);
        tape.play();
        tape.advance(15);
        assert!(
            !tape.level(),
            "the second half-period had 5 T-states left, not 20"
        );
    }

    #[test]
    fn debug_does_not_print_the_pulse_train() {
        let rendered = format!("{:?}", Tape::new(vec![7; 100_000]));
        assert!(
            rendered.len() < 200,
            "Debug printed {} bytes",
            rendered.len()
        );
        assert!(rendered.contains("pulses: 100000"));
    }
}
