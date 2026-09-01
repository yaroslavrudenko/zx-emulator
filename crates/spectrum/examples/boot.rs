//! Boot a 48K, read the screen back as text, and time the run.
//!
//! The M5 gate in one command:
//!
//! ```sh
//! cargo run --release -p spectrum --example boot -- testdata/roms/48.rom
//! ```
//!
//! The message is looked for as a run of **glyphs taken from the ROM's own character set**
//! rather than from a font table written here, so the expected bytes come from the machine
//! under test. A cell that matches no glyph prints `?`, which is what a subtly wrong screen
//! address layout looks like.
//!
//! Two numbers are printed besides the verdict, and both are regression signals rather
//! than assertions: the frame the message first appears on moves whenever anything changes
//! how long the ROM's start-up takes — contention included — and the real-time multiple is
//! the machine-level counterpart of `ARCHITECTURE.md`'s *Measured* section.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use spectrum::screen::{DISPLAY_COLUMNS, DISPLAY_ROWS, pixel_address};
use spectrum::timing::{LINES_PER_FRAME, T_STATES_PER_LINE};
use spectrum::{Memory, Spectrum};

/// Where the ROM's character set starts, for character 32 (space).
///
/// The `CHARS` system variable holds `0x3C00`, which is the font's base *minus* 256 — the
/// font has no glyphs below code 32.
const FONT: u16 = 0x3D00;

/// Bytes per glyph.
const GLYPH: u16 = 8;

/// The character codes the ROM's font covers.
const FIRST_CHARACTER: u8 = 32;
const LAST_CHARACTER: u8 = 127;

/// What the ROM prints once it has finished starting up.
const COPYRIGHT: &str = "\u{a9} 1982 Sinclair Research Ltd";

/// Frames to give the ROM before giving up — four seconds of emulated time, several times
/// what its start-up needs.
const FRAMES: u64 = 200;

/// The frame rate a 48K runs at.
const FRAMES_PER_SECOND: f64 = 50.0;

fn main() -> ExitCode {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "testdata/roms/48.rom".to_owned());

    let rom = match std::fs::read(&path) {
        Ok(rom) => rom,
        Err(err) => {
            eprintln!("cannot read {path}: {err}");
            return ExitCode::FAILURE;
        }
    };
    let mut machine = match Spectrum::new(&rom) {
        Ok(machine) => machine,
        Err(err) => {
            eprintln!("{path}: {err}");
            return ExitCode::FAILURE;
        }
    };

    let wanted = expected_glyphs(machine.memory(), COPYRIGHT);
    let mut appeared = None;

    // Timed around `run_frame` only: the screen scan between frames is this example's
    // cost, not the machine's, and including it would report a throughput nobody runs at.
    let mut emulating = Duration::ZERO;
    for _ in 0..FRAMES {
        let started = Instant::now();
        machine.run_frame();
        emulating += started.elapsed();

        if appeared.is_none() && find_glyph_run(machine.memory(), &wanted).is_some() {
            appeared = Some(machine.frames());
        }
    }
    let elapsed = emulating.as_secs_f64();

    for line in read_screen(machine.memory()) {
        println!("|{line}|");
    }

    let state = machine.cpu_state();
    println!();
    println!(
        "after {FRAMES} frames: PC={:#06X} SP={:#06X} IM={:?} IFF1={} halted={} border={}",
        state.pc,
        state.sp,
        state.im,
        state.iff1,
        state.halted,
        machine.border().index(),
    );
    println!("fault: {:?}", machine.fault());
    match appeared {
        Some(frame) => println!("copyright message first drawn on frame {frame}"),
        None => println!("copyright message never drawn"),
    }
    println!(
        "{} T-states in {elapsed:.3}s — {:.0}x real time",
        u64::from(T_STATES_PER_LINE * LINES_PER_FRAME) * FRAMES,
        FRAMES as f64 / FRAMES_PER_SECOND / elapsed,
    );

    println!(
        "\nM5 gate — screen contains {COPYRIGHT:?}: {}",
        if appeared.is_some() { "YES" } else { "NO" }
    );
    if appeared.is_some() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// The glyph bytes the ROM would draw for `text`, taken from its own character set.
fn expected_glyphs(memory: &Memory, text: &str) -> Vec<[u8; 8]> {
    text.chars().map(|c| glyph(memory, encode(c))).collect()
}

/// Where `wanted` appears on the screen as a horizontal run of character cells.
fn find_glyph_run(memory: &Memory, wanted: &[[u8; 8]]) -> Option<(usize, usize)> {
    if wanted.is_empty() || wanted.len() > DISPLAY_COLUMNS {
        return None;
    }
    (0..DISPLAY_ROWS).find_map(|row| {
        (0..=DISPLAY_COLUMNS - wanted.len())
            .find(|&column| {
                wanted
                    .iter()
                    .enumerate()
                    .all(|(offset, cell)| read_cell(memory, column + offset, row) == *cell)
            })
            .map(|column| (row, column))
    })
}

/// The display file decoded into 24 lines of 32 characters.
fn read_screen(memory: &Memory) -> Vec<String> {
    (0..DISPLAY_ROWS)
        .map(|row| {
            (0..DISPLAY_COLUMNS)
                .map(|column| character_at(memory, column, row))
                .collect()
        })
        .collect()
}

/// The character whose glyph is drawn in cell `(column, row)`, or `?` if none is.
fn character_at(memory: &Memory, column: usize, row: usize) -> char {
    let cell = read_cell(memory, column, row);
    if cell == [0; 8] {
        return ' ';
    }
    (FIRST_CHARACTER..=LAST_CHARACTER)
        .find(|&code| glyph(memory, code) == cell)
        .map_or('?', decode)
}

/// The eight bitmap bytes of one character cell.
fn read_cell(memory: &Memory, column: usize, row: usize) -> [u8; 8] {
    let mut cell = [0; 8];
    for (line, byte) in cell.iter_mut().enumerate() {
        let pixel_line = (row * 8 + line) as u8;
        *byte = memory.read(pixel_address(column as u8, pixel_line));
    }
    cell
}

/// The ROM's own glyph for `code`.
fn glyph(memory: &Memory, code: u8) -> [u8; 8] {
    let base = FONT + u16::from(code.saturating_sub(FIRST_CHARACTER)) * GLYPH;
    let mut bytes = [0; 8];
    for (offset, byte) in bytes.iter_mut().enumerate() {
        *byte = memory.read(base + offset as u16);
    }
    bytes
}

/// The ZX character set is ASCII except for its last two printable codes.
fn decode(code: u8) -> char {
    match code {
        0x60 => '\u{a3}', // POUND SIGN
        0x7F => '\u{a9}', // COPYRIGHT SIGN
        other => char::from(other),
    }
}

/// The inverse of [`decode`], for the characters this example needs.
fn encode(character: char) -> u8 {
    match character {
        '\u{a3}' => 0x60,
        '\u{a9}' => 0x7F,
        other => u8::try_from(other).unwrap_or(b'?'),
    }
}
