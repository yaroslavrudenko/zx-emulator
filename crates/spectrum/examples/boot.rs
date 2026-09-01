//! Boot a 48K, print the screen back as text, and time the run.
//!
//! ```sh
//! cargo run --release -p spectrum --example boot -- testdata/roms/48.rom
//! ```
//!
//! **This is a demonstration, not the gate.** `cargo test` builds an example and never calls
//! its `main`, so for as long as this file was the only thing checking that the ROM boots,
//! nothing checked it: deleting `testdata/roms/48.rom` left the suite green. The gate is
//! `tests/boot.rs`, which asserts the same two facts this prints.
//!
//! The screen decoding lives in [`spectrum::screen::read_text`] rather than here, because an
//! example cannot share code with a test and the alternative was two copies of it. It matches
//! each cell against the glyphs of the character set **in the machine under test**, so a cell
//! that matches nothing prints `?` — which is what a subtly wrong address layout looks like.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use spectrum::screen::read_text;
use spectrum::timing::{LINES_PER_FRAME, T_STATES_PER_LINE};
use spectrum::{Memory, Spectrum};

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

    let mut appeared = None;

    // Timed around `run_frame` only: the screen scan between frames is this example's cost,
    // not the machine's, and including it would report a throughput nobody runs at.
    let mut emulating = Duration::ZERO;
    for _ in 0..FRAMES {
        let started = Instant::now();
        machine.run_frame();
        emulating += started.elapsed();

        if appeared.is_none() && shows_copyright(machine.memory()) {
            appeared = Some(machine.frames());
        }
    }
    let elapsed = emulating.as_secs_f64();

    for line in read_text(machine.memory()) {
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

/// Whether the copyright message is on the screen now.
fn shows_copyright(memory: &Memory) -> bool {
    read_text(memory)
        .iter()
        .any(|line| line.contains(COPYRIGHT))
}
