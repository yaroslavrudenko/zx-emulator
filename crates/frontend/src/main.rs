//! A window with a ZX Spectrum in it.
//!
//! ```sh
//! cargo run --release --manifest-path crates/frontend/Cargo.toml -- \
//!     testdata/roms/48.rom testdata/tapes/z80doc.tap
//! ```
//!
//! Files are told apart by extension, in any order: `.rom` builds the machine, `.tap` goes in
//! the drive, `.z80` and `.sna` are restored over the top. With no ROM named, the machine is
//! built from `testdata/roms/48.rom`.
//!
//! | Key | |
//! |---|---|
//! | `Shift` / `Ctrl` | `CAPS SHIFT` / `SYMBOL SHIFT` — either hand |
//! | `Backspace`, arrows, `Escape`, `,` `.` `;` `'` `-` `=` `/` | the combination the Spectrum prints on the key |
//! | `F1` | show or hide the pacing readout |
//! | `F2` | save a `.z80` |
//! | `F3` `F4` `F5` | tape play, stop, rewind |
//! | `F6` | reset |
//!
//! # This file is the untestable part, and it is kept thin on purpose
//!
//! Everything with a decision in it — which key, which colour, how many frames, which file —
//! is in the library next door and is reachable from `cargo test`. What is left here needs a
//! GPU and a window and cannot run headless, so it is held to plumbing: poll, upload, draw,
//! await. `crates/frontend/src/lib.rs` carries the table of what that leaves ungraded, and
//! *"whether it looks right"* is the first row.

use std::fmt::Write as _;
use std::time::Duration;

use macroquad::prelude::*;

use frontend::keymap::Hotkey;
use frontend::media::{self, Kind};
use frontend::pacing::{Pacer, RateMeter};
use frontend::viewport::Viewport;
use frontend::{host, keymap, palette, viewport};
use spectrum::screen::{FRAME_HEIGHT, FRAME_WIDTH};
use spectrum::{Frame, Spectrum};

/// The ROM used when the command line names none.
const DEFAULT_ROM: &str = "testdata/roms/48.rom";

/// What `F2` writes, before the collision counter and the extension.
const SAVE_STEM: &str = "snapshot";

/// Whole frame pixels per window pixel when the window first opens.
///
/// Three puts a 320 × 256 frame at 960 × 768, which is a comfortable window on any display
/// made this century and still leaves room to enlarge.
const INITIAL_SCALE: i32 = 3;

/// Seconds each rate window covers. See [`RateMeter::new`].
const RATE_WINDOW: f64 = 1.0;

/// Height of the status bar, in window pixels.
const STATUS_HEIGHT: f32 = 22.0;

/// Point size of the status text.
const STATUS_TEXT: f32 = 16.0;

fn window_conf() -> Conf {
    Conf {
        window_title: "ZX Spectrum 48K".to_owned(),
        window_width: FRAME_WIDTH as i32 * INITIAL_SCALE,
        window_height: FRAME_HEIGHT as i32 * INITIAL_SCALE,
        window_resizable: true,
        // The frame is 320 × 256 of hard-edged pixels, so a retina display is worth having:
        // it lets the integer scale in `viewport::fit` land on a bigger whole number.
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let arguments = host::arguments();
    let (rom_paths, media_paths) = partition(&arguments);

    let mut roms = Vec::with_capacity(rom_paths.len());
    for path in &rom_paths {
        match load(path).await {
            Ok(bytes) => roms.push(bytes),
            Err(message) => return complain(&message).await,
        }
    }
    let borrowed: Vec<&[u8]> = roms.iter().map(Vec::as_slice).collect();
    let mut machine = match media::start(&borrowed) {
        Ok(machine) => machine,
        Err(error) => return complain(&format!("{}: {error}", rom_paths.join(", "))).await,
    };

    let mut video = Video::new();
    let mut status = Status::new();

    for path in media_paths {
        match load(&path).await {
            Ok(bytes) => status.report(insert(&mut machine, &path, &bytes)),
            Err(message) => status.report(message),
        }
    }

    let mut pacer = Pacer::new();
    let mut meter = RateMeter::new(RATE_WINDOW, get_time());

    loop {
        for &(code, action) in keymap::HOTKEYS {
            if is_key_pressed(code) {
                act(action, &mut machine, &mut status);
            }
        }
        keymap::apply(is_key_down, machine.keyboard_mut());

        // `try_from_secs_f32` rather than `from_secs_f32`: the latter panics on a negative or
        // non-finite argument, and `get_frame_time` is a number from a windowing system
        // rather than one this program computed.
        let elapsed = Duration::try_from_secs_f32(get_frame_time()).unwrap_or(Duration::ZERO);
        for _ in 0..pacer.advance(elapsed) {
            machine.run_frame();
        }
        meter.sample(get_time(), pacer.ran());

        video.draw(&machine);
        status.draw(&machine, pacer, meter);
        next_frame().await;
    }
}

/// Split the command line into the ROMs to build from and the files to load afterwards.
///
/// ROM paths accumulate **in the order they were named**, because that order is what
/// [`media::start`] reads the model off: one is a 48K, two are a 128 with the first paging in at
/// reset. `--rom PATH` is accepted as well, because a path with no extension is otherwise
/// unreachable, and it accumulates like any other.
///
/// > **This said *"the last `.rom` wins, which is what every other tool does with a repeated
/// > option and needs no error of its own"*, and M7 took that option away.** The sentence was
/// > right about repeated *options* and it stopped being right the moment a second ROM meant
/// > something: on a machine that can be a 128, `a.rom b.rom` is not a person changing their
/// > mind, it is a person naming a ROM pair. **This is the quiet kind of breaking change** — the
/// > same argument, reinterpreted, with no signature to notice it by — so it is written down
/// > here rather than left in a diff. What a repeated `--rom` no longer does is override;
/// > what it does now is add, and a third one is [`media::Error::RomCount`] rather than a
/// > silently dropped file.
fn partition(arguments: &[String]) -> (Vec<String>, Vec<String>) {
    let mut roms = Vec::new();
    let mut rest = Vec::new();
    let mut expecting_rom = false;

    for argument in arguments {
        if expecting_rom {
            roms.push(argument.clone());
            expecting_rom = false;
        } else if argument == "--rom" {
            expecting_rom = true;
        } else if media::kind_of(argument) == Some(Kind::Rom) {
            roms.push(argument.clone());
        } else {
            rest.push(argument.clone());
        }
    }
    if roms.is_empty() {
        roms.push(DEFAULT_ROM.to_owned());
    }
    (roms, rest)
}

/// Read a file, as a message rather than as an error type.
///
/// `load_file` is macroquad's, not the standard library's, and that is the whole reason this
/// returns a `String`: on `wasm32` it is an HTTP fetch whose failures do not map onto
/// [`std::io::Error`], and the shell's only response to any of them is to put the text on the
/// screen. See [`frontend::host`].
async fn load(path: &str) -> Result<Vec<u8>, String> {
    macroquad::file::load_file(path)
        .await
        .map_err(|error| format!("cannot read {path}: {error}"))
}

/// Hand a loaded file to the machine and say what happened.
fn insert(machine: &mut Spectrum, path: &str, bytes: &[u8]) -> String {
    let Some(kind) = media::kind_of(path) else {
        return format!("{path}: not a .rom, .tap, .z80 or .sna");
    };
    match media::insert(machine, kind, bytes) {
        Ok(()) => format!("loaded {path}"),
        Err(error) => format!("{path}: {error}"),
    }
}

/// Carry out a hotkey.
fn act(action: Hotkey, machine: &mut Spectrum, status: &mut Status) {
    match action {
        Hotkey::ToggleStatus => status.visible = !status.visible,
        Hotkey::SaveSnapshot => status.report(write_snapshot(machine)),
        Hotkey::PlayTape => {
            machine.tape_mut().play();
            status.report("tape playing".to_owned());
        }
        Hotkey::StopTape => {
            machine.tape_mut().stop();
            status.report("tape stopped".to_owned());
        }
        Hotkey::RewindTape => {
            machine.tape_mut().rewind();
            status.report("tape rewound".to_owned());
        }
        Hotkey::Reset => {
            machine.reset();
            status.report("reset".to_owned());
        }
    }
}

/// Write the machine out as a numbered `.z80`, and say where it went.
fn write_snapshot(machine: &Spectrum) -> String {
    let path = match host::free_path(SAVE_STEM, "z80") {
        Ok(path) => path,
        Err(error) => return error.to_string(),
    };
    match host::save(&path, &media::save(machine)) {
        Ok(()) => format!("saved {path}"),
        Err(error) => error.to_string(),
    }
}

/// Draw `message` until the window is closed.
///
/// A frontend that cannot start still has a window open — `#[macroquad::main]` opened it
/// before this function ran — so exiting silently leaves a blank rectangle and no
/// explanation. Somebody who double-clicked the binary never sees stderr.
async fn complain(message: &str) {
    loop {
        clear_background(BLACK);
        draw_text(message, 20.0, 40.0, STATUS_TEXT * 1.5, RED);
        draw_text(
            "give one 16 KB .rom (48K) or two (128) on the command line",
            20.0,
            40.0 + STATUS_TEXT * 2.0,
            STATUS_TEXT,
            GRAY,
        );
        next_frame().await;
    }
}

/// The machine's screen, as a texture on the GPU.
///
/// Three fields, all of them buffers, all built once. The per-frame path allocates nothing:
/// [`Frame`] and the `RGBA` buffer are reused in place and the texture is uploaded over
/// rather than recreated. `docs/ARCHITECTURE.md` makes performance a non-goal, so this is
/// not tuning — a texture rebuilt 50 times a second is a GPU allocation 50 times a second,
/// and that is a leak-shaped mistake rather than a slow one.
struct Video {
    // Not `Box<Frame>`: `Frame` already boxes its own 80 KB of pixels, so it is a pointer
    // wide, and wrapping it again would be a second allocation and a second indirection
    // bought for nothing.
    frame: Frame,
    rgba: Box<[u8; palette::RGBA_BYTES]>,
    texture: Texture2D,
}

impl Video {
    /// Allocate the buffers and the texture.
    fn new() -> Self {
        let rgba = palette::buffer();
        let texture = Texture2D::from_rgba8(FRAME_WIDTH as u16, FRAME_HEIGHT as u16, &rgba[..]);
        // Nearest, always. A Spectrum pixel is a square of flat colour and every filter mode
        // that is not nearest turns the whole screen into a smear at these scales.
        texture.set_filter(FilterMode::Nearest);
        Self {
            frame: Frame::new(),
            rgba,
            texture,
        }
    }

    /// Render the machine and put it on the screen.
    fn draw(&mut self, machine: &Spectrum) {
        machine.render(&mut self.frame);
        palette::write_rgba(&self.frame, &mut self.rgba);
        self.texture
            .update_from_bytes(FRAME_WIDTH as u32, FRAME_HEIGHT as u32, &self.rgba[..]);

        // The letterbox is cleared to the machine's own border, so the margin integer scaling
        // leaves over reads as a wider border rather than as black bars — and changes colour
        // with it when a program writes port 0xFE.
        clear_background(to_color(machine.border()));

        let Viewport {
            x,
            y,
            width,
            height,
            ..
        } = viewport::fit(screen_width(), screen_height());
        draw_texture_ex(
            &self.texture,
            x,
            y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(width, height)),
                ..Default::default()
            },
        );
    }
}

/// The pacing readout, and whatever happened last.
struct Status {
    visible: bool,
    message: String,
    /// Reused so the per-frame path formats without allocating.
    line: String,
}

impl Status {
    /// Visible, with nothing to say yet.
    ///
    /// Visible by default because the brief for this frontend is that a machine failing to
    /// keep 50 Hz should be *visible rather than silently drifting*, and a readout somebody
    /// has to know to switch on is the silent case with extra steps.
    fn new() -> Self {
        Self {
            visible: true,
            message: String::new(),
            line: String::with_capacity(64),
        }
    }

    /// Replace the message shown alongside the readout.
    fn report(&mut self, message: String) {
        self.message = message;
    }

    /// Draw the readout along the bottom of the window.
    fn draw(&mut self, machine: &Spectrum, pacer: Pacer, meter: RateMeter) {
        if !self.visible {
            return;
        }

        self.line.clear();
        // Infallible: writing to a `String` cannot fail. The `Result` is there for writers
        // that can, and is discarded here rather than handled.
        let _ = write!(
            self.line,
            "{:.1} Hz   dropped {}   frame {}   {}",
            meter.hz(),
            pacer.dropped(),
            machine.frames(),
            self.message,
        );

        let top = screen_height() - STATUS_HEIGHT;
        draw_rectangle(
            0.0,
            top,
            screen_width(),
            STATUS_HEIGHT,
            Color::new(0.0, 0.0, 0.0, 0.65),
        );
        // Red once frames are being lost: the count alone is easy to read past, and the
        // difference between "keeping up" and "not" is the one thing this bar is for.
        let ink = if pacer.dropped() == 0 { LIGHTGRAY } else { RED };
        draw_text(&self.line, 6.0, top + STATUS_TEXT, STATUS_TEXT, ink);
    }
}

/// A Spectrum colour as a macroquad one.
///
/// Goes through [`spectrum::Colour::rgb`] like everything else, so the gun order is applied
/// in exactly one place in this crate. See [`frontend::palette`].
fn to_color(colour: spectrum::Colour) -> Color {
    let [red, green, blue] = colour.rgb();
    Color::from_rgba(red, green, blue, palette::OPAQUE)
}
