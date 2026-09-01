//! The display file, the attribute file, and turning them into pixels.
//!
//! # The address layout
//!
//! The 6144-byte bitmap is not stored in raster order, and the reason is that the ULA and
//! the character-based ROM routines wanted different orderings and the hardware chose
//! neither. Reading the bits of a line number `y` (0–191) into the address:
//!
//! ```text
//!   bit  15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
//!         0  1  0 y7 y6 y2 y1 y0 y5 y4 y3 x4 x3 x2 x1 x0
//!                 \___/  \______/  \______/  \_________/
//!                 third   pixel     character   column
//!                          row        row
//! ```
//!
//! So consecutive addresses walk *across* the screen, the next 256 bytes are the same
//! character row's next pixel line, and only every eighth line is adjacent. The 768-byte
//! attribute file at `0x5800` is plain 32 × 24 raster order, one byte per character cell.
//!
//! # What this module does not model
//!
//! [`render`] takes the screen as it stands **at the moment it is called**. A real ULA
//! draws the frame progressively, so software that changes attributes, the border, or the
//! bitmap partway down a frame — multicolour effects, Nirvana-engine sprites, border
//! stripes — is drawn here as if the last value had applied all frame.
//!
//! That is a deliberate M5 boundary and not an oversight: drawing progressively needs the
//! frame's write history keyed by T-state, which is a different data structure and a
//! different verification story. `docs/MACHINE.md` puts exactly this software in the
//! "observation" tier, and there is no oracle for it here.

use crate::memory::Memory;

/// Pixels across the display, excluding the border.
pub const DISPLAY_WIDTH: usize = 256;

/// Pixel lines down the display, excluding the border.
pub const DISPLAY_HEIGHT: usize = 192;

/// Pixels along one side of a character cell.
pub const CELL: usize = 8;

/// Character cells across the display.
pub const DISPLAY_COLUMNS: usize = DISPLAY_WIDTH / CELL;

/// Character cells down the display.
pub const DISPLAY_ROWS: usize = DISPLAY_HEIGHT / CELL;

/// Where the bitmap starts.
pub const DISPLAY_FILE: u16 = 0x4000;

/// Bytes in the bitmap.
pub const DISPLAY_FILE_LEN: usize = DISPLAY_COLUMNS * DISPLAY_HEIGHT;

/// Where the attributes start.
pub const ATTRIBUTE_FILE: u16 = 0x5800;

/// Bytes in the attribute file.
pub const ATTRIBUTE_FILE_LEN: usize = DISPLAY_COLUMNS * DISPLAY_ROWS;

const _: () = assert!(DISPLAY_FILE_LEN == 6144);
const _: () = assert!(ATTRIBUTE_FILE_LEN == 768);
const _: () = assert!(DISPLAY_FILE as usize + DISPLAY_FILE_LEN == ATTRIBUTE_FILE as usize);

/// Border pixels rendered on each side of the display.
///
/// The hardware border is not square — it is wider at the sides than it is tall — but a
/// uniform margin is what a frame buffer wants, and nothing in this crate depends on the
/// exact figure.
pub const BORDER: usize = 32;

/// Pixels across a rendered frame, border included.
pub const FRAME_WIDTH: usize = DISPLAY_WIDTH + 2 * BORDER;

/// Pixel lines down a rendered frame, border included.
pub const FRAME_HEIGHT: usize = DISPLAY_HEIGHT + 2 * BORDER;

/// Pixels in a rendered frame.
pub const FRAME_PIXELS: usize = FRAME_WIDTH * FRAME_HEIGHT;

/// Frames each half of the `FLASH` cycle lasts.
pub const FLASH_FRAMES: u64 = 16;

/// One of the sixteen colours the ULA can put on a pixel.
///
/// Eight hues, each in a normal and a bright version. Bright black and black are the same
/// colour on the hardware, and are the same colour here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Colour(u8);

impl Colour {
    /// Colours the ULA can produce.
    pub const COUNT: u8 = 16;

    /// Ink and paper both start here.
    pub const BLACK: Self = Self(0);

    /// The colour `index` names, wrapping into range.
    #[must_use]
    pub const fn new(index: u8) -> Self {
        Self(index % Self::COUNT)
    }

    /// The colour number, 0–15: hue in bits 0–2 and brightness in bit 3.
    #[must_use]
    pub const fn index(self) -> u8 {
        self.0 % Self::COUNT
    }

    /// Whether this is one of the bright half.
    #[must_use]
    pub const fn is_bright(self) -> bool {
        self.index() & 0x08 != 0
    }

    /// The colour as 8-bit RGB.
    ///
    /// The hue bits drive the three guns directly — bit 0 blue, bit 1 red, bit 2 green —
    /// which is why the Spectrum's palette is in that peculiar order rather than a
    /// designer's. Normal is `0xD7` rather than a scaled `0x80`: the difference between the
    /// two brightnesses on real hardware is small.
    #[must_use]
    pub const fn rgb(self) -> [u8; 3] {
        const NORMAL: u8 = 0xD7;
        const BRIGHT: u8 = 0xFF;

        let index = self.index();
        let level = if index & 0x08 == 0 { NORMAL } else { BRIGHT };
        [
            gun(index, 0x02, level),
            gun(index, 0x04, level),
            gun(index, 0x01, level),
        ]
    }
}

/// One colour gun: full on when its hue bit is set, off otherwise.
const fn gun(index: u8, bit: u8, level: u8) -> u8 {
    if index & bit != 0 { level } else { 0 }
}

/// One byte of the attribute file: the two colours of a character cell, and its flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attribute(u8);

impl Attribute {
    const INK: u8 = 0x07;
    const PAPER: u8 = 0x38;
    const BRIGHT: u8 = 0x40;
    const FLASH: u8 = 0x80;

    /// Read an attribute byte.
    #[must_use]
    pub const fn new(byte: u8) -> Self {
        Self(byte)
    }

    /// The byte as stored.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    /// Whether this cell swaps its colours on the `FLASH` cycle.
    #[must_use]
    pub const fn flashes(self) -> bool {
        self.0 & Self::FLASH != 0
    }

    /// The colour a set bit is drawn in, ignoring `FLASH`.
    #[must_use]
    pub const fn ink(self) -> Colour {
        Colour::new((self.0 & Self::INK) | self.brightness())
    }

    /// The colour a clear bit is drawn in, ignoring `FLASH`.
    #[must_use]
    pub const fn paper(self) -> Colour {
        Colour::new(((self.0 & Self::PAPER) >> 3) | self.brightness())
    }

    /// The two colours to draw with, `FLASH` applied.
    ///
    /// `FLASH` swaps ink and paper rather than blinking to black, which is why a flashing
    /// cell stays legible.
    #[must_use]
    pub const fn resolve(self, flash_phase: bool) -> (Colour, Colour) {
        if self.flashes() && flash_phase {
            (self.paper(), self.ink())
        } else {
            (self.ink(), self.paper())
        }
    }

    /// `BRIGHT` as the bit it contributes to a [`Colour`] index.
    const fn brightness(self) -> u8 {
        (self.0 & Self::BRIGHT) >> 3
    }
}

/// Where the bitmap byte for character `column` of pixel line `line` lives.
///
/// `line` is a pixel line, 0–191, and `column` a character column, 0–31. Both are masked,
/// so an out-of-range argument aliases within the display file rather than escaping it.
#[must_use]
pub const fn pixel_address(column: u8, line: u8) -> u16 {
    let line = line as u16;
    let third = (line & 0xC0) << 5;
    let pixel_row = (line & 0x07) << 8;
    let character_row = (line & 0x38) << 2;
    DISPLAY_FILE | third | pixel_row | character_row | (column as u16 & 0x1F)
}

/// Where the attribute byte for character `column` of character `row` lives.
///
/// `row` is a character row, 0–23.
#[must_use]
pub const fn attribute_address(column: u8, row: u8) -> u16 {
    ATTRIBUTE_FILE | ((row as u16 & 0x1F) << 5) | (column as u16 & 0x1F)
}

/// A rendered frame: [`FRAME_WIDTH`] × [`FRAME_HEIGHT`] colour indices, row-major.
///
/// Boxed because it is 80 KB, which is not something to move through a return value or to
/// build on a test thread's stack.
#[derive(Clone, PartialEq, Eq)]
pub struct Frame {
    pixels: Box<[Colour; FRAME_PIXELS]>,
}

impl std::fmt::Debug for Frame {
    /// Deliberately not derived, for the same reason as [`crate::memory::Memory`]: a
    /// derived `Debug` prints 81920 colours and makes any failing assertion unreadable.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame")
            .field("width", &FRAME_WIDTH)
            .field("height", &FRAME_HEIGHT)
            .finish_non_exhaustive()
    }
}

impl Default for Frame {
    fn default() -> Self {
        Self::new()
    }
}

impl Frame {
    /// An all-black frame.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pixels: Box::new([Colour::BLACK; FRAME_PIXELS]),
        }
    }

    /// The colour at `(x, y)`, or `None` outside the frame.
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> Option<Colour> {
        if x >= FRAME_WIDTH || y >= FRAME_HEIGHT {
            return None;
        }
        self.pixels.get(y * FRAME_WIDTH + x).copied()
    }

    /// Every pixel, row-major from the top-left of the border.
    #[must_use]
    pub fn as_slice(&self) -> &[Colour] {
        self.pixels.as_slice()
    }

    /// Paint the whole frame one colour.
    fn fill(&mut self, colour: Colour) {
        self.pixels.fill(colour);
    }

    /// Paint one pixel, in frame coordinates.
    fn set(&mut self, x: usize, y: usize, colour: Colour) {
        // INVARIANT: every caller is a display loop bounded by DISPLAY_WIDTH/HEIGHT offset
        // by BORDER, so the index is within the frame.
        self.pixels[y * FRAME_WIDTH + x] = colour;
    }
}

/// Draw the current screen into `frame`.
///
/// `flash_phase` is the half of the 32-frame `FLASH` cycle the machine is in; see
/// [`flash_phase`].
pub fn render(memory: &Memory, border: Colour, flash_phase: bool, frame: &mut Frame) {
    frame.fill(border);
    let renderer = Renderer {
        memory,
        flash_phase,
    };
    for line in 0..DISPLAY_HEIGHT {
        renderer.line(frame, line);
    }
}

/// Which half of the `FLASH` cycle frame number `frames` falls in.
#[must_use]
pub const fn flash_phase(frames: u64) -> bool {
    (frames / FLASH_FRAMES) % 2 == 1
}

/// The screen and the `FLASH` phase, bundled so the drawing methods stay short.
struct Renderer<'a> {
    memory: &'a Memory,
    flash_phase: bool,
}

impl Renderer<'_> {
    fn line(&self, frame: &mut Frame, line: usize) {
        for column in 0..DISPLAY_COLUMNS {
            self.cell(frame, line, column);
        }
    }

    fn cell(&self, frame: &mut Frame, line: usize, column: usize) {
        let column_byte = column as u8;
        let bits = self.memory.read(pixel_address(column_byte, line as u8));
        let attribute = Attribute::new(
            self.memory
                .read(attribute_address(column_byte, (line / CELL) as u8)),
        );
        let (ink, paper) = attribute.resolve(self.flash_phase);

        let y = BORDER + line;
        let left = BORDER + column * CELL;
        for pixel in 0..CELL {
            let set = bits & (0x80 >> pixel) != 0;
            frame.set(left + pixel, y, if set { ink } else { paper });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::PAGE_SIZE;

    fn blank_memory() -> Memory {
        Memory::spectrum_48k(&[0; PAGE_SIZE]).expect("a page-sized ROM")
    }

    #[test]
    fn the_display_file_addresses_are_the_published_ones() {
        assert_eq!(pixel_address(0, 0), 0x4000);
        assert_eq!(pixel_address(31, 0), 0x401F);
        assert_eq!(pixel_address(0, 1), 0x4100, "the next pixel line is +256");
        assert_eq!(pixel_address(0, 7), 0x4700);
        assert_eq!(pixel_address(0, 8), 0x4020, "the next character row is +32");
        assert_eq!(pixel_address(0, 64), 0x4800, "the second third");
        assert_eq!(pixel_address(0, 128), 0x5000, "the third third");
        assert_eq!(
            pixel_address(31, 191),
            0x57FF,
            "the last byte of the bitmap"
        );
    }

    #[test]
    fn every_bitmap_address_is_used_exactly_once() {
        let mut seen = vec![false; DISPLAY_FILE_LEN];
        for line in 0..DISPLAY_HEIGHT {
            for column in 0..DISPLAY_COLUMNS {
                let address = pixel_address(column as u8, line as u8);
                let offset = usize::from(address - DISPLAY_FILE);
                assert!(
                    offset < DISPLAY_FILE_LEN,
                    "{address:#06X} escaped the bitmap"
                );
                assert!(!seen[offset], "{address:#06X} is used twice");
                seen[offset] = true;
            }
        }
        assert!(seen.into_iter().all(|used| used), "the bitmap has a hole");
    }

    #[test]
    fn the_attribute_file_is_plain_raster_order() {
        assert_eq!(attribute_address(0, 0), 0x5800);
        assert_eq!(attribute_address(31, 0), 0x581F);
        assert_eq!(attribute_address(0, 1), 0x5820);
        assert_eq!(attribute_address(31, 23), 0x5AFF);
    }

    #[test]
    fn every_attribute_address_is_used_exactly_once() {
        let mut seen = vec![false; ATTRIBUTE_FILE_LEN];
        for row in 0..DISPLAY_ROWS {
            for column in 0..DISPLAY_COLUMNS {
                let offset =
                    usize::from(attribute_address(column as u8, row as u8) - ATTRIBUTE_FILE);
                assert!(offset < ATTRIBUTE_FILE_LEN);
                assert!(!seen[offset]);
                seen[offset] = true;
            }
        }
        assert!(seen.into_iter().all(|used| used));
    }

    #[test]
    fn an_attribute_decomposes_into_two_colours() {
        // INK 4 (green), PAPER 2 (red), BRIGHT.
        let attribute = Attribute::new(0x40 | (2 << 3) | 4);
        assert_eq!(attribute.ink(), Colour::new(4 + 8));
        assert_eq!(attribute.paper(), Colour::new(2 + 8));
        assert!(!attribute.flashes());
        assert_eq!(attribute.resolve(true), (Colour::new(12), Colour::new(10)));
    }

    #[test]
    fn flash_swaps_ink_and_paper_only_on_the_second_half_of_the_cycle() {
        let attribute = Attribute::new(0x80 | (2 << 3) | 4);
        assert!(attribute.flashes());
        assert_eq!(attribute.resolve(false), (Colour::new(4), Colour::new(2)));
        assert_eq!(attribute.resolve(true), (Colour::new(2), Colour::new(4)));
    }

    #[test]
    fn the_flash_cycle_is_sixteen_frames_each_way() {
        assert!(!flash_phase(0));
        assert!(!flash_phase(15));
        assert!(flash_phase(16));
        assert!(flash_phase(31));
        assert!(!flash_phase(32));
    }

    #[test]
    fn the_palette_drives_the_guns_in_hardware_order() {
        assert_eq!(Colour::new(0).rgb(), [0, 0, 0], "black");
        assert_eq!(Colour::new(1).rgb(), [0, 0, 0xD7], "blue is bit 0");
        assert_eq!(Colour::new(2).rgb(), [0xD7, 0, 0], "red is bit 1");
        assert_eq!(Colour::new(4).rgb(), [0, 0xD7, 0], "green is bit 2");
        assert_eq!(Colour::new(7).rgb(), [0xD7, 0xD7, 0xD7], "white");
        assert_eq!(Colour::new(15).rgb(), [0xFF, 0xFF, 0xFF], "bright white");
        assert_eq!(Colour::new(8).rgb(), [0, 0, 0], "bright black is black");
    }

    #[test]
    fn an_empty_screen_renders_as_border_around_paper() {
        let mut memory = blank_memory();
        for offset in 0..ATTRIBUTE_FILE_LEN {
            // PAPER 7, INK 0 — what the ROM sets up.
            memory.write(ATTRIBUTE_FILE + offset as u16, 0x38);
        }
        let mut frame = Frame::new();
        render(&memory, Colour::new(2), false, &mut frame);

        assert_eq!(
            frame.pixel(0, 0),
            Some(Colour::new(2)),
            "top-left is border"
        );
        assert_eq!(
            frame.pixel(BORDER, BORDER),
            Some(Colour::new(7)),
            "the display's top-left is paper"
        );
        assert_eq!(
            frame.pixel(FRAME_WIDTH - 1, FRAME_HEIGHT - 1),
            Some(Colour::new(2)),
            "bottom-right is border"
        );
        assert_eq!(frame.pixel(FRAME_WIDTH, 0), None, "off the right edge");
        assert_eq!(frame.pixel(0, FRAME_HEIGHT), None, "off the bottom edge");
    }

    #[test]
    fn a_set_bit_is_drawn_in_ink_at_the_expected_pixel() {
        let mut memory = blank_memory();
        // Top-left character cell: leftmost pixel of the top line, INK 1 on PAPER 6.
        memory.write(pixel_address(0, 0), 0x80);
        memory.write(attribute_address(0, 0), (6 << 3) | 1);
        let mut frame = Frame::new();
        render(&memory, Colour::BLACK, false, &mut frame);

        assert_eq!(frame.pixel(BORDER, BORDER), Some(Colour::new(1)), "ink");
        assert_eq!(
            frame.pixel(BORDER + 1, BORDER),
            Some(Colour::new(6)),
            "paper"
        );
    }

    #[test]
    fn the_third_boundary_is_where_a_linear_layout_would_go_wrong() {
        // Line 64 starts the second third. A raster-ordered emulator puts it at 0x4800
        // too — by accident — so the discriminating case is line 63 against line 64.
        let mut memory = blank_memory();
        memory.write(pixel_address(0, 63), 0xFF);
        memory.write(attribute_address(0, 7), 0x07);
        let mut frame = Frame::new();
        render(&memory, Colour::BLACK, false, &mut frame);

        assert_eq!(frame.pixel(BORDER, BORDER + 63), Some(Colour::new(7)));
        assert_eq!(
            frame.pixel(BORDER, BORDER + 64),
            Some(Colour::BLACK),
            "line 64 must not be aliased by line 63"
        );
    }
}
