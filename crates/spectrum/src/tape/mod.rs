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
//! > **That prediction was cashed in, and it held.** [`tzx`] landed as a converter:
//! > [`Tape`] gained no field, no method and no variant to accommodate it, and nothing in
//! > [`crate::ula`] was touched. What the two converters do share — *"most significant bit
//! > first, two equal half-periods per bit"*, *"a data block is a pilot tone, a sync pair,
//! > then those bits"*, and the ceiling on how large a train may grow — lives once, in
//! > `signal`, because `.tzx`'s standard-speed block **is** a `.tap` block and its
//! > turbo-speed block is the same shape with the numbers read from the file instead.
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

mod reader;
mod signal;
pub mod tap;
pub mod tzx;

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
    ///
    /// Shared by [`tap`] and by [`tzx`]'s standard-speed block, which is the same block in a
    /// different wrapper and has the same reason to refuse.
    #[error("the block at offset {offset} declares a length of zero")]
    EmptyBlock {
        /// Where the two-byte length word begins.
        offset: usize,
    },

    /// The file does not open with `ZXTape!` and the end-of-text marker.
    #[error("that is not a .tzx file: it does not begin with the ZXTape! signature")]
    NotATzxFile,

    /// A `.tzx` major revision this converter does not claim to handle.
    ///
    /// The minor number is carried in the message but is never the reason: the format requires
    /// a program to accept any *minor* revision above its own, and an unhandled block within
    /// one is refused by ID instead.
    #[error("this is a .tzx revision {major}.{minor}, and only major revision 1 is supported")]
    UnsupportedVersion {
        /// The file's major revision.
        major: u8,
        /// The file's minor revision.
        minor: u8,
    },

    /// A block ID the format description does not define.
    ///
    /// Refused rather than skipped by the format's general extension rule, because that rule
    /// only covers blocks added after revision 1.10 — so applying it to an unrecognised ID
    /// would skip an arbitrary span of the file and play a wrong train in silence.
    #[error("unknown .tzx block ID {id:#04X} at offset {offset}")]
    UnknownBlock {
        /// Where the block's ID byte is.
        offset: usize,
        /// The ID byte.
        id: u8,
    },

    /// A block whose extent or whose contents this converter cannot determine.
    ///
    /// Two different situations, deliberately one error, because the consequence is identical
    /// and so is what a user can do about it: the tape cannot be played and the block is
    /// named. `0x16` and `0x17` have a length the format description gives two incompatible
    /// answers for; `0x18` and `0x19` have a knowable length and carry **signal**, so skipping
    /// them would drop part of the tape rather than part of its metadata.
    #[error("unsupported .tzx block ID {id:#04X} at offset {offset}")]
    UnplayableBlock {
        /// Where the block's ID byte is.
        offset: usize,
        /// The ID byte.
        id: u8,
    },

    /// A structural block where the format does not allow one.
    ///
    /// A loop end with no loop open, a return with no call in progress, or a nesting the
    /// format forbids.
    #[error("misplaced .tzx block ID {id:#04X} at offset {offset}")]
    MisplacedBlock {
        /// Where the block's ID byte is.
        offset: usize,
        /// The ID byte.
        id: u8,
    },

    /// A `Used bits in the last byte` field outside the format's range of 1 to 8.
    #[error("the block at offset {offset} plays {bits} bits of its last byte, not 1 to 8")]
    UsedBitsOutOfRange {
        /// Where the field is.
        offset: usize,
        /// What it said.
        bits: u8,
    },

    /// A jump, call or loop that leaves the file.
    #[error("the block at offset {offset} jumps to block {target} of {blocks}")]
    JumpOutOfRange {
        /// Where the jumping block's ID byte is.
        offset: usize,
        /// How many blocks the file has.
        blocks: usize,
        /// Where it wanted to go. Signed, because a backward jump that overshoots is negative.
        target: i64,
    },

    /// The file asks for a longer tape than this crate will build.
    ///
    /// **This is the allocation bound**, and it is what makes a `.tzx` loop block safe: three
    /// bytes can ask for a body to be replayed 65535 times, which is a train sized from the
    /// file rather than from the file's length. Reaching the ceiling is this value rather than
    /// an allocation.
    #[error("the block at offset {offset} would take the tape past {limit} half-periods")]
    TapeTooLong {
        /// Where the block that overflowed the tape begins.
        offset: usize,
        /// The ceiling.
        limit: usize,
    },

    /// A single half-period longer than a `u32` of T-states.
    ///
    /// Only a direct recording can ask for one, by multiplying a sample count by a sample
    /// rate. Refused rather than saturated: a half-period of twenty minutes is a file we have
    /// misread, not a tape.
    #[error("the block at offset {offset} asks for a half-period longer than a u32 of T-states")]
    PulseTooLong {
        /// Where the block begins.
        offset: usize,
    },

    /// The file's jumps and loops never reached its end.
    ///
    /// **This is the termination bound**, and it is separate from [`Error::TapeTooLong`]
    /// because a block can be replayed forever while emitting nothing — a jump to itself, or a
    /// loop over blocks that carry no signal — so the tape's length would never grow to catch
    /// it. Unlike every other loop in this crate, progress here is not structural: a jump
    /// revisits a block without consuming any input, so this is a budget and is named as one.
    #[error("the block at offset {offset} was still playing after {limit} blocks")]
    TooManyBlocksPlayed {
        /// Where the block being played when the budget ran out begins.
        offset: usize,
        /// The budget.
        limit: usize,
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

    /// Whether the motor is turning.
    ///
    /// # A frozen surface, widened once and on purpose
    ///
    /// The reason is written here rather than left in a commit message, because this is an
    /// addition to a published type. `crates/frontend/src/keymap.rs` has carried the finding
    /// since the tape got three keys instead of a toggle: **nothing here reported whether the
    /// drive was running**, so a frontend that needed to know had to keep its own flag — and
    /// that flag goes wrong on its own, because *the tape stops itself*. [`Tape::play`] on a
    /// wound-off tape is documented to leave it stopped, and playback clears the same field
    /// when the train runs out. A shadow copy would then say *playing* while the drive said
    /// *stopped*, and the next press would appear to do nothing.
    ///
    /// So the alternative that was refused is not *"a second accessor"*, it is *"a duplicate of
    /// this field in every consumer"* — and `keymap.rs` names the cost in as many words:
    /// *"shadowing state that the owner can change behind your back is how a frontend acquires a
    /// bug nothing can see"*. One method, reporting the one field playback actually reads, is
    /// the smaller surface of the two.
    ///
    /// It reports the **motor**, which is not the same question as *"is there anything left to
    /// play"*: a tape wound to its end reads `false` here, and so does one that was never
    /// started, because in both the head is not moving and the `EAR` line is holding still.
    #[must_use]
    pub const fn is_playing(&self) -> bool {
        self.playing
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
    /// # It reports whether the signal moved, and that return value is a hot-path decision
    ///
    /// [`crate::Ula::advance`] runs on every elapsed T-state and has to tell the audio generator
    /// when the `EAR` line flips. It first did that by comparing [`Tape::level`] against a mirror
    /// held inside the generator — which meant evaluating the *timestamp* argument on every call,
    /// a `u64` multiply-add, **before** the guard that discarded it. Measured against
    /// `benches/frame.rs`: **+21.9% on `quiet_48k`**, a machine with no tape in the drive at all.
    ///
    /// The edge was never unknown — `finish_pulse` is the only thing that moves the level and it
    /// runs right here. Returning it costs a `bool` that is already in a register and lets the
    /// caller put the timestamp inside the branch, which is where the work belongs.
    pub(crate) fn advance(&mut self, mut t_states: u32) -> bool {
        let mut flipped = false;
        while self.playing && t_states > 0 {
            if self.remaining > t_states {
                self.remaining -= t_states;
                return flipped;
            }
            t_states -= self.remaining;
            self.finish_pulse();
            flipped = true;
        }
        flipped
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
    const SOURCES: [(&str, &str); 5] = [
        ("tape/mod.rs", include_str!("mod.rs")),
        ("tape/reader.rs", include_str!("reader.rs")),
        ("tape/signal.rs", include_str!("signal.rs")),
        ("tape/tap.rs", include_str!("tap.rs")),
        ("tape/tzx.rs", include_str!("tzx.rs")),
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
        // The sibling half of this gate — the scan for slice **indexing** — is now
        // `there_is_no_indexing_anywhere_in_the_tape_module`, below. It was left open when
        // `tap.rs` was the only parser here, with a note that lifting it was *"the right fix
        // and is left for whoever next owns both modules"*; `.tzx` reads thirty fields across
        // twenty block types out of attacker-controlled bytes, which is what made the cursor
        // worth having and the gate worth writing.
        //
        // It is a second scanner rather than a shared one, because `snapshot/`'s counts over
        // `snapshot/`'s five files and this one counts over `tape/`'s five. A shared helper
        // would make one bug in the scanner turn both gates green at once, which is the
        // failure mode `docs/STATUS.md` records under a gate that verifies nothing.
        // The last two are slice calls rather than panic macros, and they are here because the
        // indexing scanner below cannot see them: `a.split_at(n)` panics when `n > a.len()` and
        // is not an index expression, so `tape/`'s totality claim had a hole exactly the width of
        // the one construct a wrong length in a file would reach. The trailing `(` is what keeps
        // `.split_at(` from also matching the total `.split_at_checked(` that `Reader::take` uses.
        const FORBIDDEN: [&str; 8] = [
            ".unwrap()",
            ".expect(",
            "panic!(",
            "todo!(",
            "unimplemented!(",
            "unreachable!(",
            ".split_at(",
            ".copy_from_slice(",
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

    /// Lines holding an index expression, as `(line number, line)`.
    ///
    /// An index expression is a `[` **immediately** preceded by an identifier character, a `)`
    /// or a `]` — which is what `a[i]`, `f()[i]` and `a[i][j]` look like, and what `[u8; N]`,
    /// `&[u8]`, `from_le_bytes([..])` and `#[derive(..)]` do not. Comments are stripped first,
    /// so a doc link cannot be mistaken for one.
    fn indexing_sites(source: &str) -> Vec<(usize, String)> {
        source
            .lines()
            .enumerate()
            .filter_map(|(number, line)| {
                let code = line.split("//").next().unwrap_or(line);
                let characters: Vec<char> = code.chars().collect();
                let indexed = characters.windows(2).any(|pair| {
                    matches!(pair, [before, '['] if before.is_alphanumeric()
                        || *before == '_'
                        || *before == ')'
                        || *before == ']')
                });
                indexed.then(|| (number + 1, code.trim().to_owned()))
            })
            .collect()
    }

    #[test]
    fn the_indexing_scanner_can_tell_an_index_from_an_array_type() {
        // The gate below is only worth running if this function distinguishes the two, so it
        // has its own failing cases in **both** directions. Without the positive ones it would
        // be a scanner that finds nothing while asserting that nothing is there, which is this
        // project's own recurring failure — a count of zero and an absence of the subject are
        // the same observation.
        for indexing in [
            "self.pulses[index]",
            "let flag = block[0];",
            "value.to_le_bytes()[0]",
            "table[row][column] = 1;",
            "SIGNATURE[0]",
        ] {
            assert_eq!(
                indexing_sites(indexing).len(),
                1,
                "{indexing:?} is an index expression"
            );
        }
        for innocent in [
            "fn f(bytes: &[u8]) -> [u8; 2] {",
            "const SIGNATURE: [u8; 7] = *b\"ZXTape!\";",
            "Ok(u32::from_le_bytes([low, middle, high, 0]))",
            "#[derive(Debug, Clone, Copy)]",
            "blocks: &'a [Block<'a>],",
            "// self.pulses[index] in a comment",
            "/// A doc link to [`Reader::take`] and a pulses[0] mention",
        ] {
            assert_eq!(
                indexing_sites(innocent),
                Vec::<(usize, String)>::new(),
                "{innocent:?} is not an index expression"
            );
        }
    }

    #[test]
    fn there_is_no_indexing_anywhere_in_the_tape_module() {
        // `docs/M6.md` Decision 6, as a property of the source rather than a sentence in a doc
        // comment. Slice indexing is one of the three panic sources a hostile file can reach in
        // safe Rust, and this crate builds with `panic = "abort"` in release — so a `.tzx` with
        // a wrong length would not be a caught error but a dead process.
        //
        // Structural impossibility beats a passing test, so this asserts the structure;
        // `tzx.rs`'s exhaustive truncation sweep and `tests/tzx_hostile.rs` assert the
        // behaviour, and the two are not substitutes for each other.
        for (name, source) in SOURCES {
            assert_eq!(
                indexing_sites(production(source)),
                Vec::<(usize, String)>::new(),
                "{name} indexes a slice; route it through Reader instead"
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
