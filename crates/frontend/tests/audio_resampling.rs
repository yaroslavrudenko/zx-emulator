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

use frontend::audio::{Resampler, cpu_hz, mix};
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

/// A sample with all four sources at full scale.
fn everything() -> Sample {
    let mut sample = Sample::default();
    sample.beeper = AMPLITUDE_MAX;
    sample.channels = [AMPLITUDE_MAX; 3];
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
fn four_sources_at_full_scale_do_not_exceed_the_headroom() {
    // The defect this exists for is clipping, which sounds like distortion and is inaudible in
    // any test that only ever plays one source. A 128 running an AY tune *and* clicking the
    // beeper is the case that reaches it.
    let full = mix(everything());
    assert!(
        (0.0..=1.0).contains(&full),
        "four sources at once mix to {full}, which is outside a device's range",
    );
    // **The beeper is 2.65x the AY, and that number is from the 128's board.** Its summing
    // network reaches the MC1376 through R112 = 6K8 for the beeper against R132 = 18K for the
    // AY, and contribution in such a network goes as 1/R — so 18000/6800. This asserts the
    // implemented ratio against that source rather than against the implementation, which is
    // the difference between a gate and a restatement.
    let beeper_only = mix(beeper(AMPLITUDE_MAX));
    let mut ay = Sample::default();
    ay.channels = [AMPLITUDE_MAX; 3];
    let ay_only = mix(ay);
    // Three channels at unity against one beeper at its gain.
    let measured = beeper_only / (ay_only / 3.0);
    let expected = 18_000.0 / 6_800.0;
    assert!(
        (measured - expected).abs() < 0.01,
        "the beeper is {measured}x an AY channel; the 128's resistors say {expected}x",
    );
    // And the 48K — beeper alone, no chip — is now a usable fraction of full scale rather than
    // the quarter an unweighted sum gave it. That was the audible cost of the earlier guess.
    assert!(
        beeper_only > full * 0.4,
        "a lone beeper is {beeper_only} against a full mix of {full}; a 48K would be thin",
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

    // A full-scale beeper mixes to a quarter of the headroom, so an undistorted square wave
    // swings that far peak to peak. Anything much below means the filter is eating the signal.
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
