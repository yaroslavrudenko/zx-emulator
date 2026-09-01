//! One painted frame, shared by the tests that need pixels.
//!
//! Here rather than copied into each test file because `tests/palette_texture.rs` and
//! `tests/ppm_encoding.rs` must grade **the same buffer** — the second asserts that the P6
//! body is byte-identical to what the first checked, and two independently-written fixtures
//! would make that comparison meaningless.
//!
//! Note that this module holds the *fixture* and never an *expectation*. Every expected
//! colour stays a literal in the test that asserts it; `docs/STATUS.md`'s standing lesson is
//! that a shared helper computing both sides of a comparison is how a suite becomes a
//! tautology, and a fixture that only paints is not that.

#![allow(dead_code)] // Each test binary compiles this module and uses only the part it needs.

use frontend::palette::{self, RGBA_BYTES};
use spectrum::memory::PAGE_SIZE;
use spectrum::screen::{self, ATTRIBUTE_FILE_LEN};
use spectrum::{Colour, Frame, Memory};

/// A frame with ink `ink` on paper `paper`, a border of `border`, and the top-left pixel set,
/// rendered to RGBA through the same path the window uses.
///
/// Deliberately a bare [`Memory`] rather than a [`spectrum::Spectrum`]: the claim under test
/// is that a frame becomes pixels, and involving a CPU would let a future change route the
/// pixels through something else without these tests noticing.
pub fn painted(ink: u8, paper: u8, border: u8) -> Box<[u8; RGBA_BYTES]> {
    let mut memory = Memory::spectrum_48k(&[0; PAGE_SIZE]).expect("a page-sized ROM");
    for offset in 0..ATTRIBUTE_FILE_LEN {
        memory.write(screen::ATTRIBUTE_FILE + offset as u16, (paper << 3) | ink);
    }
    // The leftmost pixel of the top line only, so ink and paper are both on screen.
    memory.write(screen::pixel_address(0, 0), 0x80);

    let mut frame = Frame::new();
    screen::render(&memory, Colour::new(border), false, &mut frame);

    let mut rgba = palette::buffer();
    palette::write_rgba(&frame, &mut rgba);
    rgba
}
