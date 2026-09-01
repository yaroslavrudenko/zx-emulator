//! The pacing arithmetic, against literal `Duration` sequences.
//!
//! What is checkable about frame pacing is the accounting: given this much elapsed time, how
//! many frames are owed, how many are run, and how many are declared lost. That is pure and
//! deterministic and every expectation below is a literal.
//!
//! What is **not** checkable here is whether the result looks smooth. A run reporting
//! `50.0 Hz, 0 dropped` that still stutters would be a vsync interaction happening below this
//! module, and nothing in a headless test could see it. That row is in the crate's
//! observation table and is not softened.

use std::time::Duration;

use frontend::pacing::{FRAME_NANOS, LOSS_ALARM, LossMeter, MAX_CATCH_UP, Pacer, RateMeter};

/// Exactly one frame of a 50 Hz machine.
const FRAME: Duration = Duration::from_millis(20);

/// The window the shell gives [`LossMeter`], so these read as the running emulator does.
const LOSS_WINDOW: f64 = 1.0;

#[test]
fn one_frame_of_elapsed_time_runs_one_frame() {
    let mut pacer = Pacer::new();
    assert_eq!(pacer.advance(FRAME), 1);
    assert_eq!((pacer.ran(), pacer.dropped()), (1, 0));
}

#[test]
fn less_than_a_frame_runs_nothing_and_keeps_the_remainder() {
    let mut pacer = Pacer::new();
    assert_eq!(pacer.advance(Duration::from_millis(12)), 0);
    assert_eq!(pacer.advance(Duration::from_millis(7)), 0, "19 ms total");
    assert_eq!(pacer.advance(Duration::from_millis(1)), 1, "20 ms exactly");
    assert_eq!((pacer.ran(), pacer.dropped()), (1, 0));
}

#[test]
fn a_sixty_hertz_display_averages_exactly_fifty() {
    // The reason the sub-frame remainder is kept rather than discarded. 60 Hz is 16.667 ms a
    // tick; a pacer that dropped the remainder would run one frame per tick and the machine
    // would be 17 % slow, with nothing reporting it. Over three ticks the pattern is 1, 1, 2.
    let mut pacer = Pacer::new();
    let tick = Duration::from_nanos(16_666_667);
    let run: Vec<u64> = (0..6).map(|_| pacer.advance(tick)).collect();

    assert_eq!(run, vec![0, 1, 1, 1, 1, 1]);
    assert_eq!(pacer.ran(), 5, "100 ms of elapsed time is five frames");
    assert_eq!(pacer.dropped(), 0);
}

#[test]
fn a_hundred_ticks_of_sixty_hertz_land_within_one_frame_of_the_wall_clock() {
    // The property the vector above only samples: over a long run the accumulated remainder
    // must not drift. 100 ticks is 1.6667 s, which is 83 frames.
    let mut pacer = Pacer::new();
    let tick = Duration::from_nanos(16_666_667);
    for _ in 0..100 {
        pacer.advance(tick);
    }
    assert_eq!(pacer.ran(), 83);
    assert_eq!(pacer.dropped(), 0);
}

#[test]
fn a_backlog_is_run_up_to_the_bound_and_the_rest_is_declared_lost() {
    // A stall of half a second owes 25 frames. Running all of them takes longer than a tick
    // and grows the backlog further, so the bound is the point: run four, lose twenty-one,
    // and say so.
    let mut pacer = Pacer::new();
    assert_eq!(pacer.advance(Duration::from_millis(500)), MAX_CATCH_UP);
    assert_eq!(pacer.ran(), 4);
    assert_eq!(pacer.dropped(), 21, "25 owed, 4 run");
}

#[test]
fn exactly_the_bound_drops_nothing() {
    // The boundary, in both directions: four frames owed is four run and none lost; five is
    // four run and one lost. An off-by-one in the `min` shows up here and nowhere else.
    let mut pacer = Pacer::new();
    assert_eq!(pacer.advance(FRAME * 4), 4);
    assert_eq!((pacer.ran(), pacer.dropped()), (4, 0));

    let mut pacer = Pacer::new();
    assert_eq!(pacer.advance(FRAME * 5), 4);
    assert_eq!((pacer.ran(), pacer.dropped()), (4, 1));
}

#[test]
fn a_suspended_machine_reports_a_large_loss_rather_than_overflowing() {
    // A laptop lid closed for a day hands this an elapsed time no `u64` of frames holds. A
    // frontend that aborted because the machine had been asleep would be a worse outcome than
    // one that reports a very large number of dropped frames.
    let mut pacer = Pacer::new();
    assert_eq!(pacer.advance(Duration::from_secs(86_400)), MAX_CATCH_UP);
    assert_eq!(pacer.ran(), 4);
    assert_eq!(pacer.dropped(), 86_400 * 50 - 4);

    let mut pacer = Pacer::new();
    assert_eq!(pacer.advance(Duration::MAX), MAX_CATCH_UP);
    assert_eq!(pacer.dropped(), u64::MAX - MAX_CATCH_UP);
}

#[test]
fn zero_elapsed_time_runs_nothing() {
    let mut pacer = Pacer::new();
    assert_eq!(pacer.advance(Duration::ZERO), 0);
    assert_eq!((pacer.ran(), pacer.dropped()), (0, 0));
}

#[test]
fn the_frame_length_is_fifty_hertz() {
    assert_eq!(FRAME_NANOS, 20_000_000);
    assert_eq!(FRAME_NANOS * 50, 1_000_000_000);
}

#[test]
fn the_rate_meter_reports_nothing_until_its_window_closes() {
    let mut meter = RateMeter::new(1.0, 0.0);
    meter.sample(0.5, 25);
    assert_eq!(meter.hz(), 0.0, "half a window is not a measurement");
    meter.sample(1.0, 50);
    assert_eq!(meter.hz(), 50.0);
}

#[test]
fn the_rate_meter_measures_each_window_against_the_last() {
    // The second window must not re-count the first window's frames, which is the mistake
    // that makes a struggling emulator report a rising figure.
    let mut meter = RateMeter::new(1.0, 0.0);
    meter.sample(1.0, 50);
    assert_eq!(meter.hz(), 50.0);
    meter.sample(2.0, 80);
    assert_eq!(meter.hz(), 30.0, "30 frames in the second second, not 80");
}

#[test]
fn a_burst_of_losses_clears_once_the_frames_come_back() {
    // **The defect.** The bar took its colour from `Pacer::dropped`, which is cumulative and
    // never falls, so one lost frame at any moment latched it red for the rest of the run. A
    // loss at start-up is close to certain — the window opens, the audio device initialises,
    // the first `elapsed` is enormous and `MAX_CATCH_UP` clips the rest — so the indicator was
    // very nearly always on, which is the same as being off.
    let mut loss = LossMeter::new(LOSS_WINDOW, 0.0);

    // A start-up stall: one tick owed 25 frames, four ran, twenty-one were lost.
    loss.sample(0.1, 21);
    assert_eq!(loss.recent(), 21);
    assert!(!loss.keeping_up(), "twenty-one frames lost is a stall");

    // And then nothing else goes wrong. The cumulative total stays at 21 for ever, which is
    // exactly why it is the wrong thing to colour with; the meter has to let go of it anyway.
    loss.sample(1.0, 21);
    assert!(
        !loss.keeping_up(),
        "the window holding the losses has only just closed, and they are still recent",
    );

    loss.sample(2.0, 21);
    assert!(
        loss.keeping_up(),
        "two clean seconds after the last lost frame and the bar is still red — this is the \
         latch, and it is the whole bug",
    );
    assert_eq!(loss.recent(), 0);
}

#[test]
fn a_sustained_stutter_stays_red_across_every_window_boundary() {
    // The half people forget. Recovery is easy to get right and easy to over-shoot: a meter that
    // simply restarts its count when the window turns over reads **zero** for an instant at every
    // boundary, and the bar flickers grey while frames are still being lost. A readout that
    // blinks "fine" during a stall is the same defect as one that latches, pointing the other
    // way, and only a run that crosses a boundary can tell them apart.
    let mut loss = LossMeter::new(LOSS_WINDOW, 0.0);
    let mut dropped = 0;

    // Two frames lost every tenth of a second, for two seconds — two whole windows, so the
    // boundaries at t = 1.0 and t = 2.0 both land on a tick.
    for tick in 1..=20 {
        dropped += 2;
        let now = f64::from(tick) / 10.0;
        loss.sample(now, dropped);
        assert!(
            !loss.keeping_up(),
            "grey at t = {now} with {dropped} frames lost and more arriving",
        );
    }

    // Still losing, so the count must still reflect a window's worth rather than a fresh start.
    assert_eq!(loss.recent(), 20, "a full window of losses at the boundary");
}

#[test]
fn one_lost_frame_is_counted_but_is_not_an_alarm() {
    // A frame is only ever lost when a single tick owed more than `MAX_CATCH_UP`, so any loss
    // means a tick took over 100 ms — a resize, a tab regaining focus, a page fault. That is a
    // real event and `Pacer::dropped` keeps it for ever. It is not a condition, and the colour
    // is about conditions.
    let mut loss = LossMeter::new(LOSS_WINDOW, 0.0);

    loss.sample(0.5, 1);
    assert_eq!(loss.recent(), 1, "the loss is seen, and counted");
    assert!(loss.keeping_up(), "one lost frame is a hiccup, not a stall");

    loss.sample(0.6, LOSS_ALARM);
    assert!(
        !loss.keeping_up(),
        "a second inside the window is recurring"
    );
}

#[test]
fn a_stall_colours_the_bar_before_the_window_closes() {
    // The property that rules out reusing `RateMeter` here, and the reason there are two types
    // that look alike. `RateMeter` reports **closed windows only**, which is right for a rate —
    // a partial window divides a few frames by a little time and reads as noise — and wrong for
    // an alarm. Losses are events, not a rate: one is one, immediately. Waiting for the window
    // would leave a stutter uncoloured for up to a second, by which time a short one is over and
    // the person it was for never saw it.
    let mut loss = LossMeter::new(LOSS_WINDOW, 0.0);
    loss.sample(0.01, 9);
    assert!(
        !loss.keeping_up(),
        "nine frames lost 10 ms in and the bar is still grey",
    );
}

#[test]
fn the_lifetime_total_keeps_climbing_while_the_colour_lets_go() {
    // Both numbers stay true and they answer different questions — which is the shape of the fix
    // rather than a side effect of it. The total is what happened; the meter is what is
    // happening. Deleting the total to fix the colour would have thrown away a real measurement,
    // and this is the assertion that says so.
    let mut pacer = Pacer::new();
    let mut loss = LossMeter::new(LOSS_WINDOW, 0.0);

    // Half a second of stall: 25 frames owed, four run, twenty-one lost.
    pacer.advance(Duration::from_millis(500));
    loss.sample(0.0, pacer.dropped());
    assert_eq!(pacer.dropped(), 21);
    assert!(!loss.keeping_up());

    // Then a hundred clean frames, which is two seconds of them.
    for tick in 1..=100 {
        pacer.advance(FRAME);
        loss.sample(f64::from(tick) / 50.0, pacer.dropped());
    }

    assert_eq!(
        pacer.dropped(),
        21,
        "the total does not forget, and must not"
    );
    assert_eq!(pacer.ran(), 104, "four caught up, then a hundred clean");
    assert!(
        loss.keeping_up(),
        "and the colour does forget, which is its job"
    );
}

#[test]
fn a_suspended_machine_does_not_overflow_the_loss_meter_either() {
    // `Pacer::advance` saturates rather than panicking when a laptop lid has been shut, and this
    // has to survive what that produces — `overflow-checks` is on in release here, so an
    // unguarded subtraction would abort the frontend for the same reason the pacer refuses to.
    let mut pacer = Pacer::new();
    let mut loss = LossMeter::new(LOSS_WINDOW, 0.0);

    pacer.advance(Duration::MAX);
    loss.sample(0.0, pacer.dropped());
    assert_eq!(loss.recent(), u64::MAX - MAX_CATCH_UP);
    assert!(!loss.keeping_up());

    // And a total that went backwards — which the pacer cannot produce, but a future caller
    // could — reads as nothing new rather than wrapping to a fresh alarm of its own. The window
    // has to have closed first, or the subtraction is against zero and proves nothing.
    let mut loss = LossMeter::new(LOSS_WINDOW, 0.0);
    loss.sample(0.0, 100);
    loss.sample(1.0, 100);
    assert_eq!(loss.recent(), 100, "one window's worth, now closed");
    loss.sample(1.1, 0);
    assert_eq!(
        loss.recent(),
        100,
        "the closed window still counts and the reversal added nothing",
    );
}

#[test]
fn the_rate_meter_divides_by_the_time_that_actually_passed() {
    // A window that closes late — because the whole process stalled — must divide by the real
    // elapsed time, not by the nominal window, or a stall would report as a speed-up.
    let mut meter = RateMeter::new(1.0, 0.0);
    meter.sample(4.0, 50);
    assert_eq!(meter.hz(), 12.5, "50 frames over four seconds");
}
