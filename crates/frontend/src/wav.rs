//! RIFF/WAVE, because a debugging aid should not cost a dependency.
//!
//! # Why this format, and why by hand
//!
//! This is [`crate::ppm`]'s argument, transposed from pixels to samples, and it lands the same
//! way. `docs/ARCHITECTURE.md`'s first project rule pins every dependency to a latest stable
//! release *verified against crates.io on a stated date*; an encoder crate would be another one,
//! carrying a format abstraction and a codec registry, bought so that a **debugging aid** could
//! write a file — and it would have to be re-verified at every audit forever.
//!
//! A WAV header is **44 bytes** and the body is little-endian PCM. That is the whole format for
//! the one case here: one channel, one rate, no chunks beyond `fmt ` and `data`. Every desktop
//! player, `afplay`, QuickTime, `ffprobe` and every browser reads it.
//!
//! The trade is only wrong if the format cannot express the signal, and it can. The machine
//! produces a single mixed level per output sample — [`crate::audio::mix`] sums four sources
//! into one — so the channel this format would add is the one there was never any information
//! in, exactly as P6's missing alpha was.
//!
//! # This is not a second audio path, and that is asserted rather than intended
//!
//! [`encode`] takes the `f32` levels [`crate::audio::Resampler::feed`] already produced. It does
//! not see a [`Sample`](spectrum::Sample), cannot reach a [`Spectrum`](spectrum::Spectrum), holds
//! no filter state, and has no opinion about mixing, rate conversion or DC — it scales each level
//! to an integer and prepends a header.
//!
//! That matters for the reason the P6 encoder gives and it matters more here, because a sound is
//! harder to check by eye than a picture: **a capture produced by code the speaker does not run
//! would prove nothing about the speaker.** `tests/wav_encoding.rs` asserts the property
//! directly — every 16-bit word in the body is the quantisation of the corresponding `f32` the
//! mixer emitted, in order, with a counter so that an empty body cannot pass by never entering
//! the loop. Nobody can quietly insert a second mixer, a second resampler, or a helpful filter
//! without turning `the_body_is_the_samples_the_mixer_emitted` red.
//!
//! # What a green here does not say
//!
//! **That anyone heard it.** [`crate::audio`]'s own table already refuses that claim for the
//! buffer, and writing the buffer to a file does not upgrade it: this environment has no audio
//! device and no way to capture one. What the gates below establish is that the file is
//! well-formed, that its header describes the body it actually contains, and that the body is
//! the mixer's output and not something adjacent to it. Whether the tune is the right tune, at
//! the right pitch, played at the right speed, remains a claim only a person with speakers can
//! make.

/// Bytes in the header this module writes.
///
/// The canonical minimal WAVE: a 12-byte `RIFF` descriptor, a 24-byte `fmt ` chunk, and an
/// 8-byte `data` header. Sized as a constant rather than measured from the output, so that a
/// reader slicing past it owes [`encode`] nothing — the mistake [`crate::ppm`]'s gate guards
/// against by writing its header out as a literal.
pub const HEADER_BYTES: usize = 44;

/// Channels written. One: [`crate::audio::mix`] has already summed the machine's four sources
/// into a single level, so a second channel would carry a copy rather than information.
pub const CHANNELS: u16 = 1;

/// Bits in one sample of the body.
pub const BITS_PER_SAMPLE: u16 = 16;

/// Bytes in one sample of the body, across every channel.
pub const BYTES_PER_SAMPLE: usize = (BITS_PER_SAMPLE as usize / 8) * CHANNELS as usize;

/// The `wFormatTag` that means uncompressed integer PCM.
pub const FORMAT_PCM: u16 = 1;

/// Bytes in the `fmt ` chunk's body, which is what [`FORMAT_PCM`] fixes at sixteen.
pub const FMT_CHUNK_BYTES: u32 = 16;

/// The largest level [`encode`] maps a sample onto, and the negative of the smallest.
///
/// [`i16::MAX`] rather than 32,768, so the scale is **symmetric**: `+1.0` and `-1.0` become
/// equal and opposite. Spending the one extra code the two's-complement range offers on the
/// negative side would make a symmetric input come out lopsided by one part in 32,768 — inaudible,
/// and a needless asymmetry in the one arithmetic step this module performs.
const FULL_SCALE: f32 = i16::MAX as f32;

/// The most samples a body may hold before its size stops fitting the header.
///
/// # Why this is an error and not a cast
///
/// Every size in a RIFF file is a `u32`. A body longer than this cannot be *described* by the
/// header, and `as u32` would silently write a wrapped length — a file whose header says one
/// thing and whose body is another, which is precisely the failure this module's gate exists to
/// catch. So it is refused instead.
///
/// It is roughly twelve hours at 48 kHz, so nothing this tool produces will meet it. Unreachable
/// is not the same as impossible, and the check costs one comparison per capture rather than one
/// per sample.
///
/// # This limit is eighteen samples too generous, and at the limit `encode` aborts
///
/// **The check is against the body and the RIFF size field describes the file.** At exactly
/// `MAX_SAMPLES` the body is `u32::MAX - 1` bytes, and `encode` then writes
/// `data_bytes + (HEADER_BYTES - 8)` — thirty-six more — which overflows a `u32`. This
/// workspace sets `overflow-checks = true` in **release** as well as debug, and `panic = "abort"`,
/// so that is a **process abort in shipped code**, in the one module whose whole argument is that
/// `as u32` is refused because it *"would silently write a wrapped length"*. The paragraph beside
/// the cast in `encode` states the thirty-six bytes and then does not subtract them.
///
/// The correct value is `(u32::MAX as usize - (HEADER_BYTES - 8)) / BYTES_PER_SAMPLE` —
/// `2_147_483_629`, eighteen fewer — and the change is one line with no LOC delta. **It is not
/// applied here because this is a `pub const` and its value is part of the published contract**,
/// so it needs the owner's sign-off rather than a reviewer's. Reaching the defect needs a slice
/// of about 8.6 GiB of `f32`, roughly 12.4 hours at 48 kHz, so deferring it is safe; leaving the
/// doc silent about it would not be.
pub const MAX_SAMPLES: usize = u32::MAX as usize / BYTES_PER_SAMPLE;

const _: () = assert!(
    HEADER_BYTES == 12 + 8 + FMT_CHUNK_BYTES as usize + 8,
    "RIFF descriptor, fmt header and body, data header"
);

/// Why samples could not be encoded.
///
/// One variant, and it is `#[non_exhaustive]` for the same reason [`crate::host::SaveError`] is:
/// a second failure mode should not be a breaking change to everyone who matched on this.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EncodeError {
    /// More samples than a `u32` size field can describe. See [`MAX_SAMPLES`].
    #[error("{samples} samples is more than the {MAX_SAMPLES} a RIFF size field can describe")]
    TooLong {
        /// How many were offered.
        samples: usize,
    },
}

/// One mixed level as a body word.
///
/// Clamped, **not** wrapped and not left to a cast's own idea of range. A `f32` outside `±1.0`
/// is reachable in normal operation: [`crate::audio`]'s DC blocker is a one-pole high-pass whose
/// step response overshoots, so a loud edge — every time a game starts or stops a note — can put
/// a transient past full scale even though [`crate::audio::mix`] itself never exceeds its
/// headroom. Wrapping such a sample turns the loudest instant of a tune into its opposite, which
/// is heard as a crack rather than as clipping and is the more misleading of the two failures.
///
/// Private on purpose. `tests/wav_encoding.rs` writes this arithmetic out again by hand rather
/// than importing it, so the gate compares the body against a rule stated independently instead
/// of against the function that produced it — the distinction `tests/ppm_encoding.rs` records as
/// load-bearing where it keeps `NORMAL` as a literal.
fn quantise(level: f32) -> i16 {
    // `clamp` before the multiply, so the product cannot leave `f32`'s exactly-representable
    // integer range on the way. NaN cannot survive it either: `f32::clamp` panics on a NaN
    // bound but returns NaN for a NaN input, so it is mapped explicitly.
    if level.is_nan() {
        return 0;
    }
    (level.clamp(-1.0, 1.0) * FULL_SCALE).round() as i16
}

/// Encode mixed levels as a mono 16-bit PCM WAVE file at `device_hz`.
///
/// `samples` is [`crate::audio::Resampler::feed`]'s output and nothing else — already mixed,
/// already resampled to `device_hz`, already DC-blocked. This function adds no processing of any
/// kind beyond the scale to integer that the format requires.
///
/// The buffer is sized once up front rather than grown, and the sample loop neither indexes nor
/// allocates.
///
/// # Errors
///
/// [`EncodeError::TooLong`] when the body would be larger than a RIFF size field can describe.
/// See [`MAX_SAMPLES`].
pub fn encode(samples: &[f32], device_hz: u32) -> Result<Vec<u8>, EncodeError> {
    if samples.len() > MAX_SAMPLES {
        return Err(EncodeError::TooLong {
            samples: samples.len(),
        });
    }
    // `data_bytes` is bounded by the check above and cannot exceed `u32::MAX`.
    //
    // **The RIFF size below adds the 36 bytes that follow it, and the check above does not
    // subtract them** — so at exactly `MAX_SAMPLES` that addition overflows and this function
    // aborts. See [`MAX_SAMPLES`]'s own doc: the limit is eighteen samples too generous, the fix
    // is one line, and it changes a published constant's value so it waits for sign-off. This
    // comment used to end *"which is why the limit is taken against the body rather than against
    // the file"*, presenting the gap as the reason for the design rather than as the defect in
    // it.
    let data_bytes = (samples.len() * BYTES_PER_SAMPLE) as u32;
    let block_align = BYTES_PER_SAMPLE as u16;
    let byte_rate = device_hz.saturating_mul(u32::from(block_align));

    let mut out = Vec::with_capacity(HEADER_BYTES + data_bytes as usize);
    out.extend_from_slice(b"RIFF");
    // Everything after this field: the eight bytes of `WAVE` plus the two chunk headers, plus
    // the fmt body, plus the samples. `HEADER_BYTES - 8` is that total by construction rather
    // than by a second literal that could drift from it.
    out.extend_from_slice(&(data_bytes + (HEADER_BYTES - 8) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");

    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&FMT_CHUNK_BYTES.to_le_bytes());
    out.extend_from_slice(&FORMAT_PCM.to_le_bytes());
    out.extend_from_slice(&CHANNELS.to_le_bytes());
    out.extend_from_slice(&device_hz.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());

    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes.to_le_bytes());
    for &level in samples {
        out.extend_from_slice(&quantise(level).to_le_bytes());
    }
    Ok(out)
}
