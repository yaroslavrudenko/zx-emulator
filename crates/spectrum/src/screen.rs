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
//! # The ULA reads a **bank**, not an address
//!
//! Every address in this module — [`DISPLAY_FILE`], [`ATTRIBUTE_FILE`], everything
//! [`pixel_address`] returns — is a true address at which the screen bank appears, and they
//! are unchanged by M7. But the byte the ULA actually latches does **not** come through the
//! slot map: the chip is wired to bank 5 or bank 7 directly, chosen by bit 3 of `0x7FFD`.
//!
//! On a 48K the distinction is invisible, because bank 5 is nailed to `0x4000` and there is no
//! bit 3. On a 128 it is the difference between a working shadow screen and none: bank 7 is
//! usually paged into no slot at all, so a renderer that read `Memory::read(0x4000)` would draw
//! whatever happens to be at `0x4000` — which is bank 5, the screen the program just switched
//! *away* from. Double-buffering would show the buffer being drawn into, every frame.
//!
//! So [`render`] resolves the bank once, through [`Memory::screen_bank`], and indexes it.
//! **The public signature does not move and no public constant does**, because they are still
//! the addresses at which bank 5 appears; what changed is one indirection inside the function.
//!
//! # What this module does not model
//!
//! [`render`] takes the screen as it stands **at the moment it is called**. A real ULA
//! draws the frame progressively, so software that changes attributes or the bitmap partway
//! down a frame — multicolour effects, Nirvana-engine sprites — is drawn here as if the last
//! value had applied all frame.
//!
//! That is a deliberate M5 boundary and not an oversight: drawing progressively needs the
//! frame's write history keyed by T-state, which is a different data structure and a
//! different verification story. `docs/MACHINE.md` puts exactly this software in the
//! "observation" tier, and there is no oracle for it here.
//!
//! ## The **border** is now the exception, and the boundary moved for one reason
//!
//! This paragraph listed *"border stripes"* alongside the other two, and that is no longer
//! true. The border **is** drawn as the beam painted it, row by row, from
//! `BorderTrace` — because a tape load is the one place a mid-frame write is what a person
//! is looking at, and a loading screen drawn in one colour is visibly wrong rather than
//! subtly so. Nothing else about the boundary moved: attributes and the bitmap are still
//! sampled once.
//!
//! It is worth being exact about how much that is. The border's history costs **one slot per
//! rendered row** and needs no event list, because a guest cannot create rows; the bitmap's
//! would need a write history keyed by T-state over 6912 bytes, which is the different data
//! structure the paragraph above is about. **The two are not the same problem and the cheap
//! half being done is not the expensive half being started.**
//!
//! **The shadow screen does not change that and slightly reduces the pressure for it.** A 128
//! that can double-buffer has less reason to reach for a mid-frame trick, so M7 leaves the
//! boundary where M5 put it — but a *mid-frame* switch of bit 3 is drawn here as if the last
//! value had applied all frame, exactly as a mid-frame attribute write is.

use core::fmt;

use crate::memory::{Memory, PAGE_SIZE};
use crate::timing::{Clock, Timing};

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

    /// Bit 3 of the colour byte: the brightness the other three bits are drawn at.
    const BRIGHT_BIT: u8 = 0x08;

    /// Whether this is one of the bright half.
    ///
    /// Nothing outside this file called this for a milestone while [`Colour::rgb`] open-coded
    /// the same `& 0x08` two methods below — a published item with no consumer beside a copy of
    /// its body. It is **kept rather than deleted**, because deleting a `pub` item is a semver
    /// event and this one names a real property of the byte; `rgb` calls it now, so the mask
    /// lives in one place and the item has a consumer.
    #[must_use]
    pub const fn is_bright(self) -> bool {
        self.index() & Self::BRIGHT_BIT != 0
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
        let level = if self.is_bright() { BRIGHT } else { NORMAL };
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
///
/// The line is reduced modulo [`DISPLAY_HEIGHT`] rather than masked, and that is not a
/// stylistic choice. The line's two high bits select one of *three* thirds and the fourth
/// combination does not exist, so there is no mask that both leaves every valid line alone
/// and folds the invalid quarter inside: `line & 0xBF` looks right, and silently maps line
/// 64 — the first line of the second third — to line 0.
///
/// Without the reduction, `pixel_address(0, 192)` returned `0x5800`, the attribute file, and
/// 2048 of the 8192 `(column, line)` pairs escaped the display file entirely while this
/// comment claimed they could not. A `debug_assert!` would be the wrong fix: this is called
/// 6144 times a frame, and adding a panic path to a rendering primitive is a larger change
/// than making the documented masking true.
#[must_use]
pub const fn pixel_address(column: u8, line: u8) -> u16 {
    let line = line as u16 % DISPLAY_HEIGHT as u16;
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

/// Where the ROM's character set starts — the glyph for character 32, a space.
///
/// The `CHARS` system variable holds `0x3C00`, which is this base *minus* 256: the font has
/// no glyphs below code 32. Fixed here rather than read from `CHARS`, so that reading the
/// screen back does not depend on a system variable having been initialised — and so that it
/// reports what the ROM's own font draws even for a program that has repointed `CHARS`.
///
/// # On a 128 this address is a 48K assumption, and [`read_text`] inherits it
///
/// The font is read *through the slot map*, so it comes from whichever ROM page is selected.
/// That is unambiguous on a 48K, which has one. **The 128's editor ROM does not hold a
/// character set here** — measured rather than assumed: its bytes at `0x3D00` are
/// `3F C1 38 79 2A 7F FD 7C` where a space would be eight zeros. The set lives in the 48 BASIC
/// ROM, page 1.
///
/// The consequence is not subtle and is worth stating where somebody will meet it: the 128's
/// menu loop pages ROM 1 in and out **every frame**, so [`read_text`] on a booted 128 returns
/// the screen or a wall of `?` depending on which frame it is called in. Neither answer is a
/// defect — the function does exactly what it documents — but it is a 48K instrument pointed at
/// a two-ROM machine, and a gate that asserted on its output would be flaky for a reason that
/// has nothing to do with the screen.
///
/// `crates/spectrum/tests/m7_common/mod.rs` therefore reads the screen against a font taken from
/// an explicit ROM image, which removes the dependency and makes the expectation independent of
/// the machine under test. **This is left as a documented limitation rather than fixed**: making
/// `read_text` search both ROM pages would give it a rule no hardware has, and reading `CHARS`
/// would reintroduce the dependency this constant exists to remove.
const FONT: u16 = 0x3D00;

/// Bytes in one glyph.
const GLYPH_BYTES: u16 = 8;

/// The lowest character code the ROM's font covers.
const FIRST_CHARACTER: u8 = 32;

/// The highest character code the ROM's font covers.
const LAST_CHARACTER: u8 = 127;

/// The display file read back as [`DISPLAY_ROWS`] lines of [`DISPLAY_COLUMNS`] characters.
///
/// Each cell's eight bitmap bytes are matched against the glyphs of the character set **in
/// `memory` itself**, so the expected bitmaps come from the machine under test rather than
/// from a font table written here. That is the whole point: a subtly wrong screen address
/// layout produces cells that match no glyph, and those read as `?` rather than quietly
/// resolving to a plausible letter. An all-clear cell reads as a space.
///
/// This is a debugging and gating view of the screen, not a rendering path — [`render`] is
/// what draws pixels. It exists as crate API because the boot example and the boot gate both
/// need it and an example cannot share code with a test.
#[must_use]
pub fn read_text(memory: &Memory) -> Vec<String> {
    // The font is read once rather than per cell: the same 96 glyphs were previously
    // re-fetched for every one of the 768 cells, which is 590,000 memory reads per call.
    //
    // It comes through `Memory::read` and not through the screen bank, and that is not an
    // inconsistency with the cells below: the font is ROM, so it is whichever ROM the slot map
    // currently shows — which on a 128 running 48 BASIC is the second page.
    let font: Vec<(char, [u8; GLYPH_BYTES as usize])> = (FIRST_CHARACTER..=LAST_CHARACTER)
        .map(|code| (decode(code), glyph(memory, code)))
        .collect();

    let screen = screen_page(memory);
    (0..DISPLAY_ROWS)
        .map(|row| {
            (0..DISPLAY_COLUMNS)
                .map(|column| {
                    let cell = read_cell(screen, column, row);
                    if cell == [0; GLYPH_BYTES as usize] {
                        return ' ';
                    }
                    font.iter()
                        .find(|(_, bitmap)| *bitmap == cell)
                        .map_or('?', |(character, _)| *character)
                })
                .collect()
        })
        .collect()
}

/// The page the ULA is currently drawing from.
///
/// The one place this module turns "which bank" into "which bytes", so [`render`] and
/// [`read_text`] cannot come to disagree about what is on screen.
fn screen_page(memory: &Memory) -> &[u8; PAGE_SIZE] {
    memory.bank(memory.screen_bank())
}

/// Where in the screen bank the byte at `address` lives.
///
/// The screen bank appears at `0x4000` when it is paged there at all, so the offset is the
/// address's low fourteen bits. Masked rather than subtracted: the mask makes the result
/// provably in range for a `[u8; PAGE_SIZE]`, so the index cannot panic and its bounds check
/// is elided — and unlike a subtraction it has no wrong answer to give for an address outside
/// the display file.
#[inline]
const fn screen_offset(address: u16) -> usize {
    (address as usize) & (PAGE_SIZE - 1)
}

/// The eight bitmap bytes of one character cell, out of the page the ULA is drawing.
fn read_cell(screen: &[u8; PAGE_SIZE], column: usize, row: usize) -> [u8; GLYPH_BYTES as usize] {
    let mut cell = [0; GLYPH_BYTES as usize];
    for (line, byte) in cell.iter_mut().enumerate() {
        let pixel_line = (row * CELL + line) as u8;
        *byte = screen[screen_offset(pixel_address(column as u8, pixel_line))];
    }
    cell
}

/// The ROM's own glyph for `code`.
fn glyph(memory: &Memory, code: u8) -> [u8; GLYPH_BYTES as usize] {
    let base = FONT + u16::from(code.saturating_sub(FIRST_CHARACTER)) * GLYPH_BYTES;
    let mut bytes = [0; GLYPH_BYTES as usize];
    for (offset, byte) in bytes.iter_mut().enumerate() {
        *byte = memory.read(base + offset as u16);
    }
    bytes
}

/// The ZX character set is ASCII except for its last two printable codes.
const fn decode(code: u8) -> char {
    match code {
        0x60 => '\u{a3}', // POUND SIGN
        0x7F => '\u{a9}', // COPYRIGHT SIGN
        other => other as char,
    }
}

/// A rendered frame: [`FRAME_WIDTH`] × [`FRAME_HEIGHT`] colour indices, row-major.
///
/// Boxed because it is 80 KB, which is not something to move through a return value or to
/// build on a test thread's stack.
#[derive(Clone, PartialEq, Eq)]
pub struct Frame {
    pixels: Box<[Colour; FRAME_PIXELS]>,
}

impl fmt::Debug for Frame {
    /// Deliberately not derived, for the same reason as [`crate::memory::Memory`]: a
    /// derived `Debug` prints 81920 colours and makes any failing assertion unreadable.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

    /// Paint one row of the frame, border to border.
    ///
    /// The display is drawn over the middle of it afterwards. Painting the whole row and then
    /// overwriting the middle costs [`DISPLAY_WIDTH`] redundant stores per display row and
    /// buys the property that matters: **a frame with a uniform border is byte-identical to
    /// what the whole-buffer `fill` this replaced produced**, because filling every row with
    /// one colour and filling the buffer with one colour are the same bytes.
    /// `tests/border_stripes.rs` asserts that rather than trusting the argument — and that
    /// `fill` is gone rather than kept beside this, because a private helper with no caller is
    /// indistinguishable from one whose caller was lost.
    fn fill_row(&mut self, row: usize, colour: Colour) {
        let start = row * FRAME_WIDTH;
        if let Some(pixels) = self.pixels.get_mut(start..start + FRAME_WIDTH) {
            pixels.fill(colour);
        }
    }

    /// Paint one pixel, in frame coordinates.
    fn set(&mut self, x: usize, y: usize, colour: Colour) {
        // INVARIANT: every caller is a display loop bounded by DISPLAY_WIDTH/HEIGHT offset
        // by BORDER, so the index is within the frame.
        //
        // Asserted rather than only stated, and `x` is the reason it is worth a line: a flat
        // index is in range for an `x` past the row's end, so an off-by-one there wraps onto the
        // next row and writes the wrong pixel **silently** — the raw index below cannot catch it
        // and neither can `get_mut`. `debug_assert!` puts it in every test run and emits nothing
        // in release, which is what keeps this out of an 81,920-iteration inner loop's codegen.
        debug_assert!(x < FRAME_WIDTH && y < FRAME_HEIGHT);
        self.pixels[y * FRAME_WIDTH + x] = colour;
    }
}

/// Where the border was as the beam went down the frame.
///
/// # What this is, and the one thing it is not
///
/// The ULA paints the border as it sweeps, so a program that writes `0xFE` partway down a
/// frame produces horizontal bands — which is what a tape load looks like, and what
/// [`render`] on its own draws as a single colour. This is the record that makes the bands
/// possible: **the colour in effect at the moment each rendered row began.**
///
/// It is **not** progressive drawing. Mid-frame *attribute* and *bitmap* writes are still
/// drawn as if the last value had applied all frame, and the module documentation above says
/// so. Only the border is progressive, which is why the entry point is named for the border
/// and not for the frame.
///
/// # Resolution: one row, and it is derived rather than chosen
///
/// **Vertically the frame buffer maps to hardware exactly; horizontally it does not.** A
/// display line is [`crate::timing::Timing::t_states_per_line`] T-states and there are
/// [`crate::timing::Timing::lines_per_frame`] of them, so every rendered row has an exact
/// T-state at which it begins. Across a line the buffer is admittedly *not* the hardware:
/// [`BORDER`] is 32 pixels a side because *"a uniform margin is what a frame buffer wants"*,
/// where the real border is wider at the sides than it is tall and there is flyback with no
/// pixels at all. So a T-state-to-column mapping would be inventing precision this buffer
/// cannot carry, and a T-state-to-row mapping is not.
///
/// **And per-row is far finer than the effect that prompted it.** Measured, on this machine,
/// running the real 48K ROM's own `LD-BYTES` against a real tape: the loader changes the
/// border every **1884 to 2159 T-states** — a minimum of **8.4 scanlines** and a median of
/// **9.6**. Per-row resolution is therefore about eight times finer than the loader's own
/// rate, which is why the bands come out at their true thickness rather than merely present.
///
/// **What it cannot show**, stated because it is a real limit and not a rounding: a border
/// change *within* a line. Border-multicolour demos rewrite `0xFE` every eight to twenty-four
/// T-states to paint patterns across a single line, and every one of those writes lands in
/// the same row here. The last one before a row begins is the one that row gets.
///
/// # Bounded by construction, which is stronger than a bound that is enforced
///
/// There is no event list and no capacity to overrun: the record is one slot per **rendered
/// row**, and a guest cannot create rows. So there is no allocation sized by guest behaviour,
/// no drop policy, and no failing case for a policy to have. What is lost instead is stated
/// above — several writes inside one row collapse to the last of them — and
/// `tests/border_stripes.rs` gates that collapse rather than leaving it to be discovered.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct BorderTrace {
    /// The colour last written to `0xFE` — what [`crate::Ula::border`] reports.
    ///
    /// **The border is one datum and this is where it lives.** Keeping a separate `border`
    /// field on the ULA alongside this record would be two representations of one thing that
    /// can disagree, which is the defect `crate::memory` rejects for the paging port under
    /// *"one representation, derived once"*.
    current: Colour,
    /// The frame [`BorderTrace::changes`] describes, if it describes one.
    ///
    /// Compared at read time rather than maintained at frame boundaries, because there is no
    /// frame-boundary hook that is not [`crate::Ula::tick`] — and putting anything on that
    /// path for a display effect is the trade M7's sound half refused for audio.
    frame: Option<u64>,
    /// The colour at the top of that frame.
    initial: Colour,
    /// Where the colour changed, by rendered row. `None` is a row that inherits.
    changes: [Option<Colour>; FRAME_HEIGHT],
}

impl fmt::Debug for BorderTrace {
    /// Not derived: a derived `Debug` prints 256 `Option`s and makes any failing assertion
    /// involving a `Ula` unreadable. The same reason [`Frame`] and
    /// [`Memory`](crate::Memory) have their own.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BorderTrace")
            .field("current", &self.current)
            .field("frame", &self.frame)
            .field("changes", &self.changes.iter().flatten().count())
            .finish()
    }
}

impl BorderTrace {
    /// A trace with the border at `colour` and no frame recorded.
    pub(crate) const fn new(colour: Colour) -> Self {
        Self {
            current: colour,
            frame: None,
            initial: colour,
            changes: [None; FRAME_HEIGHT],
        }
    }

    /// The colour last written.
    pub(crate) const fn current(&self) -> Colour {
        self.current
    }

    /// Set the border **without** recording it as something the beam saw.
    ///
    /// The snapshot applier's route, and the reason is the one [`crate::Ula::set_border`]
    /// already gives for existing at all: a restore is not elapsed time, so the machine being
    /// restored into did not paint a band. The record is dropped rather than kept, because
    /// keeping the old machine's bands would draw a history that never happened here.
    pub(crate) fn set(&mut self, colour: Colour) {
        *self = Self::new(colour);
    }

    /// Record a write of `colour` at the position `clock` stands at.
    ///
    /// The caller has already established that the colour is changing; a write of the colour
    /// already showing paints no band and is not a row this record needs a slot for.
    pub(crate) fn record(&mut self, clock: Clock, colour: Colour) {
        if self.frame != Some(clock.frames()) {
            self.frame = Some(clock.frames());
            self.initial = self.current;
            self.changes = [None; FRAME_HEIGHT];
        }
        self.current = colour;
        if let Some(row) = first_row(clock.timing(), clock.frame_t_state()) {
            // Several writes inside one row collapse to the last, which is correct: the row
            // is painted with what was in effect when it began, and by then all of them have
            // happened.
            self.changes[row] = Some(colour);
        }
    }

    /// The border colour of each rendered row, top to bottom, as of frame `frames`.
    ///
    /// A frame the record does not describe reads as [`BorderTrace::current`] throughout —
    /// which is right rather than a fallback: a frame in which nothing wrote the border had
    /// one border colour, and it is the one still standing.
    ///
    /// # It accepts the frame **just finished** as well as the one running, and that is the
    /// difference between working and not
    ///
    /// A frontend's loop is `run_frame(); render();`, and [`crate::Spectrum::run_frame`]
    /// returns the instant the frame **counter advances** — so at the moment it renders, the
    /// machine stands a few T-states into the *next* frame and the record describes the
    /// previous one. A rule of "this frame only" is therefore a rule that shows a frontend
    /// nothing at all, every time, while passing any test that renders mid-frame.
    ///
    /// **That was not reasoned out in advance; it was a failing gate.**
    /// `tests/border_stripes.rs` ran the real ROM's loader for twenty frames, rendered, and
    /// got a uniform frame — which is exactly what the owner would have seen. The gate that
    /// found it is `the_frontends_own_loop_shows_the_bands`, and it is named for the call
    /// pattern rather than for the mechanism because the call pattern is what was wrong.
    ///
    /// One frame back and no further: older than that and the border has been unwritten for a
    /// whole frame, so it really has been uniform.
    fn rows(&self, frames: u64) -> impl Iterator<Item = Colour> + '_ {
        let stale = !matches!(self.frame, Some(frame) if frames.saturating_sub(1) <= frame && frame <= frames);
        let mut colour = if stale { self.current } else { self.initial };
        self.changes.iter().map(move |change| {
            if let (false, Some(next)) = (stale, change) {
                colour = *next;
            }
            colour
        })
    }
}

/// The rendered row a border write at `frame_t_state` first shows in.
///
/// `None` when the write lands past the bottom of what is rendered — during the vertical
/// flyback, where the beam is not painting anything this frame.
///
/// # The mapping, derived from `Timing` and from nothing else
///
/// A second T-state-to-beam-position mapping would be a second thing that has to agree with
/// contention's, and two mappings that must agree is the defect class this project keeps
/// catching. So every term below comes out of [`Timing`]:
///
/// ```text
///   display's first frame line = first_contended_t_state / t_states_per_line + 1
///   the row-0 frame line       = that, minus BORDER
///   the first row shown        = the first frame line beginning at or after frame_t_state
/// ```
///
/// The `+ 1` is because the first contended T-state falls a few T-states **before** the line
/// boundary the display starts on — one on a 48K, two on a 128 — so the line containing it
/// is the last border line. Both are asserted at compile time below, which is what makes the
/// `+ 1` a reading of the constants rather than a fudge that happens to work twice.
///
/// *(The 128's offset read **three** until 2026-09-02, when `timing_oracle.rs` graded that
/// machine against hardware and moved `first_contended_t_state` from 14361 to 14362. The line
/// it lands on is unchanged — both floor to 62 — so nothing this function computes moved, and
/// the assertion below is what proves that rather than a claim that it did.)*
fn first_row(timing: Timing, frame_t_state: u32) -> Option<usize> {
    let line = timing.t_states_per_line();
    let first_line_shown = display_first_line(timing) - BORDER as u32;
    // The first frame line that *begins* at or after the write, so a write partway down a
    // line shows from the next one. It is exactly one line late, always in that direction,
    // and never early — which is the honest cost of a per-row record.
    let affected = frame_t_state.div_ceil(line);
    let row = usize::try_from(affected.saturating_sub(first_line_shown)).ok()?;
    (row < FRAME_HEIGHT).then_some(row)
}

/// The frame line the display's first pixel row falls on.
const fn display_first_line(timing: Timing) -> u32 {
    timing.first_contended_t_state() / timing.t_states_per_line() + 1
}

// The `+ 1` above reads the constants rather than fitting them, and these are what say so:
// the first contended T-state is a few T-states short of a line boundary on both machines,
// and the rendered frame fits inside the lines each machine has.
const _: () = assert!(
    display_first_line(Timing::SPECTRUM_48K) * Timing::SPECTRUM_48K.t_states_per_line()
        - Timing::SPECTRUM_48K.first_contended_t_state()
        == 1
);
const _: () = assert!(
    display_first_line(Timing::SPECTRUM_128) * Timing::SPECTRUM_128.t_states_per_line()
        - Timing::SPECTRUM_128.first_contended_t_state()
        == 2
);
const _: () = assert!(display_first_line(Timing::SPECTRUM_48K) >= BORDER as u32);
const _: () = assert!(display_first_line(Timing::SPECTRUM_128) >= BORDER as u32);
const _: () = assert!(
    display_first_line(Timing::SPECTRUM_48K) - BORDER as u32 + FRAME_HEIGHT as u32
        <= Timing::SPECTRUM_48K.lines_per_frame()
);
const _: () = assert!(
    display_first_line(Timing::SPECTRUM_128) - BORDER as u32 + FRAME_HEIGHT as u32
        <= Timing::SPECTRUM_128.lines_per_frame()
);

/// Draw the current screen into `frame`, with **one** border colour for the whole frame.
///
/// `flash_phase` is the half of the 32-frame `FLASH` cycle the machine is in; see
/// [`flash_phase`].
/// The signature is unchanged and deliberately so: `Memory` already knows which bank the ULA
/// is drawing, so no caller has to learn about the shadow screen to keep working.
///
/// **It is now a projection of `render_border_trace` against a uniform border**, rather than
/// a second implementation — the same instrument `crate::timing`'s public constants use, so
/// there is one drawing loop and no pair to drift. A caller wanting the border as the beam
/// painted it uses [`crate::Spectrum::render`], which is what a frontend already calls.
pub fn render(memory: &Memory, border: Colour, flash_phase: bool, frame: &mut Frame) {
    draw(memory, core::iter::repeat(border), flash_phase, frame);
}

/// Draw the current screen into `frame`, painting each row's border as the beam painted it.
pub(crate) fn render_border_trace(
    memory: &Memory,
    border: &BorderTrace,
    frames: u64,
    frame: &mut Frame,
) {
    draw(memory, border.rows(frames), flash_phase(frames), frame);
}

/// The one drawing loop: `border_rows` supplies a colour per rendered row, then the display
/// is drawn over the middle of them.
fn draw(
    memory: &Memory,
    border_rows: impl Iterator<Item = Colour>,
    flash_phase: bool,
    frame: &mut Frame,
) {
    for (row, colour) in border_rows.take(FRAME_HEIGHT).enumerate() {
        frame.fill_row(row, colour);
    }
    let renderer = Renderer {
        // Resolved once for the whole frame rather than per cell. That is the same reason
        // `read_text` hoists the font, and it is also the only place the shadow-screen
        // indirection can cost anything: 6144 cells is 6144 slot-map lookups avoided.
        screen: screen_page(memory),
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
    /// The bank the ULA is drawing, resolved once — bank 5, or bank 7 on a 128 whose
    /// `0x7FFD` bit 3 is set.
    screen: &'a [u8; PAGE_SIZE],
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
        let bits = self.screen[screen_offset(pixel_address(column_byte, line as u8))];
        let attribute = Attribute::new(
            self.screen[screen_offset(attribute_address(column_byte, (line / CELL) as u8))],
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
    fn no_argument_at_all_escapes_the_display_file() {
        // The doc comment claimed both arguments were masked. `column` was; `line` was not,
        // and `pixel_address(0, 192)` returned 0x5800 — the attribute file. 2048 of these
        // 8192 pairs escaped, so this is exhaustive rather than a sample: the failure was a
        // whole quarter of the input space, and a sample that happened to stay under 192
        // would have agreed with the comment.
        for line in 0..=u8::MAX {
            for column in 0..=u8::MAX {
                let address = pixel_address(column, line);
                assert!(
                    (DISPLAY_FILE..ATTRIBUTE_FILE).contains(&address),
                    "pixel_address({column}, {line}) = {address:#06X}, outside the display file"
                );
            }
        }
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
    fn the_screen_offset_is_the_display_files_own_offset_within_its_bank() {
        // The bank appears at 0x4000, so the offset is the address's low fourteen bits — and
        // that has to agree with the plain subtraction over the whole display and attribute
        // file, or `render` would draw from the wrong place while every address test passed.
        for address in DISPLAY_FILE..=(ATTRIBUTE_FILE + ATTRIBUTE_FILE_LEN as u16 - 1) {
            assert_eq!(
                screen_offset(address),
                usize::from(address - DISPLAY_FILE),
                "{address:#06X}"
            );
        }
        assert_eq!(screen_offset(DISPLAY_FILE), 0);
        assert_eq!(screen_offset(ATTRIBUTE_FILE), 0x1800);
    }

    #[test]
    fn the_screen_offset_is_in_range_for_every_address() {
        // Masked rather than subtracted, so the index cannot panic whatever it is handed.
        // Exhaustive because the input is 65536 wide and this costs microseconds.
        for address in 0..=u16::MAX {
            assert!(screen_offset(address) < PAGE_SIZE);
        }
    }

    #[test]
    fn a_128_draws_the_bank_bit_three_selects() {
        // `M7.md` Decision 3's third gated property, at the level this module owns it: the
        // ULA reads a bank directly, so a shadow screen is visible even though bank 7 is
        // paged into no slot at all and `Memory::read(0x4000)` still returns bank 5.
        use crate::memory::BankIndex;

        let mut memory = Memory::spectrum_128(&[0; PAGE_SIZE], &[0; PAGE_SIZE]).expect("two ROMs");
        // Bank 5: one pixel set at the top left. Bank 7: the pixel next to it.
        for (bank, column) in [(5_u8, 0_u8), (7, 1)] {
            let page = memory.bank_mut(BankIndex::new(bank));
            page[screen_offset(pixel_address(column, 0))] = 0x80;
            page[screen_offset(attribute_address(column, 0))] = 0x07; // white ink
        }

        let mut frame = Frame::new();
        render(&memory, Colour::BLACK, false, &mut frame);
        assert_eq!(frame.pixel(BORDER, BORDER), Some(Colour::new(7)), "bank 5");
        assert_eq!(frame.pixel(BORDER + CELL, BORDER), Some(Colour::BLACK));

        // Bit 3 alone, with the slot map left exactly as it was.
        let slots_before = memory.slots();
        memory.write_paging_port(0x08);
        assert_eq!(memory.slots(), slots_before, "only the screen select moved");
        assert_eq!(memory.read(0x4000), 0x80, "0x4000 is still bank 5");

        render(&memory, Colour::BLACK, false, &mut frame);
        assert_eq!(
            frame.pixel(BORDER, BORDER),
            Some(Colour::BLACK),
            "bank 5's pixel must be gone"
        );
        assert_eq!(
            frame.pixel(BORDER + CELL, BORDER),
            Some(Colour::new(7)),
            "bank 7's pixel must be drawn"
        );
    }

    #[test]
    fn reading_the_screen_as_text_follows_the_same_bank_as_render() {
        // The two views must not disagree about which screen is on screen. `read_text` is
        // what the boot gates assert against, so a `render` that followed bit 3 and a
        // `read_text` that did not would leave the 128's shadow screen half-modelled and
        // green.
        use crate::memory::BankIndex;

        let mut memory = Memory::spectrum_128(&[0; PAGE_SIZE], &[0; PAGE_SIZE]).expect("two ROMs");
        // A glyph the ROM font would resolve, written into bank 7 only. With a blank ROM the
        // font is all zeros, so any non-blank cell reads as '?' — which is enough to tell
        // "something is here" from "nothing is here", and is what a wrong bank would change.
        memory.bank_mut(BankIndex::new(7))[screen_offset(pixel_address(0, 0))] = 0xFF;

        assert!(
            read_text(&memory)[0].starts_with(' '),
            "bank 5 is blank and bit 3 is clear"
        );
        memory.write_paging_port(0x08);
        assert!(
            read_text(&memory)[0].starts_with('?'),
            "bank 7 is selected and its cell must appear"
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
