//! Boot a machine, optionally type at it, and write what the screen looks like to a file.
//!
//! ```sh
//! cargo run --release --manifest-path crates/frontend/Cargo.toml --bin zx-shot -- \
//!     --frames 120 --keys 'P;LeftControl+P;H;E;L;L;O;LeftControl+P;Enter' \
//!     --out .agent-workspace/shots/typed.ppm
//! ```
//!
//! Then `sips -s format png typed.ppm --out typed.png` on macOS, or any image viewer that
//! reads Netpbm. See [`frontend::ppm`] for why the format is P6 and not PNG.
//!
//! # Photographing a 128
//!
//! `--rom` repeats, and the **count** is what names the machine — one is a 48K, two are a 128
//! with the first paging in at reset. There is no `--model` flag and no second machine-building
//! path; [`frontend::media::start`] carries the whole decision and `tests/media_dispatch.rs`
//! grades it.
//!
//! ```sh
//! cargo run --release --manifest-path crates/frontend/Cargo.toml --bin zx-shot -- \
//!     --rom testdata/roms/128-0.rom --rom testdata/roms/128-1.rom \
//!     --frames 120 --out .agent-workspace/shots/128-menu.ppm
//! ```
//!
//! **`screen::read_text` is a 48K instrument and nothing here calls it, which is why this
//! binary can photograph a 128 at all.** That function reads the character set at `0x3D00`
//! *through the machine's slot map*; the 128 editor ROM has no font there, and the menu loop
//! pages ROM 1 in and out every frame, so the same call answers correctly or answers `?`
//! depending on which frame it is asked. This binary renders pixels — `Spectrum::render` into a
//! [`spectrum::Frame`], then [`frontend::palette::write_rgba`] — and never asks the machine what
//! its screen *says*. The hazard is real and it is somebody else's; it is named here so that a
//! future `--assert-text` flag is not added without meeting it.
//!
//! # Why this is a second binary and not a `--screenshot` flag on `zx`
//!
//! `#[macroquad::main]` **opens the window before the function body runs** — it expands to a
//! `miniquad::start` that creates the context and then drives the future. A screenshot mode
//! reached from inside that body would therefore not be headless: it would open a window,
//! take a picture, and close it, which is unusable over SSH, in CI, or on any machine without
//! a display server. This binary never calls `miniquad::start`, so linking macroquad costs a
//! symbol table and nothing else.
//!
//! # It is the same pipeline the window runs, and that is the point
//!
//! `keymap::apply` -> `Spectrum::run_frame` -> `Spectrum::render` -> `palette::write_rgba`,
//! which is exactly what `zx`'s frame loop does. Only the final step differs — this writes
//! [`frontend::ppm`] bytes where the window uploads a texture — and `tests/ppm_encoding.rs`
//! asserts those bytes are the same buffer. A screenshot produced by code the window does not
//! run would prove nothing about the window, and this repository keeps catching gates that
//! graded less than they appeared to.
//!
//! > **This said *"caught five gates"* and the figure had no source.** `docs/STATUS.md` — the
//! > document that number was resting on — recorded **no such count**: verified on 2026-09-01,
//! > before that document was corrected, by
//! > `grep -n -iE 'occasion|graded less|worst form' docs/STATUS.md`, which matched nothing at
//! > all. It matches now because the correction lives there too. The one place that document
//! > does count this family is
//! > *A gate that nothing runs, for the third time — and the form got worse*, which names
//! > **three** instances and counts something narrower than this sentence claimed — see
//! > [`frontend`](crate)'s own header, where the whole correction is set out. The mechanism is
//! > what this paragraph needed and the integer was decoration, so the integer is gone.

use std::process::ExitCode;

use macroquad::input::KeyCode;

use frontend::{host, keymap, media, palette, ppm};
use spectrum::{Frame, Spectrum};

/// The ROM used when the command line names none.
const DEFAULT_ROM: &str = "testdata/roms/48.rom";

/// Frames given to the ROM before anything is typed — four seconds of emulated time, several
/// times what its start-up needs. The same figure `crates/spectrum/examples/boot.rs` uses.
const DEFAULT_FRAMES: u64 = 120;

/// Frames a key is held, and then released.
///
/// The ROM scans the keyboard once per frame and debounces, so a tap has to outlast both. Six
/// each way is well clear of that and still only a quarter-second per key.
const HOLD_FRAMES: u64 = 6;

/// Frames run after the last key, so that an `ENTER` has time to execute what it submitted.
///
/// Enough for a line of BASIC. **Not** enough for everything an `ENTER` can start: selecting
/// *48 BASIC* from the 128's menu re-pages the ROM, clears the screen and reinitialises the
/// system variables, and photographed at 25 frames it is a black rectangle — the picture is of
/// a machine that is genuinely mid-clear, not of a failure. That is what `--settle` is for, and
/// it is a flag rather than a bigger constant because the right number is a property of what was
/// typed and nothing here can know it.
const SETTLE_FRAMES: u64 = 25;

fn main() -> ExitCode {
    match run(&host::arguments()) {
        Ok(path) => {
            println!("wrote {path}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("zx-shot: {message}");
            eprintln!(
                "usage: zx-shot --out PATH [--rom PATH]... [--frames N] [--keys SCRIPT] [--settle N]\n\
                 one --rom is a 48K; two are a 128, editor ROM first"
            );
            ExitCode::FAILURE
        }
    }
}

/// Everything the command line can say.
struct Options {
    /// In the order named. One is a 48K, two are a 128 — see [`media::start`].
    roms: Vec<String>,
    out: String,
    frames: u64,
    /// Frames run after the last key. Defaults to [`SETTLE_FRAMES`].
    settle: u64,
    /// Each entry is one tap: the host keys held together for [`HOLD_FRAMES`].
    taps: Vec<Vec<KeyCode>>,
}

fn run(arguments: &[String]) -> Result<String, String> {
    let options = parse(arguments)?;

    let mut roms = Vec::with_capacity(options.roms.len());
    for path in &options.roms {
        roms.push(std::fs::read(path).map_err(|error| format!("cannot read {path}: {error}"))?);
    }
    let borrowed: Vec<&[u8]> = roms.iter().map(Vec::as_slice).collect();
    let mut machine =
        media::start(&borrowed).map_err(|error| format!("{}: {error}", options.roms.join(", ")))?;

    machine.run_frames(options.frames);
    for tap in &options.taps {
        press(&mut machine, tap);
    }
    if !options.taps.is_empty() {
        machine.run_frames(options.settle);
    }

    // The pipeline, identical to the window's up to the last line.
    let mut frame = Frame::new();
    machine.render(&mut frame);
    let mut rgba = palette::buffer();
    palette::write_rgba(&frame, &mut rgba);

    host::save(&options.out, &ppm::encode(&rgba)).map_err(|error| error.to_string())?;
    Ok(options.out)
}

/// Hold `tap` for [`HOLD_FRAMES`], then release for the same, through the real keymap.
fn press(machine: &mut Spectrum, tap: &[KeyCode]) {
    for _ in 0..HOLD_FRAMES {
        keymap::apply(|code| tap.contains(&code), machine.keyboard_mut());
        machine.run_frame();
    }
    for _ in 0..HOLD_FRAMES {
        keymap::apply(|_| false, machine.keyboard_mut());
        machine.run_frame();
    }
}

fn parse(arguments: &[String]) -> Result<Options, String> {
    let mut roms = Vec::new();
    let mut out = None;
    let mut frames = DEFAULT_FRAMES;
    let mut settle = SETTLE_FRAMES;
    let mut taps = Vec::new();

    let mut rest = arguments.iter();
    while let Some(flag) = rest.next() {
        match flag.as_str() {
            // Repeated rather than overriding, and it is the same rule `zx`'s `partition`
            // follows: the count is what names the machine. One `--rom` is a 48K, two are a
            // 128 in the order given. This is why the shot of a 128 needs no `--model` flag
            // and no second construction path — the ROMs the user named already say it.
            "--rom" => roms.push(next(&mut rest, flag)?),
            "--out" => out = Some(next(&mut rest, flag)?),
            "--frames" => {
                let raw = next(&mut rest, flag)?;
                frames = raw
                    .parse()
                    .map_err(|_| format!("--frames wants a whole number, not {raw:?}"))?;
            }
            "--settle" => {
                let raw = next(&mut rest, flag)?;
                settle = raw
                    .parse()
                    .map_err(|_| format!("--settle wants a whole number, not {raw:?}"))?;
            }
            "--keys" => taps = script(&next(&mut rest, flag)?)?,
            other => return Err(format!("unknown argument {other:?}")),
        }
    }

    if roms.is_empty() {
        roms.push(DEFAULT_ROM.to_owned());
    }
    Ok(Options {
        roms,
        out: out.ok_or("--out is required")?,
        frames,
        settle,
        taps,
    })
}

/// The value following `flag`.
fn next<'a>(rest: &mut impl Iterator<Item = &'a String>, flag: &str) -> Result<String, String> {
    rest.next()
        .cloned()
        .ok_or_else(|| format!("{flag} needs a value"))
}

/// Parse `P;LeftControl+P;Enter` into taps of host keys.
///
/// `;` separates taps and `+` joins the keys held together for one. Names are resolved by
/// [`keymap::code_named`], so only keys the window can press are reachable and a typo is an
/// error rather than a silently ignored tap.
fn script(source: &str) -> Result<Vec<Vec<KeyCode>>, String> {
    source
        .split(';')
        .map(str::trim)
        .filter(|tap| !tap.is_empty())
        .map(|tap| {
            tap.split('+')
                .map(str::trim)
                .map(|name| {
                    keymap::code_named(name)
                        .ok_or_else(|| format!("{name:?} is not a key this emulator binds"))
                })
                .collect()
        })
        .collect()
}
