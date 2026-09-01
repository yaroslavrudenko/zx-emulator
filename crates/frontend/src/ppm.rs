//! Netpbm P6, because a debugging aid should not cost a dependency.
//!
//! # Why this format
//!
//! `docs/ARCHITECTURE.md`'s first project rule pins every dependency to a latest stable
//! release *verified against crates.io on a stated date*. A PNG encoder would be a fourth
//! dependency, carrying a compressor and a CRC, bought so that a **debugging aid** could
//! write a file — and it would have to be re-verified at every audit forever.
//!
//! P6 costs nothing: a fifteen-byte ASCII header and then RGB triples, uncompressed, and on
//! macOS `sips -s format png in.ppm --out out.png` converts it. That trade is only wrong if
//! the format cannot express the picture, and it can: the Spectrum has sixteen opaque colours
//! and no transparency, so the alpha channel P6 lacks is the one channel there was never any
//! information in.
//!
//! # This is not a second renderer, and that is asserted rather than intended
//!
//! [`encode`] takes the bytes [`crate::palette::write_rgba`] already produced. It does not
//! see a [`Frame`](spectrum::Frame), cannot reach [`spectrum::Memory`], and has no opinion
//! about colour — it drops a byte from each pixel and prepends a header.
//!
//! That matters because a screenshot produced by code the window does not run would prove
//! nothing about the window. `tests/ppm_encoding.rs` asserts the property directly: the body
//! is byte-identical, pixel for pixel, to the buffer that goes to the texture. Nobody can
//! quietly insert a second rendering path without turning that red.

use spectrum::screen::{FRAME_HEIGHT, FRAME_PIXELS, FRAME_WIDTH};

use crate::palette::{CHANNELS, RGBA_BYTES};

/// Channels in a P6 pixel: red, green, blue. The format has no alpha.
pub const RGB_CHANNELS: usize = 3;

/// The `maxval` a P6 header declares — the value a fully lit gun takes.
pub const MAX_VALUE: u8 = 255;

/// Bytes of pixel data in a frame-sized P6 body.
pub const BODY_BYTES: usize = FRAME_PIXELS * RGB_CHANNELS;

const _: () = assert!(
    RGB_CHANNELS < CHANNELS,
    "P6 drops exactly the alpha channel"
);

/// Encode a frame's RGBA bytes as a binary P6 image.
///
/// `rgba` is [`crate::palette::write_rgba`]'s output and nothing else; the alpha byte of each
/// pixel is dropped, which loses nothing because [`crate::palette::OPAQUE`] is the only value
/// it ever holds.
///
/// The buffer is sized once up front rather than grown, and the pixel loop neither indexes nor
/// allocates.
#[must_use]
pub fn encode(rgba: &[u8; RGBA_BYTES]) -> Vec<u8> {
    let header = format!("P6\n{FRAME_WIDTH} {FRAME_HEIGHT}\n{MAX_VALUE}\n");
    let mut out = Vec::with_capacity(header.len() + BODY_BYTES);
    out.extend_from_slice(header.as_bytes());

    let (pixels, remainder) = rgba.as_chunks::<CHANNELS>();
    debug_assert!(
        remainder.is_empty(),
        "RGBA_BYTES is a whole number of pixels"
    );
    for &[red, green, blue, _alpha] in pixels {
        out.extend_from_slice(&[red, green, blue]);
    }
    out
}
