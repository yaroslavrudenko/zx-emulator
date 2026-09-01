//! The `.z80` run-length codec, both directions.
//!
//! # The scheme, transcribed from the format description
//!
//! Four rules, and the fourth is the one that decides whether the files we write are
//! readable anywhere else. They are quoted rather than paraphrased because `docs/M6.md`
//! described only the first three, and a compressor written from that description alone
//! would produce a file that round-trips perfectly here and is misread by every other
//! emulator:
//!
//! 1. *"it replaces repetitions of at least five equal bytes by a four-byte code
//!    `ED ED xx yy`, which stands for 'byte yy repeated xx times'. Only sequences of length
//!    at least 5 are coded."*
//! 2. *"The exception is sequences consisting of ED's; if they are encountered, even two
//!    ED's are encoded into `ED ED 02 ED`."*
//! 3. *"Finally, every byte directly following a single ED is not taken into a block, for
//!    example `ED 6*00` is not encoded into `ED ED ED 06 00` but into `ED 00 ED ED 05 00`."*
//! 4. *"The block is terminated by an end marker, `00 ED ED 00`"* — version 1 only. In
//!    version 2 and 3 each page carries its own length and there is no marker.
//!
//! Rule 3 is not an optimisation and it is not optional. Without it the encoder emits a
//! literal `ED` immediately followed by an `ED ED` escape, and a decoder reading that stream
//! sees `ED ED` and takes the next two bytes as a count and a value. `example_from_the_format_description`
//! asserts the spec's own example byte for byte, and `a_lone_escape_is_never_followed_by_an_escape`
//! asserts the property the rule exists to create, over every page the property test reaches.
//!
//! # Why the count is capped
//!
//! `xx` is one byte, so a run longer than 255 is emitted as consecutive escapes. That is a
//! property of the format and not a choice here.

use super::Error;
use super::reader::{Full, Reader, Writer};

/// The byte that opens an escape, and the byte the exception in rule 2 is about.
pub(super) const ESCAPE: u8 = 0xED;

/// Equal bytes needed before an ordinary run is worth encoding — rule 1.
const MIN_RUN: usize = 5;

/// Equal bytes needed before a run of [`ESCAPE`] is encoded — rule 2.
const MIN_ESCAPE_RUN: usize = 2;

/// The largest run one escape can express, because the count is a single byte.
const MAX_RUN: usize = u8::MAX as usize;

/// The four bytes that terminate a version 1 memory block — rule 4.
pub(super) const V1_END_MARKER: [u8; 4] = [0x00, ESCAPE, ESCAPE, 0x00];

/// Expand run-length-encoded bytes from `source` until `destination` is exactly full.
///
/// Stops the moment the destination fills, so the caller decides what any remaining input
/// means: for version 1 it must be the end marker, and for version 2 and 3 the block's
/// declared length must have been consumed exactly. `block_offset` is where the block starts
/// in the file, and appears in the errors — the codec knows *what* went wrong and the caller
/// knows *where*, so the two are combined here rather than a wrong number being invented at
/// either end.
///
/// # Termination
///
/// Every iteration consumes at least one source byte, so progress is structural rather than
/// argued. A count of zero emits nothing and still consumes its four bytes.
///
/// # Errors
///
/// [`Error::PageUnderrun`] when the source is exhausted with the destination unfilled,
/// [`Error::PageOverrun`] when a run does not fit, and [`Error::Truncated`] when a four-byte
/// escape is cut in half — which is a different finding from the block simply ending, and is
/// reported differently so a user can tell a short file from a short page.
pub(super) fn expand(
    source: &mut Reader<'_>,
    destination: &mut Writer<'_>,
    block_offset: usize,
) -> Result<(), Error> {
    // Invariant across the whole loop — a `Writer` never grows — so it is read once and the
    // error can be built without borrowing the writer that is being written to.
    let capacity = destination.capacity();
    let overrun = Error::PageOverrun {
        offset: block_offset,
        capacity,
    };

    while !destination.is_full() {
        if source.is_empty() {
            return Err(Error::PageUnderrun {
                offset: block_offset,
                capacity,
                written: destination.written(),
            });
        }
        let byte = source.u8()?;

        // A lone `ED` is a literal; only `ED ED` opens an escape.
        if byte != ESCAPE || source.peek() != Some(ESCAPE) {
            destination.push(byte).map_err(|Full| overrun)?;
            continue;
        }

        source.skip(1)?; // the second `ED`
        let count = source.u8()?;
        let value = source.u8()?;
        destination
            .fill(value, usize::from(count))
            .map_err(|Full| overrun)?;
    }
    Ok(())
}

/// Encode `page` under the four rules above.
///
/// The result may be **longer** than the input — an alternating `ED ED xx` pattern costs
/// four output bytes for every two input ones — which is why the caller compares the length
/// against the page size and falls back to storing the page raw. Nothing here is sized from
/// a file: `page` is always one of ours.
pub(super) fn compress(page: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(page.len());
    let mut rest = page;
    // Rule 3: the byte after a literal `ED` is never taken into a block.
    let mut after_lone_escape = false;

    while let Some((&byte, tail)) = rest.split_first() {
        let run = 1 + tail
            .iter()
            .take(MAX_RUN - 1)
            .take_while(|&&next| next == byte)
            .count();
        let worth_encoding = run >= MIN_RUN || (byte == ESCAPE && run >= MIN_ESCAPE_RUN);

        if after_lone_escape || !worth_encoding {
            encoded.push(byte);
            rest = tail;
            after_lone_escape = byte == ESCAPE;
            continue;
        }

        // INVARIANT: `run` is `1 + at most MAX_RUN - 1`, so it fits a `u8` exactly.
        encoded.extend_from_slice(&[ESCAPE, ESCAPE, run as u8, byte]);
        // `run <= rest.len()` by construction. `get` is used rather than a range index so
        // that the impossibility does not have to be trusted; the `else` is unreachable.
        let Some(remaining) = rest.get(run..) else {
            break;
        };
        rest = remaining;
        after_lone_escape = false;
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::PAGE_SIZE;

    /// Where every block in these tests notionally starts, so the errors have an offset.
    const BLOCK_OFFSET: usize = 30;

    /// Expand `source` into a `capacity`-byte page, as the parsers do.
    fn expand_into(source: &[u8], capacity: usize) -> Result<Vec<u8>, Error> {
        let mut page = vec![0_u8; capacity];
        let mut reader = Reader::new(source);
        let mut writer = Writer::new(&mut page);
        expand(&mut reader, &mut writer, BLOCK_OFFSET)?;
        Ok(page)
    }

    /// `compress` then `expand`, which is the property test's subject.
    fn round_trip(page: &[u8]) -> Vec<u8> {
        expand_into(&compress(page), page.len()).unwrap_or_default()
    }

    #[test]
    fn example_from_the_format_description() {
        // Rule 3's own example, transcribed: `ED` followed by six zeros is NOT
        // `ED ED ED 06 00` but `ED 00 ED ED 05 00`. The expectation is the format
        // description's, not this function's, which is the only reason it can grade the
        // rule at all — a derived expectation would agree with whatever `compress` did.
        let page = [ESCAPE, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(
            compress(&page),
            vec![ESCAPE, 0x00, ESCAPE, ESCAPE, 0x05, 0x00],
            "rule 3: the byte directly after a single ED is not taken into a block"
        );
        assert_eq!(round_trip(&page), page);
    }

    #[test]
    fn two_escapes_are_encoded_even_though_two_is_below_the_threshold() {
        // Rule 2, and its literal example: `ED ED 02 ED`.
        assert_eq!(
            compress(&[ESCAPE, ESCAPE]),
            vec![ESCAPE, ESCAPE, 0x02, ESCAPE]
        );
    }

    #[test]
    fn four_equal_bytes_are_left_alone_and_five_are_encoded() {
        // Rule 1's boundary, from both sides. Off by one here silently changes every file
        // we write, and nothing else in the suite would notice.
        assert_eq!(compress(&[0x7F; 4]), vec![0x7F; 4]);
        assert_eq!(compress(&[0x7F; 5]), vec![ESCAPE, ESCAPE, 0x05, 0x7F]);
    }

    #[test]
    fn a_run_longer_than_the_counter_is_split_across_escapes() {
        let page = [0xA5_u8; 300];
        assert_eq!(
            compress(&page),
            vec![ESCAPE, ESCAPE, 255, 0xA5, ESCAPE, ESCAPE, 45, 0xA5]
        );
        assert_eq!(round_trip(&page), page);
    }

    #[test]
    fn a_lone_escape_is_never_followed_by_an_escape() {
        // The property rule 3 exists to create, stated directly rather than inferred from
        // the round trip: in the encoder's output, an `ED ED` pair is *always* the start of
        // a four-byte escape, never two literals. If it were not, a decoder would read the
        // following two bytes as a count and a value.
        let adversarial: &[&[u8]] = &[
            &[ESCAPE, 0, 0, 0, 0, 0, 0],
            &[ESCAPE; 40],
            &[ESCAPE, ESCAPE, 1, ESCAPE, 2, 2, 2, 2, 2],
            &[1, ESCAPE, 2, ESCAPE, 3, ESCAPE],
            &[ESCAPE, 1, ESCAPE, ESCAPE, ESCAPE],
        ];
        for page in adversarial {
            let encoded = compress(page);
            let mut index = 0;
            while let Some(window) = encoded.get(index..index + 2) {
                if window == [ESCAPE, ESCAPE] {
                    assert!(
                        encoded.get(index..index + 4).is_some(),
                        "an ED ED in {encoded:02X?} is not the start of a four-byte escape"
                    );
                    index += 4;
                } else {
                    index += 1;
                }
            }
            assert_eq!(round_trip(page), *page, "page {page:02X?}");
        }
    }

    #[test]
    fn a_count_of_zero_emits_nothing_and_still_makes_progress() {
        // `docs/M6.md`'s hostile-input table: legal, emits nothing, consumes four bytes.
        // The reason it matters is termination — a token that consumed nothing would loop
        // forever on a hostile file.
        assert_eq!(
            expand_into(&[ESCAPE, ESCAPE, 0x00, 0xFF, 0x11, 0x22], 2),
            Ok(vec![0x11, 0x22])
        );
    }

    #[test]
    fn a_run_that_overruns_the_page_is_a_fault_and_not_an_abort() {
        assert_eq!(
            expand_into(&[ESCAPE, ESCAPE, 0xFF, 0xAA], 4),
            Err(Error::PageOverrun {
                offset: BLOCK_OFFSET,
                capacity: 4
            })
        );
    }

    #[test]
    fn a_source_that_runs_out_early_underruns_rather_than_zero_filling() {
        // Strict, deliberately: zero-filling produces a wrong machine that every round trip
        // then agrees is right.
        assert_eq!(
            expand_into(&[0x01, 0x02], 4),
            Err(Error::PageUnderrun {
                offset: BLOCK_OFFSET,
                capacity: 4,
                written: 2
            })
        );
    }

    #[test]
    fn an_escape_cut_in_half_is_truncated_rather_than_underrun() {
        // A different finding from the one above, and worth telling apart: the block did
        // not merely end, a token was severed.
        for cut in [
            &[ESCAPE, ESCAPE][..],
            &[ESCAPE, ESCAPE, 0x05][..],
            &[0x00, ESCAPE, ESCAPE][..],
        ] {
            assert!(
                matches!(expand_into(cut, 16), Err(Error::Truncated { .. })),
                "{cut:02X?} should be Truncated"
            );
        }
    }

    #[test]
    fn a_lone_escape_at_the_very_end_of_the_input_is_a_literal() {
        assert_eq!(expand_into(&[0x01, ESCAPE], 2), Ok(vec![0x01, ESCAPE]));
    }

    #[test]
    fn the_named_adversarial_pages_round_trip() {
        // `docs/M6.md`'s implementation order names these: all-ED, runs of exactly four and
        // five, and an ED at the page boundary.
        let mut ed_at_the_end = vec![0_u8; PAGE_SIZE];
        if let Some(byte) = ed_at_the_end.last_mut() {
            *byte = ESCAPE;
        }

        let pages: Vec<Vec<u8>> = vec![
            vec![ESCAPE; PAGE_SIZE],
            vec![0x00; PAGE_SIZE],
            vec![0xFF; PAGE_SIZE],
            vec![0x11; 4],
            vec![0x11; 5],
            ed_at_the_end,
            (0..PAGE_SIZE).map(|i| (i & 0xFF) as u8).collect(),
        ];
        for page in &pages {
            assert_eq!(&round_trip(page), page, "len {}", page.len());
        }
    }

    #[test]
    fn the_encoding_of_a_page_of_one_value_is_smaller_than_the_page() {
        // Not a performance claim — a check that the codec is doing its job at all, which a
        // round trip alone cannot see because the identity codec round-trips perfectly.
        assert!(compress(&[0x00; PAGE_SIZE]).len() < 300);
    }

    proptest::proptest! {
        #[test]
        fn every_page_survives_the_round_trip(
            page in proptest::collection::vec(
                // Weighted towards ED and towards repetition, because uniform random bytes
                // almost never produce a run of five and would exercise none of the rules.
                proptest::prop_oneof![
                    3 => proptest::strategy::Just(ESCAPE),
                    3 => proptest::strategy::Just(0x00_u8),
                    2 => 0..=3_u8,
                    1 => proptest::num::u8::ANY,
                ],
                0..600_usize,
            )
        ) {
            proptest::prop_assert_eq!(round_trip(&page), page);
        }
    }
}
