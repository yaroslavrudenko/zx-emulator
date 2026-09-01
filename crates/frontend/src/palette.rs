//! A rendered [`Frame`] as the bytes a texture wants.
//!
//! # The one hazard here, and it is invisible until somebody looks at the screen
//!
//! The Spectrum's hue bits drive the three guns directly — **bit 0 blue, bit 1 red, bit 2
//! green** — which is why its palette runs black, blue, red, magenta, green, cyan, yellow,
//! white rather than in a designer's order. [`Colour::rgb`] already applies that, and
//! `crates/spectrum`'s own `the_palette_drives_the_guns_in_hardware_order` grades it. This
//! module does not repeat that work and must not: a second copy of the gun order is a second
//! thing to get wrong.
//!
//! What this module *can* get wrong is the step after it. A texture is bytes in a channel
//! order, and half the graphics APIs in existence want `BGRA`. Writing `[b, g, r, a]` into a
//! surface the GPU reads as `RGBA` swaps exactly blue and red — which on a Spectrum turns the
//! sky yellow and nothing panics, nothing fails to compile, and no test in `crates/spectrum`
//! can see it. That is the whole reason `tests/palette_texture.rs` asserts against **literal
//! RGBA quadruples** rather than against [`Colour::rgb`]: comparing this module's output to
//! the function it calls is a tautology, and this project has a name for that.
//!
//! # It cannot re-read memory, and that is a property of the signature
//!
//! [`write_rgba`] takes a `&Frame` and a byte buffer. [`spectrum::Memory`] is not a
//! parameter, is not reachable from a `Frame`, and is not imported — so *"the frame is drawn
//! from the frame"* is not a thing a test has to check on each run. `docs/STATUS.md` prefers
//! that shape wherever it is available, on the grounds that a property of every build beats
//! an observation about one.

use spectrum::Frame;
use spectrum::screen::FRAME_PIXELS;

/// Channels in one texture pixel: red, green, blue, alpha, in that order.
pub const CHANNELS: usize = 4;

/// Bytes one rendered frame occupies as `RGBA8`.
pub const RGBA_BYTES: usize = FRAME_PIXELS * CHANNELS;

/// The alpha every Spectrum pixel carries.
///
/// The machine has no notion of transparency: every pixel it can produce is one of sixteen
/// opaque colours. Named rather than written as a bare `0xFF` at the point of use, because a
/// `0` here is the one value that would make the whole screen vanish while every colour
/// assertion still passed.
pub const OPAQUE: u8 = 0xFF;

const _: () = assert!(RGBA_BYTES == 320 * 256 * 4);
const _: () = assert!(
    RGBA_BYTES.is_multiple_of(CHANNELS),
    "a whole number of pixels"
);

/// Write `frame` into `out` as `RGBA8`, row-major from the top-left of the border.
///
/// Total and infallible: `out` is a fixed-size array, so a caller cannot hand over a buffer
/// of the wrong length and there is no error to return, no bounds check inside the loop, and
/// no panic path in a function that runs 81,920 times a frame.
///
/// `as_chunks_mut` rather than `chunks_exact_mut`, so each pixel arrives as a `[u8; 4]` and
/// the write is a whole-array assignment with no length to compare at run time. The remainder
/// it returns alongside is provably empty — [`RGBA_BYTES`] is a multiple of [`CHANNELS`], and
/// the `const` assertion below says so — which is why discarding it is not a case being
/// dropped. Neither side of the `zip` is indexed, so there is no expression left here that
/// could be out of range.
pub fn write_rgba(frame: &Frame, out: &mut [u8; RGBA_BYTES]) {
    let (pixels, remainder) = out.as_chunks_mut::<CHANNELS>();
    debug_assert!(
        remainder.is_empty(),
        "RGBA_BYTES is a whole number of pixels"
    );

    for (colour, pixel) in frame.as_slice().iter().zip(pixels) {
        let [red, green, blue] = colour.rgb();
        *pixel = [red, green, blue, OPAQUE];
    }
}

/// A zeroed buffer [`write_rgba`] can fill.
///
/// Boxed and built once, for the same reason [`Frame`] is: 320 KB is not something to move
/// through a return value, and allocating one per frame is the one allocation on this
/// crate's per-frame path that would be easy to write by accident.
#[must_use]
pub fn buffer() -> Box<[u8; RGBA_BYTES]> {
    Box::new([0; RGBA_BYTES])
}
