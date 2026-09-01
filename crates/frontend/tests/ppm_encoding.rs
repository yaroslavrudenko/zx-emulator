//! The P6 encoder: its header, its size, and — the one that matters — that its body is the
//! very buffer the window uploads to the texture.
//!
//! # Scope, stated so it is not mistaken for more
//!
//! A screenshot path is a debugging aid, not a machine feature, and this file is sized for
//! that. It grades two things and claims nothing else:
//!
//! - **The file describes the picture it contains** — a well-formed P6 header whose declared
//!   dimensions match the frame, and a body of exactly the right length. A viewer that reads
//!   the header and then runs off the end of the data is the failure this catches.
//! - **There is only one renderer.** The body is asserted byte-identical, pixel for pixel, to
//!   [`palette::write_rgba`]'s output. A screenshot produced by code the window does not run
//!   would prove nothing about the window, so nobody may quietly insert a second rendering
//!   path — doing so turns `the_body_is_the_same_buffer_the_window_uploads` red.
//!
//! It does **not** grade whether the picture is correct; that is `tests/palette_texture.rs`,
//! against the same fixture. Nor does it grade that any viewer can open the result — that was
//! observation, done once with `sips`, and recorded in the report rather than asserted here.

mod common;

use frontend::palette::{CHANNELS, RGBA_BYTES};
use frontend::ppm::{self, BODY_BYTES, MAX_VALUE, RGB_CHANNELS};
use spectrum::screen::{BORDER, FRAME_HEIGHT, FRAME_PIXELS, FRAME_WIDTH};

/// The header, written out by hand rather than asked of the subject.
///
/// A literal, so the body offset below owes the encoder nothing — if the header changed
/// length, slicing past `ppm::encode`'s own idea of it would silently keep working while
/// every viewer broke.
const HEADER: &[u8] = b"P6\n320 256\n255\n";

/// Normal-brightness full gun, from the hardware. Not imported from `palette`; see
/// `tests/palette_texture.rs` for the mutation that made that distinction load-bearing.
const NORMAL: u8 = 0xD7;

#[test]
fn the_frame_this_file_was_written_against_is_still_the_frame() {
    // A positive control on the premise. `HEADER` is arithmetic on 320 x 256; if
    // `crates/spectrum` changed the frame, the literal would be wrong and every failure below
    // would read as an encoder defect. This one fails first and says what actually moved.
    assert_eq!((FRAME_WIDTH, FRAME_HEIGHT), (320, 256));
    assert_eq!(MAX_VALUE, 255);
    assert_eq!(RGB_CHANNELS, 3);
}

#[test]
fn the_header_is_a_well_formed_p6_declaring_the_frames_own_size() {
    let encoded = ppm::encode(&common::painted(2, 1, 4));
    assert!(
        encoded.starts_with(HEADER),
        "header was {:?}",
        String::from_utf8_lossy(&encoded[..HEADER.len().min(encoded.len())]),
    );

    // Parsed back out of the bytes rather than compared to the literal a second time, so this
    // asserts the *declared* geometry is the frame's and not merely that two spellings match.
    let text = String::from_utf8(HEADER.to_vec()).expect("ASCII header");
    let mut fields = text.split_ascii_whitespace();
    assert_eq!(fields.next(), Some("P6"));
    assert_eq!(fields.next(), Some(FRAME_WIDTH.to_string().as_str()));
    assert_eq!(fields.next(), Some(FRAME_HEIGHT.to_string().as_str()));
    assert_eq!(fields.next(), Some(MAX_VALUE.to_string().as_str()));
    assert_eq!(fields.next(), None);
}

#[test]
fn the_file_is_exactly_a_header_and_one_pixel_per_pixel() {
    let encoded = ppm::encode(&common::painted(2, 1, 4));
    assert_eq!(BODY_BYTES, FRAME_PIXELS * RGB_CHANNELS);
    assert_eq!(BODY_BYTES, 320 * 256 * 3);
    assert_eq!(
        encoded.len(),
        HEADER.len() + BODY_BYTES,
        "a body of the wrong length runs a viewer off the end of the data",
    );
}

#[test]
fn the_body_is_the_same_buffer_the_window_uploads() {
    // THE gate. Two artefacts, one derived from the other: every P6 triple must be the first
    // three bytes of the corresponding RGBA quadruple, in order. This is what makes the
    // screenshot evidence about the window rather than about the screenshot tool — a second
    // renderer, a transposed row, a flipped image or a reordered channel all turn it red.
    let rgba = common::painted(2, 1, 4);
    let encoded = ppm::encode(&rgba);
    let body = &encoded[HEADER.len()..];

    let (pixels, rgba_remainder) = rgba.as_chunks::<CHANNELS>();
    let (triples, body_remainder) = body.as_chunks::<RGB_CHANNELS>();
    assert!(rgba_remainder.is_empty() && body_remainder.is_empty());
    assert_eq!(pixels.len(), FRAME_PIXELS);

    let mut compared = 0_usize;
    for (index, (&[red, green, blue, _alpha], &triple)) in pixels.iter().zip(triples).enumerate() {
        assert_eq!(triple, [red, green, blue], "pixel {index}");
        compared += 1;
    }
    // Without this, a body of zero length would pass the loop by never entering it — the
    // "count of zero and an absence of the subject are the same observation" failure
    // `docs/STATUS.md` records against the codegen gate.
    assert_eq!(
        compared, FRAME_PIXELS,
        "every pixel must have been compared"
    );
}

#[test]
fn the_alpha_channel_is_the_only_thing_dropped() {
    // P6 carries no alpha, and the Spectrum has no transparency, so exactly one byte per
    // pixel goes and no information does. Stated as arithmetic so a change to either constant
    // has to come past it.
    assert_eq!(RGBA_BYTES - BODY_BYTES, FRAME_PIXELS);
    assert_eq!(CHANNELS - RGB_CHANNELS, 1);
}

#[test]
fn known_pixels_come_out_as_the_hardware_colours_they_should_be() {
    // Literal RGB, from the guns: bit 0 blue, bit 1 red, bit 2 green. Red and blue are both
    // present because they are the pair a channel swap exchanges, and the border is green so
    // no permutation of three channels passes by accident.
    let encoded = ppm::encode(&common::painted(2, 1, 4));
    let body = &encoded[HEADER.len()..];
    let pixel = |x: usize, y: usize| {
        let start = (y * FRAME_WIDTH + x) * RGB_CHANNELS;
        [body[start], body[start + 1], body[start + 2]]
    };

    assert_eq!(pixel(0, 0), [0, NORMAL, 0], "border is GREEN");
    assert_eq!(
        pixel(BORDER, BORDER),
        [NORMAL, 0, 0],
        "the set bit is RED ink"
    );
    assert_eq!(
        pixel(BORDER + 1, BORDER),
        [0, 0, NORMAL],
        "the clear bit is BLUE paper",
    );
}

#[test]
fn the_fixture_actually_puts_more_than_one_colour_in_the_file() {
    // A positive control: if `render` stopped drawing, every assertion above that happens to
    // expect a zero would still pass against an all-black file.
    let encoded = ppm::encode(&common::painted(2, 1, 4));
    let distinct: std::collections::BTreeSet<[u8; RGB_CHANNELS]> = encoded[HEADER.len()..]
        .as_chunks::<RGB_CHANNELS>()
        .0
        .iter()
        .copied()
        .collect();
    assert_eq!(
        distinct.len(),
        3,
        "border, ink and paper — got {distinct:?}"
    );
}
