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

use frontend::pacing::{FRAME_NANOS, MAX_CATCH_UP, Pacer, RateMeter};

/// Exactly one frame of a 50 Hz machine.
const FRAME: Duration = Duration::from_millis(20);

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
fn the_rate_meter_divides_by_the_time_that_actually_passed() {
    // A window that closes late — because the whole process stalled — must divide by the real
    // elapsed time, not by the nominal window, or a stall would report as a speed-up.
    let mut meter = RateMeter::new(1.0, 0.0);
    meter.sample(4.0, 50);
    assert_eq!(meter.hz(), 12.5, "50 frames over four seconds");
}
