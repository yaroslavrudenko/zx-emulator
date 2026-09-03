//! Gate: the border painted as the beam painted it, not as one colour for the frame.
//!
//! # What is graded, and it is less than the effect
//!
//! A loading screen looks right or it does not, and that is **observation** — `docs/MACHINE.md`
//! puts exactly this software in that tier and there is no oracle for it here. So this file
//! does not claim to grade the appearance. What it grades is the three things that are
//! arithmetic rather than appearance:
//!
//! | | Class | Why it is worth a gate |
//! |---|---|---|
//! | A write at a known T-state lands in the row the timing model says | **proven** | The mapping is the only place a second beam-position model could have crept in beside contention's, and two mappings that must agree is the defect class this project keeps catching |
//! | A frame with no border writes is **byte-identical** to what the previous implementation drew | **proven** | The strongest regression gate available here and it costs nothing: every existing screen gate keeps its meaning only if this holds |
//! | Several writes inside one row collapse to the last | **proven** | The stated limit of a per-row record, gated so it is a known cost rather than a discovery |
//!
//! **What no test here establishes:** that the bands are in the right *place* on a real
//! Spectrum, that their colours are the ROM's, or that a loading screen looks like one. The
//! last gate below runs the real ROM's loader against a real tape and asserts that bands
//! appear **and that they are more than one** — which is a long way short of "it looks right",
//! and is said that way rather than dressed up.
//!
//! # Why the expectations are literals
//!
//! Every row number below is hand-derived from the frame geometry and written out. Computing
//! them from `screen`'s own mapping would be the keyboard-matrix tautology `docs/STATUS.md`
//! records — *"a test whose expectation is computed by the subject is not a weak test; it is a
//! tautology"* — and the mapping is precisely what needs grading.

mod common;
mod m7_common;

use common::{advance_to, machine, set_pc, write_program};
use m7_common::machine_128;
use spectrum::screen::{BORDER, FRAME_HEIGHT, FRAME_WIDTH};
use spectrum::timing::{FIRST_CONTENDED_T_STATE, T_STATES_PER_LINE, Timing};
use spectrum::{Colour, Frame, Spectrum, screen};

/// Where the `OUT` under test is assembled: bank 0 on both machines, and clear of the
/// positioning prologue `advance_to` writes at `common::PROLOGUE`.
const PROGRAM: u16 = common::UNCONTENDED_CODE;

/// `OUT (n),A`, two bytes, eleven T-states.
const OUT_N_A: u8 = 0xD3;

/// `LD A,n`, two bytes, seven T-states.
const LD_A_N: u8 = 0x3E;

/// The low half of the ULA port.
const ULA_PORT_LOW: u8 = 0xFE;

/// T-states from the start of an `OUT (n),A` to the moment the port cycle opens: an M1 fetch
/// and an operand read.
const OUT_TO_PORT_CYCLE: u32 = 7;

/// T-states an `LD A,n : OUT (n),A` pair spends before the write reaches the port.
const WRITE_AT: u32 = 7 + OUT_TO_PORT_CYCLE;

/// The frame line the display's first pixel row falls on, **hand-derived per machine**.
///
/// A 48K contends from 14335, which is one T-state short of `64 x 224`, so the line containing
/// it is the last border line and the display starts on line 64. A 128 contends from **14362**,
/// two T-states short of `63 x 228`, so its display starts on line 63. *(This said 14361 and
/// "three T-states short", and the line number it derives is unchanged either way — which is
/// exactly why nothing noticed when `timing_oracle.rs` moved the constant on 2026-09-02.)*
const DISPLAY_LINE_48K: u32 = 64;
const DISPLAY_LINE_128: u32 = 63;

/// The frame line rendered row 0 shows: [`BORDER`] lines above the display.
const ROW_0_LINE_48K: u32 = DISPLAY_LINE_48K - BORDER as u32;
const ROW_0_LINE_128: u32 = DISPLAY_LINE_128 - BORDER as u32;

// The two derivations, written out so they are readable rather than inferred from the four
// constants above.
//
// **Against the shipped constants rather than against literals, and that is the repair.** These
// read `- 14335 == 1` and `- 14361 == 3`. The 48K literal was right; the 128 literal became
// wrong on 2026-09-02 and **this assertion stayed green**, because a literal cannot disagree
// with a constant it does not mention. Reading the offset out of `Timing` makes the derivation
// go red the moment the number under it moves, which is the whole reason it was written down.
const _: () = assert!(DISPLAY_LINE_48K * T_STATES_PER_LINE - FIRST_CONTENDED_T_STATE == 1);
const _: () = assert!(
    DISPLAY_LINE_128 * Timing::SPECTRUM_128.t_states_per_line()
        - Timing::SPECTRUM_128.first_contended_t_state()
        == 2
);
const _: () = assert!(ROW_0_LINE_48K == 32 && ROW_0_LINE_128 == 31);

/// Put a border write of `colour` on the machine so that it reaches the port at exactly
/// `at` T-states into frame zero.
///
/// The program is `LD A,colour : OUT (0xFE),A`, so the write lands [`WRITE_AT`] T-states after
/// `PC` is set. **Every caller keeps `at` below [`FIRST_CONTENDED_T_STATE`]**, where the ULA's
/// I/O stall is zero and the arithmetic is therefore exact rather than a subtraction of the
/// contention model this file is not trying to grade.
fn write_border_at(machine: &mut Spectrum, at: u32, colour: u8) {
    assert!(
        at < FIRST_CONTENDED_T_STATE,
        "{at} is inside the display window, where the ULA stalls the write and the landing \
         T-state is no longer the caller's arithmetic"
    );
    advance_to(machine, at - WRITE_AT);
    write_program(machine, PROGRAM, &[LD_A_N, colour, OUT_N_A, ULA_PORT_LOW]);
    set_pc(machine, PROGRAM);
    machine.step();
    machine.step();
    assert_eq!(
        machine.frame_t_state(),
        at + 4,
        "the write should have reached the port at {at}, with the cycle's four T-states after"
    );
}

/// The border colour of each rendered row, read out of a rendered frame.
///
/// Column zero, which is border on every row: the display never reaches it.
fn border_rows(frame: &Frame) -> Vec<u8> {
    (0..FRAME_HEIGHT)
        .map(|row| frame.pixel(0, row).expect("inside the frame").index())
        .collect()
}

/// Render `machine` and report its border rows.
fn rendered_rows(machine: &Spectrum) -> Vec<u8> {
    let mut frame = Frame::new();
    machine.render(&mut frame);
    border_rows(&frame)
}

// ---------------------------------------------------------------------------
// The regression gate: nothing changes when nothing writes the border
// ---------------------------------------------------------------------------

#[test]
fn a_frame_with_no_border_writes_is_byte_identical_to_a_uniform_render() {
    // **The gate that keeps every other screen test meaning what it meant.** `screen::render`
    // is now a projection of the same drawing loop against a uniform border, so this is the
    // assertion that the projection is exact — not merely equivalent-looking, byte-identical
    // over all 81920 pixels.
    for mut machine in [machine(), machine_128()] {
        // Something on the screen, so this is not two black frames agreeing.
        for offset in 0..spectrum::screen::DISPLAY_FILE_LEN as u16 {
            machine.memory_mut().write(
                spectrum::screen::DISPLAY_FILE + offset,
                (offset % 251) as u8,
            );
        }
        for offset in 0..spectrum::screen::ATTRIBUTE_FILE_LEN as u16 {
            machine
                .memory_mut()
                .write(spectrum::screen::ATTRIBUTE_FILE + offset, 0x47);
        }
        machine.run_frames(2);

        let mut through_the_machine = Frame::new();
        machine.render(&mut through_the_machine);

        let mut uniform = Frame::new();
        screen::render(
            machine.memory(),
            machine.border(),
            screen::flash_phase(machine.frames()),
            &mut uniform,
        );
        assert!(
            through_the_machine == uniform,
            "a frame nobody wrote the border in must render exactly as one border colour"
        );
        assert!(
            through_the_machine
                .as_slice()
                .iter()
                .any(|&c| c != Colour::BLACK),
            "and the comparison must be over a frame with something in it"
        );
    }
}

#[test]
fn a_frame_whose_border_write_was_last_frame_renders_uniform() {
    // The record describes one frame. A frame in which nothing wrote the border had one
    // border colour — the one still standing — and that is right rather than a fallback.
    let mut machine = machine();
    write_border_at(&mut machine, 8_000, 2);
    assert!(rendered_rows(&machine).contains(&2));

    machine.run_frames(2);
    let rows = rendered_rows(&machine);
    assert!(
        rows.iter().all(|&c| c == 2),
        "the next frame is uniformly the colour left standing, got {rows:?}"
    );
}

#[test]
fn the_frontends_own_loop_shows_the_bands() {
    // **The defect this gate exists for, as its own failing case and with the corpus taken
    // out of it.** A frontend runs `run_frame(); render();`, and `run_frame` returns the
    // instant the frame *counter* advances — so at the moment it renders, the machine stands a
    // few T-states into the next frame and the record describes the one just finished.
    //
    // A record served only for "the frame running now" therefore shows a frontend a uniform
    // border **every time**, while passing every test that renders mid-frame. It did, and this
    // is what caught it.
    let mut machine = machine();
    write_border_at(&mut machine, 8_000, 2);
    // Mid-frame, which is the call pattern that never had the bug.
    assert!(
        rendered_rows(&machine).contains(&2),
        "mid-frame renders the band"
    );

    // And now the frontend's, with nothing between the two calls.
    machine.run_frame();
    let rows = rendered_rows(&machine);
    assert!(
        rows.contains(&2) && rows.contains(&0),
        "a frontend rendering straight after run_frame must see the band it just drew: {rows:?}"
    );
}

// ---------------------------------------------------------------------------
// The mapping: a write at a known T-state lands in a known row
// ---------------------------------------------------------------------------

#[test]
fn a_border_write_first_shows_on_the_row_whose_line_begins_after_it() {
    // **Hand-derived, both machines.** Rendered row `r` shows frame line `ROW_0_LINE + r`,
    // which begins at `(ROW_0_LINE + r) * t_states_per_line`. A write shows from the first row
    // whose line begins **at or after** it — so a write landing exactly on a line boundary
    // shows on that row, and one T-state later shows on the next.
    //
    // Every case is in the top border, where the ULA's I/O stall is zero.
    for (name, timing, row_0_line, build) in [
        (
            "48K",
            Timing::SPECTRUM_48K,
            ROW_0_LINE_48K,
            machine as fn() -> Spectrum,
        ),
        (
            "128",
            Timing::SPECTRUM_128,
            ROW_0_LINE_128,
            machine_128 as fn() -> Spectrum,
        ),
    ] {
        let line = timing.t_states_per_line();
        for row in [1_u32, 5, 20, 31] {
            let boundary = (row_0_line + row) * line;

            let mut on_it = build();
            write_border_at(&mut on_it, boundary, 3);
            let rows = rendered_rows(&on_it);
            assert_eq!(
                rows[row as usize - 1],
                0,
                "{name}: a write at {boundary} must not reach row {}",
                row - 1
            );
            assert_eq!(
                rows[row as usize],
                3,
                "{name}: a write landing on line {}'s first T-state shows on row {row}",
                row_0_line + row
            );

            let mut one_late = build();
            write_border_at(&mut one_late, boundary + 1, 3);
            let rows = rendered_rows(&one_late);
            assert_eq!(
                rows[row as usize], 0,
                "{name}: one T-state past the boundary must not reach row {row}"
            );
            assert_eq!(
                rows[row as usize + 1],
                3,
                "{name}: it shows on the next row instead"
            );
        }
    }
}

#[test]
fn moving_a_write_by_one_line_moves_the_band_by_one_row() {
    // The relative form of the claim above, and the one that is immune to the exact T-state a
    // write lands at: whatever the offset, one line of movement is one row of movement. A
    // model that had scaled the mapping — a 48K's line length used on a 128, say — passes the
    // absolute test for row 0 and fails this one everywhere else.
    for (name, timing, row_0_line, build) in [
        (
            "48K",
            Timing::SPECTRUM_48K,
            ROW_0_LINE_48K,
            machine as fn() -> Spectrum,
        ),
        (
            "128",
            Timing::SPECTRUM_128,
            ROW_0_LINE_128,
            machine_128 as fn() -> Spectrum,
        ),
    ] {
        let line = timing.t_states_per_line();
        // Four lines into the *rendered* frame, not four into the frame: a first draft used
        // `8 * line`, which on a 48K is T-state 1792 — above the top of what is drawn, where
        // every write clamps to row 0 and moving it changes nothing.
        let base = (row_0_line + 4) * line;
        let first = first_coloured_row(build(), base);
        for step in 1..=6_u32 {
            let moved = first_coloured_row(build(), base + step * line);
            assert_eq!(
                moved,
                first + step as usize,
                "{name}: {step} lines later should be {step} rows lower"
            );
        }
    }
}

/// Put one border write at `at` on a fresh machine and report the first row it coloured.
fn first_coloured_row(mut machine: Spectrum, at: u32) -> usize {
    write_border_at(&mut machine, at, 5);
    rendered_rows(&machine)
        .into_iter()
        .position(|colour| colour == 5)
        .expect("the write must colour something")
}

// ---------------------------------------------------------------------------
// The limit of a per-row record, gated rather than left to be discovered
// ---------------------------------------------------------------------------

#[test]
fn several_writes_inside_one_row_collapse_to_the_last() {
    // **The stated cost of per-row resolution.** A row is painted with what was in effect when
    // it began, and by then every write inside the preceding line has happened — so the last
    // one wins. Border-multicolour demos rewrite `0xFE` many times per line and every one of
    // those writes lands here; this is what that looks like.
    let mut machine = machine();
    let line = T_STATES_PER_LINE;
    let boundary = (ROW_0_LINE_48K + 10) * line;

    // Two writes inside the line *before* row 10 begins: both map to row 10, and the second
    // is the one in effect when the row starts.
    // **One T-state past the previous boundary, not on it.** A first draft started the pair
    // exactly on `boundary - line`, which is itself a line's first T-state — so the first
    // write mapped to that row and the second to the next, and the two never shared a row at
    // all. The test was placing them wrongly; the model was right.
    advance_to(&mut machine, boundary - line + 1 - WRITE_AT);
    write_program(
        &mut machine,
        PROGRAM,
        &[
            LD_A_N,
            1,
            OUT_N_A,
            ULA_PORT_LOW, // the first, discarded
            LD_A_N,
            6,
            OUT_N_A,
            ULA_PORT_LOW, // the second, which shows
        ],
    );
    set_pc(&mut machine, PROGRAM);
    for _ in 0..4 {
        machine.step();
    }
    assert!(
        machine.frame_t_state() < boundary,
        "both writes must land before row 10 begins"
    );

    let rows = rendered_rows(&machine);
    assert_eq!(rows[10], 6, "the later write is the one the row shows");
    assert!(
        !rows.contains(&1),
        "and the earlier one appears nowhere: {rows:?}"
    );
}

#[test]
fn a_write_below_the_visible_frame_still_sets_the_border() {
    // A write during the vertical flyback paints no band this frame — the beam is not drawing
    // — but it is still the colour the machine is showing, and the *next* frame starts from
    // it. A model that dropped it entirely would lose a border change.
    let mut machine = machine();
    let below = (ROW_0_LINE_48K + FRAME_HEIGHT as u32) * T_STATES_PER_LINE;
    assert!(below < spectrum::timing::T_STATES_PER_FRAME);

    // Positioning that far into the frame runs through the display window, so this one is
    // placed by running the machine rather than by the exact-arithmetic helper.
    advance_to(&mut machine, 8_000);
    write_program(&mut machine, PROGRAM, &[LD_A_N, 4, OUT_N_A, ULA_PORT_LOW]);
    while machine.frame_t_state() < below {
        machine.step();
    }
    set_pc(&mut machine, PROGRAM);
    machine.step();
    machine.step();

    assert_eq!(machine.border(), Colour::new(4), "the border moved");

    // Rendered right after that frame, the picture is the one the beam drew: no band, because
    // the write happened after the beam had left the bottom of what is rendered.
    machine.run_frame();
    let during = rendered_rows(&machine);
    assert!(
        during.iter().all(|&colour| colour == 0),
        "a write in the flyback paints no band in the frame it lands in: {during:?}"
    );

    // And the frame *after* that starts from it, which is what makes the write a real change
    // rather than one that was dropped.
    machine.run_frame();
    let after = rendered_rows(&machine);
    assert!(
        after.iter().all(|&colour| colour == 4),
        "the next frame starts from it: {after:?}"
    );
}

#[test]
fn a_restore_drops_the_bands_the_saved_machine_painted() {
    // A restore is not elapsed time, so the machine being restored into did not paint them.
    // Keeping them would draw a history that never happened here — the same reason
    // `Ula::set_border` exists rather than routing a restore through `out_port`.
    let mut source = machine();
    write_border_at(&mut source, 8_000, 2);
    assert!(rendered_rows(&source).contains(&2));
    let snapshot = source.snapshot();

    let mut target = machine();
    target.restore(&snapshot).expect("both machines are 48K");
    let rows = rendered_rows(&target);
    assert!(
        rows.iter().all(|&colour| colour == 2),
        "a restored machine shows its border colour, not the saved machine's bands: {rows:?}"
    );
}

// ---------------------------------------------------------------------------
// The effect itself, at the tier it belongs to
// ---------------------------------------------------------------------------

#[test]
fn the_roms_own_loader_paints_more_than_one_band() {
    // **The thing that prompted this, at the honest tier.** It runs the real 48K ROM's
    // `LD-BYTES` against a real tape and asserts that the border comes out in *bands* rather
    // than one colour. That is a long way short of "a loading screen looks right", which is
    // observation and which nothing here can automate.
    //
    // What it does establish is that the mechanism fires on the software it was built for, at
    // a thickness the measurement predicted: the loader changes the border every 1884 to 2159
    // T-states, which at 224 T-states a line is a band every 8.4 to 9.6 rows — so a frame of
    // loading should show on the order of 25 to 30 bands, not two and not two hundred.
    let Some(rom) = common::sinclair_rom() else {
        return;
    };
    let mut machine = loading_machine(&rom);

    // Run **exactly as a frontend does** — `run_frame` then `render`, with nothing in
    // between. That is the call pattern that was broken, and it is why this loop is written
    // as one rather than as `run_frames(n)` followed by a render.
    //
    // The frame count is polled rather than fixed because a `.tap`'s leading pause is derived
    // from the machine's own frame length, not chosen here: measured, the loader sits silent
    // for **3497263 T-states** — about fifty frames — before its first border edge. A first
    // draft rendered at frame 20 and got a uniform frame, which was the pause and not the
    // mechanism.
    let mut rows = Vec::new();
    for _ in 0..MAX_FRAMES_TO_THE_PILOT {
        machine.run_frame();
        rows = rendered_rows(&machine);
        if rows.windows(2).any(|pair| pair[0] != pair[1]) {
            break;
        }
    }

    let bands = rows.windows(2).filter(|pair| pair[0] != pair[1]).count();
    assert!(
        bands >= 8,
        "a loading frame should be banded, not one colour: {bands} changes in {rows:?}"
    );
    let thickness = FRAME_HEIGHT / (bands + 1);
    assert!(
        (4..=20).contains(&thickness),
        "bands of {thickness} rows are not what a 1884-to-2159 T-state edge rate predicts"
    );
    assert!(
        rows.iter().collect::<std::collections::BTreeSet<_>>().len() >= 2,
        "and there must be at least two colours in them"
    );
}

/// Frames to give the loader before concluding it is not flashing the border.
///
/// The measured pause is about fifty; this is generous enough that a change to `tape::tap`'s
/// pause constant does not turn this gate red for a reason that has nothing to do with it,
/// and tight enough that a loader which never flashes fails rather than hangs.
const MAX_FRAMES_TO_THE_PILOT: usize = 300;

/// A 48K running the ROM's own `LD-BYTES` against a tape of our own making.
fn loading_machine(rom: &[u8]) -> Spectrum {
    /// `LD-BYTES`, the ROM's tape reader.
    const LD_BYTES: u16 = 0x0556;
    const STUB: u16 = 0xC000;
    const DESTINATION: u16 = 0x9000;
    const STACK: u16 = 0xBF00;
    const PAYLOAD_LEN: u16 = 256;

    let mut block = vec![0xFF_u8];
    block.extend(std::iter::repeat_n(0xA5_u8, PAYLOAD_LEN as usize));
    let parity = block.iter().fold(0_u8, |a, b| a ^ b);
    block.push(parity);
    let mut file = Vec::new();
    file.extend_from_slice(&(block.len() as u16).to_le_bytes());
    file.extend_from_slice(&block);

    let tape = spectrum::tape::tap::parse(&file).expect("a well-formed block");
    let mut machine = Spectrum::new(rom).expect("the 48K ROM is one page");

    // LD IX,DESTINATION : LD DE,len : LD A,0xFF : SCF : CALL LD-BYTES : JR $
    let mut code = vec![0xDD, 0x21];
    code.extend_from_slice(&DESTINATION.to_le_bytes());
    code.push(0x11);
    code.extend_from_slice(&PAYLOAD_LEN.to_le_bytes());
    code.extend_from_slice(&[0x3E, 0xFF, 0x37, 0xCD]);
    code.extend_from_slice(&LD_BYTES.to_le_bytes());
    code.extend_from_slice(&[0x18, 0xFE]);
    write_program(&mut machine, STUB, &code);

    common::with_cpu_state(&mut machine, |state| {
        state.pc = STUB;
        state.sp = STACK;
    });
    machine.insert_tape(tape);
    machine.tape_mut().play();
    machine
}

// The frame's own geometry, asserted once so a change to `BORDER` or `FRAME_WIDTH` shows up
// here rather than as a mysteriously shifted band.
const _: () = assert!(FRAME_WIDTH == 320 && FRAME_HEIGHT == 256);
const _: () = assert!(BORDER == 32);
