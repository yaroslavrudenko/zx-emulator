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
//! cannot be written down as a `.tap` at any speed, and `.tzx` — which can — becomes a second
//! converter here rather than a rewrite of the ULA side.
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
const PILOT_PULSE: u32 = 2168;

/// Pilot half-periods before a **header** block.
///
/// `SA-BYTES` loads `HL = 0x1F80` for a flag under 128. `L` counts down per pulse and `H` per
/// wrap, and the loop ends when `H` goes negative — 128 pulses for the first pass and 256 for
/// each of the 31 that follow, so **8064 edges** and therefore 8063 complete half-periods
/// between them. The first edge arrives after a two-iteration `DJNZ` rather than a full pilot
/// period, which is why the pulse count is one below the edge count.
const HEADER_PILOT_PULSES: usize = 8063;

/// Pilot half-periods before a **data** block.
///
/// The same loop with `HL = 0x0C98`: 152 pulses then 12 passes of 256, so 3224 edges and
/// **3223** half-periods.
const DATA_PILOT_PULSES: usize = 3223;

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
const SYNC_FIRST: u32 = 667;

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
const SYNC_SECOND: u32 = 735;

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
const BIT_ZERO: u32 = 855;

/// Half-period of a one bit, in T-states.
///
/// The same path with the carry set, so the `JR NC` falls through into a second delay:
/// `12` becomes `7 + 7 + 853` — `LD B,$42` and `DJNZ` at `B = 66` — which is `855 + 855`,
/// exactly **double** the zero bit. That the ratio is two rather than something near it is
/// what lets the ROM's loader discriminate bits with a single threshold.
const BIT_ONE: u32 = BIT_ZERO * 2;

const _: () = assert!(BIT_ONE == 1710);

/// Frames of silence after each block: one second at the 48K's 50 Hz frame rate.
///
/// Written as a count of frames rather than as 3.5 million so that it is derived from
/// [`T_STATES_PER_FRAME`] — the machine's own constant — rather than from the clock speed
/// restated here. A 128's longer frame makes its second longer too, which is correct: the
/// gap exists to give the loader time between blocks, not to be a physical second.
const PAUSE_FRAMES: u32 = 50;

/// The silence after a block, as one long half-period.
const PAUSE_T_STATES: u32 = T_STATES_PER_FRAME * PAUSE_FRAMES;

/// Half-periods one byte contributes: two per bit.
const PULSES_PER_BYTE: usize = 2 * u8::BITS as usize;

/// Half-periods a block contributes besides its bytes: two sync pulses and the trailing pause.
const PULSES_PER_BLOCK_FRAMING: usize = 3;

/// Read a `.tap` file into the signal it describes.
///
/// # Errors
///
/// [`Error`], naming the offset that failed. Every failure is a returned value: this function
/// does not panic on any input, which is a property of its construction rather than of the
/// inputs it has seen — there is no indexing expression and no slice operation here that is
/// not total, and the only allocation is a `Vec` whose size is derived from the block lengths
/// the file actually contains rather than from a length field.
pub fn parse(bytes: &[u8]) -> Result<Tape, Error> {
    let mut pulses = Vec::new();
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

        append_block(&mut pulses, flag, block);
        offset += BLOCK_LENGTH_BYTES + length;
        rest = tail;
    }

    Ok(Tape::new(pulses))
}

/// Append one block's signal: pilot tone, sync pair, the bytes, and the trailing silence.
fn append_block(pulses: &mut Vec<u32>, flag: u8, block: &[u8]) {
    let pilot = pilot_pulses(flag);
    pulses.reserve(pilot + block.len() * PULSES_PER_BYTE + PULSES_PER_BLOCK_FRAMING);

    pulses.extend(std::iter::repeat_n(PILOT_PULSE, pilot));
    pulses.push(SYNC_FIRST);
    pulses.push(SYNC_SECOND);
    for &byte in block {
        append_byte(pulses, byte);
    }
    // After every block rather than between blocks, so there is no last-one special case and
    // so the final data edge is followed by a defined stretch of silence rather than by the
    // end of the tape.
    pulses.push(PAUSE_T_STATES);
}

/// Append one byte's signal: most significant bit first, two equal half-periods per bit.
fn append_byte(pulses: &mut Vec<u32>, byte: u8) {
    for bit in (0..u8::BITS).rev() {
        let length = if byte >> bit & 1 == 0 {
            BIT_ZERO
        } else {
            BIT_ONE
        };
        pulses.push(length);
        pulses.push(length);
    }
}

/// How long a pilot tone `flag` calls for.
const fn pilot_pulses(flag: u8) -> usize {
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

    #[test]
    fn a_byte_is_eight_bits_most_significant_first() {
        // Written out by hand: 0x80 is one long pair then seven short ones.
        let mut pulses = Vec::new();
        append_byte(&mut pulses, 0x80);
        let mut expected = vec![BIT_ONE, BIT_ONE];
        expected.extend(std::iter::repeat_n(BIT_ZERO, 14));
        assert_eq!(pulses, expected);

        // ...and 0x01 is the mirror image, which is what separates "most significant first"
        // from "least significant first" rather than merely asserting a length.
        let mut pulses = Vec::new();
        append_byte(&mut pulses, 0x01);
        let mut expected = vec![BIT_ZERO; 14];
        expected.extend([BIT_ONE, BIT_ONE]);
        assert_eq!(pulses, expected);
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
            pulses[..DATA_PILOT_PULSES]
                .iter()
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
}
