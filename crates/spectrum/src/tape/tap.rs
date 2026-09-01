//! `.tap` — block data with the ROM's standard timings implied, converted into a pulse train.
//!
//! # The format
//!
//! A flat sequence of blocks, each a two-byte little-endian length followed by that many
//! bytes. The block's first byte is the **flag** — under 128 for a header, 128 or above for
//! data — and its last is a parity byte the ROM checks. Nothing in the file names a timing:
//! every block is understood to have been recorded by the ROM's own `SA-BYTES`, so this
//! module supplies the timings that routine emits.
//!
//! That implication is the format's whole limitation and the reason
//! [`Tape`] is a pulse train rather than a block list: a turbo loader's tape
//! cannot be written down as a `.tap` at any speed, and [`tzx`](super::tzx) — which can — **is**
//! a second converter beside this one rather than a rewrite of the ULA side. That was a
//! prediction when this module was written and it is now a fact: `.tzx` landed without `Tape`
//! gaining a field, and the two converters share their emitter through
//! [`signal`](super::signal) rather than each carrying their own copy of what a data block is.
//!
//! # The parity byte is not checked, deliberately
//!
//! It is the loader's business. A `.tap` whose checksum is wrong is a perfectly well-formed
//! tape that fails to load, exactly as a damaged cassette is, and refusing to play it here
//! would move a decision out of the machine and into the file reader. Parse-don't-validate
//! applies to the file's **structure**, which is what this module is total over.
//!
//! # Every timing here is transcribed from the ROM, and then measured against it
//!
//! The five constants below are not remembered: each is derived by counting T-states through
//! `SA-BYTES`, the 48K ROM's own tape writer, whose instruction stream is quoted at each
//! constant. `crates/spectrum/tests/tape_rom_timings.rs` then **runs that routine on the real
//! machine and measures what it emits**, which is an independent implementation of the same
//! timings — one this project did not write — grading the numbers below rather than grading
//! them against themselves.
//!
//! They are `pub(super)` because [`tzx`](super::tzx)'s standard-speed block is defined by
//! reference to exactly them — *"This block must be replayed with the standard Spectrum ROM
//! timing values"* — and a second copy of a number carrying a derivation this careful is a
//! second number to get wrong. The derivations stay here, where the ROM listing that produced
//! them is quoted.

use super::signal::{Data, MAX_PULSES, PULSES_PER_BYTE, Signal, SpeedData, UsedBits};
use super::{Error, Tape};
use crate::timing::T_STATES_PER_FRAME;

/// Bytes of the little-endian length word each block begins with.
const BLOCK_LENGTH_BYTES: usize = 2;

/// Half-period of the pilot tone, in T-states.
///
/// Counted through `SA-BYTES`' pilot loop, whose steady state is one `OUT` per iteration:
///
/// ```text
///   04D8  SA-LEAP  DJNZ SA-LEAP      163 x 13 + 8 = 2127   (B = 0xA4 = 164)
///   04DA           OUT  ($FE),A                      11
///   04DC           XOR  $0F                           7
///   04DE           LD   B,$A4                         7
///   04E0           DEC  L                             4
///   04E1           JR   NZ,SA-LEAP                   12
/// ```
///
/// `OUT` to `OUT` is `11 + 7 + 7 + 4 + 12 + 2127` = **2168**.
pub(super) const PILOT_PULSE: u32 = 2168;

/// Pilot half-periods before a **header** block.
///
/// `SA-BYTES` loads `HL = 0x1F80` for a flag under 128. `L` counts down per pulse and `H` per
/// wrap, and the loop ends when `H` goes negative — 128 pulses for the first pass and 256 for
/// each of the 31 that follow, so **8064 edges** and therefore 8063 complete half-periods
/// between them. The first edge arrives after a two-iteration `DJNZ` rather than a full pilot
/// period, which is why the pulse count is one below the edge count.
pub(super) const HEADER_PILOT_PULSES: usize = 8063;

/// Pilot half-periods before a **data** block.
///
/// The same loop with `HL = 0x0C98`: 152 pulses then 12 passes of 256, so 3224 edges and
/// **3223** half-periods.
pub(super) const DATA_PILOT_PULSES: usize = 3223;

/// The flag-byte bit `SA-BYTES` tests to choose between the two pilot lengths.
///
/// ```text
///   04C6  LD   HL,$1F80     ; the header's 8064 edges
///   04C9  BIT  7,A
///   04CB  JR   Z,SA-FLAG
///   04CD  LD   HL,$0C98     ; the data block's 3224
/// ```
const DATA_BLOCK_FLAG_BIT: u8 = 0x80;

/// First sync half-period, in T-states.
///
/// The pilot loop's exit path, from its last `OUT` to the sync `OUT` at `0x04EC`:
/// `11 + 7 + 7 + 4 + 7 + 4 + 4 + 10 + 7 + 606` = **667**, where 606 is `DJNZ` with `B = 0x2F`
/// and the three 4s are the `DEC L` / `DEC B` / `DEC H` the exit passes through.
pub(super) const SYNC_FIRST: u32 = 667;

/// Second sync half-period, in T-states.
///
/// ```text
///   04EC  OUT  ($FE),A                     11
///   04EE  LD   A,$0D                        7
///   04F0  LD   B,$37                        7
///   04F2  DJNZ                54 x 13 + 8 = 710
/// ```
///
/// `11 + 7 + 7 + 710` = **735**.
pub(super) const SYNC_SECOND: u32 = 735;

/// Half-period of a zero bit, in T-states.
///
/// `SA-BYTES`' bit loop with the carry clear, so the `JR NC` to `SA-OUT` is taken:
///
/// ```text
///   051C  SA-OUT   OUT  ($FE),A                     11
///   051E           LD   B,$3E                        7
///   0520           JR   NZ,SA-BIT-2                 12
///   0511  SA-BIT-2 LD   A,C                          4
///   0512           BIT  7,B                          8
///   0514  SA-BIT-1 DJNZ SA-BIT-1      61 x 13 + 8 = 801   (B = 0x3E = 62)
///   0516           JR   NC,SA-OUT                   12
/// ```
///
/// `11 + 7 + 12 + 4 + 8 + 801 + 12` = **855**.
pub(super) const BIT_ZERO: u32 = 855;

/// Half-period of a one bit, in T-states.
///
/// The same path with the carry set, so the `JR NC` falls through into a second delay:
/// `12` becomes `7 + 7 + 853` — `LD B,$42` and `DJNZ` at `B = 66` — which is `855 + 855`,
/// exactly **double** the zero bit. That the ratio is two rather than something near it is
/// what lets the ROM's loader discriminate bits with a single threshold.
pub(super) const BIT_ONE: u32 = BIT_ZERO * 2;

const _: () = assert!(BIT_ONE == 1710);

/// Frames of silence after each block: one second at the 48K's 50 Hz frame rate.
///
/// Written as a count of frames rather than as 3.5 million so that it is derived from
/// [`T_STATES_PER_FRAME`] — the machine's own constant — rather than from the clock speed
/// restated here.
///
/// > **It is the 48K's second, and it does not track the model.** The sentence that stood here
/// > said *"a 128's longer frame makes its second longer too, which is correct"*. That was a rule
/// > designed, documented as honoured, and not implemented — the same shape as the `ID 15` level
/// > defect `docs/M6.md` records, in the module that records it. It cannot be true as written:
/// > `T_STATES_PER_FRAME` is `Timing::SPECTRUM_48K`'s projection, pinned by a compile-time
/// > assertion in `crate::timing`, and [`parse`] takes no `Model` with which to select another.
/// >
/// > Withdrawn rather than left standing, and pinned by
/// > `the_gap_is_the_48ks_second_and_does_not_vary_with_the_model`, so that making it true later
/// > turns a test red instead of leaving this comment wrong a second time. Nothing rests on it:
/// > the gap exists to give the loader time between blocks rather than to be a physical second,
/// > a 128's frame is 1.46% longer, and `.tzx`'s pauses are pinned to the same 48K clock through
/// > its own `T_STATES_PER_MILLISECOND`. Honouring it would mean giving [`parse`] a `Model`
/// > parameter, which is a public-API change this work has no mandate for.
const PAUSE_FRAMES: u32 = 50;

/// The silence after a block, as one long half-period.
const PAUSE_T_STATES: u32 = T_STATES_PER_FRAME * PAUSE_FRAMES;

/// Bytes the cheapest block a `.tap` can hold occupies in the file: a length word and a flag.
const MINIMAL_BLOCK_BYTES: usize = BLOCK_LENGTH_BYTES + 1;

/// Half-periods that cheapest block contributes — **the axis the tape ceiling binds on**.
///
/// It is not the byte count, and the note on [`parse`] used to say it was. A block's pilot tone
/// is a fixed cost that its payload does not pay for, so the worst case is not a big file but the
/// *shortest legal block repeated*: three bytes buying a header pilot, the sync pair, one byte's
/// sixteen half-periods and the trailing second of silence.
///
/// That is **2694 half-periods per input byte**, where the figure everyone reaches for — and the
/// one that note reached for — is sixteen. Both figures are asserted by
/// `the_ceiling_is_reached_by_block_count_rather_than_by_file_size` rather than cited here.
/// The terms are the pilot, the sync **pair**, the byte, and the **one** trailing silence, in
/// that order — spelled as literals the way this module's sibling assertions already spell them.
pub(super) const MINIMAL_BLOCK_PULSES: usize = HEADER_PILOT_PULSES + 2 + PULSES_PER_BYTE + 1;

/// How many blocks a `.tap` may hold before [`Error::TapeTooLong`], worst case.
///
/// Derived — the ceiling divided by the cheapest block — rather than measured. It is named
/// because [`Error::TapeTooLong`] reports a count of *half-periods*, which is not a quantity
/// anybody holding a `.tap` has; without this constant a user refused on a six-kilobyte file
/// would have no way to discover that what he ran out of was **blocks**.
pub(super) const MAX_PLAYABLE_BLOCKS: usize = MAX_PULSES / MINIMAL_BLOCK_PULSES;

// The derivation, pinned where it cannot be skipped: a figure that stopped being this figure
// fails the **build** rather than a test, which is the treatment `BIT_ONE` above already gets.
// `2694` is the one that matters — it is the number the note on `parse` used to give as 16.
const _: () = assert!(MINIMAL_BLOCK_BYTES == 3);
const _: () = assert!(MINIMAL_BLOCK_PULSES == 8082);
const _: () = assert!(MINIMAL_BLOCK_PULSES / MINIMAL_BLOCK_BYTES == 2694);
const _: () = assert!(MAX_PLAYABLE_BLOCKS == 2075);

/// Read a `.tap` file into the signal it describes.
///
/// # Errors
///
/// [`Error`], naming the offset that failed. Every failure is a returned value: this function
/// does not panic on any input, which is a property of its construction rather than of the
/// inputs it has seen — there is no indexing expression and no slice operation here that is
/// not total, and the train is bounded by [`MAX_PULSES`](super::signal::MAX_PULSES) rather
/// than by whatever the file asked for.
///
/// > **[`Error::TapeTooLong`] became reachable here when `.tzx` landed**, and it is a fix rather
/// > than a new restriction. What it fixes is not what this note used to say. The old wording —
/// > *"a `.tap` byte is sixteen half-periods, so a 1 GB file used to be a 4 GB allocation"* — was
/// > wrong by 16× on its own arithmetic (sixteen `u32`s per byte is 64 bytes per byte, so 64 GB)
/// > and wrong again about its **axis**: what binds is the block count, not the byte count,
/// > because three bytes of file buy a whole pilot tone. See [`MINIMAL_BLOCK_PULSES`] for that
/// > derivation and [`MAX_PLAYABLE_BLOCKS`] for the limit it implies — a limit no tape in
/// > `testdata/` comes within two orders of magnitude of, since every one of them is four to six
/// > blocks. Both are asserted by
/// > `the_ceiling_is_reached_by_block_count_rather_than_by_file_size`, which is also the failing
/// > case this variant went without.
pub fn parse(bytes: &[u8]) -> Result<Tape, Error> {
    let mut signal = Signal::new();
    let mut rest = bytes;
    let mut offset = 0;

    while !rest.is_empty() {
        let (length_bytes, tail) =
            rest.split_first_chunk::<BLOCK_LENGTH_BYTES>()
                .ok_or(Error::Truncated {
                    offset,
                    needed: BLOCK_LENGTH_BYTES,
                    available: rest.len(),
                })?;
        let length = usize::from(u16::from_le_bytes(*length_bytes));

        let (block, tail) = tail.split_at_checked(length).ok_or(Error::Truncated {
            offset: offset + BLOCK_LENGTH_BYTES,
            needed: length,
            available: tail.len(),
        })?;
        let (&flag, _) = block.split_first().ok_or(Error::EmptyBlock { offset })?;

        append_block(&mut signal, flag, block, offset)?;
        offset += BLOCK_LENGTH_BYTES + length;
        rest = tail;
    }

    Ok(Tape::new(signal.into_pulses()))
}

/// Append one block's signal: pilot tone, sync pair, the bytes, and the trailing silence.
///
/// The pilot-sync-data shape and the two-half-periods-per-bit rule both live in
/// [`Signal`](super::signal::Signal), because `.tzx`'s standard-speed and turbo-speed blocks
/// are the same shape with different numbers — one representation of one piece of knowledge,
/// rather than three that can drift apart.
fn append_block(signal: &mut Signal, flag: u8, block: &[u8], at: usize) -> Result<(), Error> {
    signal.speed_data(
        &SpeedData {
            pilot: PILOT_PULSE,
            pilot_pulses: pilot_pulses(flag),
            sync_first: SYNC_FIRST,
            sync_second: SYNC_SECOND,
            data: Data {
                bytes: block,
                used_bits: UsedBits::ALL,
                zero: BIT_ZERO,
                one: BIT_ONE,
            },
        },
        at,
    )?;
    // After every block rather than between blocks, so there is no last-one special case and
    // so the final data edge is followed by a defined stretch of silence rather than by the
    // end of the tape.
    signal.pulse(PAUSE_T_STATES, at)
}

/// How long a pilot tone `flag` calls for.
pub(super) const fn pilot_pulses(flag: u8) -> usize {
    if flag & DATA_BLOCK_FLAG_BIT == 0 {
        HEADER_PILOT_PULSES
    } else {
        DATA_PILOT_PULSES
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-byte data block: flag `0xFF`, the byte, and its parity.
    fn one_byte_block(byte: u8) -> Vec<u8> {
        let mut file = vec![0x03, 0x00, 0xFF, byte];
        file.push(0xFF ^ byte);
        file
    }

    #[test]
    fn the_flag_bit_chooses_the_pilot_length() {
        // "In 48K mode": `BIT 7,A` selects 0x1F80 for a header and 0x0C98 for data, so every
        // flag under 128 is a header. Asserted at the boundary rather than at 0 and 255.
        assert_eq!(pilot_pulses(0x00), HEADER_PILOT_PULSES);
        assert_eq!(pilot_pulses(0x7F), HEADER_PILOT_PULSES);
        assert_eq!(pilot_pulses(0x80), DATA_PILOT_PULSES);
        assert_eq!(pilot_pulses(0xFF), DATA_PILOT_PULSES);
    }

    /// Half-periods one byte contributes: two per bit.
    const PULSES_PER_BYTE: usize = 2 * u8::BITS as usize;

    #[test]
    fn a_byte_is_eight_bits_most_significant_first() {
        // Written out by hand through the whole converter rather than through a bit helper:
        // `0x80` is one long pair then seven short ones, and `0x01` is its mirror image, which
        // is what separates "most significant first" from "least significant first" rather
        // than merely asserting a length.
        let payload = |byte: u8| -> Vec<u32> {
            let mut signal = Signal::new();
            append_block(&mut signal, 0xFF, &[0xFF, byte], 0).expect("a one-byte block");
            signal
                .into_pulses()
                .into_iter()
                .skip(DATA_PILOT_PULSES + 2 + PULSES_PER_BYTE)
                .take(PULSES_PER_BYTE)
                .collect()
        };

        let mut expected = vec![BIT_ONE, BIT_ONE];
        expected.extend(std::iter::repeat_n(BIT_ZERO, 14));
        assert_eq!(payload(0x80), expected);

        let mut expected = vec![BIT_ZERO; 14];
        expected.extend([BIT_ONE, BIT_ONE]);
        assert_eq!(payload(0x01), expected);
    }

    #[test]
    fn a_block_is_pilot_then_sync_then_bytes_then_silence() {
        let tape = parse(&one_byte_block(0x00)).expect("a well-formed block");
        let pulses = tape.pulses();
        assert_eq!(
            pulses.len(),
            DATA_PILOT_PULSES + 2 + 3 * PULSES_PER_BYTE + 1
        );
        assert!(
            pulses
                .iter()
                .take(DATA_PILOT_PULSES)
                .all(|&p| p == PILOT_PULSE)
        );
        assert_eq!(
            pulses.get(DATA_PILOT_PULSES..DATA_PILOT_PULSES + 2),
            Some(&[SYNC_FIRST, SYNC_SECOND][..])
        );
        assert_eq!(pulses.last(), Some(&PAUSE_T_STATES));
    }

    #[test]
    fn two_blocks_are_two_pilots_and_two_pauses() {
        let mut file = one_byte_block(0x00);
        file.extend(one_byte_block(0xFF));
        let tape = parse(&file).expect("two well-formed blocks");
        let pauses = tape
            .pulses()
            .iter()
            .filter(|&&p| p == PAUSE_T_STATES)
            .count();
        assert_eq!(pauses, 2, "one gap per block, including the last");
        let pilots = tape.pulses().iter().filter(|&&p| p == PILOT_PULSE).count();
        assert_eq!(pilots, 2 * DATA_PILOT_PULSES);
    }

    #[test]
    fn an_empty_file_is_a_blank_tape_rather_than_an_error() {
        // A cassette with nothing recorded on it is a real thing, and it is the state the
        // machine is in with no tape at all.
        let tape = parse(&[]).expect("an empty file is a blank tape");
        assert_eq!(tape.pulses(), &[] as &[u32]);
    }

    #[test]
    fn a_truncated_block_is_refused_at_every_length() {
        // Exhaustive over the axis that matters, the way the snapshot parsers are: for a
        // valid file of length N, every prefix shorter than N must be an error and none of
        // them may panic.
        let file = one_byte_block(0x5A);
        for k in 1..file.len() {
            let prefix = file.get(..k).expect("k < len");
            assert!(
                parse(prefix).is_err(),
                "a {k}-byte prefix of a {}-byte file must not parse",
                file.len()
            );
        }
        assert!(parse(&file).is_ok(), "and the whole file must");
    }

    #[test]
    fn a_zero_length_block_is_refused_rather_than_skipped() {
        // The flag byte is what chooses the pilot tone, so a block without one cannot be
        // converted into a signal. Refusing beats silently dropping a block.
        assert_eq!(parse(&[0x00, 0x00]), Err(Error::EmptyBlock { offset: 0 }));
        let mut file = one_byte_block(0x11);
        file.extend([0x00, 0x00]);
        assert_eq!(parse(&file), Err(Error::EmptyBlock { offset: 5 }));
    }

    #[test]
    fn a_block_length_longer_than_the_file_names_the_offset_it_failed_at() {
        assert_eq!(
            parse(&[0x10, 0x00, 0xFF]),
            Err(Error::Truncated {
                offset: 2,
                needed: 16,
                available: 1
            })
        );
    }

    #[test]
    fn the_parity_byte_is_not_checked_here() {
        // A damaged cassette is a well-formed tape that fails to load, and deciding that is
        // the loader's job. This is a ruling, so it gets a failing case rather than a comment.
        let mut file = one_byte_block(0x5A);
        let last = file.len() - 1;
        file[last] ^= 0xFF;
        assert!(parse(&file).is_ok(), "a wrong parity byte still plays");
    }

    #[test]
    fn the_pause_is_derived_from_the_frame_rather_than_from_a_clock_speed() {
        assert_eq!(PAUSE_T_STATES, 69_888 * 50);
        assert_eq!(PAUSE_T_STATES, 3_494_400);
    }

    #[test]
    fn the_gap_is_the_48ks_second_and_does_not_vary_with_the_model() {
        // The claim withdrawn from `PAUSE_FRAMES`, as a failing case rather than as a corrected
        // comment — because a corrected comment is exactly what the wrong one was.
        //
        // Asserted against `Timing`'s **two** frame lengths rather than against
        // `T_STATES_PER_FRAME`, so it grades *which machine the constant came from* instead of
        // agreeing with whatever it holds. If the gap is ever made model-dependent, the second
        // assertion is what says the comment above has to move with it.
        use crate::timing::Timing;
        assert_eq!(
            PAUSE_T_STATES,
            Timing::SPECTRUM_48K.frame_t_states() * PAUSE_FRAMES
        );
        assert_ne!(
            PAUSE_T_STATES,
            Timing::SPECTRUM_128.frame_t_states() * PAUSE_FRAMES,
            "a 128's second is a different number, and `tap::parse` cannot ask for it"
        );
    }

    /// A file of `count` blocks, each the cheapest one a `.tap` can hold.
    ///
    /// Flag `0x00` is under 128, so every block draws the **header** pilot — the expensive branch
    /// of `pilot_pulses`, and therefore the one the worst case runs through.
    fn minimal_blocks(count: usize) -> Vec<u8> {
        std::iter::repeat_n([0x01, 0x00, 0x00], count)
            .flatten()
            .collect()
    }

    #[test]
    fn the_ceiling_is_reached_by_block_count_rather_than_by_file_size() {
        // **`Error::TapeTooLong` had no failing case here at all.** The variant became reachable
        // from `tap::parse` when `.tzx` landed, and the note on `parse` describing when it fires
        // was therefore an ungraded sentence — which is how that note came to be wrong by 16×
        // and wrong about its axis with nothing going red.
        //
        // The figures themselves — 8082 half-periods, 2694 per input byte, 2075 blocks — are
        // pinned at compile time beside the constants. What is left for a runtime test is the
        // half a `const` assertion cannot reach: that the **parser** agrees with them, at the
        // boundary, in both directions.
        let largest = minimal_blocks(MAX_PLAYABLE_BLOCKS);
        assert_eq!(
            largest.len(),
            6_225,
            "the whole worst case is six kilobytes"
        );
        let tape = parse(&largest).expect("the last block that fits must still play");
        assert_eq!(
            tape.pulses().len(),
            MAX_PLAYABLE_BLOCKS * MINIMAL_BLOCK_PULSES
        );
        assert!(
            tape.pulses().len() <= MAX_PULSES,
            "and it is inside the ceiling"
        );
        drop(tape);

        // ...and one block more is refused, naming the offset of the block that overflowed
        // rather than the end of the file, so the message points at something the user can find.
        let one_too_many = minimal_blocks(MAX_PLAYABLE_BLOCKS + 1);
        assert_eq!(one_too_many.len(), 6_228);
        assert_eq!(
            parse(&one_too_many),
            Err(Error::TapeTooLong {
                offset: MAX_PLAYABLE_BLOCKS * MINIMAL_BLOCK_BYTES,
                limit: MAX_PULSES,
            })
        );
    }

    #[test]
    fn a_tape_the_size_of_a_real_one_is_nowhere_near_the_ceiling() {
        // The proportion, so the ruling above is not read as a practical limit. `testdata/`'s
        // tapes are four to six blocks against a ceiling of 2075, and this is the largest of
        // them expressed the same way: 48 KB in six blocks.
        const REAL_TAPE_BLOCKS: usize = 6;
        const REAL_TAPE_BYTES: usize = 48 * 1024;
        let pulses = REAL_TAPE_BLOCKS * (HEADER_PILOT_PULSES + 2 + 1) + REAL_TAPE_BYTES * 16;
        assert!(
            pulses * 20 < MAX_PULSES,
            "a real tape must be orders of magnitude inside the ceiling, not near it"
        );
    }
}
