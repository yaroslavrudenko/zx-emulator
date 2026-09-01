//! Running faster than a real Spectrum, and the four claims that has to survive.
//!
//! # What this grades
//!
//! 1. **The arithmetic**, in the shape `tests/pacing_accounting.rs` established: literal
//!    [`Duration`] sequences against literal `(run, dropped)` pairs, now at every multiplier
//!    [`RUNGS`] holds. The one that matters is the **catch-up bound**, because it is the place a
//!    naive multiplier goes wrong silently: [`MAX_CATCH_UP`] is four *frames* standing in for
//!    eighty *milliseconds*, and above real time those stop being the same sentence.
//!
//! 2. **That the machine does not notice.** The whole claim of this feature is that nothing is
//!    bypassed — `docs/M6.md` Decision 4 rules out the ROM trap and
//!    `crates/spectrum/tests/tape_rom_load.rs`'s `no_shortcut_exists_past_the_ear_bit` keeps it
//!    ruled out — so a tape still loads by its own signal and a turbo loader still works. That is
//!    only true if the fastest multiplier and 1× produce the *same machine*, and an argument is not
//!    a measurement. Two Spectrums are therefore run to the same frame count through pacers at
//!    different speeds and compared: the CPU state, and every one of the 327,680 bytes the window
//!    would have uploaded.
//!
//! 3. **That a tape loaded fast is the same tape**, which is a different claim and needs its own
//!    machine. Section 2's ROM runs with an **empty drive**: nothing is moving but the CPU, so a
//!    pacing bug that stalled the tape — the one bug fast *loading* could actually have — would
//!    leave both screens identical and sail through. Section 4 therefore puts a cassette in and a
//!    guest on top of it that reads the `EAR` bit every pass and accumulates it into screen
//!    memory, so the picture **is** the signal. Then it compares the drive itself: index,
//!    remaining T-states, level, and whether it is still running.
//!
//!    The train is sized to run out **before** the comparison, so what is graded includes the end
//!    of the cassette rather than stopping short of it. That is the moment the emulator would key
//!    an automatic fast-load off, and it is the moment worth being sure about.
//!
//! 4. **That the rung which decides for itself decides the same machine.** [`Rung::Automatic`]
//!    reaches its frames a different way from every multiplier above it — not `elapsed × factor`
//!    but *"keep going until [`FLAT_OUT_BUDGET`] of wall clock is gone"* — so section 3's
//!    comparison says nothing about it and had to be run again against the new loop. Section 5
//!    does that over the same playing cassette, and it grades the **transition** as well as the
//!    burst: the train runs out inside the run, so an automatic machine must key itself back to
//!    real time and finish the comparison paced, byte for byte with one that was paced
//!    throughout. That sentence in section 3 — *"the moment the emulator would key an automatic
//!    fast-load off"* — was a prediction when it was written, and section 5 is where it is cashed.
//!
//!    Its two discriminating cases are deliberately opposite. `automatic_runs_flat_out_only_while_
//!    the_drive_is_turning` is the **trigger**: blind it so that automatic never speeds anything up
//!    and the equivalence above stays perfectly green, because two identical machines are still
//!    identical — so the assertion that has to exist is that automatic reached the frame count in
//!    *far fewer ticks*. `a_flat_out_tick_stops_when_its_budget_is_spent` is the **bound**, because
//!    a burst that never ended would not fail anything either; it would hang, and the window it
//!    hangs is the one holding the keyboard.
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
//! # Why the ROMs and the cassette are written here rather than fetched
//!
//! `crates/testsupport` exists because a gate backed by a corpus is a gate that might not run, and
//! the claims under test need machines that *do something observable per frame* rather than
//! Sinclair ones. So [`painting_rom`] and [`listening_rom`] are assembled below — the same move
//! `crates/frontend/gate-bundled.sh` makes, for the same reason — and so is the tape:
//! [`spectrum::tape::Tape::new`] takes a pulse train directly, because `docs/M6.md` Decision 5
//! makes that train *the* representation of a cassette rather than a detail of one, so a real
//! pilot tone is [`pilot_tone`] and not a file. This file runs on a clean checkout with no
//! `testdata/` at all.

use std::time::Duration;

use frontend::keymap;
use frontend::pacing::{FLAT_OUT_BUDGET, MAX_CATCH_UP, Pacer, RUNGS, Rung, Speed, Tick};
use frontend::palette::{self, RGBA_BYTES};
use spectrum::memory::PAGE_SIZE;
use spectrum::tape::tap;
use spectrum::{Frame, Spectrum, Tape};

/// Every multiplier [`RUNGS`] holds — which is every rung but the one that decides for itself.
///
/// The sections below grade the *arithmetic* of a multiplier, and [`Rung::Automatic`] has no
/// multiplier to grade: it is a wall-clock budget, and section 5 is where it is asked about.
/// Filtering here rather than in each test keeps that distinction in one place, and reading it
/// out of the table rather than restating the four numbers is what stops a rung added upstairs
/// from being silently excluded from every test below.
fn multipliers() -> impl Iterator<Item = Speed> {
    RUNGS.iter().filter_map(|rung| match *rung {
        Rung::Fixed(speed) => Some(speed),
        Rung::Automatic => None,
    })
}

/// The fastest fixed multiplier the window can reach.
fn fastest() -> Speed {
    multipliers()
        .max_by_key(|speed| speed.factor())
        .expect("RUNGS holds at least one multiplier")
}

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
fn the_table_starts_at_real_time_and_climbs_and_ends_at_the_rung_that_decides() {
    // Ascending, so `Hotkey::CycleSpeed`'s modulo walks 1x -> 4x -> 16x -> 64x -> auto -> 1x
    // rather than some order nobody predicted, and starting at real time so the cycle both begins
    // and returns to the state somebody wants back in a hurry.
    assert_eq!(RUNGS.first(), Some(&Rung::Fixed(Speed::REAL_TIME)));
    let multipliers: Vec<Speed> = multipliers().collect();
    assert!(
        multipliers.is_sorted_by(|a, b| a.factor() < b.factor()),
        "the multipliers are not strictly ascending, so cycling repeats or goes backwards: \
         {multipliers:?}",
    );
    assert!(RUNGS.len() >= 2, "a cycle of one is not a cycle");

    // **Automatic is last, and exactly once.** Last because the cycle has to end somewhere a
    // person can predict, and *"press it once more and you are back at real time"* is the
    // property `Hotkey::CycleSpeed` sells; putting it in the middle would make the way home
    // depend on where you happened to be. Once because two of them would be a cycle with an
    // invisible repeat — a press that appeared to do nothing.
    assert_eq!(RUNGS.last(), Some(&Rung::Automatic));
    assert_eq!(
        RUNGS
            .iter()
            .filter(|rung| **rung == Rung::Automatic)
            .count(),
        1,
        "the cycle holds the automatic rung more than once, so one press of F8 does nothing",
    );
    assert_eq!(
        multipliers.len() + 1,
        RUNGS.len(),
        "a rung is neither a multiplier nor the automatic one, so `multipliers` is silently \
         dropping it and every test in section 2 is running over less than the table",
    );
}

// ---------------------------------------------------------------------------------------
// 2. The arithmetic: the bound is a wall-clock one and stays one
// ---------------------------------------------------------------------------------------

#[test]
fn a_multiplier_owes_that_many_times_the_frames() {
    // The feature, in one line: the same second of wall clock buys `factor` times the emulated
    // time. Fifty ticks is one second of a 50 Hz display.
    for speed in multipliers() {
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
    // an ordinary 60 Hz display must lose nothing at 64x exactly as at 1x, or the status bar is red
    // for the entire load and means nothing for the rest of the session.
    let tick = Duration::from_nanos(16_666_667);
    for speed in multipliers() {
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
    // The discriminating case, written as the defect rather than as the fix. At 64x a single
    // 20 ms display frame owes sixty-four emulated frames, and sixty-four is far more than
    // `MAX_CATCH_UP` — so a ceiling left at four would have run four of them and declared the
    // other sixty lost, on every tick, for ever. The machine would have sat at 4x whatever the
    // multiplier said, losing 3000 frames a second, with the bar red throughout.
    //
    // Restoring that is a one-word edit (`MAX_CATCH_UP` for `MAX_CATCH_UP * factor`), and this is
    // the test it reddens.
    let fastest = fastest();
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
    // keeps meaning one physical thing — *a tick took longer than 80 ms* — at 1x and at 64x alike,
    // which is what lets `LossMeter` need no exception and the colour need no suppression.
    for speed in multipliers() {
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
    for speed in multipliers() {
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
// 3. The machine: the fastest multiplier and 1x are the same Spectrum
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

    let fastest = fastest();
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
    // a 50 Hz display at real time and one at 64x, which is the feature stated as two integers.
    //
    // `div_ceil` rather than `/`, and the difference is not cosmetic: a tick delivers a whole
    // batch, so a multiplier that does not divide `FRAMES` still needs the tick that carries the
    // remainder. Plain division was right only while the top rung happened to divide forty, and it
    // returned **zero** at the first factor that did not — asserting that the machine reached
    // frame forty in no ticks at all.
    assert_eq!(slow_ticks, FRAMES);
    assert_eq!(fast_ticks, FRAMES.div_ceil(u64::from(fastest.factor())));
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
// 4. The tape: a cassette played fast is the same cassette
// ---------------------------------------------------------------------------------------

#[test]
fn a_tape_plays_the_same_signal_at_every_multiplier() {
    // **The claim fast *loading* rests on, as opposed to fast running.** Section 3 compares two
    // machines with an empty drive: nothing is moving there but the CPU, so the one bug this
    // feature could actually have — a tape that advances differently when frames arrive in
    // batches — would leave both of its screens identical and pass. `crates/spectrum` is careful
    // about this in its own right: `Ula::advance` moves the clock and the tape together, and
    // `one_long_advance_lands_where_many_short_ones_do` pins that a stall of six T-states and six
    // single ticks land in the same place. This is the frontend's half of the same question, asked
    // through the thing a person actually presses.
    let rom = listening_rom();

    let mut slow = tape_machine(&rom);
    let mut slow_pacer = Pacer::new();
    drive(&mut slow, &mut slow_pacer, FRAMES);

    let fastest = fastest();
    let mut fast = tape_machine(&rom);
    let mut fast_pacer = Pacer::new();
    fast_pacer.set_speed(fastest);
    drive(&mut fast, &mut fast_pacer, FRAMES);

    assert_eq!(slow.frames(), FRAMES, "the real-time run did not finish");
    assert_eq!(fast.frames(), FRAMES, "the fast run did not finish");

    // The drive itself, field for field — `Tape`'s `PartialEq` covers the index, the T-states left
    // in the current half-period, the level it is driving the line to, and whether the motor is
    // still turning. This is an **exact** comparison of where the head is, which is the one thing
    // a loader is reading, and it needs no argument about aliasing: a signal one edge out fails it.
    //
    // `Debug` is what a failure prints and it is deliberately short — `crates/spectrum` writes it
    // by hand so a mismatch does not dump a million half-periods at somebody.
    assert_eq!(
        slow.tape(),
        fast.tape(),
        "the same forty frames left the head in two different places",
    );

    // And that the **CPU** saw it, which the comparison above cannot say: the guest counts every
    // pass on which the `EAR` bit was high and keeps the running total in the top-left byte of the
    // screen, so a single extra or missing edge moves the picture.
    assert_eq!(
        slow.cpu_state(),
        fast.cpu_state(),
        "the same forty frames over the same tape left the CPU in two different states",
    );
    assert_eq!(
        first_difference(&screen(&slow), &screen(&fast)),
        None,
        "the two machines counted a different number of `EAR` highs off the same cassette",
    );

    // The cassette must have **run out** inside the comparison rather than still be playing at the
    // end of it. That is the transition an automatic fast-load would key off — run flat out while
    // the tape is moving, drop back when it stops — and a test that only ever graded mid-tape
    // would have nothing to say about the moment that matters.
    assert!(
        !slow.tape().pulses().is_empty(),
        "there was no cassette in the drive, so nothing above compared a tape",
    );
    assert_eq!(
        slow.tape().level(),
        fast.tape().level(),
        "the line was left at two different levels",
    );
}

#[test]
fn a_tape_that_never_moved_would_fail_this() {
    // The positive control, and this file already carries the argument for why it has to exist:
    // every assertion above is an `assert_eq!` that a pair of machines ignoring the tape entirely
    // would also satisfy. So one machine gets PLAY and the other does not, and the difference must
    // be visible **on the screen** — not merely in the drive, which would only prove that `play`
    // sets a flag.
    let rom = listening_rom();

    let mut playing = tape_machine(&rom);
    playing.run_frames(FRAMES);

    // Inserted and never started: the same cassette, the same ROM, the same frame count.
    let mut stopped = Spectrum::new(&rom).expect("a page-sized ROM");
    stopped.insert_tape(Tape::new(pilot_tone(&stopped)));
    stopped.run_frames(FRAMES);

    assert!(
        first_difference(&screen(&playing), &screen(&stopped)).is_some(),
        "a tape that was never played drew the same picture as one that played out, so the guest \
         is not reading the `EAR` bit and every comparison in this section is vacuous",
    );
    assert_ne!(playing.tape(), stopped.tape(), "PLAY moved nothing at all",);
}

// ---------------------------------------------------------------------------------------
// 5. The rung that decides: automatic is the same machine, and it keys itself off
// ---------------------------------------------------------------------------------------

#[test]
fn automatic_runs_flat_out_only_while_the_drive_is_turning() {
    // **The trigger, as a table over the whole cycle**, because the claim has two halves and only
    // one of them is about automatic. The other half is that *nothing else reads the drive*: a
    // person parked at 1× to watch the loading stripes must not be overtaken when they press
    // PLAY, and a person at 64× must not be dropped to real time when the tape ends. That is what
    // makes a multiplier a thing somebody chose rather than something that happens to them, and
    // it is asserted here rather than argued in `Rung::this_tick`'s doc comment.
    for &rung in RUNGS {
        match rung {
            Rung::Fixed(speed) => {
                assert_eq!(rung.this_tick(false), Tick::Paced(speed));
                assert_eq!(
                    rung.this_tick(true),
                    Tick::Paced(speed),
                    "putting a cassette in moved a machine parked at {}x",
                    speed.factor(),
                );
            }
            Rung::Automatic => {
                assert_eq!(
                    rung.this_tick(true),
                    Tick::FlatOut,
                    "the automatic rung did not speed up for a turning drive, which is the whole \
                     of what it is for",
                );
                assert_eq!(
                    rung.this_tick(false),
                    Tick::Paced(Speed::REAL_TIME),
                    "the automatic rung stayed flat out with the drive stopped, so a loaded game \
                     would run at four thousand frames a second",
                );
            }
        }
    }

    // And the readout's half, which is a different failure: a rung that works and cannot be seen
    // working is one a person reports as broken. `auto` and `auto (loading)` must be two strings,
    // and every fixed rung must add nothing at all.
    assert_ne!(
        Rung::Automatic.note(true),
        Rung::Automatic.note(false),
        "the bar reads the same whether or not the automatic rung is doing anything",
    );
    for speed in multipliers() {
        let rung = Rung::Fixed(speed);
        assert_eq!(rung.note(true), "");
        assert_eq!(rung.note(false), "");
    }
    assert_eq!(format!("{}", Rung::Fixed(fastest())), "64x");
    assert_eq!(format!("{}", Rung::Automatic), "auto");
}

#[test]
fn a_flat_out_tick_stops_when_its_budget_is_spent() {
    // **The bound.** A burst decides its own length, so the thing that has to be true of it is not
    // a frame count but a *duration*: it spends the budget and stops. Nothing else in this file
    // can catch a runaway one — a burst that never ended would hang rather than fail, and the
    // window it hangs is the one holding the keyboard — so this is where the budget is pinned.
    //
    // A millisecond a frame is a deliberately slow host, and it is also a blunt instrument on
    // purpose: it makes the frame count a direct reading of the budget in milliseconds, so the
    // ceiling below can be stated in the unit a person feels rather than in frames.
    const STEP: f64 = 0.001;
    const A_TENTH_OF_A_SECOND: u64 = 100;

    let mut pacer = Pacer::new();
    let mut frames = 0_u64;
    let run = pacer.run_flat_out(ticking_clock(STEP), || frames += 1);

    assert_eq!(
        run, frames,
        "the count returned is not the count of frames run"
    );
    assert!(
        run >= 1,
        "a tick that runs no frames at all is a stopped machine"
    );

    // Stated as the budget rather than as a literal, and to within one frame in each direction —
    // which is the true property, since a burst cannot stop in the middle of a frame.
    let spent = run as f64 * STEP;
    assert!(
        spent + STEP >= FLAT_OUT_BUDGET && spent - STEP <= FLAT_OUT_BUDGET,
        "a burst spent {spent} s against a budget of {FLAT_OUT_BUDGET} s",
    );

    // The absolute ceiling, and the one a person would actually notice being wrong: past about a
    // tenth of a second a response stops feeling caused by the key that asked for it, so a tick
    // may not run longer than that however the budget is written. `pacing`'s own `const` assertion
    // refuses such a budget at **compile** time; this is the same bound asserted where it is
    // spent, so neutralising either one still leaves the other.
    assert!(
        run <= A_TENTH_OF_A_SECOND,
        "one flat-out tick ran {run} ms of wall clock with nothing drawn and no key read",
    );

    // The frames are `Pacer::ran`'s, so the readout's `Hz` describes the same machine on this rung
    // as on every other — and nothing is *dropped*, because a budget is a decision to run for a
    // while rather than a debt the machine was owed and did not get.
    assert_eq!((pacer.ran(), pacer.dropped()), (run, 0));
}

#[test]
fn a_tape_loaded_under_automatic_is_the_same_tape() {
    // **Section 4's claim, asked of the loop that reaches its frames the other way.** Everything
    // above gets its frame count from `elapsed × factor`; this one gets it by spending a
    // wall-clock budget, so none of it carries over and the comparison has to be run again. Same
    // ROM reading the `EAR` bit every pass, same cassette, same frame count, and the same three
    // things compared: the drive field for field, the CPU, and all 327,680 bytes.
    let rom = listening_rom();

    let mut paced = tape_machine(&rom);
    let mut paced_pacer = Pacer::new();
    let paced_ticks = drive(&mut paced, &mut paced_pacer, FRAMES);

    let mut automatic = tape_machine(&rom);
    let mut automatic_pacer = Pacer::new();
    let automatic_ticks = drive_automatically(&mut automatic, &mut automatic_pacer, FRAMES);

    assert_eq!(paced.frames(), FRAMES, "the real-time run did not finish");
    assert_eq!(
        automatic.frames(),
        FRAMES,
        "the automatic run did not finish"
    );

    assert_eq!(
        paced.tape(),
        automatic.tape(),
        "the same forty frames left the head in two different places",
    );
    assert_eq!(
        paced.cpu_state(),
        automatic.cpu_state(),
        "the same forty frames over the same tape left the CPU in two different states",
    );
    assert_eq!(
        first_difference(&screen(&paced), &screen(&automatic)),
        None,
        "the two machines counted a different number of `EAR` highs off the same cassette",
    );

    // **The transition, which is the half only this rung has.** The cassette is thirty frames of a
    // forty-frame run, so an automatic machine must key itself back to real time partway and
    // finish paced. Both halves have to be asserted: that the drive really did stop, and that the
    // machine really did notice — the second is what the frame count above proves, since a machine
    // still flat out would have overshot.
    assert!(
        !automatic.tape().is_playing(),
        "the cassette was still playing at the end, so the run never reached the transition this \
         test exists for",
    );
    assert_eq!(
        Rung::Automatic.this_tick(automatic.tape().is_playing()),
        Tick::Paced(Speed::REAL_TIME),
        "the drive has stopped and the rung is still asking to run flat out",
    );

    // **And that automatic actually sped anything up**, which every assertion above would pass
    // without. Blind the trigger so that `Rung::Automatic` always resolves to real time and the
    // two machines stay byte-identical for ever — because they would be the *same* machine, run
    // the same way, twice. This is the assertion that reddens, and it is a wide margin rather
    // than a close one so that it is about the mechanism and not about a burst size.
    assert_eq!(paced_ticks, FRAMES, "a paced run is one frame a tick");
    assert!(
        automatic_ticks * 2 <= paced_ticks,
        "automatic reached frame {FRAMES} in {automatic_ticks} display ticks against a paced \
         run's {paced_ticks}, which is not a fast-forward",
    );
}

// ---------------------------------------------------------------------------------------
// 6. The measurement: a real cassette, end to end, on a real clock
// ---------------------------------------------------------------------------------------

/// Frames the ROM is given before anything is typed at it. `zx-shot`'s own `DEFAULT_FRAMES`.
const BOOT_FRAMES: u64 = 120;

/// Frames each key is held, and then released. `zx-shot`'s own `HOLD_FRAMES`, and for its reason:
/// under five the ROM misses the tap and over thirty-five the editor types it twice.
const HOLD_FRAMES: u64 = 10;

/// Frames run after the drive stops, so the loader can hand over to what it loaded.
///
/// Outside the measured window on purpose. What is being timed is the **cassette**, and what the
/// game does afterwards is the game running at the speed it was written for — which is exactly
/// what [`Rung::Automatic`] drops back to, and is not load time by any reading.
const SETTLE_FRAMES: u64 = 200;

/// `LOAD ""`, one tap per entry, exactly as `docs/images/README.md` publishes it.
const LOAD_SCRIPT: [&[&str]; 4] = [
    &["J"],
    &["LeftControl", "P"],
    &["LeftControl", "P"],
    &["Enter"],
];

/// Distinct colours below which a screen is a boot screen rather than a loaded game.
///
/// The 48K boot screen carries **two** — black paper and its border. A loaded *Manic Miner*
/// carries twelve or thirteen. Eight is comfortably between the two and is a floor rather than a
/// fingerprint: this is here to tell *"a game arrived"* from *"a blank frame arrived and the wall
/// clock was measured against nothing"*, which is this repository's recurring failure and not a
/// hypothetical one.
const COLOURS_OF_A_GAME: usize = 8;

/// How long a real cassette takes to load under [`Rung::Automatic`], on this machine, right now.
///
/// ```sh
/// cargo test --release -p frontend --test speed_multiplier -- --ignored --nocapture
/// ```
///
/// # Why it is `#[ignore]`d rather than a gate
///
/// It reports a **wall clock**, and a wall clock is a property of the machine the suite happens to
/// be running on. Asserting a number would be asserting how busy this laptop was, which is the
/// kind of gate that fails for reasons nobody can act on. So the timing is printed and what is
/// *asserted* is only what is genuinely invariant: that a real cassette really was loaded, that
/// the picture at the end is a game rather than a blank frame, and that the run was nowhere near
/// the three minutes it would have taken at real time.
///
/// It also needs `testdata/games/`, which is gitignored — so on a clean checkout it is a gate that
/// cannot run, and `crates/testsupport` is where this workspace decided what happens then.
///
/// # What it measures, and the two things it deliberately leaves out
///
/// The window is **PLAY to the drive stopping**, through the real [`Rung::Automatic`] decision and
/// the real [`Pacer::run_flat_out`] on a real clock. Left out at the front: booting the ROM and
/// typing `LOAD ""`, which happen before anybody presses PLAY. Left out at the back:
/// [`SETTLE_FRAMES`], because after the drive stops the rung is paced again and what runs is the
/// game.
///
/// **It is headless, so there is nothing to draw and the budget is spent almost entirely on the
/// machine.** That is the honest upper bound on the rung rather than a simulation of the window:
/// in a window a tick also has to produce a picture, and what that costs is measured next door in
/// [`the_cost_of_one_picture`]. The two numbers together are what the report reasons from.
#[test]
#[ignore = "a measurement rather than a gate: it needs testdata/games and reports a wall clock"]
fn a_real_cassette_end_to_end_under_automatic() {
    let testdata = testsupport::testdata_dir();
    let tape_path = testdata.join("games").join("ManicMiner.tap");
    let (Ok(rom), Ok(cassette)) = (
        std::fs::read(testdata.join("roms").join("48.rom")),
        std::fs::read(&tape_path),
    ) else {
        testsupport::skip_absent_corpus("a Sinclair 48K ROM and a real cassette", &tape_path);
        return;
    };

    let mut machine = Spectrum::new(&rom).expect("a 16 KB ROM");
    machine.insert_tape(tap::parse(&cassette).expect("a .tap this crate can read"));
    machine.run_frames(BOOT_FRAMES);
    for tap in LOAD_SCRIPT {
        type_at(&mut machine, tap, HOLD_FRAMES);
    }

    let at_play = machine.frames();
    machine.tape_mut().play();

    let origin = std::time::Instant::now();
    let clock = || origin.elapsed().as_secs_f64();
    let mut pacer = Pacer::new();
    let mut ticks = 0_u64;
    // Through the rung's own decision every pass, so what is timed is the thing that ships rather
    // than a loop written to look like it. It ends when the rung says so, which is the drive
    // stopping itself at the end of the train.
    while Rung::Automatic.this_tick(machine.tape().is_playing()) == Tick::FlatOut {
        ticks += 1;
        pacer.run_flat_out(clock, || machine.run_frame());
    }
    let wall = origin.elapsed().as_secs_f64();
    let frames = machine.frames() - at_play;

    machine.run_frames(SETTLE_FRAMES);
    let colours = distinct_colours(&screen(&machine));

    let emulated = frames as f64 / 50.0;
    println!(
        "PLAY to the end of the cassette, under `auto`, headless:\n  \
         {wall:.3} s of wall clock\n  \
         {frames} frames = {emulated:.1} s of emulated time\n  \
         {:.0}x real time, over {ticks} bursts of {:.0} ms\n  \
         {:.1} us per emulated frame\n  \
         {colours} distinct colours on the screen {SETTLE_FRAMES} frames later",
        emulated / wall,
        FLAT_OUT_BUDGET * 1000.0,
        wall * 1e6 / frames as f64,
    );

    // A real *Manic Miner* cassette is about 9,500 frames. The floor is loose because the point is
    // to catch a run that loaded nothing at all — an empty drive would leave this at zero and every
    // number above it would be a division by it.
    assert!(
        frames > 1000,
        "only {frames} frames of cassette played, so nothing above measured a tape load",
    );
    assert!(
        colours >= COLOURS_OF_A_GAME,
        "the screen carries {colours} colours, which is a boot screen or a blank frame — the wall \
         clock above was measured against a machine that loaded nothing",
    );
    // The one timing assertion, and it is wide on purpose: this cassette is 190 seconds of
    // emulated time, so a run anywhere near that is a rung that did not engage at all. Thirty
    // seconds is unreachable by a working one and unmissable by a broken one.
    assert!(
        wall < 30.0,
        "the cassette took {wall:.1} s, which is not a fast-forward — at real time it would be \
         {emulated:.0} s",
    );
}

/// What producing one picture costs, which is the other half of [`FLAT_OUT_BUDGET`]'s derivation.
///
/// The budget is *"a tick a person still reads as instant, minus what the tick spends drawing"*,
/// and the subtrahend is measured here rather than guessed: `Spectrum::render` into a [`Frame`]
/// and [`palette::write_rgba`] into the buffer the window uploads, which is the whole of the
/// window's picture path up to the GPU.
///
/// **It cannot see the upload**, and that is why the budget is rounded *down* from what this
/// number allows rather than up to it: the rounding is the headroom for the step this cannot
/// measure headlessly.
#[test]
#[ignore = "a measurement rather than a gate: it reports a wall clock"]
fn the_cost_of_one_picture() {
    const PICTURES: u32 = 500;

    let mut machine = Spectrum::new(&painting_rom()).expect("a page-sized ROM");
    machine.run_frames(FRAMES);

    let mut frame = Frame::new();
    let mut rgba = palette::buffer();
    let origin = std::time::Instant::now();
    for _ in 0..PICTURES {
        // **`black_box` on both ends, and it is not decoration.** Without it this loop measured
        // **0.005 ms** — 65 GB/s of writes, which is not a number any machine produces. Nothing
        // reads `rgba` afterwards and the body is idempotent, so LLVM is entitled to delete the
        // whole loop, and it did: the reading was of an empty loop and would have been pasted
        // into a report as the cost of drawing.
        machine.render(std::hint::black_box(&mut frame));
        palette::write_rgba(
            std::hint::black_box(&frame),
            std::hint::black_box(&mut rgba),
        );
    }
    let each = origin.elapsed().as_secs_f64() / f64::from(PICTURES);

    println!(
        "render + write_rgba: {:.3} ms a picture, so a {:.1} ms tick spends {:.1} ms on the \
         machine and this crate's half of the picture",
        each * 1000.0,
        (FLAT_OUT_BUDGET + each) * 1000.0,
        FLAT_OUT_BUDGET * 1000.0,
    );

    // The property, as opposed to the reading: the budget has to leave room for the picture inside
    // the tick it was derived from, or the window redraws less often than the derivation claims.
    assert!(
        each < FLAT_OUT_BUDGET,
        "producing a picture costs {each:.4} s against a budget of {FLAT_OUT_BUDGET} s, so most \
         of a tick is drawing and the budget is derived from a tick that does not exist",
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

/// A 16 KB ROM that paints the attribute file and then **counts `EAR` highs** for ever.
///
/// ```text
/// 0000  21 00 58     LD HL,0x5800      ; the attribute file
/// 0003  11 01 58     LD DE,0x5801
/// 0006  01 FF 02     LD BC,767
/// 0009  36 47        LD (HL),0x47      ; bright white ink on black paper
/// 000B  ED B0        LDIR
/// 000D  21 00 40     LD HL,0x4000      ; the top-left eight pixels
/// 0010  DB FE        IN A,(0xFE)       ; bit 6 is the tape
/// 0012  07           RLCA              ; bit 6 -> bit 7
/// 0013  07           RLCA              ; bit 7 -> bit 0
/// 0014  E6 01        AND 1
/// 0016  86           ADD A,(HL)
/// 0017  77           LD (HL),A         ; the running total, mod 256
/// 0018  18 F6        JR -10            ; back to the IN, for ever
/// ```
///
/// # Why it is a second ROM and not [`painting_rom`] with a port read bolted on
///
/// They are instruments for two different questions and the difference is the `IN`. `painting_rom`
/// makes the *frame index* visible by writing to contended memory, which is what section 3 needs:
/// a machine whose picture changes every frame whether or not anything else in the world does.
/// This one makes the **tape** visible, and nothing else about it matters — the accumulation is
/// mod 256 in one byte because a count is all that is wanted, and a count that moves by one when a
/// single edge is missed is a sharper instrument than a picture that merely differs.
///
/// The `AND 1` after two `RLCA`s rather than `AND 0x40` and an `ADD` is what buys that sharpness:
/// adding 0x40 wraps every four highs, so four dropped edges would have been invisible. Adding one
/// wraps every 256, and no plausible drift is 256 edges.
///
/// The two `RLCA`s leave the accumulated total in `A`'s high bits when the next `IN A,(0xFE)`
/// executes, so the half-rows that read are a function of the count. That is deliberate and it is
/// harmless: no key is pressed, every half-row reads high, and bit 6 is the tape whichever row was
/// selected. It is also one more way a divergence shows up, since the two machines would then be
/// addressing different rows.
fn listening_rom() -> Vec<u8> {
    const PROGRAM: [u8; 26] = [
        0x21, 0x00, 0x58, 0x11, 0x01, 0x58, 0x01, 0xFF, 0x02, 0x36, 0x47, 0xED, 0xB0, 0x21, 0x00,
        0x40, 0xDB, 0xFE, 0x07, 0x07, 0xE6, 0x01, 0x86, 0x77, 0x18, 0xF6,
    ];
    let mut rom = vec![0; PAGE_SIZE];
    rom[..PROGRAM.len()].copy_from_slice(&PROGRAM);
    rom
}

/// Half-period of the ROM loader's pilot tone, in T-states.
///
/// A real number rather than a convenient one: this is what a `.tap`'s leader is built from, so the
/// train below is the shape of an actual cassette rather than a square wave chosen to be easy.
/// `crates/spectrum/src/tape/signal.rs` is where it comes from.
const PILOT_HALF_PERIOD: u32 = 2168;

/// Frames the cassette lasts.
///
/// Comfortably inside [`FRAMES`], so the tape **runs out** with ten frames still to run and the
/// comparison spans the end of it. A train that outlasted the run would grade a load in progress
/// and say nothing about the moment it finishes, which is the moment an automatic fast-load would
/// have to notice.
const TAPE_FRAMES: u64 = 30;

const _: () = assert!(
    TAPE_FRAMES < FRAMES,
    "the cassette must run out before the comparison, or the end of it is never graded"
);

/// A pilot tone lasting [`TAPE_FRAMES`] frames of whatever machine `machine` is.
///
/// The frame length is read off the machine rather than written down, the way `zx-shot`'s
/// `tape_frames` reads it: a 128 runs 70,908 T-states to a 48K's 69,888, and a literal here would
/// silently make this cassette a different length on the other model.
fn pilot_tone(machine: &Spectrum) -> Vec<u32> {
    let frame = u64::from(machine.ula().clock().timing().frame_t_states());
    let pulses = frame * TAPE_FRAMES / u64::from(PILOT_HALF_PERIOD);
    vec![
        PILOT_HALF_PERIOD;
        usize::try_from(pulses).expect("thirty frames of pilot tone is a few hundred pulses")
    ]
}

/// A machine running [`listening_rom`] with [`pilot_tone`] in the drive and the motor **on**.
fn tape_machine(rom: &[u8]) -> Spectrum {
    let mut machine = Spectrum::new(rom).expect("a page-sized ROM");
    machine.insert_tape(Tape::new(pilot_tone(&machine)));
    // Separately from the insert, because `media::insert` puts a tape in stopped and the window's
    // `F3` is what starts it — the same two steps in the same order the shell performs them.
    machine.tape_mut().play();
    machine
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

/// Frames one flat-out burst runs in this file.
///
/// A burst's real length is *"however many fit in [`FLAT_OUT_BUDGET`] on this host"*, which is not
/// a thing a test can assert on — it is a property of how busy the machine is when the suite runs.
/// So [`drive_automatically`] hands [`Pacer::run_flat_out`] a clock of its own, stepped so that a
/// burst is this many frames wherever it runs.
///
/// Sixteen because it has to be two things at once: plainly a burst against `drive`'s one frame a
/// tick, and small enough that [`TAPE_FRAMES`] of cassette runs out **inside** a burst rather than
/// exactly at the edge of one. Thirty frames of tape is the middle of the second burst, which is
/// the awkward case rather than the tidy one — the drive stops with the tick already committed to
/// running flat out, and the machine has to be identical anyway.
const FLAT_OUT_FRAMES: u64 = 16;

const _: () = assert!(
    !TAPE_FRAMES.is_multiple_of(FLAT_OUT_FRAMES),
    "the cassette ends exactly on a burst boundary, so the run never covers the case where a \
     drive stops with the tick already committed"
);

/// A clock that advances by `step` every time it is read.
///
/// Deterministic where a wall clock is not. [`Pacer::run_flat_out`] reads its clock once before
/// the first frame and once after every frame, so a step of `budget / n` makes a burst `n` frames
/// long on any machine however loaded — and a frame count is then something this file can assert
/// on. The window hands that method macroquad's `get_time`; nothing here could assert against one.
///
/// It multiplies rather than accumulating so that the `k`th reading is one rounding of `k × step`
/// rather than `k` of them, which is what keeps a burst the same length on every target.
fn ticking_clock(step: f64) -> impl FnMut() -> f64 {
    let mut reads = 0.0;
    move || {
        reads += 1.0;
        reads * step
    }
}

/// Drive `machine` on [`Rung::Automatic`] until it has run `frames`, and say how many ticks it took.
///
/// [`drive`]'s counterpart, and deliberately the same shape: a bounded tick loop, a `TICK` of
/// display time each pass, and an early return the moment the target is reached. What differs is
/// the one line that decides how many frames a tick runs — which is the whole of what this rung
/// changes, and therefore the whole of what the comparison has to isolate.
fn drive_automatically(machine: &mut Spectrum, pacer: &mut Pacer, frames: u64) -> u64 {
    let budget = frames + 1;
    let mut clock = ticking_clock(FLAT_OUT_BUDGET / FLAT_OUT_FRAMES as f64);
    for tick in 1..=budget {
        match Rung::Automatic.this_tick(machine.tape().is_playing()) {
            Tick::Paced(speed) => {
                pacer.set_speed(speed);
                for _ in 0..pacer.advance(TICK) {
                    run_frame_unless_done(machine, frames);
                }
            }
            Tick::FlatOut => {
                pacer.run_flat_out(&mut clock, || run_frame_unless_done(machine, frames));
            }
        }
        if machine.frames() >= frames {
            return tick;
        }
    }
    budget
}

/// Run one frame, unless `machine` has already reached `frames` — in which case do nothing.
///
/// **A burst cannot be broken out of from inside.** [`Pacer::run_flat_out`] owns its own loop,
/// which is the point of it, so the frame it is handed **declines** past the target instead and
/// the tick spends the rest of its budget on air. That is [`drive`]'s early return written for a
/// loop this file does not own, and it is what keeps the two machines comparable at the *same*
/// frame rather than at whichever one a burst happened to overshoot to.
///
/// Both arms of [`drive_automatically`] go through it, so the target is checked in exactly one
/// place and the tick loop needs only one test at the end rather than one per arm.
fn run_frame_unless_done(machine: &mut Spectrum, frames: u64) {
    if machine.frames() < frames {
        machine.run_frame();
    }
}

/// What the window would have uploaded, through the only path from a machine to pixels.
fn screen(machine: &Spectrum) -> Box<[u8; RGBA_BYTES]> {
    let mut frame = Frame::new();
    machine.render(&mut frame);
    let mut rgba = palette::buffer();
    palette::write_rgba(&frame, &mut rgba);
    rgba
}

/// Hold `tap`'s keys together for `hold` frames, then release them for the same, through the
/// real keymap.
///
/// The same two-phase step `zx-shot`'s own `press` performs, and it has to be: a key re-applied
/// between frames is what the window does, and collapsing it into one `apply` and a run of frames
/// would be a different gesture. It is written again here rather than shared because that function
/// is private to a binary and drives a `Recorder` this file has no use for — the shape is common,
/// the couplings are not.
fn type_at(machine: &mut Spectrum, tap: &[&str], hold: u64) {
    let codes: Vec<_> = tap
        .iter()
        .map(|name| keymap::code_named(name).expect("a key this emulator binds"))
        .collect();
    for held in [true, false] {
        for _ in 0..hold {
            keymap::apply(|code| held && codes.contains(&code), machine.keyboard_mut());
            machine.run_frame();
        }
    }
}

/// How many different colours a frame carries.
///
/// The cheapest question that separates a loaded game from a blank frame, and the one the gallery
/// already uses to tell them apart. Over the **RGBA** the window uploads rather than over the
/// machine's attribute bytes, so it is asking about the picture a person would have seen.
fn distinct_colours(picture: &[u8; RGBA_BYTES]) -> usize {
    // `as_chunks` rather than `chunks_exact`: the four is a *type* parameter, so each pixel
    // arrives as a `[u8; 4]` already and there is no fallible conversion to unwrap on the way
    // past. The remainder is discarded because there cannot be one — `RGBA_BYTES` is four bytes
    // a pixel by construction, which is the same fact the type is asserting.
    picture
        .as_chunks::<4>()
        .0
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
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
