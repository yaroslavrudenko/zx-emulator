//! The mix and the resampler, against numbers that owe them nothing.
//!
//! # What these can and cannot say
//!
//! Everything here is arithmetic on a buffer, and that is the whole of what it grades. **None
//! of it establishes that a person hears the right tune at the right pitch**, which is the
//! thing that was actually asked for. `crates/frontend/src/audio.rs`'s own table says so and
//! this file says it again, because a green suite about audio is exactly the kind of green that
//! gets read as more than it is.
//!
//! What they do catch is the class of defect that is silent and permanent: a mix that clips, a
//! DC offset that eats the headroom, a rate that drifts a few samples a minute, and a filter
//! that quietly removes the signal along with the offset.

use frontend::audio::{Resampler, cpu_hz, mix, queue_target};
use spectrum::Sample;
use spectrum::audio::AMPLITUDE_MAX;

/// A common device rate, and the one this machine reported.
const DEVICE_HZ: u32 = 48_000;

/// A sample with every source silent.
fn silent() -> Sample {
    Sample::default()
}

/// A sample with only the beeper, at `level`.
fn beeper(level: u16) -> Sample {
    let mut sample = Sample::default();
    sample.beeper = level;
    sample
}

/// A sample with all five sources at full scale.
///
/// **Five and not four**: the tape's `EAR` signal joined the mix when the machine started
/// emitting it, and the clipping gate below is only a clipping gate if it carries every source
/// that can be loud at once. A loading tape over a game's own music is not a contrived case —
/// it is what a turbo loader with a title tune does.
fn everything() -> Sample {
    let mut sample = Sample::default();
    sample.beeper = AMPLITUDE_MAX;
    sample.tape = AMPLITUDE_MAX;
    sample.channels = [AMPLITUDE_MAX; 3];
    sample
}

/// A sample with only the tape's `EAR` signal, at `level`.
fn tape(level: u16) -> Sample {
    let mut sample = Sample::default();
    sample.tape = level;
    sample
}

/// Feed `count` copies of `sample` and return what came out.
fn feed_constant(resampler: &mut Resampler, sample: Sample, count: usize) -> Vec<f32> {
    let input = vec![sample; count];
    let mut out = Vec::new();
    resampler.feed(&input, &mut out);
    out
}

// ---------------------------------------------------------------------------------------
// The mix
// ---------------------------------------------------------------------------------------

#[test]
fn silence_mixes_to_nothing() {
    assert_eq!(mix(silent()), 0.0);
}

#[test]
fn five_sources_at_full_scale_stay_under_the_device_range() {
    // **This gate was named `five_sources_at_full_scale_do_not_exceed_the_headroom`** until the
    // S4 ruling. Since the tape left the shared denominator, the five-source sum deliberately
    // exceeds `HEADROOM` (0.88125 against 0.6) — what the assertion below actually holds is the
    // device's full scale, so the name now says so. The old name asserted a property the mix no
    // longer has, and a gate whose name promises something its assertion does not check is how
    // a false claim survives a green suite.
    //
    // The defect this exists for is clipping, which sounds like distortion and is inaudible in
    // any test that only ever plays one source. A 128 running an AY tune *and* clicking the
    // beeper *while a loader screeches* is the case that reaches it.
    //
    // The bound is restated from the rulings rather than read back from the implementation:
    // the game sources top out at the 0.6 headroom, and the tape — decoupled from their
    // denominator by the S4 ruling — adds the beeper's own share of the tape-free scale,
    // 45/96 of that same 0.6, on top. The compile-time assertion in
    // `crates/frontend/src/audio.rs` proves the sum sits under 1.0; this measures the same
    // bound through `mix`, so proof and measurement cannot drift apart.
    let full = mix(everything());
    let beeper_gain = 18_000.0_f32 / 6_800.0;
    let ceiling = 0.6 + beeper_gain / (beeper_gain + 3.0) * 0.6;
    assert!(
        full <= ceiling + 1e-6,
        "five sources at once mix to {full}, past the {ceiling} the two rulings add up to",
    );
    assert!(
        full < 1.0,
        "five sources at once mix to {full}, which is outside a device's range",
    );
    // **The beeper is 2.65x the AY, and that number is from the 128's board.** Its summing
    // network reaches the MC1376 through R112 = 6K8 for the beeper against R132 = 18K for the
    // AY, and contribution in such a network goes as 1/R — so 18000/6800. This asserts the
    // implemented ratio against that source rather than against the implementation, which is
    // the difference between a gate and a restatement. Both sides now divide by the tape-free
    // scale, so the S4 decoupling moved this ratio by nothing — which is itself part of what
    // it asserts.
    let beeper_only = mix(beeper(AMPLITUDE_MAX));
    let mut ay = Sample::default();
    ay.channels = [AMPLITUDE_MAX; 3];
    let ay_only = mix(ay);
    // Three channels at unity against one beeper at its gain.
    let measured = beeper_only / (ay_only / 3.0);
    assert!(
        (measured - beeper_gain).abs() < 0.01,
        "the beeper is {measured}x an AY channel; the 128's resistors say {beeper_gain}x",
    );
    // The thin-48K floor, recomputed against the game mix — the floor's own sentence is about
    // a 48K *game* being thin, and a 48K game has no tape playing. Under the shared
    // denominator this inequality held by 1.1% and `TAPE_GAIN`'s doc warned that any change
    // to the gains would break it; against the tape-free mix the beeper's share is
    // 45/96 = 0.46875 — a 17.2% margin over the 0.4 floor. The decoupling is what turned the
    // fragile inequality structural.
    let mut game = Sample::default();
    game.beeper = AMPLITUDE_MAX;
    game.channels = [AMPLITUDE_MAX; 3];
    let game_full = mix(game);
    assert!(
        beeper_only > game_full * 0.4,
        "a lone beeper is {beeper_only} against a tape-free full mix of {game_full}; a 48K \
         would be thin",
    );
}

#[test]
fn the_mix_is_monotonic_in_the_beeper() {
    // A positive control for the mix itself: a function returning a constant would satisfy both
    // tests above if the constant happened to sit in range.
    let quiet = mix(beeper(0));
    let middle = mix(beeper(AMPLITUDE_MAX / 2));
    let loud = mix(beeper(AMPLITUDE_MAX));
    assert!(quiet < middle && middle < loud, "{quiet} {middle} {loud}");
}

// ---------------------------------------------------------------------------------------
// The DC blocker
// ---------------------------------------------------------------------------------------

#[test]
fn silence_in_is_silence_out() {
    let mut resampler = Resampler::new(cpu_hz(false), DEVICE_HZ);
    let out = feed_constant(&mut resampler, silent(), 20_000);
    assert!(!out.is_empty(), "nothing came out at all");
    for (index, value) in out.iter().enumerate() {
        assert_eq!(*value, 0.0, "sample {index} is {value}, not silence");
    }
}

#[test]
fn a_constant_level_decays_to_nothing() {
    // The speaker held out of position. A machine that sets the beeper high and leaves it there
    // is making no sound at all, and without this filter it would arrive as a permanent offset
    // that eats headroom and thumps when it ends.
    let mut resampler = Resampler::new(cpu_hz(false), DEVICE_HZ);
    let out = feed_constant(&mut resampler, beeper(AMPLITUDE_MAX), 400_000);
    assert!(out.len() > 10_000, "only {} samples out", out.len());

    let first = out[0];
    assert!(
        first > 0.01,
        "the step should arrive before it decays: {first}"
    );

    let settled = out[out.len() - 1];
    assert!(
        settled.abs() < 1e-3,
        "after {} samples a constant level is still at {settled}",
        out.len(),
    );
}

#[test]
fn a_square_wave_keeps_its_amplitude() {
    // The other half of the DC blocker's job, and the one a too-aggressive filter fails: a
    // filter that removes the offset by removing everything would pass the test above and make
    // the emulator silent.
    //
    // ~1 kHz at the machine's rate: a Spectrum beeper tone sits in this range and it is well
    // clear of the filter's ~7 Hz corner.
    let machine_hz = u64::from(cpu_hz(false)) / 32;
    let half_period = (machine_hz / 1000 / 2) as usize;

    let mut input = Vec::new();
    for cycle in 0..200 {
        let level = if cycle % 2 == 0 { AMPLITUDE_MAX } else { 0 };
        input.extend(std::iter::repeat_n(beeper(level), half_period));
    }

    let mut resampler = Resampler::new(cpu_hz(false), DEVICE_HZ);
    let mut out = Vec::new();
    resampler.feed(&input, &mut out);

    // Measure over the second half, after the filter has settled.
    let tail = &out[out.len() / 2..];
    let high = tail.iter().copied().fold(f32::MIN, f32::max);
    let low = tail.iter().copied().fold(f32::MAX, f32::min);
    let peak_to_peak = high - low;

    // A full-scale beeper mixes to its share of the headroom — 45/96 of it since the S4
    // decoupling; this line once said "a quarter", the unweighted mix's figure — so an
    // undistorted square wave swings that far peak to peak. Anything much below means the
    // filter is eating the signal.
    let expected = mix(beeper(AMPLITUDE_MAX));
    assert!(
        peak_to_peak > expected * 0.9,
        "a 1 kHz square wave came out {peak_to_peak} peak-to-peak, against {expected} expected",
    );
}

// ---------------------------------------------------------------------------------------
// The rate
// ---------------------------------------------------------------------------------------

#[test]
fn a_long_run_does_not_drift() {
    // The defect this exists for is invisible for a frame and audible over an hour: a ratio
    // held as an `f32` loses about a part in ten million per step, which is several samples a
    // minute. Both machines are checked, because the 128's sample rate is **not** a whole
    // number — 3,546,900 / 32 = 110,840.625 — and a design that rounded it would pass on the
    // 48K and drift only on the machine that has the sound chip.
    for is_128 in [false, true] {
        let clock = cpu_hz(is_128);
        let mut resampler = Resampler::new(clock, DEVICE_HZ);

        // Sixty seconds of machine time, in whole input samples.
        let seconds = 60_u64;
        let input_samples = (u64::from(clock) * seconds / 32) as usize;
        let out = feed_constant(&mut resampler, silent(), input_samples);

        let expected = u64::from(DEVICE_HZ) * seconds;
        let produced = out.len() as u64;
        let drift = expected.abs_diff(produced);
        assert!(
            drift <= 1,
            "{} seconds at {clock} Hz produced {produced} samples, wanted {expected} (drift {drift})",
            seconds,
        );
    }
}

#[test]
fn the_two_machines_have_different_rates_so_the_test_above_is_not_one_case_twice() {
    // The "I was not looking at the thing" assertion for the loop above. If `cpu_hz` returned
    // the same figure for both, that test would run the 48K case twice and report two passes.
    assert_ne!(cpu_hz(false), cpu_hz(true));
    assert_eq!(cpu_hz(false), 3_500_000);
    assert_eq!(cpu_hz(true), 3_546_900);
}

#[test]
fn feeding_in_pieces_matches_feeding_in_one_go() {
    // A frame loop feeds one frame at a time and depends on this without ever saying so: the
    // filter's history and the resampling phase must carry across calls. A `Resampler` that
    // reset either would click once per frame — fifty times a second, which is a 50 Hz buzz
    // laid over everything.
    let input: Vec<Sample> = (0..50_000)
        .map(|index| {
            beeper(if (index / 37) % 2 == 0 {
                AMPLITUDE_MAX
            } else {
                0
            })
        })
        .collect();

    let mut whole = Resampler::new(cpu_hz(false), DEVICE_HZ);
    let mut at_once = Vec::new();
    whole.feed(&input, &mut at_once);

    let mut piecewise = Resampler::new(cpu_hz(false), DEVICE_HZ);
    let mut in_pieces = Vec::new();
    for chunk in input.chunks(2184) {
        piecewise.feed(chunk, &mut in_pieces);
    }

    assert_eq!(at_once.len(), in_pieces.len(), "different sample counts");
    for (index, (a, b)) in at_once.iter().zip(&in_pieces).enumerate() {
        assert!(
            (a - b).abs() < 1e-6,
            "sample {index}: {a} in one go, {b} in pieces",
        );
    }
    // And the signal is not trivially zero, or the comparison above proves nothing.
    assert!(at_once.iter().any(|value| value.abs() > 0.01));
}

#[test]
fn feed_appends_and_does_not_replace() {
    // Structural: the frame loop reuses one buffer, and a `feed` that cleared it would silently
    // drop whatever the caller had not yet handed to the device.
    let mut resampler = Resampler::new(cpu_hz(false), DEVICE_HZ);
    let mut out = vec![0.25_f32; 3];
    resampler.feed(&vec![silent(); 10_000], &mut out);
    assert!(out.len() > 3);
    assert_eq!(&out[..3], &[0.25, 0.25, 0.25]);
}

// ---------------------------------------------------------------------------------------
// The tape reaches the mix
// ---------------------------------------------------------------------------------------

#[test]
fn a_playing_tape_is_audible_on_its_own() {
    // The defect this closes: the machine loaded tapes correctly and in silence, because the
    // `EAR` line reached bit 6 of a `0xFE` read and nothing else. A level on the line must now
    // move the mix, with every other source quiet.
    assert!(
        mix(tape(AMPLITUDE_MAX)) > 0.0,
        "a tape at full deflection must be heard"
    );
    assert_eq!(mix(tape(0)), 0.0, "and a stopped one must not");
}

#[test]
fn the_tape_is_as_loud_as_the_beeper_at_the_same_level() {
    // **This gate is `the_tape_is_quieter_than_the_beeper_at_the_same_level`, inverted, and
    // the inversion is deliberate rather than drift.** The old assertion read the shared
    // denominator's artefact as the intended property: quieter was never ruled, it was what
    // `TAPE_GAIN`'s ceiling left over once the thin-48K floor capped it, and the constant's
    // own doc called the result structural and pointed at the design change. S4 is that
    // change: the tape left the shared scale, and `TAPE_LEVEL` is *derived* from the beeper's
    // own level — the machine being modelled played its tape through its own speaker, and the
    // screech is famous precisely because it was as loud as the games.
    //
    // Near-equality rather than `==`: the two levels are one expression by construction, but
    // the runtime arithmetic in `mix` takes a different operation order from the const's, so
    // the results may differ by ULPs — never by more than a relative 1e-3.
    let tape_peak = mix(tape(AMPLITUDE_MAX));
    let beeper_peak = mix(beeper(AMPLITUDE_MAX));
    assert!(
        beeper_peak > 0.0,
        "a silent beeper would make the ratio below vacuous"
    );
    assert!(
        ((tape_peak - beeper_peak) / beeper_peak).abs() < 1e-3,
        "the tape is ruled equal to the beeper and came out {tape_peak} against {beeper_peak}",
    );
}

// ---------------------------------------------------------------------------------------
// The rate correction
// ---------------------------------------------------------------------------------------

/// Output samples produced from `frames` frames' worth of input, with the queue held at `queued`.
///
/// **Open-loop: the queue is pinned, so this measures gain and direction and nothing else.** It
/// cannot tell a stabilising controller from a destabilising one — that is what
/// [`settled_queue_depth`] is for, and the reason it exists is that an inverted sign passed every
/// test written against this helper.
fn produced_with_queue(queued: Option<u32>, target: u32, frames: usize) -> usize {
    let mut resampler = Resampler::new(cpu_hz(false), DEVICE_HZ);
    let mut out = Vec::new();
    let frame = vec![silent(); 2184];
    for _ in 0..frames {
        resampler.track(queued, target);
        resampler.feed(&frame, &mut out);
    }
    out.len()
}

/// Run the loop **closed** for `frames` frames and return the queue depth it ends at.
///
/// The queue is fed by what `feed` produces and drained by what a device consumes in one frame —
/// `device_hz / 50`, since the device runs on its own clock and does not care what the emulator
/// is doing. `drift` scales the emulator's frame rate against the device's: `1.002` is an
/// emulator running 0.2% fast, which is the real case that made the backlog grow.
///
/// This is the shape that grades **stability**. An open-loop test holds the error constant and so
/// cannot see whether applying the correction makes the error smaller or larger — which is
/// exactly how a sign error survives a green suite.
fn settled_queue_depth(start: i32, target: u32, drift: f64, frames: usize) -> i32 {
    let mut resampler = Resampler::new(cpu_hz(false), DEVICE_HZ);
    let mut out = Vec::new();
    let mut queued = start;
    let per_frame = vec![silent(); 2184];
    for _ in 0..frames {
        resampler.track(u32::try_from(queued).ok(), target);
        out.clear();
        resampler.feed(&per_frame, &mut out);
        queued += out.len() as i32;
        // What the device took while that frame was being emulated, including the drift.
        let drained = (f64::from(DEVICE_HZ) / 50.0 * drift) as i32;
        queued = (queued - drained).max(0);
    }
    queued
}

#[test]
fn a_deep_queue_is_given_less_and_a_shallow_one_more() {
    // **The sign, and this test asserted the opposite of it until 2026-09-03.** The reasoning
    // behind the error was that a backlog is drained by consuming input faster — but the input
    // rate is not this function's to choose. The machine emits 2184 samples a frame regardless,
    // and `feed` turns each into `corrected_step / cpu_hz` outputs, so a larger step puts *more*
    // into a queue that drains at a fixed device rate.
    //
    // Direction only. Whether applying it converges is `the_loop_converges_instead_of_running_away`,
    // which is the test this one was mistaken for.
    let target = 2400;
    let at_target = produced_with_queue(Some(target), target, 200);
    let too_deep = produced_with_queue(Some(target * 2), target, 200);
    let too_shallow = produced_with_queue(Some(0), target, 200);

    assert!(
        too_deep < at_target,
        "a backlog must be given fewer samples, not more: {too_deep} vs {at_target}"
    );
    assert!(
        too_shallow > at_target,
        "and an emptying queue must be given more: {too_shallow} vs {at_target}"
    );
}

#[test]
fn the_correction_stays_within_its_bound() {
    // What keeps the correction inaudible. `MAX_CORRECTION` is 0.5%, and a queue at ten times
    // its target must not exceed it — the clamp, not the arithmetic, is what is being measured.
    let target = 2400;
    let frames = 500;
    let at_target = produced_with_queue(Some(target), target, frames) as f64;
    let extreme = produced_with_queue(Some(target * 10), target, frames) as f64;
    let ratio = extreme / at_target;
    assert!(
        ratio >= 0.994,
        "a correction beyond the bound would be heard as pitch: ratio {ratio}"
    );
    assert!(
        ratio < 1.0,
        "and it must still be correcting: ratio {ratio}"
    );
}

#[test]
fn no_device_yet_leaves_the_rate_alone() {
    // `page::audio_push` answers `-1` when there is no device, and `main.rs` decodes that to
    // `None` before it reaches here — a depth that does not exist is not a measurement of
    // anything, and correcting against it would start every session with a rate error nothing
    // asked for. It is also the answer a browser gives while its `AudioContext` is suspended,
    // which is the case that would otherwise leave this loop steering on a frozen number.
    let target = 2400;
    let untouched = produced_with_queue(Some(target), target, 100);
    let no_device = produced_with_queue(None, target, 100);
    assert_eq!(no_device, untouched);
}

#[test]
fn the_loop_converges_instead_of_running_away() {
    // **The gate the first version of `track` needed and did not have.** It shipped with the sign
    // inverted — a larger step for a deeper queue — and every open-loop test above passed under
    // it, because pinning the queue measures direction without ever asking whether applying the
    // correction moves the error towards zero or away from it.
    //
    // Simulated over twenty minutes of browser drift the inverted sign reached 8.9 seconds of
    // backlog. Here: start away from the setpoint in both directions and require that the loop
    // ends nearer to it than it began.
    let target = 2400;
    let frames = 3000;

    let from_deep = settled_queue_depth(target as i32 * 2, target, 1.0, frames);
    let from_empty = settled_queue_depth(0, target, 1.0, frames);

    let deep_error = (from_deep - target as i32).abs();
    let empty_error = (from_empty - target as i32).abs();
    assert!(
        deep_error < target as i32,
        "a queue starting at twice the target settled at {from_deep}, error {deep_error}"
    );
    assert!(
        empty_error < target as i32,
        "a queue starting empty settled at {from_empty}, error {empty_error}"
    );
}

#[test]
fn a_drifting_emulator_does_not_accumulate_latency() {
    // The case that produced the defect in the first place: the emulator paces to 50.08 Hz and the
    // device consumes one second per second, so without a loop the backlog grows without bound.
    // 210 ms after four minutes was the browser observation; this is the same shape, run long.
    //
    // **Both rails, and the lower one is not decoration.** A ceiling-only assertion passes on the
    // inverted sign: under it the queue does not run away, it collapses to zero and stays there —
    // which on the desktop is `desktop::fill` substituting silence for the samples that never
    // arrive, once per frame, a continuous 50 Hz rasp in place of the tick this branch set out to
    // remove. Measured: with the sign flipped this test was the one closed-loop gate that still
    // passed. Zero millisecond of backlog is not success.
    let target = 2400;
    let settled = settled_queue_depth(target as i32, target, 1.002, 15_000);
    let milliseconds = f64::from(settled) / f64::from(DEVICE_HZ) * 1000.0;
    assert!(
        (1.0..250.0).contains(&milliseconds),
        "0.2% of drift settled at {milliseconds:.1} ms of backlog over 15,000 frames; the band \
         is (1, 250) — above it the loop lost, at the bottom it is a permanent underrun"
    );
}

#[test]
fn the_setpoint_is_half_the_device_buffer() {
    // `queue_target` is the ruling the frame loop used to spell `* BUFFER_MILLISECONDS / 2000`,
    // where `2000` was a unit conversion and a policy multiplied together. Both halves are
    // asserted separately here, so a change to either is a change a test names.
    let whole_buffer = DEVICE_HZ * page::BUFFER_MILLISECONDS / 1000;
    assert_eq!(
        queue_target(DEVICE_HZ),
        whole_buffer / 2,
        "the setpoint is half the ring, so the correction has the same authority in both \
         directions"
    );
    // And in the unit a person reads off the status bar: half of 100 ms.
    assert_eq!(queue_target(DEVICE_HZ), 2400);
    // A 44.1 kHz device gets its own figure rather than the 48 kHz one — the setpoint is a
    // duration, and the sample count that expresses it is the device's.
    assert_eq!(queue_target(44_100), 2205);
}
