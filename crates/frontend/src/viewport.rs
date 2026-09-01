//! Where the 320 × 256 frame lands in a window of some other size.
//!
//! # Integer scale, and why the wasted pixels are the right trade
//!
//! A Spectrum pixel is a square of solid colour and there are only 81,920 of them. Scaling by
//! 3.2 means some rows are three window pixels tall and some are four, which under any
//! sampling mode produces a visible ripple that moves when the window is resized — on a
//! screen this coarse it is the most obvious artefact there is. So the scale is floored to a
//! whole number whenever at least one whole frame fits, and the remainder becomes margin.
//!
//! The margin is not wasted the way it looks. The shell clears the window to the machine's
//! **current border colour**, so the letterbox is continuous with the border the ULA is
//! already drawing — the image reads as a slightly wider border rather than as black bars,
//! and it changes colour along with the border when a program writes to port `0xFE`.
//!
//! Below one whole frame the scale is left fractional. A window smaller than 320 × 256 is a
//! window somebody has deliberately made tiny, and showing them a quarter of the screen
//! crisply is worse than showing all of it softly.

use spectrum::screen::{BORDER, DISPLAY_HEIGHT, DISPLAY_WIDTH, FRAME_HEIGHT, FRAME_WIDTH};

/// Where a frame is drawn, in window pixels.
///
/// Fields rather than accessors: this is a value describing a rectangle, it has no invariant
/// beyond what [`fit`] establishes, and every consumer wants all five numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// Window pixels per frame pixel. Whole whenever a whole frame fits.
    pub scale: f32,
    /// Left edge of the frame, border included.
    pub x: f32,
    /// Top edge of the frame, border included.
    pub y: f32,
    /// Drawn width, border included.
    pub width: f32,
    /// Drawn height, border included.
    pub height: f32,
}

impl Viewport {
    /// The top-left of the 256 × 192 display, inside the border.
    ///
    /// The border is drawn as part of the frame, so this is not where anything is *drawn* —
    /// it is where the display *is*, which is what a caller pointing at a pixel needs.
    #[must_use]
    pub fn display_origin(self) -> (f32, f32) {
        let inset = BORDER as f32 * self.scale;
        (self.x + inset, self.y + inset)
    }

    /// The size of the 256 × 192 display, scaled.
    #[must_use]
    pub fn display_size(self) -> (f32, f32) {
        (
            DISPLAY_WIDTH as f32 * self.scale,
            DISPLAY_HEIGHT as f32 * self.scale,
        )
    }
}

/// Fit a frame into a window of `width` × `height`, centred.
///
/// A window size that is not a positive finite number yields a zero-sized viewport rather than
/// a `NaN`. That is not a hypothetical: a window being dragged between displays can report a
/// zero height for a frame or two, and a `NaN` scale propagates into every draw call and
/// leaves the screen blank *after* the window is resized back, with nothing logged.
///
/// `is_finite` and not merely `> 0.0`, because the three ways this can go wrong are different
/// and only one of them is caught by a comparison: zero divides cleanly, a `NaN` makes every
/// comparison false, and an infinity makes the scale infinite. The guard names all three.
#[must_use]
pub fn fit(width: f32, height: f32) -> Viewport {
    let usable = width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0;
    if !usable {
        return Viewport {
            scale: 0.0,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        };
    }

    let exact = (width / FRAME_WIDTH as f32).min(height / FRAME_HEIGHT as f32);
    let scale = if exact >= 1.0 { exact.floor() } else { exact };

    let drawn_width = FRAME_WIDTH as f32 * scale;
    let drawn_height = FRAME_HEIGHT as f32 * scale;

    Viewport {
        scale,
        x: ((width - drawn_width) / 2.0).floor(),
        y: ((height - drawn_height) / 2.0).floor(),
        width: drawn_width,
        height: drawn_height,
    }
}
