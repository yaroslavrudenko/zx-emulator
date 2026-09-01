//! The texture's bytes, against literal `RGBA` quadruples.
//!
//! # The defect this is aimed at
//!
//! Half the graphics APIs in existence want `BGRA`. Writing `[b, g, r, a]` into a surface the
//! GPU reads as `RGBA` swaps **exactly blue and red** — the Spectrum's colours 1 and 2, and
//! 9 and 10. Nothing panics, nothing fails to compile, and no test in `crates/spectrum` can
//! see it, because `Colour::rgb` is still returning the right triple; only the packing is
//! wrong. What a person sees is a yellow sky.
//!
//! So every expectation here is a **literal quadruple**, and blue and red are both on the
//! screen in the same frame. Comparing `write_rgba`'s output against `Colour::rgb` — the
//! function it calls — would be the tautology `docs/STATUS.md` keeps recording: it would pass
//! under any swap that both halves shared.
//!
//! The literals are derived from the hardware, not from this repository: the hue bits drive
//! the guns as **bit 0 blue, bit 1 red, bit 2 green**, and the two brightness levels are
//! `0xD7` and `0xFF`.
//!
//! # What this cannot see
//!
//! Whether the screen *looks* right. There is no reference image here and there should not
//! be one — an image rendered by this crate and committed as its own expectation is the same
//! tautology in a larger package. Colour correctness belongs to `crates/spectrum`'s
//! `the_palette_drives_the_guns_in_hardware_order`; this file grades only the step after it.

mod common;

use common::painted;
use frontend::palette::{self, CHANNELS, RGBA_BYTES};
use spectrum::screen::{BORDER, FRAME_HEIGHT, FRAME_PIXELS, FRAME_WIDTH};

/// Normal-brightness full gun, from the hardware.
const NORMAL: u8 = 0xD7;

/// Bright full gun.
const BRIGHT: u8 = 0xFF;

/// The alpha every pixel must carry — a literal, and **deliberately not**
/// [`frontend::palette::OPAQUE`].
///
/// It was `palette::OPAQUE` until the mutation run, and that made the alpha channel of every
/// assertion in this file worthless: setting `OPAQUE = 0x00` moved the written value and the
/// expected value together, and all six tests here stayed **green** while the whole screen
/// would have been invisible. `every_pixel_is_opaque` was the worst of them, because its entire
/// subject is the alpha.
///
/// That is `docs/STATUS.md`'s *The keyboard matrix was graded against itself* — *"a test whose
/// expectation is computed by the subject is not a weak test; it is a tautology with a cross
/// product attached"* — committed inside the file whose own header cites it. It was found by
/// mutation and not by reading, which is the argument for running mutations against a gate you
/// have just finished being careful about.
const OPAQUE: u8 = 0xFF;

/// The `RGBA` bytes of the pixel at `(x, y)`.
fn pixel_at(rgba: &[u8; RGBA_BYTES], x: usize, y: usize) -> [u8; CHANNELS] {
    let start = (y * FRAME_WIDTH + x) * CHANNELS;
    let mut bytes = [0; CHANNELS];
    bytes.copy_from_slice(&rgba[start..start + CHANNELS]);
    bytes
}

#[test]
fn blue_and_red_do_not_swap_on_the_way_to_the_texture() {
    // The discriminating case, and the whole reason this file exists. INK 2 is red, PAPER 1
    // is blue, and the border is green — three colours whose three quadruples are distinct
    // under every permutation of the channels, so no channel order but the right one passes.
    let rgba = painted(2, 1, 4);

    assert_eq!(
        pixel_at(&rgba, 0, 0),
        [0, NORMAL, 0, OPAQUE],
        "the border is GREEN, which is gun bit 2",
    );
    assert_eq!(
        pixel_at(&rgba, BORDER, BORDER),
        [NORMAL, 0, 0, OPAQUE],
        "the set bit is INK 2, which is RED, which is gun bit 1",
    );
    assert_eq!(
        pixel_at(&rgba, BORDER + 1, BORDER),
        [0, 0, NORMAL, OPAQUE],
        "the clear bit is PAPER 1, which is BLUE, which is gun bit 0 — \
         and this is the assertion a BGRA packing fails",
    );
}

#[test]
fn the_eight_hues_come_out_in_hardware_gun_order() {
    // Every hue, as a border, against literals. Bit 0 blue, bit 1 red, bit 2 green.
    let expected: [(u8, [u8; 3]); 8] = [
        (0, [0, 0, 0]),                // black
        (1, [0, 0, NORMAL]),           // blue
        (2, [NORMAL, 0, 0]),           // red
        (3, [NORMAL, 0, NORMAL]),      // magenta
        (4, [0, NORMAL, 0]),           // green
        (5, [0, NORMAL, NORMAL]),      // cyan
        (6, [NORMAL, NORMAL, 0]),      // yellow
        (7, [NORMAL, NORMAL, NORMAL]), // white
    ];
    for (index, [red, green, blue]) in expected {
        let rgba = painted(0, 0, index);
        assert_eq!(
            pixel_at(&rgba, 0, 0),
            [red, green, blue, OPAQUE],
            "colour {index}",
        );
    }
}

#[test]
fn the_bright_half_raises_every_lit_gun_and_leaves_black_alone() {
    let cases: [(u8, [u8; 3]); 4] = [
        (8, [0, 0, 0]),                 // bright black is black on the hardware
        (9, [0, 0, BRIGHT]),            // bright blue
        (10, [BRIGHT, 0, 0]),           // bright red
        (15, [BRIGHT, BRIGHT, BRIGHT]), // bright white
    ];
    for (index, [red, green, blue]) in cases {
        let rgba = painted(0, 0, index);
        assert_eq!(
            pixel_at(&rgba, 0, 0),
            [red, green, blue, OPAQUE],
            "colour {index}"
        );
    }
}

#[test]
fn every_pixel_is_opaque() {
    // A zero alpha is the one mistake that leaves every colour assertion above passing and
    // the window empty.
    let rgba = painted(2, 1, 4);
    let (pixels, _) = rgba.as_chunks::<CHANNELS>();
    let transparent = pixels
        .iter()
        .filter(|&&[_, _, _, alpha]| alpha != OPAQUE)
        .count();
    assert_eq!(transparent, 0, "{transparent} pixels are not opaque");
    assert_eq!(pixels.len(), FRAME_PIXELS, "every pixel was examined");
}

#[test]
fn the_buffer_is_exactly_one_frame_of_rgba() {
    assert_eq!(RGBA_BYTES, FRAME_PIXELS * CHANNELS);
    assert_eq!(
        palette::buffer().len(),
        FRAME_WIDTH * FRAME_HEIGHT * CHANNELS
    );
}

#[test]
fn the_fixture_actually_puts_more_than_one_colour_on_the_screen() {
    // A positive control, in `docs/STATUS.md`'s sense: an assertion whose failure means "I was
    // not looking at the thing". If `render` stopped drawing, or the fixture stopped setting
    // attributes, every assertion above would compare an all-black buffer against expectations
    // that happen to include black — and a count of zero and an absence of the subject are the
    // same observation.
    let rgba = painted(2, 1, 4);
    let distinct: std::collections::BTreeSet<[u8; CHANNELS]> = (0..FRAME_HEIGHT)
        .flat_map(|y| (0..FRAME_WIDTH).map(move |x| (x, y)))
        .map(|(x, y)| pixel_at(&rgba, x, y))
        .collect();
    assert_eq!(
        distinct.len(),
        3,
        "the fixture should paint border, ink and paper and nothing else, not {distinct:?}",
    );
}
