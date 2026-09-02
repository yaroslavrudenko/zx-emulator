//! Running faster than a real Spectrum, and the five claims that has to survive.
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
//!    Its two discriminating cases are deliberately opposite.
//!    `automatic_runs_flat_out_only_while_the_machine_is_decoding` is the **trigger**: blind it so
//!    that automatic never speeds anything up and the equivalence above stays perfectly green,
//!    because two identical machines are still identical — so the assertion that has to exist is
//!    that automatic reached the frame count in *far fewer ticks*.
//!    `a_flat_out_tick_stops_when_its_budget_is_spent` is the **bound**, because a burst that
//!    never ended would not fail anything either; it would hang, and the window it hangs is the
//!    one holding the keyboard.
//!
//!    **Section 5's transition is not the one it used to be, and the change is the point.** The
//!    trigger was the *motor*, and the cassette running out was therefore the moment automatic
//!    keyed off. It is now the machine's `EAR` read rate, so the guest going quiet is — and
//!    section 5's ROM never does. What crosses inside that run today is a drive stopping under a
//!    guest that carries on reading, which is `LOAD ""` waiting for a tape and is exactly the
//!    state the new signal exists to accelerate.
//!
//! 5. **That the order the two gestures arrive in does not matter**, which is section 6 and is
//!    the claim the trigger was changed for. Pressing PLAY and *then* typing `LOAD ""` is what a
//!    person does and is free on real hardware, because the ROM's leader is five seconds long.
//!    Keyed off the motor it was not free at 90×: the leader went by in 0.055 s and the loader
//!    found silence. So a fixture with **two** rates — a prompt's and a loader's, either side of
//!    the threshold — is driven through both orders with the keyboard rebuilt once a *tick*, and
//!    what is asserted is that the guest counted a cassette's worth of edges either way.
//!
//!    Section 7 asks the same question of the Sinclair ROM and a real cassette, which is where
//!    the real idle rate, the real poll rate and a real wall clock are.
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
//! Sinclair ones. So [`painting_rom`], [`listening_rom`] and [`prompt_rom`] are assembled below —
//! the same move
//! `crates/frontend/gate-bundled.sh` makes, for the same reason — and so is the tape:
//! [`spectrum::tape::Tape::new`] takes a pulse train directly, because `docs/M6.md` Decision 5
//! makes that train *the* representation of a cassette rather than a detail of one, so a real
//! pilot tone is [`pilot_tone`] and not a file. This file runs on a clean checkout with no
//! `testdata/` at all.

use std::time::Duration;

use frontend::keymap;
use frontend::pacing::{EarMeter, FLAT_OUT_BUDGET, MAX_CATCH_UP, Pacer, RUNGS, Rung, Speed, Tick};
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
    stopped.insert_tape(Tape::new(pilot_tone(&stopped, TAPE_FRAMES)));
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
fn automatic_runs_flat_out_only_while_the_machine_is_decoding() {
    // **The trigger, as a table over the whole cycle**, because the claim has two halves and only
    // one of them is about automatic. The other half is that *nothing else reads the machine*: a
    // person parked at 1× to watch the loading stripes must not be overtaken when a loader
    // starts, and a person at 64× must not be dropped to real time when it finishes. That is what
    // makes a multiplier a thing somebody chose rather than something that happens to them, and
    // it is asserted here rather than argued in `Rung::this_tick`'s doc comment.
    for &rung in RUNGS {
        match rung {
            Rung::Fixed(speed) => {
                assert_eq!(rung.this_tick(false), Tick::Paced(speed));
                assert_eq!(
                    rung.this_tick(true),
                    Tick::Paced(speed),
                    "starting a load moved a machine parked at {}x",
                    speed.factor(),
                );
            }
            Rung::Automatic => {
                assert_eq!(
                    rung.this_tick(true),
                    Tick::FlatOut,
                    "the automatic rung did not speed up for a machine that is decoding a tape, \
                     which is the whole of what it is for",
                );
                assert_eq!(
                    rung.this_tick(false),
                    Tick::Paced(Speed::REAL_TIME),
                    "the automatic rung stayed flat out with nothing being decoded, so a loaded \
                     game would run at four thousand frames a second",
                );
            }
        }
    }

    // And the readout's half, which is a different failure: a rung that works and cannot be seen
    // working is one a person reports as broken. `auto` and `auto (loading)` must be two strings,
    // and every fixed rung must add nothing at all.
    //
    // **`(loading)` now means what it says**, which it did not while it was written from the
    // drive: it followed a turning motor, so it appeared over a cassette nobody was reading. It
    // is derived from `Rung::this_tick` rather than from a condition of its own, so the two
    // cannot disagree — the assertion below is that they are two strings, and this comment is
    // why the argument they take is the one the pacing decision took.
    assert_ne!(
        Rung::Automatic.note(true),
        Rung::Automatic.note(false),
        "the bar reads the same whether or not the automatic rung is doing anything",
    );
    assert_eq!(
        Rung::Automatic.note(false),
        "",
        "the bar says the automatic rung is doing something over a tick it asked to run paced, \
         which is the drive-shaped lie one field along",
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
    let mut rung = Automatic::new(&automatic);
    let automatic_ticks = drive_automatically(&mut automatic, &mut rung, FRAMES);

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

    // **The transition, which is the half only this rung has — and the signal that keys it is no
    // longer the drive, so what crosses inside this run is not what used to.** The cassette is
    // still thirty frames of a forty-frame run, so the *tape* really does end partway. The
    // *guest* does not stop: `listening_rom` reads the `EAR` line for ever, so the meter goes on
    // reporting a machine that is decoding and the rung goes on asking for flat out.
    //
    // **That is the decision working, not failing.** A machine spinning on `IN A,(0xFE)` with an
    // empty drive is `LOAD ""` waiting for a cassette — the state the motor could not see, and
    // the exact state a person lands in when they press PLAY before typing. Accelerating it is
    // the point; `a_cassette_played_before_the_loader_asks_is_still_there_when_it_does` is where
    // that is graded on a machine that does eventually stop reading.
    assert!(
        !automatic.tape().is_playing(),
        "the cassette was still playing at the end, so the run never reached the moment the drive \
         stops and the guest carries on, which is what this test compares across",
    );
    assert!(
        rung.ear.decoding(),
        "the guest is still reading the `EAR` line every pass and the meter says it is not, so \
         the two machines above were compared across a transition that did not happen",
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
// 6. The key order: PLAY before `LOAD ""`, which is the order a person actually presses
// ---------------------------------------------------------------------------------------

/// Which order the two gestures arrive in.
///
/// Two names rather than a `bool`, because at the call site `ear_edges_in_order(true)` says
/// nothing and this section exists precisely because the two orders are not interchangeable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Order {
    /// `LOAD ""`, then PLAY. What `docs/images/README.md` publishes.
    TypeThenPlay,
    /// PLAY, then `LOAD ""`. What a person reaches for first, and what cost the owner an evening.
    PlayThenType,
}

/// Display ticks between the first gesture and the second.
///
/// A person reaching across for the other input, in the unit the window counts in: ten ticks is
/// 200 ms of wall clock, which is brisk. At real time that is ten emulated frames against a
/// [`LEADER_FRAMES`]-frame leader, so the pause spends a third of the grace a cassette gives and
/// the documented order has room to spare. Under a fast-forward that should not have started it
/// is ten **bursts** of [`FLAT_OUT_FRAMES`] frames, which is five times the whole leader — and
/// that ratio, not the absolute numbers, is what this section is about.
const REACHING_FOR_THE_OTHER_KEY: u64 = 10;

/// Ticks the key is held down, which is the shape of a press rather than of a switch.
const KEY_HELD: u64 = 3;

/// Ticks each arm runs for: past both gestures and past the end of the cassette.
const ORDER_TICKS: u64 = 40;

/// Frames of pilot tone on the cassette this section uses.
///
/// A real ROM leader is five seconds — 250 frames — and this is a twentieth of it, because the
/// only thing that has to be true of the number is that it outlasts
/// [`REACHING_FOR_THE_OTHER_KEY`] ticks of real time and does **not** outlast the same number of
/// bursts. Both are asserted below rather than eyeballed. A leader sized like the real one would
/// grade the same claim and take twenty times as long about it.
const LEADER_FRAMES: u64 = 30;

const _: () = assert!(
    LEADER_FRAMES > REACHING_FOR_THE_OTHER_KEY,
    "the leader is shorter than the pause between the two gestures, so even the documented order \
     would find a spent tape and neither arm below is discriminating"
);

const _: () = assert!(
    LEADER_FRAMES < REACHING_FOR_THE_OTHER_KEY * FLAT_OUT_FRAMES,
    "a fast-forward through the pause would not outlast the leader, so the cassette would survive \
     the defect this section exists to catch and the test would pass on the code that had it"
);

/// The address [`prompt_rom`] counts `EAR` edges at: the top-left eight pixels.
const TALLY: u16 = 0x4000;

/// What [`prompt_rom`]'s tally saturates at, and therefore what a guest reading a real signal
/// reaches.
///
/// [`LEADER_FRAMES`] of pilot tone carry about 32 edges a frame — nine hundred in all — so any
/// arm that listens to any appreciable part of the leader reaches this, and the assertion is a
/// clean equality rather than a threshold somebody would have to justify. A guest that finds a
/// **spent** tape sees a line that never changes and reaches at most one.
const TALLY_FULL: u8 = u8::MAX;

/// The key this section presses, standing in for `LOAD ""`.
///
/// `J` is `LOAD` on a Spectrum's keyboard and is the first tap [`LOAD_SCRIPT`] makes, so the
/// gesture graded here is the first half of the one the measurement below performs for real.
const LOAD_KEY: &str = "J";

/// Frames each rate below is measured over.
///
/// Long enough that the `LDIR` at the top of [`prompt_rom`] is out of the window and a whole
/// number of idle iterations is in it, short enough to stay a *rate* rather than an average over
/// a run.
const OVER: u64 = 10;

#[test]
fn the_prompt_rom_idles_below_the_threshold_and_listens_above_it() {
    // **The instrument's own calibration, and what the section below rests on.** Both arms of the
    // key-order test are readings taken with one ROM, and a reading is worth nothing unless that
    // ROM really does straddle the threshold `EarMeter` compares against. So the two rates are
    // measured here, through the same meter the window uses, and asserted to land on opposite
    // sides of it. If a future edit to the delay loop drifts the idle rate up past 64 a frame,
    // this reddens here — where the cause is one screen away — instead of turning the key-order
    // arms into two tests that agree because the machine never paced.
    let mut machine = Spectrum::new(&prompt_rom()).expect("a page-sized ROM");
    let mut ear = EarMeter::new(frame_t_states(&machine));

    machine.run_frames(OVER);
    ear.sample(machine.ear_reads(), machine.frames());
    machine.run_frames(OVER);
    ear.sample(machine.ear_reads(), machine.frames());
    assert!(
        !ear.decoding(),
        "a machine sitting at its prompt reads the `EAR` line often enough to be taken for a \
         loader, so `Rung::Automatic` would run flat out for ever and never pace anything",
    );

    let load_key = keymap::code_named(LOAD_KEY).expect("a key this emulator binds");
    keymap::apply(|code| code == load_key, machine.keyboard_mut());
    machine.run_frames(OVER);
    ear.sample(machine.ear_reads(), machine.frames());
    assert!(
        ear.decoding(),
        "a machine polling the `EAR` line every pass is not taken for a loader, so the trigger \
         never fires and both arms below would pass by running everything at real time",
    );
}

#[test]
fn a_cassette_played_before_the_loader_asks_is_still_there_when_it_does() {
    // **The owner's evening, as a gate.** He pressed PLAY and then typed `LOAD ""` — the order a
    // person reaches for, and one that is free on real hardware because the leader is five
    // seconds long. Keyed off the *motor*, `auto` spent those five seconds in 0.055 s: the tape
    // ran off its end while he was still typing, and nothing on the bar could say so, because
    // nothing about the drive was wrong.
    //
    // The tally is the assertion because it is the only thing that distinguishes *the guest read
    // a cassette* from *a cassette went past*. `prompt_rom` counts the **edges** it sees on the
    // line and saturates, so a spent tape — a level that never changes again — leaves it at
    // nothing however far the drive wound, and a live pilot tone fills it. Nothing about frame
    // counts or drive state can tell those two apart, which is exactly the confusion this whole
    // change is about. Counting *highs* instead cannot tell them apart either, and `prompt_rom`
    // records what that cost.
    let documented = ear_edges_in_order(Order::TypeThenPlay);
    let the_owners = ear_edges_in_order(Order::PlayThenType);

    assert_eq!(
        the_owners, TALLY_FULL,
        "PLAY then `LOAD \"\"` left the guest {the_owners} edges to read: the cassette was wound \
         off its end during the {REACHING_FOR_THE_OTHER_KEY} ticks it took to reach the keyboard, \
         which is the defect this test exists for",
    );
    assert_eq!(
        documented, TALLY_FULL,
        "`LOAD \"\"` then PLAY read {documented} edges, so the fixture is broken rather than the \
         order — the guest never listened, or the tape never played",
    );
}

// ---------------------------------------------------------------------------------------
// 7. The measurement: a real cassette, end to end, on a real clock
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

    // **Both orders, because the order is the thing that broke.** Section 6 grades this claim on
    // a fixture; this is the same claim against the Sinclair ROM, the real `KEY-SCAN` idle rate,
    // the real `LD-BYTES` poll rate, the real five-second leader and a real wall clock — none of
    // which a fixture can promise. The two lines it prints are what a person would have felt.
    for order in [Order::TypeThenPlay, Order::PlayThenType] {
        let run = a_real_cassette_in_order(order, &rom, &cassette);
        let emulated = run.frames as f64 / 50.0;
        println!(
            "{order:?}: PLAY to the end of the cassette, under `auto`, headless:\n  \
             {:.3} s of wall clock\n  \
             {} frames = {emulated:.1} s of emulated time\n  \
             {:.0}x real time, over {} ticks\n  \
             {:.1} us per emulated frame\n  \
             {} distinct colours on the screen {SETTLE_FRAMES} frames later",
            run.wall,
            run.frames,
            emulated / run.wall,
            run.ticks,
            run.wall * 1e6 / run.frames as f64,
            run.colours,
        );

        // A real *Manic Miner* cassette is about 9,500 frames. The floor is loose because the
        // point is to catch a run that loaded nothing at all — an empty drive would leave this at
        // zero and every number above it would be a division by it.
        assert!(
            run.frames > 1000,
            "{order:?}: only {} frames of cassette played, so nothing above measured a tape load",
            run.frames,
        );
        // **The assertion the whole change is for, and the one that used to fail.** Keyed off the
        // drive, `Order::PlayThenType` reached this line with a boot screen: the leader had been
        // spent at 90× while `LOAD ""` was still being typed, so the loader found silence and the
        // machine sat at `©1982 Sinclair Research Ltd` for ever.
        assert!(
            run.colours >= COLOURS_OF_A_GAME,
            "{order:?}: the screen carries {} colours, which is a boot screen or a blank frame — \
             the cassette went past and nothing read it",
            run.colours,
        );
        // The one timing assertion, and it is wide on purpose: this cassette is 190 seconds of
        // emulated time, so a run anywhere near that is a rung that did not engage at all. Thirty
        // seconds is unreachable by a working one and unmissable by a broken one.
        assert!(
            run.wall < 30.0,
            "{order:?}: the cassette took {:.1} s, which is not a fast-forward — at real time it \
             would be {emulated:.0} s",
            run.wall,
        );
    }
}

/// What one run of a real cassette under [`Rung::Automatic`] came to.
struct Loaded {
    /// Wall clock from PLAY to the drive stopping.
    wall: f64,
    /// Frames of emulated time in that window.
    frames: u64,
    /// Display ticks in it.
    ticks: u64,
    /// Colours on the screen [`SETTLE_FRAMES`] later, which is how a game is told from a prompt.
    colours: usize,
}

/// Ticks one [`LOAD_SCRIPT`] takes to type: every tap held, then released, for [`HOLD_FRAMES`].
///
/// Ticks rather than frames, and at real time they are the same thing — which is the point. A
/// person's 200 ms is 200 ms whatever the machine is doing, so a harness that counted *frames*
/// would type faster and faster as the machine ran faster and would never reproduce a person
/// holding a key while a cassette went past at ninety times real time.
const TYPING_TICKS: u64 = LOAD_SCRIPT.len() as u64 * 2 * HOLD_FRAMES;

/// Wall clock after which a run is not a slow load, it is a hang.
///
/// A guard rather than a bound on anything real: the working figure is two seconds and the whole
/// cassette at real time is 190, so a minute is unreachable either way and exists only so a
/// broken decision fails this file rather than stopping it.
const GIVE_UP: f64 = 60.0;

/// Which tap of [`LOAD_SCRIPT`] is held on tick `tick` of a run whose typing began at
/// `started_at`, or `None` on the ticks between taps and after the script.
fn typing_at(tick: u64, started_at: u64) -> Option<usize> {
    let step = tick.checked_sub(started_at)?;
    if step >= TYPING_TICKS {
        return None;
    }
    // Held for the first half of each tap's stride and released for the second, which is the
    // two-phase press `zx-shot`'s own `press` performs and has to be: the ROM's editor misses a
    // tap that is never released, and types it twice if it is held too long. `HOLD_FRAMES`
    // carries both bounds.
    let stride = 2 * HOLD_FRAMES;
    (step % stride < HOLD_FRAMES).then(|| usize::try_from(step / stride).expect("four taps"))
}

/// Boot a real Sinclair ROM, make the two gestures in `order`, and time the load that follows.
///
/// Every tick goes through [`Automatic::tick`] — the rung's own decision, the real
/// [`Pacer::run_flat_out`], a real clock — so what is timed is the thing that ships rather than a
/// loop written to look like it. The window closes when the drive stops itself at the end of the
/// train, which is the same moment the old measurement stopped at.
///
/// **The keyboard is rebuilt once a tick and the tape key is pressed on a tick**, because that is
/// where the defect lived: at 90× a tick is a hundred and fifty emulated frames, so the gap
/// between two gestures a person makes 200 ms apart is fifteen emulated *seconds* — three times
/// the leader — and no per-frame harness can see that.
fn a_real_cassette_in_order(order: Order, rom: &[u8], cassette: &[u8]) -> Loaded {
    let mut machine = Spectrum::new(rom).expect("a 16 KB ROM");
    machine.insert_tape(tap::parse(cassette).expect("a .tap this crate can read"));
    machine.run_frames(BOOT_FRAMES);

    // Resolved once rather than per tick: `keymap::code_named` formats every binding to compare
    // it, and doing that inside a loop that runs ninety times a second is the redundant work
    // `frontend::pacing`'s own header spends a paragraph on.
    let script: Vec<Vec<_>> = LOAD_SCRIPT
        .iter()
        .map(|tap| {
            tap.iter()
                .map(|name| keymap::code_named(name).expect("a key this emulator binds"))
                .collect()
        })
        .collect();
    let (typed_at, played_at) = match order {
        Order::TypeThenPlay => (1, 1 + TYPING_TICKS + REACHING_FOR_THE_OTHER_KEY),
        Order::PlayThenType => (1 + REACHING_FOR_THE_OTHER_KEY, 1),
    };

    let origin = std::time::Instant::now();
    let mut clock = || origin.elapsed().as_secs_f64();
    let mut rung = Automatic::new(&machine);
    let (mut ticks, mut tick) = (0_u64, 0_u64);
    let (mut wall_at_play, mut frames_at_play) = (0.0, 0);
    let mut started = false;
    while clock() < GIVE_UP {
        tick += 1;
        let held: &[_] = match typing_at(tick, typed_at) {
            Some(tap) => &script[tap],
            None => &[],
        };
        keymap::apply(|code| held.contains(&code), machine.keyboard_mut());
        if tick == played_at {
            machine.tape_mut().play();
            started = machine.tape().is_playing();
            wall_at_play = clock();
            frames_at_play = machine.frames();
        }
        rung.tick(&mut machine, &mut clock, Spectrum::run_frame);
        if started {
            ticks += 1;
            if !machine.tape().is_playing() {
                break;
            }
        }
    }

    let wall = clock() - wall_at_play;
    let frames = machine.frames() - frames_at_play;
    machine.run_frames(SETTLE_FRAMES);
    Loaded {
        wall,
        frames,
        ticks,
        colours: distinct_colours(&screen(&machine)),
    }
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

/// A 16 KB ROM that idles like a prompt and becomes a loader the moment a key goes down.
///
/// ```text
/// 0000  21 00 58     LD HL,0x5800      ; the attribute file
/// 0003  11 01 58     LD DE,0x5801
/// 0006  01 FF 02     LD BC,767
/// 0009  36 47        LD (HL),0x47      ; bright white ink on black paper
/// 000B  ED B0        LDIR
/// 000D  21 00 40     LD HL,0x4000      ; the tally
/// 0010  0E 00        LD C,0            ; the level last seen on the line
/// 0012  AF           XOR A             ; every half-row at once
/// 0013  DB FE        IN A,(0xFE)
/// 0015  E6 1F        AND 0x1F
/// 0017  FE 1F        CP 0x1F           ; is any key down?
/// 0019  20 0A        JR NZ,0x0025      ; then start listening, and never come back
/// 001B  06 00        LD B,0
/// 001D  10 FE        DJNZ -2           ; two full delay loops, which is what puts the idle
/// 001F  06 00        LD B,0            ;   rate at about ten reads a frame instead of two
/// 0021  10 FE        DJNZ -2           ;   thousand
/// 0023  18 ED        JR 0x0012
/// 0025  DB FE        IN A,(0xFE)       ; bit 6 is the tape
/// 0027  E6 40        AND 0x40
/// 0029  B9           CP C              ; the same level as last time?
/// 002A  28 F9        JR Z,0x0025       ; then nothing has happened
/// 002C  4F           LD C,A            ; an edge: remember the new level
/// 002D  34           INC (HL)          ; and count it
/// 002E  20 F5        JR NZ,0x0025
/// 0030  35           DEC (HL)          ; saturating at 255 rather than wrapping
/// 0031  18 F2        JR 0x0025
/// ```
///
/// # Why a third ROM, when [`listening_rom`] already reads the line
///
/// Because that one reads it from its first instruction and never stops, which makes it a fine
/// instrument for *"is this the same machine"* and a useless one for *"when does the trigger
/// fire"*: a machine that is always decoding cannot demonstrate a decision that turns on and off.
/// This one has the two states the signal exists to tell apart, and a person's keypress is what
/// moves it between them — the same gesture, through the same [`keymap`], that the measurement
/// below makes at a real Sinclair ROM.
///
/// The delay loops are the load-bearing part and are the reason this is not
/// [`listening_rom`] with a keyboard check bolted on. A Spectrum at its BASIC prompt reads this
/// port **eight** times a frame — the ROM's `KEY-SCAN` walks one half-row per interrupt — and a
/// bare polling loop reads it two thousand times, so a fixture without the delay would idle above
/// the threshold and the pacing decision would never be exercised in its *off* state.
/// `the_prompt_rom_idles_below_the_threshold_and_listens_above_it` measures both rates rather
/// than trusting this paragraph.
///
/// `XOR A` before the `IN` selects every half-row at once, so any key on the membrane answers.
/// That is a wider question than a real ROM asks in one pass and it is deliberate: which key was
/// pressed is not what is being graded, and a fixture that scanned row by row would be modelling
/// `KEY-SCAN` rather than using it.
///
/// # It counts **edges**, and that correction is the whole reason the fixture is trustworthy
///
/// It counted *highs* first — `listening_rom`'s accumulation — and the mutation that should have
/// reddened the key-order test sailed through it. The reason is a property of a spent cassette
/// that no amount of reasoning about pacing would have surfaced: [`spectrum::tape::Tape`] flips
/// the line at every half-period and stops, so a train with an odd number of pulses **parks the
/// line high for ever**. A guest polling a dead tape then adds one on every pass, and *"the tape
/// was read"* and *"the tape was gone before anybody listened"* produce the same non-zero number.
///
/// An edge count cannot be fooled that way, because a level that never changes yields none. It
/// saturates at [`TALLY_FULL`] instead of wrapping, so *"hundreds of edges"* is a value a test
/// can assert on rather than a residue mod 256 — and a dead line leaves at most the single edge
/// of the first sample disagreeing with the initial `C`.
fn prompt_rom() -> Vec<u8> {
    const PROGRAM: [u8; 51] = [
        0x21, 0x00, 0x58, 0x11, 0x01, 0x58, 0x01, 0xFF, 0x02, 0x36, 0x47, 0xED, 0xB0, 0x21, 0x00,
        0x40, 0x0E, 0x00, 0xAF, 0xDB, 0xFE, 0xE6, 0x1F, 0xFE, 0x1F, 0x20, 0x0A, 0x06, 0x00, 0x10,
        0xFE, 0x06, 0x00, 0x10, 0xFE, 0x18, 0xED, 0xDB, 0xFE, 0xE6, 0x40, 0xB9, 0x28, 0xF9, 0x4F,
        0x34, 0x20, 0xF5, 0x35, 0x18, 0xF2,
    ];
    let mut rom = vec![0; PAGE_SIZE];
    rom[..PROGRAM.len()].copy_from_slice(&PROGRAM);
    rom
}

/// Press the two gestures in `order` under [`Rung::Automatic`], and say how many `EAR` edges the
/// guest counted off the cassette, saturating at [`TALLY_FULL`].
///
/// The tick loop is the window's — [`Automatic::tick`] is the same three lines `src/main.rs`
/// runs — and the keyboard is rebuilt **once a tick**, before the frames, exactly where the
/// window rebuilds it. That placement is not a detail: a key held 150 ms is held thirteen
/// emulated seconds at 90×, so a harness applying it once per *frame* would hold it for one
/// frame of every hundred and would be grading a machine nobody runs.
fn ear_edges_in_order(order: Order) -> u8 {
    let mut machine = Spectrum::new(&prompt_rom()).expect("a page-sized ROM");
    machine.insert_tape(Tape::new(pilot_tone(&machine, LEADER_FRAMES)));

    // The two gestures, as the ticks they land on. Reading them out of the order rather than
    // branching inside the loop keeps the loop identical for both arms, which is what makes the
    // comparison about the order and not about two different harnesses.
    let (typed_at, played_at) = match order {
        Order::TypeThenPlay => (1, 1 + REACHING_FOR_THE_OTHER_KEY),
        Order::PlayThenType => (1 + REACHING_FOR_THE_OTHER_KEY, 1),
    };
    let load_key = keymap::code_named(LOAD_KEY).expect("a key this emulator binds");

    let mut rung = Automatic::new(&machine);
    let mut clock = ticking_clock(FLAT_OUT_BUDGET / FLAT_OUT_FRAMES as f64);
    for tick in 1..=ORDER_TICKS {
        let held = (typed_at..typed_at + KEY_HELD).contains(&tick);
        keymap::apply(|code| held && code == load_key, machine.keyboard_mut());
        if tick == played_at {
            machine.tape_mut().play();
        }
        rung.tick(&mut machine, &mut clock, Spectrum::run_frame);
    }
    machine.memory().read(TALLY)
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

/// How long one frame of `machine` is, in T-states.
///
/// Read off the machine rather than written down, the way `zx-shot`'s `tape_frames` reads it: a
/// 128 runs 70,908 T-states to a 48K's 69,888, and a literal anywhere below would silently give
/// one model the other's arithmetic. `frontend::pacing::EarMeter` takes it for the same reason.
fn frame_t_states(machine: &Spectrum) -> u32 {
    machine.ula().clock().timing().frame_t_states()
}

/// A pilot tone lasting `frames` frames of whatever machine `machine` is.
fn pilot_tone(machine: &Spectrum, frames: u64) -> Vec<u32> {
    let pulses = u64::from(frame_t_states(machine)) * frames / u64::from(PILOT_HALF_PERIOD);
    vec![
        PILOT_HALF_PERIOD;
        usize::try_from(pulses).expect("a few dozen frames of pilot tone is a few hundred pulses")
    ]
}

/// A machine running [`listening_rom`] with [`pilot_tone`] in the drive and the motor **on**.
fn tape_machine(rom: &[u8]) -> Spectrum {
    let mut machine = Spectrum::new(rom).expect("a page-sized ROM");
    machine.insert_tape(Tape::new(pilot_tone(&machine, TAPE_FRAMES)));
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

/// `src/main.rs`'s tick loop for [`Rung::Automatic`], as much of it as a headless test can run.
///
/// The [`Pacer`] and the [`EarMeter`] are one value here because they are one mechanism there:
/// the meter decides what the pacer does, every tick, and a test holding them apart could sample
/// one without the other and grade a loop nobody runs. Three loops below drive ticks — a bounded
/// one, a typing one and a measured one — and the tick itself is the same in all three, so it is
/// written once.
struct Automatic {
    pacer: Pacer,
    ear: EarMeter,
}

impl Automatic {
    /// A loop about to run `machine`, whose frame length sets the meter's threshold.
    fn new(machine: &Spectrum) -> Self {
        Self {
            pacer: Pacer::new(),
            ear: EarMeter::new(frame_t_states(machine)),
        }
    }

    /// One display tick: sample the machine, ask the rung, run whatever it asks for.
    ///
    /// The three lines `src/main.rs` runs, in the order it runs them — the meter is sampled
    /// **before** the decision, so the rate describes frames that have actually happened rather
    /// than the ones about to.
    ///
    /// `frame` takes the machine rather than closing over it because one caller declines to run
    /// past a target frame and the others do not; see [`run_frame_unless_done`].
    fn tick(
        &mut self,
        machine: &mut Spectrum,
        clock: &mut impl FnMut() -> f64,
        mut frame: impl FnMut(&mut Spectrum),
    ) {
        self.ear.sample(machine.ear_reads(), machine.frames());
        match Rung::Automatic.this_tick(self.ear.decoding()) {
            Tick::Paced(speed) => {
                self.pacer.set_speed(speed);
                for _ in 0..self.pacer.advance(TICK) {
                    frame(machine);
                }
            }
            Tick::FlatOut => {
                self.pacer.run_flat_out(clock, || frame(machine));
            }
        }
    }
}

/// Drive `machine` on [`Rung::Automatic`] until it has run `frames`, and say how many ticks it took.
///
/// [`drive`]'s counterpart, and deliberately the same shape: a bounded tick loop, a `TICK` of
/// display time each pass, and an early return the moment the target is reached. What differs is
/// the one line that decides how many frames a tick runs — which is the whole of what this rung
/// changes, and therefore the whole of what the comparison has to isolate.
fn drive_automatically(machine: &mut Spectrum, rung: &mut Automatic, frames: u64) -> u64 {
    let budget = frames + 1;
    let mut clock = ticking_clock(FLAT_OUT_BUDGET / FLAT_OUT_FRAMES as f64);
    for tick in 1..=budget {
        rung.tick(machine, &mut clock, |machine| {
            run_frame_unless_done(machine, frames);
        });
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
