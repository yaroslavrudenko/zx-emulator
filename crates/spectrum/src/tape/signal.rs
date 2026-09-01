//! The pulse train under construction, and the **one** place its size is bounded.
//!
//! # Why this module exists
//!
//! [`Tape`](super::Tape) is a `Vec<u32>` of half-period lengths — `docs/M6.md` Decision 5 —
//! and every tape format is a converter into it. Two converters exist ([`tap`](super::tap) and
//! [`tzx`](super::tzx)) and they share three pieces of knowledge that must not be written down
//! twice:
//!
//! - **a byte is eight bits, most significant first, two equal half-periods per bit**;
//! - **a data block is a pilot tone, a sync pair, then those bits** — which is why `.tzx`'s
//!   standard-speed block and its turbo block are the *same* code with different numbers, and
//!   why a `.tap` block is too;
//! - **the train has a ceiling**, and reaching it is a returned error rather than an
//!   allocation.
//!
//! # The ceiling, and the two different reasons the converters need one
//!
//! [`Tape`](super::Tape) is materialised, so both converters allocate everything they emit. They
//! are unbounded in different ways, and the difference is worth stating carefully because the
//! paragraph that stood here got the `.tap` half wrong in the direction that made it look safe.
//!
//! **`.tap` is linear in the file's length, at a constant far larger than the obvious one.** The
//! obvious one is 16 half-periods per data byte, and it is not the bound: a block's **pilot tone
//! is a fixed cost that its payload does not pay for**. The cheapest block a `.tap` can hold is
//! a two-byte length word and a lone flag byte, and those *three* bytes buy a header pilot, the
//! sync pair, one byte's sixteen half-periods and the trailing second of silence — 8082
//! half-periods, or **2694 per input byte**. So the amplification is 2694×, not 64×, and the
//! figure this paragraph used to give was wrong twice over: *"a 1 GB `.tap` used to be a 4 GB
//! allocation"* is wrong by 16× on its own premise, since sixteen `u32` half-periods per byte is
//! 64 bytes per byte and therefore 64 GB, and wrong again by another 168× because the premise is
//! the wrong axis. The real figure is **10.8 TB**. The derivation lives once, at
//! `tap::MINIMAL_BLOCK_PULSES`, and is asserted rather than cited.
//!
//! **`.tzx` is not a function of the file's length at all**, which is a difference in kind and
//! not in size: a loop block asks for a body to be played up to 65535 times while costing three
//! bytes. **A loop count multiplied by a block length is an allocation sized from the file**,
//! which is exactly the construct `docs/M6.md` Decision 6 forbids.
//!
//! So every push goes through [`Signal::room`], and exceeding [`MAX_PULSES`] is
//! [`Error::TapeTooLong`] — a value the caller handles, not a `Vec` growing to whatever the file
//! asked for. One ceiling covers both, because at the point of allocation 2694× a gigabyte and
//! an unbounded loop are the same problem.
//!
//! # Where the level comes from, and why it is derived rather than stored
//!
//! [`Tape`](super::Tape) starts low and flips at the end of every half-period, so the level
//! during pulse *i* is *i*'s parity and the level after the whole train is the parity of its
//! **length**. That makes the current level a function of `pulses.len()` — so it is
//! [`Signal::level`], computed, and there is no second representation to drift. A zero-length
//! half-period is an instant edge (`Tape::new` documents it), which is how
//! [`Signal::set_level`] forces a polarity the `.tzx` format asks for without the train needing
//! any concept beyond a length.

use super::Error;

/// The memory one tape's half-periods may occupy.
///
/// **This is the number that was chosen**, and naming it here is the point: 64 MiB is what an
/// emulator can spend on a cassette without anyone noticing, and it is a budget rather than a
/// consequence of anything. Stating the choice in the units the choice is actually about leaves
/// [`MAX_PULSES`] as arithmetic instead of a round number wearing a derivation.
const MAX_TAPE_BYTES: usize = 64 << 20;

/// The largest tape this crate will build, in half-periods.
///
/// **Chosen, not derived** — and the sentence that stood here claimed the opposite. It cited a
/// whole-address-space tape (789,657 half-periods, which
/// `a_whole_address_space_tape_is_far_inside_the_ceiling` still asserts) and called this ceiling
/// *"twenty-one times"* it. Twenty-one is not a factor anyone picked: it is `1 << 24` divided by
/// 789,657 and rounded down, which is the conclusion read backwards into its own premise. An
/// admitted choice is worth more than a derivation that runs the wrong way, so this is now
/// [`MAX_TAPE_BYTES`] over the width of a half-period. **The value is unchanged.**
///
/// What *is* derived is the **headroom**, and on both axes a `.tap` can exhaust — because they
/// are different axes and only one of them is the file's size:
///
/// - the **block** axis, `tap::MAX_PLAYABLE_BLOCKS`, which is the worst case and is reached by a
///   six-kilobyte file;
/// - the **byte** axis, one mebibyte of block payload, which is what a real tape spends its
///   ceiling on — `MAX_PULSES / 16` is exactly `1 << 20`.
///
/// Every tape in `testdata/` is four to six blocks and under 800,000 half-periods, so both have
/// room to spare today. What would change it: a real tape refused. That is the same escape hatch
/// `docs/M6.md` attaches to every other strict ruling in the snapshot parsers.
pub(super) const MAX_PULSES: usize = MAX_TAPE_BYTES / size_of::<u32>();

/// Half-periods one whole byte contributes: two per bit.
///
/// `pub(super)` because `tap`'s worst-case block derivation is built from it, and a second copy
/// of it there would be a second number to get wrong. The `.tap` **tests** keep their own
/// independent restatement on purpose — an expectation computed by the subject is a tautology.
pub(super) const PULSES_PER_BYTE: usize = 2 * u8::BITS as usize;

/// How many bits of a data run's **last** byte are played.
///
/// A newtype rather than a `u8`, so the 1..=8 range the format states is a property of the type
/// and a block carrying a nonsense count cannot reach [`Signal`] at all. That is
/// parse-don't-validate at the only boundary that can enforce it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct UsedBits(u8);

impl UsedBits {
    /// The whole byte, which is what `.tap` always means and what `.tzx` defaults to.
    pub(super) const ALL: Self = Self(u8::BITS as u8);

    /// `bits` if the format allows it, and `None` otherwise.
    ///
    /// The `.tzx` description gives the range as `(1-8)` for a direct recording and `{8}` with
    /// the worked example `xxxxxx00` for the two data blocks. Zero is refused along with
    /// everything above eight: a block that plays none of its last byte is not a thing the
    /// format describes, and guessing which of "no bits" and "all eight" was meant is the kind
    /// of silent choice that produces a wrong train.
    pub(super) const fn new(bits: u8) -> Option<Self> {
        if bits >= 1 && bits <= u8::BITS as u8 {
            Some(Self(bits))
        } else {
            None
        }
    }

    /// How many bits, as the shift arithmetic wants it.
    const fn count(self) -> u32 {
        self.0 as u32
    }
}

/// A run of data bytes and the two half-period lengths its bits are played at.
///
/// Constructed as a struct literal at every call site rather than through a positional
/// constructor, because `zero` and `one` are both `u32` and a positional pair of them is a
/// silent swap waiting to happen — the defect class `docs/M6.md` Decision 7 names as a
/// permutation that a round trip cannot see.
pub(super) struct Data<'a> {
    /// The bytes, in the order they were recorded.
    pub(super) bytes: &'a [u8],
    /// How much of the last one is played.
    pub(super) used_bits: UsedBits,
    /// Half-period of a zero bit, in T-states.
    pub(super) zero: u32,
    /// Half-period of a one bit, in T-states.
    pub(super) one: u32,
}

/// A pilot tone, a sync pair, and a data run: what a `.tap` block is, and what `.tzx`'s
/// standard-speed and turbo-speed blocks both are.
pub(super) struct SpeedData<'a> {
    /// Half-period of the pilot tone, in T-states.
    pub(super) pilot: u32,
    /// How many pilot half-periods precede the sync pair.
    pub(super) pilot_pulses: usize,
    /// First sync half-period, in T-states.
    pub(super) sync_first: u32,
    /// Second sync half-period, in T-states.
    pub(super) sync_second: u32,
    /// The bytes and their bit timings.
    pub(super) data: Data<'a>,
}

/// A run of one-bit samples of the `EAR` line, played at a fixed rate.
///
/// `.tzx`'s direct-recording block. Unlike every other block here it describes **levels**
/// rather than half-periods, so converting it means finding the runs of equal samples — see
/// [`Signal::direct`].
pub(super) struct Samples<'a> {
    /// The samples, one per bit, most significant bit of each byte first.
    pub(super) bytes: &'a [u8],
    /// How much of the last byte is played.
    pub(super) used_bits: UsedBits,
    /// How long one sample lasts, in T-states.
    pub(super) t_states_per_sample: u32,
}

/// The bits of `bytes`, most significant first, playing only `used_bits` of the last byte.
///
/// One function rather than two, because "most significant bit first" is one piece of knowledge
/// and both the data blocks and the direct recording need it. A data block turns each bit into
/// a pair of half-periods; a direct recording turns each bit into a level.
fn bits(bytes: &[u8], used_bits: UsedBits) -> impl Iterator<Item = bool> + '_ {
    let last = bytes.len().saturating_sub(1);
    bytes.iter().enumerate().flat_map(move |(index, &byte)| {
        let count = if index == last {
            used_bits.count()
        } else {
            u8::BITS
        };
        (u8::BITS.saturating_sub(count)..u8::BITS)
            .rev()
            .map(move |shift| byte >> shift & 1 == 1)
    })
}

/// A pulse train being built, and nothing else.
///
/// One field on purpose: this type's whole job is to own the `Vec` and to be the only thing
/// that grows it, which is what makes [`MAX_PULSES`] enforceable by reading one file.
pub(super) struct Signal {
    /// Half-period lengths in T-states, in playback order.
    pulses: Vec<u32>,
}

impl Signal {
    /// An empty train: no pulses, and the line low.
    pub(super) const fn new() -> Self {
        Self { pulses: Vec::new() }
    }

    /// The train, finished.
    pub(super) fn into_pulses(self) -> Vec<u32> {
        self.pulses
    }

    /// The level the line is being driven to now.
    ///
    /// Derived from the train's length rather than stored, because
    /// [`Tape`](super::Tape) starts low and flips at every half-period boundary — so the parity
    /// of the length **is** the level, and a stored copy could only ever disagree with it.
    pub(super) const fn level(&self) -> bool {
        self.pulses.len() % 2 == 1
    }

    /// Refuse `count` more half-periods if they would breach [`MAX_PULSES`].
    ///
    /// Every growth in this module goes through here, so the ceiling is one comparison in one
    /// place. `checked_add` rather than a subtraction, so a hostile `count` near `usize::MAX`
    /// is refused rather than wrapping — `overflow-checks = true` makes a wrap an abort, and
    /// this crate aborts on panic in release.
    fn room(&self, count: usize, at: usize) -> Result<(), Error> {
        match self.pulses.len().checked_add(count) {
            Some(total) if total <= MAX_PULSES => Ok(()),
            _ => Err(Error::TapeTooLong {
                offset: at,
                limit: MAX_PULSES,
            }),
        }
    }

    /// Append one half-period of `length` T-states.
    pub(super) fn pulse(&mut self, length: u32, at: usize) -> Result<(), Error> {
        self.room(1, at)?;
        self.pulses.push(length);
        Ok(())
    }

    /// Append `count` half-periods of `length` T-states.
    ///
    /// The `reserve` inside `extend` is sized from `count`, which came from the file — and that
    /// is safe **only** because [`Signal::room`] has already bounded it by a constant. That is
    /// the difference `docs/M6.md` Decision 6 draws between an allocation sized from the file
    /// and one bounded before it happens.
    pub(super) fn tone(&mut self, length: u32, count: usize, at: usize) -> Result<(), Error> {
        self.room(count, at)?;
        self.pulses.extend(std::iter::repeat_n(length, count));
        Ok(())
    }

    /// Drive the line to `want`, with an instant edge if it is not there already.
    ///
    /// A zero-length half-period flips the level and consumes no time, which
    /// [`Tape::new`](super::Tape::new) documents as legal and
    /// `a_zero_length_half_period_is_an_instant_edge_and_cannot_spin` asserts. It is how the
    /// `.tzx` format's *"force low level"* and its *"set signal level"* block are expressed in
    /// a train that knows only about lengths.
    pub(super) fn set_level(&mut self, want: bool, at: usize) -> Result<(), Error> {
        if self.level() == want {
            return Ok(());
        }
        self.pulse(0, at)
    }

    /// Append a data run: two equal half-periods per bit, most significant bit first.
    pub(super) fn data(&mut self, data: &Data<'_>, at: usize) -> Result<(), Error> {
        let room = data
            .bytes
            .len()
            .checked_mul(PULSES_PER_BYTE)
            .ok_or(Error::TapeTooLong {
                offset: at,
                limit: MAX_PULSES,
            })?;
        self.room(room, at)?;

        for bit in bits(data.bytes, data.used_bits) {
            let length = if bit { data.one } else { data.zero };
            self.pulses.push(length);
            self.pulses.push(length);
        }
        Ok(())
    }

    /// Append a pilot tone, a sync pair, and a data run.
    pub(super) fn speed_data(&mut self, block: &SpeedData<'_>, at: usize) -> Result<(), Error> {
        self.tone(block.pilot, block.pilot_pulses, at)?;
        self.pulse(block.sync_first, at)?;
        self.pulse(block.sync_second, at)?;
        self.data(&block.data, at)
    }

    /// Append a direct recording: one half-period per **run** of equal samples.
    ///
    /// The block describes levels and the train describes edges, so the conversion is a
    /// run-length encoding. The first run also fixes the polarity — a recording that starts
    /// high must actually start high, which is the whole reason this block exists rather than
    /// being expressible as a pure data block.
    pub(super) fn direct(&mut self, samples: &Samples<'_>, at: usize) -> Result<(), Error> {
        let mut levels = bits(samples.bytes, samples.used_bits);
        let Some(first) = levels.next() else {
            return Ok(());
        };
        self.set_level(first, at)?;

        let mut level = first;
        let mut run: u64 = 1;
        for sample in levels {
            if sample == level {
                run += 1;
                continue;
            }
            self.sample_run(run, samples.t_states_per_sample, at)?;
            level = sample;
            run = 1;
        }
        self.sample_run(run, samples.t_states_per_sample, at)?;

        // **The last sample is the level the line is left at**, and this block is the one place
        // in the format where that is true: *"The 'current pulse level' after playing a Direct
        // Recording block or CSW recording block is the last level played"*, where after every
        // other signal block it is *"the opposite of the last pulse level played, so that a
        // subsequent pulse will produce an edge"*.
        //
        // A train ends every half-period by flipping, so without this the line would be left at
        // the **opposite** of the recording's final sample — a spurious edge the recording never
        // contained. The zero-length half-period cancels that flip in zero time, so no sampler
        // can observe the intermediate level and the next block starts where the recording ended.
        self.set_level(level, at)
    }

    /// One half-period covering `samples` samples of `t_states_per_sample` each.
    ///
    /// Accumulated in `u64` and narrowed here. A 24-bit block length permits 134 million
    /// samples and the rate is a `u16`, so the product can exceed `u32::MAX` — which
    /// `overflow-checks = true` turns into an abort, in a crate that aborts on panic. So the
    /// narrowing is the check, and it is [`Error::PulseTooLong`] rather than a saturation: a
    /// half-period of twenty minutes is a file we have misread, not a tape.
    fn sample_run(
        &mut self,
        samples: u64,
        t_states_per_sample: u32,
        at: usize,
    ) -> Result<(), Error> {
        let length = samples.saturating_mul(u64::from(t_states_per_sample));
        let length = u32::try_from(length).map_err(|_| Error::PulseTooLong { offset: at })?;
        self.pulse(length, at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A data run whose two bit lengths are distinct and neither is zero, so a swap between
    /// them is visible and a dropped one does not look like a correct default —
    /// `docs/M6.md` Decision 7's rule about fixtures, applied to this module's own.
    const ZERO: u32 = 100;
    const ONE: u32 = 300;

    fn data<'a>(bytes: &'a [u8], used_bits: UsedBits) -> Data<'a> {
        Data {
            bytes,
            used_bits,
            zero: ZERO,
            one: ONE,
        }
    }

    #[test]
    fn used_bits_admits_one_through_eight_and_nothing_else() {
        assert_eq!(UsedBits::new(0), None, "a byte with no bits played");
        assert_eq!(UsedBits::new(9), None);
        assert_eq!(UsedBits::new(255), None);
        for bits in 1..=8 {
            assert!(
                UsedBits::new(bits).is_some(),
                "{bits} is in the format's range"
            );
        }
        assert_eq!(UsedBits::new(8), Some(UsedBits::ALL));
    }

    #[test]
    fn bits_come_out_most_significant_first() {
        // Written out by hand rather than produced by a shift: 0x2A is 0b0010_1010, and it is
        // not its own bit-reversal, so a reader that ran the other way would disagree.
        let observed: Vec<bool> = bits(&[0x2A], UsedBits::ALL).collect();
        assert_eq!(
            observed,
            [false, false, true, false, true, false, true, false]
        );
    }

    #[test]
    fn a_partial_last_byte_plays_its_top_bits() {
        // The format's own worked example: "if this is 6, then the bits used (x) in the last
        // byte are: xxxxxx00, where MSb is the leftmost bit".
        let used = UsedBits::new(6).expect("six is in range");
        let observed: Vec<bool> = bits(&[0b1111_1111], used).collect();
        assert_eq!(observed.len(), 6);

        let observed: Vec<bool> = bits(&[0b1010_1111], used).collect();
        assert_eq!(observed, [true, false, true, false, true, true]);

        // ...and only the *last* byte is partial.
        let observed: Vec<bool> = bits(&[0xFF, 0x00], used).collect();
        assert_eq!(observed.len(), 8 + 6);
    }

    #[test]
    fn the_level_is_the_parity_of_the_train() {
        // The invariant the whole module rests on, asserted against `Tape` itself rather than
        // against this type's own arithmetic: a train of N pulses leaves the line where a
        // `Tape` holding the same N pulses leaves it.
        let mut signal = Signal::new();
        for expected in [false, true, false, true] {
            assert_eq!(
                signal.level(),
                expected,
                "at {} pulses",
                signal.pulses.len()
            );

            let mut tape = super::super::Tape::new(signal.pulses.clone());
            tape.play();
            tape.advance(u32::MAX);
            assert_eq!(tape.level(), expected, "and `Tape` agrees");

            signal.pulse(1, 0).expect("far inside the ceiling");
        }
    }

    #[test]
    fn setting_a_level_that_already_holds_emits_nothing() {
        let mut signal = Signal::new();
        signal.set_level(false, 0).expect("already low");
        assert_eq!(
            signal.pulses.len(),
            0,
            "no edge is needed, so none is emitted"
        );

        signal.set_level(true, 0).expect("room");
        assert_eq!(
            signal.into_pulses(),
            vec![0],
            "an instant edge, and nothing more"
        );
    }

    #[test]
    fn a_data_run_is_two_equal_half_periods_per_bit() {
        let mut signal = Signal::new();
        signal.data(&data(&[0x80], UsedBits::ALL), 0).expect("room");
        let mut expected = vec![ONE, ONE];
        expected.extend(std::iter::repeat_n(ZERO, 14));
        assert_eq!(signal.into_pulses(), expected);
    }

    /// The train a one-byte direct recording produces at `RATE` T-states per sample.
    fn recording(byte: u8) -> Vec<u32> {
        const RATE: u32 = 79;
        let mut signal = Signal::new();
        signal
            .direct(
                &Samples {
                    bytes: &[byte],
                    used_bits: UsedBits::ALL,
                    t_states_per_sample: RATE,
                },
                0,
            )
            .expect("room");
        signal.into_pulses()
    }

    #[test]
    fn a_direct_recording_is_one_half_period_per_run() {
        // 0b1100_0101: runs of 2 high, 3 low, 1 high, 1 low, 1 high — five runs. The first is
        // high, so the train opens with an instant edge to get there; the last is high too, so
        // it closes with one to stay there.
        const RATE: u32 = 79;
        assert_eq!(
            recording(0b1100_0101),
            vec![0, 2 * RATE, 3 * RATE, RATE, RATE, RATE, 0]
        );
    }

    #[test]
    fn a_direct_recording_that_starts_low_needs_no_opening_edge() {
        // The mirror of the case above, and the reason `set_level` is conditional: a recording
        // that already starts at the line's resting level must not gain a spurious edge. Its
        // final sample is high, so it still closes with one.
        const RATE: u32 = 79;
        assert_eq!(recording(0b0011_1111), vec![2 * RATE, 6 * RATE, 0]);
    }

    #[test]
    fn a_direct_recording_leaves_the_line_at_its_last_sample() {
        // **The rule this block does not share with any other**: *"The 'current pulse level'
        // after playing a Direct Recording block or CSW recording block is the last level
        // played"*, where after every other signal block it is *"the opposite of the last pulse
        // level played"*. The two sentences are one paragraph apart in the format description
        // and they disagree on purpose.
        //
        // This test exists because the rule was **designed, documented as honoured, and not
        // implemented** — and the two tests above passed anyway, because their expected trains
        // were worked out from the code's run-length logic rather than from the sentence above.
        // A derived expectation gets the wrong answer right in both places at once.
        for (byte, last_sample) in [
            (0b0000_0000_u8, false),
            (0b1111_1111, true),
            (0b0101_0101, true),
        ] {
            let train = recording(byte);
            let level = train.len() % 2 == 1;
            assert_eq!(
                level, last_sample,
                "{byte:#010b} ends on {last_sample}, so the line must be left there"
            );
        }
    }

    #[test]
    fn the_ceiling_is_a_returned_error_and_not_an_allocation() {
        // The property the module exists for. A count near `usize::MAX` must be refused by a
        // comparison rather than attempted — and the refusal must leave the train untouched.
        let mut signal = Signal::new();
        assert_eq!(
            signal.tone(1, usize::MAX, 7),
            Err(Error::TapeTooLong {
                offset: 7,
                limit: MAX_PULSES
            })
        );
        assert_eq!(
            signal.tone(1, MAX_PULSES + 1, 7),
            Err(Error::TapeTooLong {
                offset: 7,
                limit: MAX_PULSES
            })
        );
        assert_eq!(signal.pulses.len(), 0, "a refused tone emits nothing");

        // ...and the boundary itself is legal, so the ceiling is off-by-one free.
        signal.tone(1, MAX_PULSES, 0).expect("exactly the ceiling");
        assert_eq!(signal.pulses.len(), MAX_PULSES);
        assert!(signal.pulse(1, 0).is_err(), "and one more is not");
    }

    #[test]
    fn the_ceiling_is_the_memory_budget_divided_by_a_half_period() {
        // The ceiling used to be written `1 << 24` under a sentence calling it derived. It is a
        // memory budget, so it is now spelled as one — and this asserts the arithmetic **and**
        // that spelling it that way moved nothing. Written against literals rather than against
        // `MAX_TAPE_BYTES`, so a change to the budget disagrees with a number rather than with
        // itself.
        assert_eq!(
            MAX_TAPE_BYTES,
            64 * 1024 * 1024,
            "64 MiB, and it is a choice"
        );
        assert_eq!(MAX_PULSES, 16_777_216);
        assert_eq!(MAX_PULSES, 1 << 24, "unchanged by being spelled honestly");
        assert_eq!(MAX_PULSES * size_of::<u32>(), MAX_TAPE_BYTES);
    }

    #[test]
    fn a_whole_address_space_tape_is_far_inside_the_ceiling() {
        // This grades the ceiling's **headroom**; it does not derive the ceiling, and the
        // comment that stood here said it did. A 48 KB tape recorded as one data block: a data
        // pilot, the sync pair, and sixteen half-periods per byte.
        const DATA_PILOT: usize = 3223;
        const ADDRESS_SPACE: usize = 49152;
        let whole_machine = DATA_PILOT + 2 + ADDRESS_SPACE * PULSES_PER_BYTE;
        assert_eq!(whole_machine, 789_657);
        assert!(
            whole_machine * 21 < MAX_PULSES,
            "the ceiling must leave room for a tape far larger than the machine"
        );
    }

    #[test]
    fn the_byte_axis_of_the_ceiling_is_exactly_one_mebibyte_of_payload() {
        // The axis a *real* tape spends its ceiling on, as opposed to the worst case — which is
        // the block axis and lives in `tap`. Both are named because `Error::TapeTooLong` reports
        // half-periods, and a half-period count is not a quantity anyone holding a tape has.
        assert_eq!(MAX_PULSES / PULSES_PER_BYTE, 1 << 20);
        assert_eq!(
            MAX_PULSES / PULSES_PER_BYTE,
            1_048_576,
            "one mebibyte of blocks"
        );
    }

    #[test]
    fn a_sample_run_longer_than_a_u32_is_refused_rather_than_aborting() {
        // `overflow-checks = true` makes the product an abort rather than a wrap, and this
        // crate aborts on panic in release. So the narrowing is the guard.
        let mut signal = Signal::new();
        assert_eq!(
            signal.sample_run(u64::from(u32::MAX), 2, 9),
            Err(Error::PulseTooLong { offset: 9 })
        );
        signal
            .sample_run(u64::from(u32::MAX), 1, 9)
            .expect("exactly a u32 still fits");
    }
}
