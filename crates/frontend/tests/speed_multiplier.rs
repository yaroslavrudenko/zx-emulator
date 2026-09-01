//! Running faster than a real Spectrum, and the two claims that has to survive.
//!
//! # What this grades
//!
//! 1. **The arithmetic**, in the shape `tests/pacing_accounting.rs` established: literal
//!    [`Duration`] sequences against literal `(run, dropped)` pairs, now at every multiplier
//!    [`SPEEDS`] holds. The one that matters is the **catch-up bound**, because it is the place a
//!    naive multiplier goes wrong silently: [`MAX_CATCH_UP`] is four *frames* standing in for
//!    eighty *milliseconds*, and above real time those stop being the same sentence.
//!
//! 2. **That the machine does not notice.** The whole claim of this feature is that nothing is
//!    bypassed — `docs/M6.md` Decision 4 rules out the ROM trap and
//!    `crates/spectrum/tests/tape_rom_load.rs`'s `no_shortcut_exists_past_the_ear_bit` keeps it
//!    ruled out — so a tape still loads by its own signal and a turbo loader still works. That is
//!    only true if 8× and 1× produce the *same machine*, and an argument is not a measurement. Two
//!    Spectrums are therefore run to the same frame count through pacers at different speeds and
//!    compared: the CPU state, and every one of the 327,680 bytes the window would have uploaded.
//!
//! # What it does not grade
//!
//! Whether fast-forward *looks* like anything. The pixels compared here are compared to each
//! other; nobody watched them. That is [`frontend`](frontend)'s standing row — *whether it looks
//! right* has no oracle in this crate — and the multiplier does not change it.
//!
//! Nor does it grade the **audio** decision. `src/main.rs` mutes above real time, and the reason
//! is written at the push site; what a person hears is this crate's other standing gap, and no
//! green here bears on it.
//!
//! # Why the ROM is written here rather than fetched
//!
//! `crates/testsupport` exists because a gate backed by a corpus is a gate that might not run, and
//! the claim under test needs a machine that *does something observable per frame* rather than a
//! Sinclair one. So [`painting_rom`] is eight instructions assembled below — the same move
//! `crates/frontend/gate-bundled.sh` makes, for the same reason, and this file runs on a clean
//! checkout with no `testdata/` at all.

use std::time::Duration;

use frontend::pacing::{MAX_CATCH_UP, Pacer, SPEEDS, Speed};
use frontend::palette::{self, RGBA_BYTES};
use spectrum::memory::PAGE_SIZE;
use spectrum::{Frame, Spectrum};

/// One frame of a 50 Hz display, which is what the desktop shell is handed when it keeps up.
const TICK: Duration = Duration::from_millis(20);

/// A display tick that owes exactly [`MAX_CATCH_UP`] frames at real time.
///
/// The boundary the bound is written in terms of: at every multiplier this must lose nothing,
/// because it is the longest tick that is not yet a stall.
const AT_THE_BOUND: Duration = Duration::from_millis(80);

/// A tick long enough to be a stall at any multiplier.
const A_STALL: Duration = Duration::from_millis(500);

/// Frames the two machines are compared at.
///
/// Enough that the `LDIR` filling the attribute file has finished and the loop below it has run
/// for a long while, so the comparison is against a machine doing work rather than one still in
/// its first instruction.
const FRAMES: u64 = 40;

// ---------------------------------------------------------------------------------------
// 1. The type: zero is not a speed
// ---------------------------------------------------------------------------------------

#[test]
fn a_multiplier_of_zero_is_not_a_speed() {
    // A factor of nought owes nothing however long the wall clock takes, so the screen freezes
    // while the readout insists everything is on time. `Speed` exists to make that state
    // unrepresentable rather than to guard against it at each use, and this is the assertion that
    // says the constructor really is the narrow point.
    assert_eq!(Speed::new(0), None);
    assert_eq!(Speed::new(1), Some(Speed::REAL_TIME));
    assert_eq!(Speed::REAL_TIME.factor(), 1);
    assert_eq!(Speed::new(8).map(Speed::factor), Some(8));
}

#[test]
fn a_default_pacer_runs_at_real_time() {
    // `Pacer` derives `Default`, and a derive over a new field is exactly where a default of zero
    // would have arrived unnoticed — a `Pacer::default()` that never ran a frame, in a type whose
    // whole job is to run frames. `Speed`'s own `Default` is what keeps the derive honest.
    assert_eq!(Pacer::new().speed(), Speed::REAL_TIME);
    assert_eq!(Pacer::default(), Pacer::new());
    assert_eq!(Pacer::default().speed(), Speed::REAL_TIME);
}

#[test]
fn the_table_starts_at_real_time_and_climbs() {
    // Ascending, so `Hotkey::CycleSpeed`'s modulo walks 1x -> 2x -> 4x -> 8x -> 1x rather than
    // some order nobody predicted, and starting at real time so the cycle both begins and returns
    // to the state somebody wants back in a hurry.
    assert_eq!(SPEEDS.first(), Some(&Speed::REAL_TIME));
    assert!(
        SPEEDS.is_sorted_by(|a, b| a.factor() < b.factor()),
        "SPEEDS is not strictly ascending, so cycling repeats or goes backwards: {SPEEDS:?}",
    );
    assert!(SPEEDS.len() >= 2, "a cycle of one is not a cycle");
}

// ---------------------------------------------------------------------------------------
// 2. The arithmetic: the bound is a wall-clock one and stays one
// ---------------------------------------------------------------------------------------

#[test]
fn a_multiplier_owes_that_many_times_the_frames() {
    // The feature, in one line: the same second of wall clock buys `factor` times the emulated
    // time. Fifty ticks is one second of a 50 Hz display.
    for &speed in SPEEDS {
        let mut pacer = Pacer::new();
        pacer.set_speed(speed);
        for _ in 0..50 {
            pacer.advance(TICK);
        }
        assert_eq!(
            (pacer.ran(), pacer.dropped()),
            (u64::from(speed.factor()) * 50, 0),
            "one second at {}x",
            speed.factor(),
        );
    }
}

#[test]
fn a_steady_display_loses_nothing_at_any_multiplier() {
    // **The false red this feature could so easily have shipped.** Fast-forward and a machine
    // failing to keep up look identical from outside — frames arriving in batches — and the only
    // thing that tells them apart is whether the pacer had to *throw any away*. A hundred ticks of
    // an ordinary 60 Hz display must lose nothing at 8x exactly as at 1x, or the status bar is red
    // for the entire load and means nothing for the rest of the session.
    let tick = Duration::from_nanos(16_666_667);
    for &speed in SPEEDS {
        let mut pacer = Pacer::new();
        pacer.set_speed(speed);
        for _ in 0..100 {
            pacer.advance(tick);
        }
        assert_eq!(
            pacer.dropped(),
            0,
            "a healthy 60 Hz display reported a stall at {}x",
            speed.factor(),
        );
    }
}

#[test]
fn an_unscaled_bound_would_have_clipped_every_ordinary_tick() {
    // The discriminating case, written as the defect rather than as the fix. At 8x a single 20 ms
    // display frame owes eight emulated frames, and eight is more than `MAX_CATCH_UP` — so a
    // ceiling left at four would have run four of them and declared the other four lost, on every
    // tick, for ever. The machine would have sat at 4x whatever the multiplier said, losing 200
    // frames a second, with the bar red throughout.
    //
    // Restoring that is a one-word edit (`MAX_CATCH_UP` for `MAX_CATCH_UP * factor`), and this is
    // the test it reddens.
    let fastest = *SPEEDS.last().expect("SPEEDS is never empty");
    assert!(
        u64::from(fastest.factor()) > MAX_CATCH_UP,
        "the fastest multiplier no longer exceeds the bound, so this test proves nothing",
    );

    let mut pacer = Pacer::new();
    pacer.set_speed(fastest);
    assert_eq!(pacer.advance(TICK), u64::from(fastest.factor()));
    assert_eq!(
        pacer.dropped(),
        0,
        "an ordinary tick was treated as a stall"
    );
}

#[test]
fn the_longest_tick_that_is_not_a_stall_is_the_same_at_every_multiplier() {
    // What the scaling buys, stated as the property rather than as the formula: eighty
    // milliseconds is the boundary at real time, and it stays the boundary. A lost frame therefore
    // keeps meaning one physical thing — *a tick took longer than 80 ms* — at 1x and at 8x alike,
    // which is what lets `LossMeter` need no exception and the colour need no suppression.
    for &speed in SPEEDS {
        let mut pacer = Pacer::new();
        pacer.set_speed(speed);
        pacer.advance(AT_THE_BOUND);
        assert_eq!(
            (pacer.ran(), pacer.dropped()),
            (MAX_CATCH_UP * u64::from(speed.factor()), 0),
            "the bound itself was treated as a stall at {}x",
            speed.factor(),
        );
    }
}

#[test]
fn a_real_stall_is_still_a_stall_at_every_multiplier() {
    // The other direction, and the half a scaled bound could have quietly removed: half a second
    // of nothing is a stall at any speed, and the frames it costs are counted rather than absorbed.
    // A ceiling that grew without limit would make fast-forward a mode in which the emulator never
    // reports a problem.
    for &speed in SPEEDS {
        let factor = u64::from(speed.factor());
        let mut pacer = Pacer::new();
        pacer.set_speed(speed);
        pacer.advance(A_STALL);
        assert_eq!(
            (pacer.ran(), pacer.dropped()),
            (MAX_CATCH_UP * factor, 21 * factor),
            "half a second of stall at {}x",
            speed.factor(),
        );
    }
}

#[test]
fn a_suspended_machine_at_the_highest_multiplier_does_not_overflow() {
    // `Pacer::advance` multiplies a `u128` of nanoseconds by the factor, and the two extremes
    // together are the widest that product can be: `Duration::MAX` is about 1.8e28 ns and the
    // largest multiplier a `Speed` can hold is 4.3e9, whose product is 7.9e37 against a `u128`
    // ceiling of 3.4e38. `overflow-checks` is on in release here, so an unguarded multiply would
    // abort the frontend rather than wrap — which is the outcome the saturating `due` was written
    // to avoid one step later, and it would have been reintroduced above it.
    let widest = Speed::new(u32::MAX).expect("u32::MAX is not zero");
    let mut pacer = Pacer::new();
    pacer.set_speed(widest);

    assert_eq!(
        pacer.advance(Duration::MAX),
        MAX_CATCH_UP * u64::from(u32::MAX)
    );
    assert_eq!(
        pacer.dropped(),
        u64::MAX - MAX_CATCH_UP * u64::from(u32::MAX),
        "everything owed past the ceiling is lost, and the count is saturated rather than wrapped",
    );
}

#[test]
fn changing_speed_keeps_the_time_already_owed() {
    // The remainder is emulated time the machine is owed and has not been given, and it does not
    // belong to the multiplier that accrued it. A `set_speed` that cleared it would silently lose
    // up to 20 ms of the machine on every press — the exact loss `Pacer::advance` keeps the
    // remainder to avoid, arriving through a different door.
    let mut pacer = Pacer::new();
    assert_eq!(pacer.advance(Duration::from_millis(19)), 0, "19 ms is owed");

    pacer.set_speed(Speed::new(2).expect("2 is not zero"));
    // 1 ms at 2x is 2 ms of emulated time, and 19 + 2 crosses the frame.
    assert_eq!(pacer.advance(Duration::from_millis(1)), 1);
    assert_eq!((pacer.ran(), pacer.dropped()), (1, 0));
}

// ---------------------------------------------------------------------------------------
// 3. The machine: 1x and 8x are the same Spectrum
// ---------------------------------------------------------------------------------------

#[test]
fn the_fastest_multiplier_produces_the_same_machine_as_real_time() {
    // **The claim the whole feature rests on.** Nothing is bypassed, so a tape still loads by its
    // own signal and a turbo loader still works — but only because the machine cannot tell. Two
    // Spectrums, the same ROM, the same frame count, reached through pacers whose *only* difference
    // is how much of the wall clock they hand over.
    let rom = painting_rom();

    let mut slow = Spectrum::new(&rom).expect("a page-sized ROM");
    let mut slow_pacer = Pacer::new();
    let slow_ticks = drive(&mut slow, &mut slow_pacer, FRAMES);

    let fastest = *SPEEDS.last().expect("SPEEDS is never empty");
    let mut fast = Spectrum::new(&rom).expect("a page-sized ROM");
    let mut fast_pacer = Pacer::new();
    fast_pacer.set_speed(fastest);
    let fast_ticks = drive(&mut fast, &mut fast_pacer, FRAMES);

    // Both really got there, which is what stops a pacer that counted frames it never handed over
    // from passing this by leaving two identical machines at frame zero.
    assert_eq!(slow.frames(), FRAMES, "the real-time run did not finish");
    assert_eq!(fast.frames(), FRAMES, "the fast run did not finish");

    assert_eq!(
        slow.cpu_state(),
        fast.cpu_state(),
        "the same forty frames left the CPU in two different states",
    );
    let (slow_screen, fast_screen) = (screen(&slow), screen(&fast));
    assert_eq!(
        first_difference(&slow_screen, &fast_screen),
        None,
        "the same forty frames drew two different pictures",
    );

    // And the wall clock — the one thing that *is* meant to differ. Forty frames is forty ticks of
    // a 50 Hz display at real time and five at 8x, which is the feature stated as two integers.
    assert_eq!(slow_ticks, FRAMES);
    assert_eq!(fast_ticks, FRAMES / u64::from(fastest.factor()));
}

#[test]
fn the_comparison_can_fail() {
    // A positive control, because everything above it is a chain of `assert_eq!`s that would also
    // pass if this ROM drew the same picture every frame — and a gate whose subject never changes
    // is a gate that cannot fail. `gate-bundled.sh` carries the same control for the same reason,
    // in as many words: *"a different ROM must produce a different picture"*.
    let rom = painting_rom();

    let mut early = Spectrum::new(&rom).expect("a page-sized ROM");
    early.run_frames(FRAMES);
    let mut late = Spectrum::new(&rom).expect("a page-sized ROM");
    late.run_frames(FRAMES + 1);

    assert_ne!(early.cpu_state(), late.cpu_state());
    assert!(
        first_difference(&screen(&early), &screen(&late)).is_some(),
        "one more frame changed nothing on screen, so this ROM cannot tell two runs apart and \
         every comparison above it is vacuous",
    );
}

// ---------------------------------------------------------------------------------------
// The fixtures
// ---------------------------------------------------------------------------------------

/// A 16 KB ROM that paints the attribute file and then changes the screen every frame.
///
/// ```text
/// 0000  21 00 58     LD HL,0x5800      ; the attribute file
/// 0003  11 01 58     LD DE,0x5801
/// 0006  01 FF 02     LD BC,767
/// 0009  36 47        LD (HL),0x47      ; bright white ink on black paper
/// 000B  ED B0        LDIR
/// 000D  21 00 40     LD HL,0x4000      ; the top-left eight pixels
/// 0010  34           INC (HL)
/// 0011  18 FD        JR -3             ; back to the INC, for ever
/// ```
///
/// The attributes come first because an unwritten Spectrum draws black on black, and a picture
/// with no contrast is a picture two runs agree on however far apart they are. The loop is what
/// makes the frame index visible: `0x4000` is contended memory, so each pass costs a number of
/// T-states the ULA decides, and the byte therefore stands at a different value at the end of
/// every frame. `the_comparison_can_fail` is what checks that rather than this paragraph.
fn painting_rom() -> Vec<u8> {
    const PROGRAM: [u8; 19] = [
        0x21, 0x00, 0x58, 0x11, 0x01, 0x58, 0x01, 0xFF, 0x02, 0x36, 0x47, 0xED, 0xB0, 0x21, 0x00,
        0x40, 0x34, 0x18, 0xFD,
    ];
    let mut rom = vec![0; PAGE_SIZE];
    rom[..PROGRAM.len()].copy_from_slice(&PROGRAM);
    rom
}

/// Drive `machine` through `pacer` on a steady 50 Hz display until it has run `frames` frames,
/// and say how many display ticks that took.
///
/// The tick budget is `frames + 1` — exactly enough at real time and generous above it — and it is
/// a **bound** rather than a `loop` on purpose: a pacer that counted frames it never handed over
/// would otherwise hang the suite instead of failing it, and a hang is the one test outcome nobody
/// reads.
fn drive(machine: &mut Spectrum, pacer: &mut Pacer, frames: u64) -> u64 {
    let budget = frames + 1;
    for tick in 1..=budget {
        for _ in 0..pacer.advance(TICK) {
            machine.run_frame();
            if machine.frames() >= frames {
                return tick;
            }
        }
    }
    budget
}

/// What the window would have uploaded, through the only path from a machine to pixels.
fn screen(machine: &Spectrum) -> Box<[u8; RGBA_BYTES]> {
    let mut frame = Frame::new();
    machine.render(&mut frame);
    let mut rgba = palette::buffer();
    palette::write_rgba(&frame, &mut rgba);
    rgba
}

/// Where two frames first disagree, or `None` when they do not.
///
/// An index rather than `assert_eq!` on the buffers themselves: these are 327,680 bytes each, and
/// a failure that prints both of them in full is a failure nobody can read.
fn first_difference(left: &[u8; RGBA_BYTES], right: &[u8; RGBA_BYTES]) -> Option<usize> {
    left.iter()
        .zip(right.iter())
        .position(|(left, right)| left != right)
}
