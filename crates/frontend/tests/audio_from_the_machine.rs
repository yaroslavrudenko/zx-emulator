//! The one audio gate with an oracle outside this repository: `BEEP 1,0` is middle C.
//!
//! # Why this exists when `tests/audio_resampling.rs` already passes
//!
//! That file grades the resampler against signals it makes up itself, which is worth having and
//! is a closed loop: it cannot tell whether the **machine** produces sound at all, whether the
//! frontend drains it correctly, or whether what comes out has anything to do with what a
//! Spectrum would play. Every one of its assertions would still pass with the emulator silent.
//!
//! This one runs the real path — type into the machine, run frames, take samples, mix,
//! resample — and checks the result against a number **from outside this project**:
//!
//! > `BEEP duration, pitch` plays a note `pitch` semitones from middle C. `BEEP 1,0` is
//! > therefore one second of **middle C, 261.63 Hz**, and that is a fact about the Sinclair
//! > BASIC manual rather than about this code.
//!
//! `docs/MACHINE.md` ranks a gate with an external referent well above one whose expectation
//! was transcribed from the subject, and this is the only audio gate here that has one.
//!
//! # What it still cannot say
//!
//! **That a person hears it.** This measures zero crossings in a buffer. There is no audio
//! device in this environment and no way to capture one, so *"the tune is right"* remains
//! observation by a person and is recorded as such. A green here means the machine made a tone
//! of the right pitch and the frontend carried it without destroying it — which is the whole
//! chain up to the speaker, and not the speaker.

use frontend::audio::{Resampler, cpu_hz};
use frontend::{keymap, media};
use macroquad::input::KeyCode;
use spectrum::Spectrum;

/// Where the committed 48K ROM is, from the workspace root.
const ROM: &str = "testdata/roms/48.rom";

/// A common device rate, and what this machine's own device reported.
const DEVICE_HZ: u32 = 48_000;

/// Middle C, which is what `BEEP 1,0` plays. From the Sinclair BASIC manual, not from here.
const MIDDLE_C_HZ: f32 = 261.63;

/// Frames a key is held, and released.
///
/// **Ten, because four is not enough.** A key held four frames or fewer is missed entirely and
/// five registers — measured against this ROM on 2026-09-01 — so a script written at six sits
/// one frame above a cliff. `crates/frontend/src/bin/zx-shot.rs` carries the same figure and
/// the same reason.
const HOLD: u64 = 10;

/// Frames drained and discarded so [`Resampler`]'s DC blocker can settle before anything is
/// measured. Its pole is 0.999, so a time constant is 1000 samples — a little under one frame.
const DC_SETTLE_FRAMES: u64 = 5;

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root")
}

/// Hold `keys` for [`HOLD`] frames, then release for the same, through the real keymap.
///
/// Drains the audio buffer as it goes and throws it away. The machine holds two frames of
/// samples and then **counts what it lost** — so a test that ran hundreds of frames without
/// draining would leave `Spectrum::dropped_samples` in the hundreds of thousands, which is a
/// real number describing a real thing that nothing here cares about, sitting where a future
/// reader would reasonably read it as a defect.
fn press(machine: &mut Spectrum, keys: &[KeyCode]) {
    for _ in 0..HOLD {
        keymap::apply(|code| keys.contains(&code), machine.keyboard_mut());
        machine.run_frame();
        let _ = machine.take_samples();
    }
    for _ in 0..HOLD {
        keymap::apply(|_| false, machine.keyboard_mut());
        machine.run_frame();
        let _ = machine.take_samples();
    }
}

/// Run `frames` with nothing held, discarding the audio.
fn settle(machine: &mut Spectrum, frames: u64) {
    for _ in 0..frames {
        machine.run_frame();
        let _ = machine.take_samples();
    }
}

/// Run `frames`, draining audio through the real frontend path, and return what a device
/// would have been handed.
fn collect(machine: &mut Spectrum, resampler: &mut Resampler, frames: u64) -> Vec<f32> {
    let mut out = Vec::new();
    for _ in 0..frames {
        machine.run_frame();
        let produced = machine.take_samples();
        resampler.feed(produced, &mut out);
    }
    out
}

/// The tone in `samples`, from the **spacing** of its zero crossings.
///
/// # This function was the defect, and it reported one in `crates/spectrum` for a day
///
/// It used to count crossings and divide by the length of the region whose *amplitude* was above
/// a threshold. Both halves were defensible and the combination was not: the count comes from the
/// **crossings** and the duration came from the **envelope**, and those two stop at different
/// places. [`Resampler`]'s DC blocker has a pole of 0.999, so when the note stops its output rings
/// down exponentially — measured 2026-09-01: 1653 samples, 34 ms, every one of them above the
/// threshold and none of them carrying a crossing. `rposition` counted all of them as more period.
/// That alone read 5.15 % flat.
///
/// A second, smaller error compounded it. The resampler is built here *while the note is already
/// playing*, so its DC blocker starts with `last_output = 0` in the middle of a square wave and
/// its first few outputs are a step response rather than the signal: the first three falling edges
/// came out at −0.003, −0.003 and −0.027 against a −0.028 threshold and were never counted, making
/// the first interval 377 samples where every other one is 91 or 92. Another 0.64 %.
///
/// 0.948521 × 0.993608 = 0.94245, against the 0.94273 actually observed. The two account for the
/// whole of it, and there was never anything wrong with the machine: the same buffer, measured
/// from its crossing spacing, is **261.71 Hz** against the ROM's own 261.69 and the manual's
/// 261.63 — 0.03 % out.
///
/// So the duration now comes from the same events as the count. `first` and `last` are crossing
/// indices, the span between them holds exactly `crossings - 1` half-periods, and anything before
/// the first crossing or after the last one cannot affect the answer by construction.
fn frequency(samples: &[f32], rate: u32) -> f32 {
    // A threshold, not zero: the DC blocker leaves a little wander around the axis and counting
    // every sign flip would count noise as a tone. A tenth of the peak is well below a real
    // square wave's excursion and well above the settling ripple.
    let peak = samples
        .iter()
        .fold(0.0_f32, |best, value| best.max(value.abs()));
    let threshold = peak / 10.0;
    if threshold <= 0.0 {
        return 0.0;
    }

    let mut first = None;
    let mut last = 0;
    let mut crossings = 0_u32;
    let mut high = false;
    for (index, &value) in samples.iter().enumerate() {
        let crossed = if high {
            value < -threshold
        } else {
            value > threshold
        };
        if !crossed {
            continue;
        }
        high = !high;
        crossings += 1;
        first.get_or_insert(index);
        last = index;
    }

    // Fewer than two crossings is not a tone: there is no interval to measure.
    let Some(first) = first else {
        return 0.0;
    };
    if crossings < 2 || last == first {
        return 0.0;
    }
    // `crossings - 1` half-periods span `last - first` samples, and a period is two of them.
    f64::from(rate) as f32 * (crossings - 1) as f32 / (2.0 * (last - first) as f32)
}

/// `count` samples of a square wave at `hz`, for grading [`frequency`] against a known answer.
fn square(hz: f32, rate: u32, count: usize) -> Vec<f32> {
    let half = f64::from(rate) as f32 / hz / 2.0;
    (0..count)
        .map(|index| {
            if ((index as f32 / half) as u32).is_multiple_of(2) {
                0.5
            } else {
                -0.5
            }
        })
        .collect()
}

#[test]
fn beep_one_zero_is_middle_c_all_the_way_to_the_device() {
    let Ok(rom) = std::fs::read(workspace_root().join(ROM)) else {
        eprintln!("skipping: {ROM} is absent — see testdata/README.md");
        return;
    };
    let mut machine = media::start(&[&rom[..]]).expect("one ROM is a 48K");
    settle(&mut machine, 120);

    // **Typing `BEEP` on a 48K took three attempts, and each wrong one produced a *silent*
    // machine — which is why the screen, not the sample buffer, is what settled it.**
    //
    //   `B`                        -> `BORDER`. At the start of a line the machine is in keyword
    //                                 mode, so the obvious script typed `BORDER 1,0`: a syntax
    //                                 error, and no sound.
    //   extended mode, then `Z`    -> `LN`. Extended mode alone gives the word printed *above*
    //                                 the key. Photographed: `LN ?1,0`, the `?` being the ROM's
    //                                 own error marker.
    //   extended mode, `SS`+`Z`    -> `BEEP`. The word below the key needs SYMBOL SHIFT as well.
    //                                 Photographed: `0 OK, 0:1`.
    //
    // Every wrong version failed with *"the machine produced nothing audible"* — a message that
    // reads like a defect in the beeper and was a defect in the test. `zx-shot` answered it in
    // one command by taking a picture, which is the instrument this repository already had and
    // the reason it exists.
    for keys in [
        &[KeyCode::LeftShift, KeyCode::LeftControl][..], // extended mode
        &[KeyCode::LeftControl, KeyCode::Z],             // SYMBOL SHIFT + Z = BEEP
        &[KeyCode::Key1],                                // duration: one second
        &[KeyCode::Comma],                               // SYMBOL SHIFT + N
        &[KeyCode::Key0],                                // pitch: middle C
        &[KeyCode::Enter],
    ] {
        press(&mut machine, keys);
    }

    let mut resampler = Resampler::new(cpu_hz(false), DEVICE_HZ);
    // **Thrown away while the DC blocker settles.** It is built here in the middle of a note that
    // is already playing, so `last_output = 0` against a signal at full excursion and its first
    // outputs are a step response, not the tone. The pole is 0.999, so the time constant is 1000
    // samples — about one frame — and five frames leaves under a percent of it. Measuring across
    // that transient is one of the two errors that produced the semitone that was not there.
    let _ = collect(&mut machine, &mut resampler, DC_SETTLE_FRAMES);
    // A second of emulated time, which is about how long `BEEP 1,0` lasts. Started right after
    // ENTER so the note is in the middle of the window rather than at its edge.
    let samples = collect(&mut machine, &mut resampler, 45);

    let peak = samples
        .iter()
        .fold(0.0_f32, |best, value| best.max(value.abs()));
    assert!(
        peak > 0.01,
        "the machine produced nothing audible: peak {peak} over {} samples.\n\
         If crates/spectrum's beeper is not wired yet, this is the gate that says so.",
        samples.len(),
    );

    let measured = frequency(&samples, DEVICE_HZ);
    // **Two parts in a thousand, where this used to allow one in ten.**
    //
    // The loose bound was not caution, it was cover. This gate reported 246.65 Hz — 5.7 % flat,
    // very nearly exactly a semitone — passed anyway, and the miss was written up in four other
    // places as a timing defect in `crates/spectrum`. There was no defect. The pitch was right
    // and [`frequency`] was measuring it wrongly, in the two compounding ways its own
    // documentation now sets out.
    //
    // The machine was independently correct all along: the ROM's `BEEP` computes
    // `HL = 437500/f - 30.125`, which for middle C is 1642, giving a half-period of 6687.2
    // T-states and so **261.69 Hz** — 0.02 % from the manual. The same buffer this test collects,
    // measured from its crossing spacing, comes out at **261.71 Hz**.
    //
    // So the tolerance can now grade what it claims to. The measurement is deterministic — fixed
    // ROM, integer emulation, fixed resampler phase, IEEE-754 arithmetic in a fixed order — so
    // there is no run-to-run spread for a wide bound to absorb, and 0.2 % still leaves seven
    // times the observed error.
    //
    // **Where the bound was put, and why not lower.** Below it are the defects worth catching: a
    // semitone is 5.9 %, and confusing the two machines' clocks (3,500,000 against 3,546,900) is
    // 1.34 % — the latter would have sailed through the old 10 % bound and through a 1 % one.
    // Above it is the floor: the ROM's own `HL = 437500/f - 30.125` only lands within 0.02 % of
    // the manual, so a tighter bound would be grading Sinclair's arithmetic rather than this
    // crate's.
    //
    // **A percent was measured and rejected, because it let a real hole live.** With `error < 0.01`
    // the run reports 261.71 Hz (0.03 %) and passes — and it *also* passes at 259.26 Hz (0.91 %)
    // with [`DC_SETTLE_FRAMES`] set to zero, so the settle above could have been deleted by
    // anyone who checked whether the gate still went green. At 0.2 % it cannot: removing the
    // settle turns this red. Measured 2026-09-01, both directions.
    let error = (measured - MIDDLE_C_HZ).abs() / MIDDLE_C_HZ;
    assert!(
        error < 0.002,
        "BEEP 1,0 should be middle C at {MIDDLE_C_HZ} Hz and measured {measured} Hz \
         ({:.2} % out)",
        error * 100.0,
    );
}

#[test]
fn the_frequency_probe_measures_a_tone_it_was_given() {
    // **The assertion this file did not have, and the whole reason it published a wrong number.**
    //
    // `tests/audio_resampling.rs` grades the resampler against signals it makes up itself, and
    // this file grades the machine against the Sinclair manual. Nobody graded the *ruler*.
    // `frequency` had only ever been pointed at the subject, so when it disagreed with the manual
    // the disagreement was attributed to the subject — which is the failure mode this repository
    // has now caught six times, and the first time the instrument was one of its own tests.
    for hz in [110.0_f32, MIDDLE_C_HZ, 440.0, 1000.0] {
        let measured = frequency(&square(hz, DEVICE_HZ, DEVICE_HZ as usize), DEVICE_HZ);
        let error = (measured - hz).abs() / hz;
        assert!(
            error < 0.01,
            "a synthetic {hz} Hz square measured {measured} Hz ({:.2} % out); the probe is \
             wrong before any emulator is involved",
            error * 100.0,
        );
    }
}

#[test]
fn the_frequency_probe_ignores_a_ring_down_after_the_tone_stops() {
    // The exact shape that fooled it, kept as a vector so it cannot come back. A tone that stops,
    // followed by a DC blocker's exponential decay: the tail carries no crossings and every
    // sample of it is above the threshold, so a probe that takes its duration from the amplitude
    // envelope divides the right count by too long a window and reports flat.
    //
    // 30 000 samples of tone and 18 000 of tail is what the real capture looked like. The old
    // probe read 243.7 Hz here — 6.9 % out, comfortably inside the 10 % bound it was given, which
    // is how it stayed green while being wrong.
    let mut samples = square(MIDDLE_C_HZ, DEVICE_HZ, 30_000);
    let mut level = -0.5_f32;
    for _ in 0..18_000 {
        samples.push(level);
        level *= 0.999;
    }

    let measured = frequency(&samples, DEVICE_HZ);
    let error = (measured - MIDDLE_C_HZ).abs() / MIDDLE_C_HZ;
    assert!(
        error < 0.01,
        "a {MIDDLE_C_HZ} Hz tone followed by a ring-down measured {measured} Hz ({:.2} % out)",
        error * 100.0,
    );
}

#[test]
fn the_frequency_probe_says_nothing_rather_than_something_about_silence() {
    // `assert!(error < ..)` is satisfiable by accident, so the probe's floor is worth pinning
    // too: no signal and a single edge are both "no measurement", not a small number that would
    // read as a very low note.
    assert_eq!(frequency(&[0.0; 1000], DEVICE_HZ), 0.0, "silence");
    assert_eq!(frequency(&[], DEVICE_HZ), 0.0, "nothing at all");
    let mut once = vec![-0.5_f32; 500];
    once.extend([0.5_f32; 500]);
    assert_eq!(frequency(&once, DEVICE_HZ), 0.0, "one edge is not a period");
}

#[test]
fn a_machine_typing_nothing_makes_no_sound() {
    // The assertion whose failure means "I was not looking at the thing". The test above
    // measures a frequency, and a frequency measured from noise is still a number — so a
    // resampler emitting garbage, or a beeper stuck high and rattling, could land near 261 Hz
    // by accident. A machine sitting at the BASIC prompt must be **silent**, and if it is not,
    // the tone measured above was not the BEEP.
    let Ok(rom) = std::fs::read(workspace_root().join(ROM)) else {
        eprintln!("skipping: {ROM} is absent — see testdata/README.md");
        return;
    };
    let mut machine = media::start(&[&rom[..]]).expect("one ROM is a 48K");
    settle(&mut machine, 120);

    let mut resampler = Resampler::new(cpu_hz(false), DEVICE_HZ);
    let samples = collect(&mut machine, &mut resampler, 50);

    let peak = samples
        .iter()
        .fold(0.0_f32, |best, value| best.max(value.abs()));
    assert!(
        peak < 0.01,
        "an idle 48K at the prompt is making noise: peak {peak}",
    );
}
