//! Shared fixtures for the M7 gates — the 128-shaped ones only.
//!
//! `tests/common/mod.rs` is the home for everything model-independent and this file does not
//! duplicate any of it: every gate here declares both modules and reaches through
//! `crate::common` for `write_program`, `set_pc`, `advance_to` and the rest. What lives here is
//! the three things that module cannot express, because it was written for a machine with one
//! ROM and no paging port.
//!
//! # Positioning a 128, and why `common::advance_to` is still the right tool
//!
//! `common::elapsed` computes `frames * T_STATES_PER_FRAME + frame_t_state` against the **48K**
//! constant, so it is wrong for a 128 the moment a frame boundary is crossed. Every measurement
//! here therefore stays inside frame zero, where `frames` is 0 and the constant never enters
//! the arithmetic — and `common::advance_to` asserts its own landing position, so a gate that
//! drifted out of frame zero would fail loudly rather than measure the wrong thing.
//!
//! [`elapsed`] here is the one that asks the machine for its own frame length. It exists so a
//! gate that *does* cross a boundary has a correct instrument, and so that the difference
//! between the two is written down rather than discovered.

#![allow(dead_code)]

use spectrum::Spectrum;
use spectrum::memory::PAGE_SIZE;
use spectrum::timing::Timing;

/// A 16 KB ROM image whose every byte is its own low address byte, XORed with `seed`.
///
/// Two ROMs built with different seeds are distinguishable at **every** address, which is what
/// makes "which ROM is paged in" answerable by reading one byte rather than by trusting a slot
/// comparison.
#[must_use]
pub fn pattern_rom(seed: u8) -> Vec<u8> {
    (0..PAGE_SIZE).map(|i| ((i & 0xFF) as u8) ^ seed).collect()
}

/// The seed [`machine_128`]'s editor ROM is built with.
pub const EDITOR_SEED: u8 = 0x00;

/// The seed [`machine_128`]'s 48 BASIC ROM is built with.
pub const BASIC_SEED: u8 = 0xFF;

/// A 128 holding two distinguishable pattern ROMs, at the start of frame zero.
///
/// Nothing executes the ROMs — every gate here sets `PC` into RAM first — so a pattern is more
/// useful than the real images: it tells a ROM byte from an unwritten RAM byte from a byte a
/// test wrote, at a glance.
#[must_use]
pub fn machine_128() -> Spectrum {
    Spectrum::spectrum_128(&pattern_rom(EDITOR_SEED), &pattern_rom(BASIC_SEED))
        .expect("two page-sized ROMs")
}

/// The real Sinclair 128 ROMs as `(editor, 48 BASIC)`, or `None` when the shared policy says
/// this run may skip them.
///
/// Absence goes through `crates/testsupport` exactly as `common::sinclair_rom` does — the same
/// convention, not a new one — because a missing corpus must move the pass/fail surface rather
/// than print a notice libtest captures on success.
#[must_use]
pub fn sinclair_rom_128() -> Option<(Vec<u8>, Vec<u8>)> {
    // Unconditionally, not only on the absent path: an obsolete spelling must be an error in
    // *every* gate, or a CI file still exporting one is silently ignored by whichever gate
    // happens to find its corpus present.
    testsupport::reject_obsolete_env();

    let roms = testsupport::testdata_dir().join("roms");
    let editor = roms.join("128-0.rom");
    let basic = roms.join("128-1.rom");
    for path in [&editor, &basic] {
        if !path.is_file() {
            testsupport::skip_absent_corpus("the Sinclair 128 ROMs", path);
            return None;
        }
    }
    Some((read(&editor), read(&basic)))
}

fn read(path: &std::path::Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()))
}

/// T-states elapsed since power-on, using **this machine's own** frame length.
///
/// `common::elapsed` uses the 48K constant, which is right for the machine it was written for
/// and silently wrong for a 128 across a frame boundary. Asking the machine is the only version
/// that is right for both.
#[must_use]
pub fn elapsed(machine: &Spectrum) -> u64 {
    let frame = u64::from(timing(machine).frame_t_states());
    machine.frames() * frame + u64::from(machine.frame_t_state())
}

/// The frame geometry the machine is actually running.
#[must_use]
pub fn timing(machine: &Spectrum) -> Timing {
    machine.ula().clock().timing()
}

/// Run `steps` instructions from `address` and report what the machine's **clock** charged.
///
/// The clock, never the sum of what `step` returns: `docs/MACHINE.md` Decision 1 is a
/// measurement, and contention is added on the bus's side where `step`'s return cannot see it.
/// Every existing assertion of the form `accepted == ACKNOWLEDGE` pins a *nominal* length and
/// can never witness a stall, which is exactly why anything measuring a contended cost has to
/// come through here.
#[must_use]
pub fn cost_of_running(machine: &mut Spectrum, address: u16, steps: usize) -> u64 {
    crate::common::set_pc(machine, address);
    let before = elapsed(machine);
    for _ in 0..steps {
        machine.step();
    }
    elapsed(machine) - before
}

/// `LD BC,port : LD A,value : OUT (C),A`, as bytes — a real paging write.
///
/// Assembled rather than reached through `Ula::out_port` because the two are not the same test:
/// a direct call proves the bus method, and this proves the **wiring** — that a guest executing
/// an ordinary `OUT` reaches the paging port through the CPU, the bus and the decode.
#[must_use]
pub fn out_c_a(port: u16, value: u8) -> Vec<u8> {
    let [low, high] = port.to_le_bytes();
    vec![
        0x01, low, high, // LD BC,port
        0x3E, value, // LD A,value
        0xED, 0x79, // OUT (C),A
    ]
}

/// Instructions [`out_c_a`] assembles, for a caller that has to step exactly that many.
pub const OUT_C_A_STEPS: usize = 3;

/// `LD BC,port : IN A,(C)`, as bytes — a real port read.
///
/// [`out_c_a`]'s partner, and it exists for the same reason: `common::read_port` calls
/// `Ula::in_port` directly, which proves the bus method and cannot see whether a guest's `IN`
/// reaches it. The AY's read arm sits **before** the floating-bus fallback in that function,
/// and an arm placed after it would never run — so the wiring is exactly what needs a gate.
#[must_use]
pub fn in_c_a(port: u16) -> Vec<u8> {
    let [low, high] = port.to_le_bytes();
    vec![
        0x01, low, high, // LD BC,port
        0xED, 0x78, // IN A,(C)
    ]
}

/// Instructions [`in_c_a`] assembles.
pub const IN_C_A_STEPS: usize = 2;

/// Run the assembled `program` from `address`, `steps` instructions of it.
pub fn run_program(machine: &mut Spectrum, address: u16, program: &[u8], steps: usize) {
    crate::common::write_program(machine, address, program);
    crate::common::set_pc(machine, address);
    for _ in 0..steps {
        machine.step();
    }
}

/// The AY's register-select and register-read port.
pub const AY_SELECT_PORT: u16 = 0xFFFD;

/// The AY's register-write port.
pub const AY_WRITE_PORT: u16 = 0xBFFD;

/// Select `register` and write `value` to it, the way a guest does.
///
/// Two real `OUT (C),A` instructions through `AY_SELECT_PORT` and `AY_WRITE_PORT`, never a
/// direct call into the chip: the port decode and the bus wiring are part of what a gate using
/// this is grading, and a helper that reached past them would grade neither.
pub fn ay_poke(machine: &mut Spectrum, register: u8, value: u8) {
    ay_poke_via(machine, AY_SELECT_PORT, AY_WRITE_PORT, register, value);
}

/// The same, through a caller's choice of ports, so the decode family can be exercised.
pub fn ay_poke_via(machine: &mut Spectrum, select: u16, write: u16, register: u8, value: u8) {
    run_program(machine, PROGRAM, &out_c_a(select, register), OUT_C_A_STEPS);
    run_program(machine, PROGRAM, &out_c_a(write, value), OUT_C_A_STEPS);
}

/// Read the selected register the way a guest does, and report what landed in `A`.
pub fn ay_peek(machine: &mut Spectrum) -> u8 {
    ay_peek_via(machine, AY_SELECT_PORT)
}

/// The same, through a caller's choice of port.
pub fn ay_peek_via(machine: &mut Spectrum, port: u16) -> u8 {
    run_program(machine, PROGRAM, &in_c_a(port), IN_C_A_STEPS);
    (machine.cpu_state().af >> 8) as u8
}

/// Where [`ay_poke`] and its neighbours assemble: bank 2, which no paging value moves.
const PROGRAM: u16 = crate::common::PROLOGUE;

/// The screen read back as text, matched against a font taken from **an explicit ROM image**.
///
/// # Why this exists rather than [`spectrum::screen::read_text`]
///
/// `read_text` takes the font from address `0x3D00` *through the slot map*, and on a 48K that is
/// always the one ROM there is. **On a 128 it is whichever ROM happens to be paged**, and the
/// 128 editor ROM does not hold a character set at `0x3D00` — measured, not assumed: its bytes
/// there are `3F C1 38 79 2A 7F FD 7C` where a space should be all zeros.
///
/// That matters because the 128's menu loop pages ROM 1 in and out **every frame** to reach the
/// 48K ROM's routines. So `read_text` on a booted 128 answers correctly or answers `?` depending
/// on which frame it is called in, and neither answer is wrong — the function is doing exactly
/// what it documents. It is a 48K instrument being pointed at a two-ROM machine.
///
/// Taking the font from a ROM image the caller already holds removes the dependency entirely,
/// and it makes the expectation **independent of the machine under test** — the same reason
/// `common::MEMBRANE` is transcribed rather than derived from `spectrum::keyboard`.
#[must_use]
pub fn screen_text(machine: &Spectrum, font_rom: &[u8]) -> Vec<String> {
    /// Where the ROM's character set starts: the glyph for code 32, a space.
    const FONT: usize = 0x3D00;
    const GLYPH: usize = 8;

    let page = machine.memory().bank(machine.memory().screen_bank());
    (0..24)
        .map(|row| {
            (0..32)
                .map(|column| glyph_at(page, font_rom, row, column, FONT, GLYPH))
                .collect()
        })
        .collect()
}

/// One cell of [`screen_text`], as the character whose glyph its eight bytes match.
fn glyph_at(
    page: &[u8; PAGE_SIZE],
    font_rom: &[u8],
    row: usize,
    column: usize,
    font: usize,
    glyph: usize,
) -> char {
    let mut cell = [0_u8; 8];
    for (line, byte) in cell.iter_mut().enumerate() {
        let address = spectrum::screen::pixel_address(
            u8::try_from(column).expect("32 columns"),
            u8::try_from(row * 8 + line).expect("192 lines"),
        );
        *byte = page[usize::from(address - spectrum::screen::DISPLAY_FILE)];
    }
    if cell == [0; 8] {
        return ' ';
    }
    (32_u8..=127)
        .find(|&code| {
            let base = font + (usize::from(code) - 32) * glyph;
            font_rom.get(base..base + glyph) == Some(&cell[..])
        })
        .map_or('?', |code| match code {
            0x60 => '\u{a3}', // POUND SIGN
            0x7F => '\u{a9}', // COPYRIGHT SIGN
            other => other as char,
        })
}

/// Press `keys` together, hold them, then let go — one keystroke as a person makes it.
///
/// The ROM debounces and scans once per frame, so a key pressed and released inside one frame
/// can be missed entirely. Four frames held and six released is comfortably outside that, and
/// the figures are deliberately generous: this is a fixture for reaching a screen, not a
/// measurement of the ROM's key-repeat timing, which nothing here grades.
pub fn tap(machine: &mut Spectrum, keys: &[spectrum::Key]) {
    for &key in keys {
        machine.keyboard_mut().press(key);
    }
    machine.run_frames(4);
    for &key in keys {
        machine.keyboard_mut().release(key);
    }
    machine.run_frames(6);
}
