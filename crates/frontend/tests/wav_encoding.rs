//! The WAVE encoder: its header, its declared sizes, and — the one that matters — that its body
//! is the very buffer the resampler handed the device.
//!
//! # Scope, stated so it is not mistaken for more
//!
//! A capture path is a debugging aid, not a machine feature, and this file is sized for that. It
//! grades three things and claims nothing else:
//!
//! - **The file describes the sound it contains** — a well-formed 44-byte RIFF/WAVE header whose
//!   two size fields match the body that is really there. A player that reads the header and then
//!   runs off the end of the data is the failure this catches, and it is
//!   `tests/ppm_encoding.rs`'s `the_file_is_exactly_a_header_and_one_pixel_per_pixel` transposed
//!   from pixels to samples.
//! - **There is only one audio path.** Every 16-bit word is asserted to be the quantisation of
//!   the corresponding `f32` [`frontend::audio::Resampler::feed`] emitted, in order. A capture
//!   produced by code the speaker does not run would prove nothing about the speaker, so nobody
//!   may quietly insert a second mixer, a second resampler, or a helpful filter — doing so turns
//!   `the_body_is_the_samples_the_mixer_emitted` red.
//! - **A loud instant clips rather than wrapping**, because the difference is audible and one of
//!   the two is a lie about what the machine played.
//!
//! # What no green here says, and it is the whole of the interesting question
//!
//! **That anyone heard it.** Every assertion below is arithmetic on a `Vec<u8>`. There is no
//! audio device in this environment and no way to capture one, so *the tune is right* is
//! observation by a person with speakers and is recorded in the report rather than asserted here.
//!
//! What these gates buy is a **composition**, and it is worth stating because neither half is
//! interesting alone. `tests/audio_from_the_machine.rs` establishes that the machine's own
//! `BEEP 1,0`, carried through mix and resampling, is a tone of the right *note* — measured
//! against the Sinclair BASIC manual, which is an oracle outside this repository. This file
//! establishes that what lands in a `.wav` body is that buffer and not something adjacent to it.
//! Together they say the file contains the tone the machine played. Neither says it sounds like
//! anything.
//!
//! **This paragraph used to warn that the tone is a semitone flat. It is not, and the warning was
//! wrong on its own terms as well as at second hand.** The 246.65 Hz it cited came from next
//! door's measuring function counting its own filter's 34 ms ring-down as signal; the capture
//! carries middle C at 261.71 Hz. A `.wav` written here is not flat, so there was never anything
//! for this encoder to be excused from — which is the trouble with recording another file's
//! finding as a fact rather than as a citation: when the finding turns out to be an artefact of
//! the instrument, the copy does not get corrected with the original.
//!
//! # The quantisation rule is written out again on purpose
//!
//! [`frontend::wav`] keeps its `quantise` private and this file re-derives it from literals.
//! Importing it would make the discriminating gate compare the encoder's output against the
//! encoder, which is the tautology `docs/STATUS.md` keeps recording — and it is the same reason
//! `tests/ppm_encoding.rs` writes `NORMAL: u8 = 0xD7` by hand instead of reaching into
//! `palette`.

use frontend::wav::{
    self, BITS_PER_SAMPLE, BYTES_PER_SAMPLE, CHANNELS, FMT_CHUNK_BYTES, FORMAT_PCM, HEADER_BYTES,
    MAX_SAMPLES,
};

/// A common device rate, and the one this machine's own device reported.
const DEVICE_HZ: u32 = 48_000;

/// Full scale for a 16-bit body, written from the format rather than imported from the subject.
const FULL_SCALE: f32 = 32_767.0;

/// A handful of levels spanning what the resampler really produces: silence, both polarities,
/// full scale both ways, and values that must round rather than truncate.
///
/// Not a round number of samples and not symmetric, so an encoder that dropped the last word, or
/// wrote the buffer backwards, has somewhere to fail.
fn levels() -> Vec<f32> {
    vec![
        0.0, 0.5, -0.5, 1.0, -1.0, 0.25, -0.25, 0.000_01, -0.000_01, 0.999, -0.999, 0.1, -0.7,
    ]
}

/// The body's 16-bit words, read back out of the encoded bytes.
fn body_words(encoded: &[u8]) -> Vec<i16> {
    encoded[HEADER_BYTES..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| i16::from_le_bytes(pair))
        .collect()
}

/// A little-endian `u32` at `offset`.
fn u32_at(encoded: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(encoded[offset..offset + 4].try_into().expect("four bytes"))
}

/// A little-endian `u16` at `offset`.
fn u16_at(encoded: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(encoded[offset..offset + 2].try_into().expect("two bytes"))
}

#[test]
fn the_format_this_file_was_written_against_is_still_the_format() {
    // A positive control on the premise, the way `ppm_encoding.rs` checks the frame size first.
    // Every offset below is arithmetic on mono 16-bit PCM; if the module changed to stereo or to
    // float, the literals would be wrong and every failure would read as an encoder defect. This
    // one fails first and says what actually moved.
    assert_eq!(CHANNELS, 1, "the offsets below assume one channel");
    assert_eq!(BITS_PER_SAMPLE, 16);
    assert_eq!(BYTES_PER_SAMPLE, 2);
    assert_eq!(FORMAT_PCM, 1);
    assert_eq!(FMT_CHUNK_BYTES, 16);
    assert_eq!(HEADER_BYTES, 44);
}

#[test]
fn the_header_is_a_well_formed_riff_wave_declaring_the_rate_it_was_given() {
    let encoded = wav::encode(&levels(), DEVICE_HZ).expect("a short capture fits");

    // Parsed back out of the bytes field by field, at the offsets the format fixes, rather than
    // compared against a second copy of the same header — so this asserts the *declared* format
    // is the one claimed and not merely that two spellings of it match.
    assert_eq!(&encoded[0..4], b"RIFF");
    assert_eq!(&encoded[8..12], b"WAVE");
    assert_eq!(&encoded[12..16], b"fmt ");
    assert_eq!(u32_at(&encoded, 16), FMT_CHUNK_BYTES);
    assert_eq!(u16_at(&encoded, 20), FORMAT_PCM, "uncompressed integer PCM");
    assert_eq!(u16_at(&encoded, 22), CHANNELS);
    assert_eq!(
        u32_at(&encoded, 24),
        DEVICE_HZ,
        "the rate a player will resample from must be the one the resampler targeted",
    );
    assert_eq!(u16_at(&encoded, 34), BITS_PER_SAMPLE);
    assert_eq!(&encoded[36..40], b"data");

    // The two derived fields, each recomputed here from the rate rather than read twice. A
    // wrong `byte rate` plays at the wrong speed in some players and is ignored by others,
    // which is the worst combination to leave ungated.
    assert_eq!(u16_at(&encoded, 32), 2, "block align: one 16-bit channel");
    assert_eq!(
        u32_at(&encoded, 28),
        DEVICE_HZ * 2,
        "byte rate is rate x block align",
    );
}

#[test]
fn the_declared_sizes_describe_the_body_that_is_actually_there() {
    let levels = levels();
    let encoded = wav::encode(&levels, DEVICE_HZ).expect("a short capture fits");

    let data_bytes = u32_at(&encoded, 40) as usize;
    assert_eq!(
        data_bytes,
        levels.len() * BYTES_PER_SAMPLE,
        "the data chunk must declare the samples that follow it",
    );
    assert_eq!(
        encoded.len(),
        HEADER_BYTES + data_bytes,
        "a body of the wrong length runs a player off the end of the data",
    );

    // The RIFF size counts everything after its own field — the classic off-by-eight, and the
    // one a player most often reports as a truncated file.
    assert_eq!(
        u32_at(&encoded, 4) as usize,
        encoded.len() - 8,
        "the RIFF size counts every byte after itself",
    );
}

#[test]
fn the_body_is_the_samples_the_mixer_emitted() {
    // THE gate. Two artefacts, one derived from the other: every 16-bit word must be the
    // quantisation of the corresponding `f32`, in order. This is what makes a capture evidence
    // about the speaker rather than about the capture tool — a second mixer, a reversed buffer,
    // a dropped sample, a stray filter or a big-endian word all turn it red.
    //
    // The rule is stated here from the format's own literals and never imported from `wav`.
    let levels = levels();
    let encoded = wav::encode(&levels, DEVICE_HZ).expect("a short capture fits");
    let words = body_words(&encoded);
    assert_eq!(words.len(), levels.len());

    let mut compared = 0_usize;
    for (index, (&level, &word)) in levels.iter().zip(&words).enumerate() {
        let expected = (level.clamp(-1.0, 1.0) * FULL_SCALE).round() as i16;
        assert_eq!(word, expected, "sample {index} was {level}");
        compared += 1;
    }
    // Without this, a body of zero length would pass the loop by never entering it — the
    // "count of zero and an absence of the subject are the same observation" failure
    // `docs/STATUS.md` records against the codegen gate, and the reason
    // `tests/ppm_encoding.rs` carries the identical counter.
    assert_eq!(
        compared,
        levels.len(),
        "every sample must have been compared",
    );
}

#[test]
fn the_fixture_actually_puts_more_than_one_level_in_the_file() {
    // A positive control on the gate above: if the encoder wrote a constant, or the resampler
    // went silent, every assertion that happens to expect a zero would still pass against an
    // all-zero body. A capture of silence is a real thing and must not be what proves the path.
    let encoded = wav::encode(&levels(), DEVICE_HZ).expect("a short capture fits");
    let distinct: std::collections::BTreeSet<i16> = body_words(&encoded).into_iter().collect();
    assert!(
        distinct.len() > 5,
        "the fixture should span the range, got {distinct:?}",
    );
    assert!(
        distinct.contains(&0) && distinct.iter().any(|&word| word < 0),
        "both polarities and silence must be represented: {distinct:?}",
    );
}

#[test]
fn a_loud_transient_clips_rather_than_wrapping() {
    // Reachable in normal operation, which is why it is gated rather than assumed away:
    // `frontend::audio`'s DC blocker is a one-pole high-pass and its step response overshoots,
    // so a note starting or stopping can put a sample past full scale even though `mix` itself
    // never exceeds its headroom.
    //
    // A wrap sends the loudest instant of a tune to the opposite rail — heard as a crack, and
    // read by anyone looking at the waveform as a defect in the machine rather than in this
    // encoder. Clipping is the honest failure of the two.
    let encoded = wav::encode(&[2.0, -2.0, 1.5, -1.5, f32::MAX, f32::MIN], DEVICE_HZ)
        .expect("a short capture fits");
    assert_eq!(
        body_words(&encoded),
        vec![32_767, -32_767, 32_767, -32_767, 32_767, -32_767],
    );
}

#[test]
fn a_sample_that_is_not_a_number_becomes_silence_rather_than_a_rail() {
    // `f32` arrives here from a filter, and a filter that has gone unstable produces NaN. The
    // cast alone would answer zero, but only because Rust defines it to; stating it makes the
    // choice deliberate and stops a future clamp-then-cast reordering from turning a numerical
    // failure into a full-scale square wave, which is the loudest possible way to report a bug.
    let encoded = wav::encode(&[f32::NAN, 0.5], DEVICE_HZ).expect("a short capture fits");
    assert_eq!(body_words(&encoded), vec![0, 16_384]);
}

#[test]
fn a_capture_of_nothing_is_still_a_file_a_player_can_open() {
    // The empty case is not hypothetical: `--wav` on a run whose recording window never opens
    // produces it, and a zero-length file, or one whose header promised samples that are not
    // there, would be reported by a player as corrupt rather than as silent.
    let encoded = wav::encode(&[], DEVICE_HZ).expect("nothing fits");
    assert_eq!(encoded.len(), HEADER_BYTES);
    assert_eq!(u32_at(&encoded, 40), 0, "the data chunk declares nothing");
    assert_eq!(
        u32_at(&encoded, 4) as usize,
        HEADER_BYTES - 8,
        "and the RIFF size still counts what is really there",
    );
}

#[test]
fn every_body_the_encoder_accepts_is_one_the_header_can_describe() {
    // The limit is not reachable in a test — it is about twelve hours of audio, and allocating
    // it to watch the check fire would cost four gigabytes to learn nothing. What *is* checkable
    // is the property the limit exists for, which is arithmetic: no accepted body can overflow
    // the `u32` size fields that have to describe it.
    //
    // The failure this forecloses is a silent one. `as u32` on a longer body would write a
    // wrapped length, producing a file whose header disagrees with its contents — the exact
    // defect `the_declared_sizes_describe_the_body_that_is_actually_there` catches, arriving by
    // a route that test cannot reach.
    assert!(
        MAX_SAMPLES
            .checked_mul(BYTES_PER_SAMPLE)
            .expect("no overflow")
            <= u32::MAX as usize
    );
    assert_eq!(MAX_SAMPLES * BYTES_PER_SAMPLE, u32::MAX as usize - 1);
}
