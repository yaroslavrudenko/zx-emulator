//! `.tzx` graded by expectations that owe nothing to the converter.
//!
//! # Why this file exists, given there is no round trip
//!
//! This project does not write `.tzx`, so the instrument that grades the snapshot codec —
//! `parse(write(s)) == s` — does not exist here, and `docs/M6.md` Decision 7's finding is that
//! it would be the weaker instrument anyway: **every round trip in this workspace is green on a
//! symmetric error**, measured rather than argued, including the ones over files a third party
//! wrote. Only an expectation that owes nothing to the code under test sees one.
//!
//! So there are three instruments here and none of them is a round trip:
//!
//! 1. **Hand-transcribed vectors.** A block written out byte by byte from the format
//!    description, each field commented with its offset and meaning, and the pulse train it
//!    must produce written out **separately** as literals. Neither is derived from the other.
//!    `docs/STATUS.md` records this project catching the derived-expectation defect three
//!    times, so the fixture rule from Decision 7 is applied and asserted: **every timing in the
//!    vector is pairwise distinct and none is zero**, or a permutation would be invisible even
//!    to a hand-written expectation.
//! 2. **Equivalence with `.tap`.** A `.tzx` of nothing but standard-speed blocks carries the
//!    same data as the equivalent `.tap`, and both media are generated here, so this needs no
//!    corpus at all. It is honest about what it grades: the two share their emitter, so this is
//!    a check of the `.tzx` **framing** — where the flag byte and the data are found in a
//!    standard-speed block — and not of the timings, which `tzx_rom_timings.rs` grades against
//!    the ROM.
//! 3. **A decoder this file owns**, reading a turbo train back into bytes using the threshold
//!    rule a loader uses, over arbitrary payloads *and arbitrary timings*. It shares no code
//!    with `spectrum::tape`, so this is not `f(f_inverse(x))`.
//!
//! # What is deliberately not here
//!
//! A corpus. `.tzx` files are somebody else's work and are not committed — `testdata/README.md`
//! carries the fetch instructions and the shared absence policy, and `tzx_corpus.rs` is the
//! sweep. Everything in this file runs on every clone with no fetch.

use spectrum::Model;
use spectrum::tape::{tap, tzx};

/// The ten-byte header every `.tzx` opens with, transcribed from the description's own table:
/// `"ZXTape!"` at `0x00`, the end-of-text marker `0x1A` at `0x07`, then the major and minor
/// revision numbers at `0x08` and `0x09`.
const HEADER: [u8; 10] = [b'Z', b'X', b'T', b'a', b'p', b'e', b'!', 0x1A, 0x01, 0x14];

/// `ID 10 - Standard Speed Data Block`.
const STANDARD_SPEED_DATA: u8 = 0x10;
/// `ID 11 - Turbo Speed Data Block`.
const TURBO_SPEED_DATA: u8 = 0x11;

/// The ROM's own timings, which is what a standard-speed block means and what `.tap` implies.
const ROM_PILOT: u32 = 2168;
const ROM_SYNC: [u32; 2] = [667, 735];
const ROM_BIT_ZERO: u32 = 855;
const ROM_BIT_ONE: u32 = 1710;
const ROM_HEADER_PILOT_PULSES: usize = 8063;
const ROM_DATA_PILOT_PULSES: usize = 3223;

/// A `.tzx` file with `blocks` after the header.
fn file(blocks: &[&[u8]]) -> Vec<u8> {
    let mut bytes = HEADER.to_vec();
    for block in blocks {
        bytes.extend_from_slice(block);
    }
    bytes
}

fn train(blocks: &[&[u8]]) -> Vec<u32> {
    tzx::parse(&file(blocks), Model::Spectrum48K)
        .expect("a well-formed file")
        .pulses()
        .to_vec()
}

// ---------------------------------------------------------------------------------------
// 1 — the hand-transcribed turbo block
// ---------------------------------------------------------------------------------------

/// Pilot half-period, in T-states. Not the ROM's 2168 — the point of this block is that the
/// number comes from the file.
const PILOT: u32 = 1000;
/// First sync half-period.
const SYNC_FIRST: u32 = 300;
/// Second sync half-period.
const SYNC_SECOND: u32 = 400;
/// Half-period of a zero bit.
const BIT_ZERO: u32 = 500;
/// Half-period of a one bit. **Not** twice the zero bit, deliberately: the ROM's two happen to
/// be in that ratio, and a converter that derived one from the other would pass every test
/// built from ROM timings and fail on a real turbo loader.
const BIT_ONE: u32 = 700;
/// How many pilot half-periods. Small, so the whole train can be a literal.
const PILOT_PULSES: usize = 3;

/// The payload byte. `0x2A` rather than a palindrome: it is not its own bit-reversal, so a
/// converter emitting the bits least-significant-first produces a different train rather than
/// the same one. `0xA5` — the obvious choice — **is** its own reversal and would have hidden it.
const PAYLOAD: u8 = 0x2A;

/// `ID 11 - Turbo Speed Data Block`, byte by byte, each field at the offset the description
/// gives it. Little-endian throughout: *"Any value requiring more than one byte is stored in
/// little endian format"*.
///
/// **`rustfmt` is held off this deliberately.** One line per field, carrying that field's offset
/// and its meaning, is not formatting — it is the transcription, and it is the only thing that
/// makes the array checkable against the format description by eye. Reflowed to fill the line
/// width, the bytes are still correct and nobody can ever audit them again.
#[rustfmt::skip]
const TURBO_VECTOR: [u8; 20] = [
    TURBO_SPEED_DATA,       // the block ID
    0xE8, 0x03,             // 0x00 WORD    length of PILOT pulse             = 1000
    0x2C, 0x01,             // 0x02 WORD    length of SYNC first pulse        =  300
    0x90, 0x01,             // 0x04 WORD    length of SYNC second pulse       =  400
    0xF4, 0x01,             // 0x06 WORD    length of ZERO bit pulse          =  500
    0xBC, 0x02,             // 0x08 WORD    length of ONE bit pulse           =  700
    0x03, 0x00,             // 0x0A WORD    length of PILOT tone, in pulses   =    3
    0x08,                   // 0x0C BYTE    used bits in the last byte        =    8
    0x00, 0x00,             // 0x0D WORD    pause after this block, ms        =    0
    0x01, 0x00, 0x00,       // 0x0F BYTE[3] length of the data that follows   =    1
    PAYLOAD,                // 0x12 BYTE[N] data
];

#[test]
fn the_vectors_own_fields_are_pairwise_distinct_and_none_is_zero() {
    // `docs/M6.md` Decision 7's rule about fixtures, asserted about this one rather than
    // trusted: "otherwise a permutation is invisible even to a hand-written expectation, and a
    // dropped field looks like a correct default". Five timings read from five different
    // offsets — if any two were equal, swapping those two offsets would be undetectable.
    let fields = [PILOT, SYNC_FIRST, SYNC_SECOND, BIT_ZERO, BIT_ONE];
    for (index, &field) in fields.iter().enumerate() {
        assert_ne!(
            field, 0,
            "field {index} is zero, so a dropped read looks correct"
        );
        for (other, &against) in fields.iter().enumerate().skip(index + 1) {
            assert_ne!(
                field, against,
                "fields {index} and {other} cannot be told apart"
            );
        }
    }
    assert_ne!(
        BIT_ONE,
        BIT_ZERO * 2,
        "the ROM's ratio must not be reconstructible from this fixture"
    );
    assert_ne!(
        PAYLOAD,
        PAYLOAD.reverse_bits(),
        "the payload must not be its own bit-reversal"
    );
}

#[test]
fn the_turbo_vector_produces_the_train_written_out_by_hand() {
    // The whole train as literals, taken from the format's rules — a pilot tone of the declared
    // length and count, the two sync half-periods in order, then two equal half-periods per
    // data bit, most significant first — and **not** from the converter.
    //
    // `0x2A` is `0b0010_1010`, so the bits are 0,0,1,0,1,0,1,0.
    // One line per element of the signal, for the same reason the vector itself is pinned: the
    // layout is what lets a reader check the train against the format's rules bit by bit.
    #[rustfmt::skip]
    let expected = vec![
        PILOT, PILOT, PILOT,        // the pilot tone: three half-periods of the declared length
        SYNC_FIRST, SYNC_SECOND,    // the sync pair, in the order the file lists them
        BIT_ZERO, BIT_ZERO,         // bit 7 = 0
        BIT_ZERO, BIT_ZERO,         // bit 6 = 0
        BIT_ONE,  BIT_ONE,          // bit 5 = 1
        BIT_ZERO, BIT_ZERO,         // bit 4 = 0
        BIT_ONE,  BIT_ONE,          // bit 3 = 1
        BIT_ZERO, BIT_ZERO,         // bit 2 = 0
        BIT_ONE,  BIT_ONE,          // bit 1 = 1
        BIT_ZERO, BIT_ZERO,         // bit 0 = 0
        // ...and no silence, because the pause field is zero and the format says a pause of
        // zero duration is completely ignored.
    ];
    assert_eq!(train(&[&TURBO_VECTOR]), expected);
}

#[test]
fn moving_any_one_field_of_the_vector_changes_the_train() {
    // The vector is only worth having if it discriminates, so this is its positive control: for
    // every one of the five timing words, a file identical except for that word must produce a
    // different train. Without this the vector could be passing because the converter ignores
    // the fields and the expectation happens to match something else.
    let baseline = train(&[&TURBO_VECTOR]);
    for offset in [1, 3, 5, 7, 9] {
        let mut mutated = TURBO_VECTOR;
        let byte = mutated.get_mut(offset).expect("a field of the vector");
        *byte = byte.wrapping_add(1);
        assert_ne!(
            train(&[&mutated]),
            baseline,
            "changing the field at block offset {} left the train identical",
            offset - 1
        );
    }
}

#[test]
fn a_partial_last_byte_plays_only_its_top_bits() {
    // "if this is 6, then the bits used (x) in the last byte are: xxxxxx00, where MSb is the
    // leftmost bit". Two bytes, so the rule's "last byte only" half is graded too: the first is
    // played whole and the second is not.
    let mut block = TURBO_VECTOR.to_vec();
    let used_bits = block.get_mut(1 + 0x0C).expect("the used-bits field");
    *used_bits = 3;
    let length = block.get_mut(1 + 0x0F).expect("the length field");
    *length = 2;
    block.push(0b1110_0000);

    let mut expected = vec![PILOT, PILOT, PILOT, SYNC_FIRST, SYNC_SECOND];
    // `0x2A` whole: 0,0,1,0,1,0,1,0.
    for bit in [false, false, true, false, true, false, true, false] {
        let length = if bit { BIT_ONE } else { BIT_ZERO };
        expected.extend([length, length]);
    }
    // ...then only the top three bits of `0b1110_0000`.
    expected.extend([BIT_ONE, BIT_ONE, BIT_ONE, BIT_ONE, BIT_ONE, BIT_ONE]);
    assert_eq!(train(&[&block]), expected);
}

// ---------------------------------------------------------------------------------------
// 2 — equivalence with `.tap`, on media generated here
// ---------------------------------------------------------------------------------------

/// A `.tap` file holding one block: a length word, the flag, the payload, and its parity.
fn tap_file(flag: u8, payload: &[u8]) -> Vec<u8> {
    let parity = payload.iter().fold(flag, |sum, &byte| sum ^ byte);
    let length = u16::try_from(payload.len() + 2).expect("a short block");
    let mut file = length.to_le_bytes().to_vec();
    file.push(flag);
    file.extend_from_slice(payload);
    file.push(parity);
    file
}

/// `ID 10 - Standard Speed Data Block` carrying the same block, with `pause` milliseconds after.
///
/// The body is a pause word, a length word, and then *"Data as in .TAP files"* — which is the
/// `.tap` block **without** its own length word, since the `.tzx` block carries its own.
fn standard_block(flag: u8, payload: &[u8], pause: u16) -> Vec<u8> {
    let mut data = vec![flag];
    data.extend_from_slice(payload);
    data.push(payload.iter().fold(flag, |sum, &byte| sum ^ byte));

    let mut block = vec![STANDARD_SPEED_DATA];
    block.extend(pause.to_le_bytes());
    block.extend(
        u16::try_from(data.len())
            .expect("a short block")
            .to_le_bytes(),
    );
    block.extend(data);
    block
}

#[test]
fn a_standard_speed_block_carries_the_same_signal_as_the_equivalent_tap() {
    // The equivalence the format asserts: "This block must be replayed with the standard
    // Spectrum ROM timing values". Both media are generated here, so no corpus is involved.
    //
    // **What this grades is the framing, not the timings.** The two converters share their
    // emitter — one representation of "a pilot, a sync pair, and two half-periods per bit" —
    // so a wrong *timing* would move both sides together. What cannot move together is where
    // the flag byte and the data are found: a `.tap` block opens with its own length word and a
    // `.tzx` standard-speed block does not, and reading either wrongly shifts every byte.
    // The timings are graded against the ROM in `tzx_rom_timings.rs`.
    let payload = [0x00, 0xFF, 0x2A, 0xD5, 0x01, 0x80];
    for flag in [0x00_u8, 0xFF] {
        let from_tap = tap::parse(&tap_file(flag, &payload)).expect("a well-formed .tap");
        let from_tzx = train(&[&standard_block(flag, &payload, 0)]);

        // The `.tap` converter appends its own inter-block gap and the `.tzx` block's pause is
        // zero, so the comparison is over everything before that gap — a difference in what the
        // two formats *say*, not a disagreement about the signal.
        let (_gap, signal) = from_tap
            .pulses()
            .split_last()
            .expect("a block produces pulses");
        assert_eq!(from_tzx, signal, "flag {flag:#04X}");
    }
}

/// A payload byte chosen so the block's **parity byte always lands in the opposite pilot class
/// from the flag**, whatever the flag is.
///
/// Parity is the XOR of every byte, so a payload with bit 7 set flips bit 7 of the parity
/// relative to the flag — and bit 7 is exactly what chooses between the two pilot lengths.
///
/// **This is not decoration; the test below was blind without it.** With a payload of `0x01`
/// the parity stayed in the flag's own class for all four flags, so a converter reading the
/// pilot rule from the block's **last** byte instead of its first produced identical trains and
/// the test passed. A mutation found that, and the fixture is what closes it.
const PARITY_FLIPPING_PAYLOAD: u8 = 0x80;

#[test]
fn the_flag_byte_chooses_the_pilot_length_in_a_tzx_too() {
    // "The pilot tone consists in 8063 pulses if the first data byte (flag byte) is < 128, 3223
    // otherwise." Asserted at the boundary rather than at 0 and 255, and by counting the train
    // rather than by trusting the converter's own constant.
    for (flag, expected) in [
        (0x00_u8, ROM_HEADER_PILOT_PULSES),
        (0x7F, ROM_HEADER_PILOT_PULSES),
        (0x80, ROM_DATA_PILOT_PULSES),
        (0xFF, ROM_DATA_PILOT_PULSES),
    ] {
        let parity = flag ^ PARITY_FLIPPING_PAYLOAD;
        assert_ne!(
            flag < 0x80,
            parity < 0x80,
            "flag {flag:#04X}: the fixture must put the parity byte in the other class, or a \
             converter reading the rule from the wrong end of the block is invisible here"
        );

        let pulses = train(&[&standard_block(flag, &[PARITY_FLIPPING_PAYLOAD], 0)]);
        let pilot = pulses.iter().take_while(|&&p| p == ROM_PILOT).count();
        assert_eq!(pilot, expected, "flag {flag:#04X}");
        assert_eq!(
            pulses.get(pilot..pilot + 2),
            Some(&ROM_SYNC[..]),
            "flag {flag:#04X}: the sync pair follows the pilot"
        );
    }
}

// ---------------------------------------------------------------------------------------
// 3 — a decoder this file owns
// ---------------------------------------------------------------------------------------

/// Read a turbo block's data region back into bytes.
///
/// Written from the format's rule and **not** from `spectrum::tape`: skip the pilot tone and the
/// two sync half-periods, then take half-periods in pairs and call each pair a bit by comparing
/// it against the midpoint of the two declared lengths, which is what a loader's edge timing
/// amounts to. The pilot count and the timings come from the **file**, which is where a real
/// loader gets them too.
fn decode(pulses: &[u32], pilot_pulses: usize, zero: u32, one: u32) -> Vec<u8> {
    let threshold = u32::midpoint(zero, one);
    let bits: Vec<bool> = pulses
        .iter()
        .skip(pilot_pulses + 2)
        .step_by(2)
        .map(|&pulse| pulse > threshold)
        .collect();
    bits.as_chunks::<8>()
        .0
        .iter()
        .map(|byte| {
            byte.iter()
                .fold(0_u8, |value, &bit| (value << 1) | u8::from(bit))
        })
        .collect()
}

#[test]
fn the_decoder_can_tell_a_wrong_train_from_a_right_one() {
    // The decoder is only worth running if it discriminates, so it has its own failing cases
    // before anything is built on it. Without them it would be a reader that returns whatever
    // it is given, asserting that the converter is right.
    let mut right = vec![PILOT, PILOT, PILOT, SYNC_FIRST, SYNC_SECOND];
    for bit in [false, false, true, false, true, false, true, false] {
        let length = if bit { BIT_ONE } else { BIT_ZERO };
        right.extend([length, length]);
    }
    assert_eq!(
        decode(&right, PILOT_PULSES, BIT_ZERO, BIT_ONE),
        vec![PAYLOAD]
    );

    let swapped: Vec<u32> = right
        .iter()
        .enumerate()
        .map(|(index, &pulse)| {
            if index < PILOT_PULSES + 2 {
                pulse
            } else if pulse == BIT_ZERO {
                BIT_ONE
            } else {
                BIT_ZERO
            }
        })
        .collect();
    assert_eq!(
        decode(&swapped, PILOT_PULSES, BIT_ZERO, BIT_ONE),
        vec![!PAYLOAD],
        "swapping the two bit lengths must swap every bit"
    );

    let mut reversed = vec![PILOT, PILOT, PILOT, SYNC_FIRST, SYNC_SECOND];
    for pair in right
        .get(PILOT_PULSES + 2..)
        .expect("the data region")
        .as_chunks::<2>()
        .0
        .iter()
        .rev()
    {
        reversed.extend_from_slice(pair);
    }
    assert_eq!(
        decode(&reversed, PILOT_PULSES, BIT_ZERO, BIT_ONE),
        vec![PAYLOAD.reverse_bits()],
        "reading the bits the other way round must produce the mirror byte"
    );
}

/// `ID 11` carrying `payload` at the given timings, with no pause.
fn turbo_block(timings: &[u32; 5], pilot_pulses: u16, payload: &[u8]) -> Vec<u8> {
    let mut block = vec![TURBO_SPEED_DATA];
    for &timing in timings {
        let word = u16::try_from(timing).expect("a timing inside a WORD");
        block.extend(word.to_le_bytes());
    }
    block.extend(pilot_pulses.to_le_bytes());
    block.push(8);
    block.extend(0_u16.to_le_bytes());
    let length = u32::try_from(payload.len()).expect("a short payload");
    block.extend(length.to_le_bytes().get(..3).expect("a 24-bit length"));
    block.extend_from_slice(payload);
    block
}

proptest::proptest! {
    #[test]
    fn any_turbo_block_decodes_back_to_its_own_bytes(
        payload: Vec<u8>,
        zero in 1_u32..30_000,
        gap in 1_u32..30_000,
        pilot_pulses in 0_u16..8,
    ) {
        // Arbitrary payloads **and arbitrary timings**, which is what separates this from the
        // vector above: the vector grades one set of numbers at known offsets, and this grades
        // that the numbers are carried through at all. `decode` shares no code with the
        // converter, so this is not `f(f_inverse(x))`.
        //
        // Its blind spot is a rule both readings share — the bit *order*, say — which is what
        // the hand-written literal train closes.
        let one = zero + gap;
        let pilot = one + 1;
        let timings = [pilot, pilot + 1, pilot + 2, zero, one];
        let block = turbo_block(&timings, pilot_pulses, &payload);
        let tape = tzx::parse(&file(&[&block]), Model::Spectrum48K)
            .expect("a well-formed turbo block");
        proptest::prop_assert_eq!(
            decode(tape.pulses(), usize::from(pilot_pulses), zero, one),
            payload
        );
    }
}

#[test]
fn the_degenerate_turbo_block_is_still_a_signal() {
    // The smallest thing the `ID 11` fields can describe: **no pilot tone at all**, and bit
    // half-periods of one and two T-states. Legal — nothing in the format sets a minimum — and
    // it is the corner where an off-by-one in the pilot count or a threshold computed as
    // `(zero + one) / 2` on tiny numbers would show up.
    //
    // It is written out by name rather than left as a `proptest` seed file. The sweep did
    // generate this shape while a **mutant** was in the tree, and a recorded hash saying "this
    // once failed" would send a reader looking for a defect in code that never had one. A case
    // worth re-running is worth being able to read.
    let block = turbo_block(&[3, 4, 5, 1, 2], 0, &[0x00]);
    let pulses = train(&[&block]);

    assert_eq!(
        pulses,
        vec![4, 5, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        "no pilot, the sync pair, then sixteen one-T-state half-periods for the zero byte"
    );
    assert_eq!(decode(&pulses, 0, 1, 2), vec![0x00]);
}

#[test]
fn the_bit_timings_survive_a_block_that_is_faster_than_the_rom() {
    // The whole reason `.tzx` exists, as an assertion. A turbo loader's tape has bit
    // half-periods well under the ROM's 855, and nothing in `.tap` can express one — so a
    // converter that quietly substituted the ROM's timings would produce a plausible train that
    // no turbo loader could read.
    const FAST_ZERO: u32 = 400;
    const FAST_ONE: u32 = 800;
    let block = turbo_block(&[1500, 500, 600, FAST_ZERO, FAST_ONE], 2, &[0xFF, 0x00]);
    let pulses = train(&[&block]);

    assert_eq!(
        pulses.get(..5),
        Some(&[1500, 1500, 500, 600, FAST_ONE][..]),
        "two pilot half-periods, the sync pair, then the first bit of 0xFF"
    );
    assert!(
        pulses
            .iter()
            .all(|&pulse| pulse != ROM_BIT_ZERO && pulse != ROM_BIT_ONE),
        "no ROM timing may appear in a train whose file asked for none"
    );
    assert_eq!(
        pulses.iter().filter(|&&p| p == FAST_ONE).count(),
        16,
        "the 0xFF byte is sixteen one-bit half-periods"
    );
    assert_eq!(
        pulses.iter().filter(|&&p| p == FAST_ZERO).count(),
        16,
        "and the 0x00 byte is sixteen zero-bit half-periods"
    );
}
