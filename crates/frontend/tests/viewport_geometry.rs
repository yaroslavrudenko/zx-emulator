//! Window geometry, against literal rectangles.

use frontend::viewport::{self, Viewport};
use spectrum::screen::{BORDER, DISPLAY_HEIGHT, DISPLAY_WIDTH, FRAME_HEIGHT, FRAME_WIDTH};

#[test]
fn the_frame_this_file_was_written_against_is_still_the_frame() {
    // A positive control on the *premise* rather than on the subject. Every literal below is
    // arithmetic on 320 x 256; if `crates/spectrum` changes the border or the display, those
    // literals become wrong and the failures would read as viewport defects. This assertion
    // fails first and says what actually moved.
    assert_eq!((FRAME_WIDTH, FRAME_HEIGHT), (320, 256));
    assert_eq!((DISPLAY_WIDTH, DISPLAY_HEIGHT), (256, 192));
    assert_eq!(BORDER, 32);
}

#[test]
fn an_exact_fit_uses_the_whole_window() {
    assert_eq!(
        viewport::fit(320.0, 256.0),
        Viewport {
            scale: 1.0,
            x: 0.0,
            y: 0.0,
            width: 320.0,
            height: 256.0
        },
    );
    assert_eq!(
        viewport::fit(1280.0, 1024.0),
        Viewport {
            scale: 4.0,
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 1024.0
        },
    );
}

#[test]
fn a_wide_window_centres_the_frame_and_floors_the_scale() {
    // 1024 / 320 = 3.2 and 768 / 256 = 3.0, so height is the constraint and the scale floors
    // to 3 rather than stretching to 3.2 — which would make some pixel rows taller than
    // others. The 64 left over becomes 32 of margin on each side.
    assert_eq!(
        viewport::fit(1024.0, 768.0),
        Viewport {
            scale: 3.0,
            x: 32.0,
            y: 0.0,
            width: 960.0,
            height: 768.0
        },
    );
}

#[test]
fn a_tall_window_centres_vertically() {
    // 640 / 320 = 2.0 and 700 / 256 = 2.73, so width constrains. 700 - 512 = 188, halved to
    // 94 top and bottom.
    assert_eq!(
        viewport::fit(640.0, 700.0),
        Viewport {
            scale: 2.0,
            x: 0.0,
            y: 94.0,
            width: 640.0,
            height: 512.0
        },
    );
}

#[test]
fn an_odd_margin_floors_rather_than_landing_on_a_half_pixel() {
    // 1000 / 320 = 3.125, so scale 3 and a drawn width of 960. The 40 left over halves to 20
    // exactly; 1001 would halve to 20.5, and a texture drawn at a half pixel is the other way
    // to get a soft image after taking the trouble to use an integer scale.
    let odd = viewport::fit(1001.0, 768.0);
    assert_eq!(odd.scale, 3.0);
    assert_eq!(odd.x, 20.0);
    assert_eq!(odd.x.fract(), 0.0, "the origin must be a whole pixel");
    assert_eq!(odd.y.fract(), 0.0);
}

#[test]
fn a_window_smaller_than_one_frame_keeps_a_fractional_scale() {
    // Below 1.0 the choice reverses: flooring would give 0 and draw nothing at all, and
    // showing a quarter of the screen crisply is worse than showing all of it softly.
    assert_eq!(
        viewport::fit(160.0, 128.0),
        Viewport {
            scale: 0.5,
            x: 0.0,
            y: 0.0,
            width: 160.0,
            height: 128.0
        },
    );
}

#[test]
fn a_degenerate_window_yields_nothing_rather_than_a_nan() {
    // A window being dragged between displays can report a zero dimension for a frame or two.
    // A NaN scale propagates into every draw call and the screen stays blank after the window
    // is resized back, with nothing logged.
    for (width, height) in [(0.0, 768.0), (1024.0, 0.0), (0.0, 0.0), (-100.0, 768.0)] {
        let fitted = viewport::fit(width, height);
        assert_eq!(
            fitted,
            Viewport {
                scale: 0.0,
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0
            },
            "fit({width}, {height})",
        );
        assert!(!fitted.scale.is_nan());
    }
    assert!(!viewport::fit(f32::NAN, 768.0).scale.is_nan());
}

#[test]
fn the_display_sits_one_border_inside_the_frame() {
    // The geometry a caller pointing at a screen pixel needs: the drawn rectangle includes
    // the border, and the 256 x 192 the programs write to starts one scaled border in.
    let fitted = viewport::fit(1024.0, 768.0);
    assert_eq!(fitted.display_origin(), (32.0 + 96.0, 0.0 + 96.0));
    assert_eq!(fitted.display_size(), (768.0, 576.0));

    // And it stays inside, on both axes, which is the property that would break if the
    // border were ever added on one side only.
    let (origin_x, origin_y) = fitted.display_origin();
    let (display_width, display_height) = fitted.display_size();
    assert!(origin_x >= fitted.x);
    assert!(origin_y >= fitted.y);
    assert!(origin_x + display_width <= fitted.x + fitted.width);
    assert!(origin_y + display_height <= fitted.y + fitted.height);
}

#[test]
fn the_frame_always_fits_inside_the_window_it_was_given() {
    // A sweep rather than a sample: the failure mode is a rounding direction that only shows
    // up at particular sizes, and a handful of chosen sizes is exactly what would miss it.
    for width in (1..=2000).step_by(7) {
        for height in (1..=1600).step_by(11) {
            let (width, height) = (width as f32, height as f32);
            let fitted = viewport::fit(width, height);
            assert!(
                fitted.width <= width && fitted.height <= height,
                "fit({width}, {height}) drew {} x {}",
                fitted.width,
                fitted.height,
            );
            assert!(fitted.x >= 0.0 && fitted.y >= 0.0, "fit({width}, {height})");
            assert!(
                fitted.scale < 1.0 || fitted.scale.fract() == 0.0,
                "fit({width}, {height}) scaled by {}",
                fitted.scale,
            );
        }
    }
}
