//! Gate: what the AY puts into the sample stream — and the frame hash, which grades change.
//!
//! # Two kinds of gate live here and they must not be confused
//!
//! `docs/M7.md` Decision 6 draws the line this file is organised around: **structure is
//! provable in-repo and every magnitude is a transcription nothing here can adjudicate.** The
//! tests below are split accordingly, and the split is the point:
//!
//! | | What it establishes |
//! |---|---|
//! | The periodicity tests | **proven.** A tone register of `n` produces a square wave in the stream whose period is an exact function of `n`, derived from the two constants and asserted by value. A generator that dropped a step at a window boundary, or slipped a sample, fails them |
//! | The separation test | **proven**, and it is the one `docs/M7.md` Decision 6 requires by name |
//! | [`FRAME_HASH`] | **change, and nothing else.** It is the only number in this suite recorded from a run rather than derived, because that is what a regression hash *is* |
//!
//! # What a frame hash is worth, stated before the number rather than after
//!
//! `docs/MACHINE.md`'s verification item 4 is exact about it: *"Does not prove correctness —
//! proves change, which is what catches a regression once something works."* Its later re-read
//! adds the trap: *"a hash taken at an unstated frame position grades the position as much as
//! the pixels."*
//!
//! So the position is stated rather than implicit. [`FRAME_HASH`] covers **exactly
//! [`MEASURED_SAMPLES`] samples**, taken from a machine at a stated register state, after the
//! stream has been flushed, over a run of a stated length. Change any of those and the hash is
//! about something else.
//!
//! **And it covers the AY's channels alone.** That is not tidiness: it is the constraint
//! Decision 6 imposes on this milestone, *"so that adding the beeper does not falsify an M7
//! gate"*. `the_ay_hash_does_not_move_when_the_beeper_does` is that constraint as a failing
//! case rather than as a note, and it is what makes the separation real instead of intended.
//!
//! # What nothing here grades
//!
//! - **Whether the numbers are right.** A different volume table, a different prescaler or a
//!   different tap position each produce a different hash, and nothing in this repository can
//!   say which of two hashes is the correct one. Only a human ear or a hardware trace can.
//! - **Pitch.** No test here distinguishes correct from consistently-wrong-by-a-factor.
//! - **When in the frame the writes land.** Music drivers write from the interrupt handler and
//!   the audible result depends on it; nothing measures it.

mod common;
mod m7_common;

use m7_common::{ay_poke, machine_128, run_program};
use spectrum::audio::{AMPLITUDE_MAX, SAMPLE_PERIOD_T_STATES};
use spectrum::{Sample, Spectrum};

/// Where the filler runs: bank 2, uncontended on both machines, so a `NOP` costs four
/// T-states wherever in the frame it falls and the run length below is arithmetic.
const FILLER: u16 = common::SLED;

/// T-states in one call to `Ay::step`, transcribed rather than imported.
///
/// Eight AY master clocks at two T-states each. It is written as a literal here for the same
/// reason `m7_beeper.rs`'s port constants are: an expectation taken from the subject agrees
/// with it by construction, which `docs/STATUS.md` records as a tautology rather than a test.
const AY_STEP_T_STATES: u32 = 16;

/// Samples one `Ay::step` spans. Two steps to a sample, so this is a fraction written as its
/// reciprocal below rather than as a number.
const STEPS_PER_SAMPLE: u32 = SAMPLE_PERIOD_T_STATES / AY_STEP_T_STATES;

const _: () = assert!(STEPS_PER_SAMPLE == 2);

/// Full volume on a channel, and the amplitude that produces.
const FULL_VOLUME: u8 = 0x0F;

/// Mixer bits, **active low**: tone A only, every noise source off.
const TONE_A_ONLY: u8 = 0b0011_1110;

/// Mixer bits: noise A only.
const NOISE_A_ONLY: u8 = 0b0011_0111;

/// Mixer bits: everything off on every channel.
const ALL_OFF: u8 = 0b0011_1111;

/// Run `nops` `NOP`s out of uncontended memory — exactly `4 * nops` T-states.
fn fill(machine: &mut Spectrum, nops: usize) {
    run_program(machine, FILLER, &vec![0x00_u8; nops], nops);
}

/// Discard everything the chip has emitted so far, and one clean window besides.
///
/// # Why the extra window, which is a real finding rather than a fudge
///
/// A register write lands **inside** a sample window, so the window containing it is a
/// genuine partial: part of it was accumulated under the old register value and part under
/// the new one. That is correct — a guest really can change the volume a third of the way
/// through 32 T-states, and integrating is what makes the change audible at the moment it
/// happened rather than at the next boundary.
///
/// It is also not what any steady-state expectation below describes. Measured: configuring a
/// bare tone leaves the last write at T-state 228 and the window `224..256` therefore
/// carrying 12 T-states of output where the steady state carries 16. The first draft of these
/// tests started measuring there and disagreed with its own derivation by exactly that
/// margin — which is the derivation being right and the measurement starting in the wrong
/// place.
///
/// So: run past the last write, then discard. [`SETTLE_NOPS`] is two windows' worth, which is
/// more than one write can contaminate.
fn settle(machine: &mut Spectrum) {
    fill(machine, SETTLE_NOPS);
    let _ = machine.take_samples();
}

/// `NOP`s [`settle`] runs: two sample periods.
const SETTLE_NOPS: usize = 2 * SAMPLE_PERIOD_T_STATES as usize / 4;

/// Configure channel A as a bare tone at `period`, at full volume, and settle the stream.
fn tone_a(period: u16) -> Spectrum {
    let mut machine = machine_128();
    ay_poke(&mut machine, 0, (period & 0xFF) as u8);
    ay_poke(&mut machine, 1, (period >> 8) as u8);
    ay_poke(&mut machine, 7, TONE_A_ONLY);
    ay_poke(&mut machine, 8, FULL_VOLUME);
    settle(&mut machine);
    machine
}

/// The smallest `p` for which `stream` repeats every `p` samples.
///
/// # Why the *period* and not the gap between edges
///
/// A first draft measured the gaps between value changes and derived a half-period from them.
/// **It was the wrong instrument, and measuring showed why:** the chip's steps fall every 16
/// T-states and a sample window is 32, so a toggle can land in the *middle* of a window. The
/// window it lands in is then a partial — neither of the wave's two levels — and *every*
/// sample differs from its neighbour even though the wave has one edge per two samples. The
/// gap read 1 where the derivation said 2, and the derivation was right.
///
/// The period is the claim that survives that, because a mid-window toggle produces a stream
/// that is still exactly periodic — the partials repeat too. It is also the claim a
/// generator that slipped or doubled a step anywhere would fail.
fn period_in_samples(stream: &[u16], what: &str) -> usize {
    assert!(
        stream.iter().any(|&value| value != stream[0]),
        "{what}: a constant stream is periodic at every period, so this would say nothing"
    );
    (1..stream.len())
        .find(|&period| {
            stream
                .iter()
                .skip(period)
                .zip(stream.iter())
                .all(|(a, b)| a == b)
        })
        .unwrap_or_else(|| panic!("{what}: the stream does not repeat within its own length"))
}

/// `stream` with runs of equal values collapsed — the sequence of levels it passes through.
fn levels(stream: &[u16]) -> Vec<u16> {
    let mut out: Vec<u16> = stream.to_vec();
    out.dedup();
    out
}

/// Channel A's amplitude in each sample of a run of `nops` `NOP`s.
fn channel_a(machine: &mut Spectrum, nops: usize) -> Vec<u16> {
    fill(machine, nops);
    machine
        .take_samples()
        .iter()
        .map(|sample| sample.channels[0])
        .collect()
}

// ---------------------------------------------------------------------------
// Structure: the chip's timing survives the trip into the sample stream
// ---------------------------------------------------------------------------

#[test]
fn a_tone_periods_square_wave_arrives_in_the_stream_at_the_rate_it_names() {
    // **Derived, not observed.** A tone counter of `n` toggles every `n` steps, a step is
    // `AY_STEP_T_STATES` T-states, and a sample is `SAMPLE_PERIOD_T_STATES` — so one half of
    // the square wave is `n / STEPS_PER_SAMPLE` samples wide, and the whole wave twice that.
    //
    // The claim is asserted through the *gaps between edges*, which is stronger than either a
    // transition count or a fixed pattern: a count is an integer that a run's two ends can
    // move by one, and a fixed pattern additionally asserts a phase this test does not derive.
    // Uniform gaps say the generator neither slipped nor doubled a step anywhere in the run.
    for period in [4_u16, 8, 16, 64] {
        let expected = usize::try_from(2 * u32::from(period) / STEPS_PER_SAMPLE)
            .expect("a wave of a few samples");
        let mut machine = tone_a(period);
        let stream = channel_a(&mut machine, expected * 8 * 16);
        let what = format!("period {period}");

        assert_eq!(period_in_samples(&stream, &what), expected, "{what}");
        // And it must be a square wave between the two extremes rather than a constant with a
        // wobble, or the gap measurement above holds over edges that mean nothing.
        assert!(stream.contains(&AMPLITUDE_MAX), "{what}: no high half");
        assert!(stream.contains(&0), "{what}: no low half");
    }
}

#[test]
fn doubling_the_tone_period_halves_the_pitch() {
    // The relation between two register values, which is a stronger claim than either absolute
    // period and is the one thing here that no transcribed prescaler can affect: whatever
    // `AY_STEP_T_STATES` really is, `2n` must produce exactly twice `n`'s half-period.
    let wave = |period: u16| -> usize {
        let mut machine = tone_a(period);
        let stream = channel_a(&mut machine, 4096);
        period_in_samples(&stream, "doubling")
    };
    let base = wave(16);
    assert!(base > 1, "the base case must resolve more than one sample");
    assert_eq!(wave(32), base * 2);
    assert_eq!(wave(64), base * 4);
}

#[test]
fn a_channel_with_nothing_mixed_in_does_not_move() {
    // The mixer's polarity, seen from the output rather than from the chip. With every source
    // disabled the channel sits at its level — silence to a speaker, and **not** the same
    // statement as "the amplitude is zero", which is the confusion an inverted mixer hides in.
    let mut machine = tone_a(4);
    ay_poke(&mut machine, 7, ALL_OFF);
    settle(&mut machine);
    let stream = channel_a(&mut machine, 1024);
    assert!(
        stream.iter().all(|&value| value == AMPLITUDE_MAX),
        "a disabled channel at full volume is a constant, not a zero"
    );

    // And turning the volume down is what silences it, which is a different mechanism.
    ay_poke(&mut machine, 8, 0);
    settle(&mut machine);
    assert!(channel_a(&mut machine, 1024).iter().all(|&v| v == 0));
}

#[test]
fn the_noise_source_reaches_the_stream_and_is_not_a_tone() {
    // Structure, not magnitude: the noise generator's output must actually vary, and must not
    // be periodic at any short period — which is the difference between a noise channel and a
    // channel somebody wired to a tone by accident.
    let mut machine = machine_128();
    ay_poke(&mut machine, 6, 1); // the fastest noise there is
    ay_poke(&mut machine, 7, NOISE_A_ONLY);
    ay_poke(&mut machine, 8, FULL_VOLUME);
    let _ = machine.take_samples();

    let stream = channel_a(&mut machine, 4096);
    assert!(stream.contains(&AMPLITUDE_MAX) && stream.contains(&0));
    for period in 1..64_usize {
        assert!(
            stream
                .iter()
                .skip(period)
                .zip(stream.iter())
                .any(|(a, b)| a != b),
            "the noise must not repeat every {period} samples"
        );
    }
}

#[test]
fn the_envelope_reaches_the_stream_as_a_sixteen_step_ramp() {
    // **Structure, and the strongest claim available about the envelope's output.** Bit 4 of
    // an amplitude register hands the channel to the envelope, which walks sixteen levels. So
    // the stream, with its runs of equal values collapsed, must be exactly sixteen strictly
    // monotonic values from one end of the volume table to the other — and the *direction* is
    // what the shape's `ATT` bit decides.
    //
    // A model that ignored bit 4 produces a constant; one that inverted `ATT` produces the
    // mirror image; one whose ramp had fifteen or seventeen steps fails the count. None of
    // that needs a single value from the volume table, which nothing here can grade.
    //
    // **Derived before it was chosen, and re-derived when a constant moved.** One envelope
    // count is `ENVELOPE_DIVISOR / STEP_MASTER_CLOCKS = 2` steps of 16 T-states, so at period
    // `ENVELOPE_PERIOD` a level lasts `2 * 16 * 16 = 512` T-states — 16 samples — and a
    // sixteen-step ramp is 8192 T-states, two whole ramps in the run below.
    //
    // The divisor was **256 rather than 16** in a first draft, which made every envelope
    // sixteen times too slow. Nothing here caught it and nothing here could: every assertion
    // in this file is relative — a ramp of sixteen monotonic levels, a period that scales with
    // its register — and all of them pass at any divisor. The datasheet's formula is about a
    // *cycle*, not a step, and re-reading it is what caught it. See `ay::ENVELOPE_DIVISOR`.
    for (shape, rising) in [(0x0C_u8, true), (0x08, false)] {
        let mut machine = machine_128();
        ay_poke(&mut machine, 7, ALL_OFF); // the channel sits at its level: this reads the ramp
        ay_poke(&mut machine, 8, 0x10); // channel A follows the envelope
        ay_poke(&mut machine, 11, ENVELOPE_PERIOD);
        ay_poke(&mut machine, 12, 0x00);
        ay_poke(&mut machine, 13, shape);
        settle(&mut machine);

        let stream = channel_a(&mut machine, 4096);
        let ramp: Vec<u16> = levels(&stream).into_iter().take(LEVELS).collect();
        assert_eq!(ramp.len(), LEVELS, "shape {shape:#04X}: a short ramp");

        let ordered: Vec<u16> = if rising {
            ramp.clone()
        } else {
            ramp.iter().rev().copied().collect()
        };
        assert!(
            ordered.windows(2).all(|pair| pair[0] < pair[1]),
            "shape {shape:#04X}: the ramp must be strictly monotonic, got {ramp:?}"
        );
        assert_eq!(ordered[0], 0, "shape {shape:#04X}: one end is silence");
        assert_eq!(
            ordered[LEVELS - 1],
            AMPLITUDE_MAX,
            "shape {shape:#04X}: the other is full scale"
        );
    }
}

/// Levels the envelope and the amplitude registers both walk.
const LEVELS: usize = 16;

/// The envelope period these tests run at: one level per sixteen samples.
///
/// Slow enough that a level spans several samples and fast enough that whole ramps fit in a
/// run. Not the chip's fastest, which would put one level in one sample and make every
/// assertion about the ramp an assertion about the sample grid as well.
const ENVELOPE_PERIOD: u8 = 16;

#[test]
fn a_repeating_envelope_comes_back_round() {
    // The `CONT` half of the shape decode, at the output. A shape with `CONT` set repeats; the
    // ramp above would look identical for one pass either way, so this is what tells a
    // repeating shape from one that ran once and held.
    let mut machine = machine_128();
    ay_poke(&mut machine, 7, ALL_OFF);
    ay_poke(&mut machine, 8, 0x10);
    ay_poke(&mut machine, 11, ENVELOPE_PERIOD);
    ay_poke(&mut machine, 13, 0x0C); // rising, repeating
    settle(&mut machine);
    let repeating = levels(&channel_a(&mut machine, 8192));
    assert!(
        repeating.len() > LEVELS,
        "a repeating shape must pass full scale more than once, got {} levels",
        repeating.len()
    );

    // And `CONT` clear runs one ramp and holds at silence, which is the same eight-behaviours
    // aliasing `spectrum::ay`'s own tests derive from the four shape bits.
    ay_poke(&mut machine, 13, 0x04); // rising, then off — behaves as shape 15
    settle(&mut machine);
    let once = channel_a(&mut machine, 8192);
    assert_eq!(
        levels(&once).len(),
        LEVELS + 1,
        "one ramp, then the drop to silence it holds at"
    );
    assert_eq!(once.last().copied(), Some(0), "and it holds there");
}

// ---------------------------------------------------------------------------
// The separation `docs/M7.md` Decision 6 requires
// ---------------------------------------------------------------------------

#[test]
fn the_ay_hash_does_not_move_when_the_beeper_does() {
    // **The constraint Decision 6 imposes on this milestone, as a failing case.** If the hash
    // covered "the machine's audio", driving the speaker would change it — and a gate that
    // goes red for a reason unrelated to what it grades is a gate that gets muted.
    //
    // Two runs of the identical AY program, one of them also writing the speaker. The AY
    // fields must be bit-identical and the beeper fields must not be.
    let quiet = fingerprint(0x00);
    let loud = fingerprint(0x10);
    assert_eq!(
        quiet.0, loud.0,
        "the AY's own output must not see the beeper"
    );
    assert_ne!(quiet.1, loud.1, "and the beeper must really have moved");
}

/// Hash the AY's channels and the beeper separately over [`MEASURED_SAMPLES`] samples, with
/// the speaker either flipping on every write or not.
///
/// **The two runs execute the identical program and differ in one byte** — the operand of the
/// `XOR`. That matters more than it looks: an earlier draft used a shorter program for the
/// idle run and padded it with `NOP`s, which made the two runs cover *different* emulated
/// intervals. The AY's output is a function of absolute time, so the channel hashes then
/// differed for a reason that had nothing to do with the beeper, and the test would have
/// reported the separation broken when it was the fixture that was.
///
/// It is also, incidentally, an end-to-end check of the property `spectrum::audio`'s own tests
/// call *"rendering is the same however it is split up"*: the toggling run drives the
/// generator at every speaker write and the idle run does not drive it at all until the take,
/// and the channels must still agree bit for bit.
fn fingerprint(mask: u8) -> (u64, u64) {
    let mut machine = reference_machine();
    let mut ay = FNV_OFFSET;
    let mut speaker = FNV_OFFSET;
    let mut produced = 0;

    // `XOR n : OUT (0xFE),A` — two instructions, eighteen T-states, whatever `n` is.
    let program = [0xEE, mask, 0xD3, 0xFE];
    while produced < MEASURED_SAMPLES {
        run_program(&mut machine, FILLER, &program, 2);
        for sample in machine.take_samples() {
            if produced < MEASURED_SAMPLES {
                hash_channels(&mut ay, sample);
                speaker = fnv(speaker, &sample.beeper.to_le_bytes());
                produced += 1;
            }
        }
    }
    (ay, speaker)
}

// ---------------------------------------------------------------------------
// The frame hash. It proves change.
// ---------------------------------------------------------------------------

/// Samples the hash covers — a little under half a 128 frame, taken as a stated quantity
/// rather than as "one frame", because a 128's frame is not a whole number of samples.
const MEASURED_SAMPLES: usize = 1024;

/// `NOP`s that produce them: four T-states each, thirty-two T-states to a sample.
const MEASURED_NOPS: usize = MEASURED_SAMPLES * SAMPLE_PERIOD_T_STATES as usize / 4;

/// FNV-1a's 64-bit offset basis and prime.
///
/// Written out rather than reached for through `std`. `std`'s default hasher is explicitly not
/// stable across releases, so a hash taken through it would move with the toolchain and the
/// gate would be grading the compiler. `spectrum::audio::Sample` deliberately does not derive
/// `Hash` for the same reason.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv(mut hash: u64, bytes: &[u8]) -> u64 {
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Fold one sample's **AY channels** into `hash`. The beeper is not offered to it.
fn hash_channels(hash: &mut u64, sample: &Sample) {
    for channel in sample.channels {
        *hash = fnv(*hash, &channel.to_le_bytes());
    }
}

/// A 128 with a fixed chord set up and the stream flushed: three tones at different periods,
/// noise on channel C, an envelope driving channel B.
///
/// Deliberately busy. A hash over a chip doing one simple thing would be insensitive to most
/// of the state machine, and a regression hash is worth exactly the surface it touches.
fn reference_machine() -> Spectrum {
    let mut machine = machine_128();
    for (register, value) in [
        (0_u8, 0x55_u8), // A tone, fine
        (1, 0x01),       // A tone, coarse
        (2, 0xA0),       // B tone
        (3, 0x00),
        (4, 0x11), // C tone
        (5, 0x02),
        (6, 0x07),         // noise period
        (7, 0b0011_0110),  // tone on A and B, noise on C
        (8, FULL_VOLUME),  // A at a fixed level
        (9, 0x10),         // B follows the envelope
        (10, FULL_VOLUME), // C at a fixed level
        (11, 0x00),        // envelope period
        (12, 0x01),
        (13, 0x0A), // envelope shape: alternating
        (14, 0x00),
    ] {
        ay_poke(&mut machine, register, value);
    }
    settle(&mut machine);
    machine
}

/// The hash of [`MEASURED_SAMPLES`] samples of the AY's own output from [`reference_machine`].
///
/// # This is the one number in the suite that was recorded rather than derived
///
/// It has to be. A hash has no closed form and no hand-derivation, which is exactly why
/// `docs/MACHINE.md` ranks item 4 as proving *change* and not correctness. Everything the
/// number depends on — the volume table, the three divisors, the tap position, the sample
/// period, the mixer rule, the envelope decode, and the T-state cost of the configuration
/// program that runs before the measurement — is transcribed or derived somewhere else and
/// graded, or not graded, on its own terms. **This value ranks none of them.** It says only
/// that today's machine emits what yesterday's did.
///
/// `the_frame_hash_is_sensitive_to_the_chip_it_covers` is the positive control that keeps it
/// from being a hash of nothing, and it is not optional: a gate whose green cannot be
/// distinguished from a gate that was not looking is the failure `docs/STATUS.md` records
/// three times.
///
/// **If this goes red, do not update it.** Find what moved first. A hash updated to match a
/// changed machine records the change and grades nothing thereafter.
///
/// # It has moved once, and here is what moved
///
/// Recorded first as `0x37bd_433a_f783_aac3`, then changed to the value below when two
/// magnitudes in [`spectrum::ay`] were corrected against sources found after the fact: the
/// envelope divisor, which had read the datasheet's *cycle* frequency as a *step* frequency and
/// was sixteen times too slow, and the noise generator's output bit, which a die-level analysis
/// showed is **inverted** on the silicon where MAME and `ayumi` emit it uninverted.
///
/// **Both were invisible to every other test in this file**, and that is the point of writing
/// the history down rather than replacing the number quietly. The structural gates are all
/// relative — a ramp of sixteen monotonic levels, a wave whose period scales with its register,
/// a mixer that is active low — and every one of them passes at any divisor and either
/// polarity. The hash was the only thing that moved, which is a regression hash doing exactly
/// what `docs/MACHINE.md` says it does and nothing more: it did not say the old value was
/// wrong. **A source said that, and the hash said something had changed.**
const FRAME_HASH: u64 = 0x12d4_8fc1_8535_fb3f;

#[test]
fn the_frame_hash_is_the_recorded_one() {
    let mut machine = reference_machine();
    let mut hash = FNV_OFFSET;
    let mut produced = 0;
    while produced < MEASURED_SAMPLES {
        fill(&mut machine, MEASURED_NOPS.min(4096));
        for sample in machine.take_samples() {
            if produced < MEASURED_SAMPLES {
                hash_channels(&mut hash, sample);
                produced += 1;
            }
        }
    }
    assert_eq!(
        produced, MEASURED_SAMPLES,
        "the hash must cover a stated quantity of samples"
    );
    assert_eq!(
        hash, FRAME_HASH,
        "the AY's output has changed. Find what moved before touching this number: it \
         records that today's machine emits what yesterday's did, and updating it to match a \
         change grades nothing thereafter"
    );
}

#[test]
fn the_frame_hash_is_sensitive_to_the_chip_it_covers() {
    // **The positive control.** Without it, a hash over an all-zero stream would be just as
    // green — and a chip that had stopped emitting anything at all would pass.
    let hash_of = |edit: &dyn Fn(&mut Spectrum)| -> u64 {
        let mut machine = reference_machine();
        edit(&mut machine);
        let _ = machine.take_samples();
        let mut hash = FNV_OFFSET;
        let mut produced = 0;
        while produced < MEASURED_SAMPLES {
            fill(&mut machine, 4096);
            for sample in machine.take_samples() {
                if produced < MEASURED_SAMPLES {
                    hash_channels(&mut hash, sample);
                    produced += 1;
                }
            }
        }
        hash
    };

    let untouched = hash_of(&|_| {});
    // One bit of one register, in each of the four mechanisms the reference exercises.
    for (what, register, value) in [
        ("a tone period", 0_u8, 0x56_u8),
        ("the mixer", 7, 0b0011_0010),
        ("an amplitude", 8, 0x0E),
        ("the envelope shape", 13, 0x0C),
        ("the noise period", 6, 0x08),
    ] {
        let moved = hash_of(&|machine| ay_poke(machine, register, value));
        assert_ne!(moved, untouched, "{what} must move the hash");
    }

    // And silence is not what the reference produces, which is the other way this could pass
    // vacuously.
    let silent = hash_of(&|machine| {
        for register in 8..=10 {
            ay_poke(machine, register, 0);
        }
    });
    assert_ne!(silent, untouched);
}

#[test]
fn two_identical_machines_emit_identical_streams() {
    // Determinism, which every hash above rests on and none of them asserts. A generator whose
    // output depended on when it happened to be asked would make each of them a gate on the
    // consumer's call pattern.
    let stream = |chunk: usize| -> Vec<Sample> {
        let mut machine = reference_machine();
        let mut out = Vec::new();
        while out.len() < MEASURED_SAMPLES {
            fill(&mut machine, chunk);
            out.extend_from_slice(machine.take_samples());
        }
        out.truncate(MEASURED_SAMPLES);
        out
    };
    // Two different call patterns over the same emulated interval: the samples must agree,
    // because when the generator runs is not part of what it produces.
    assert_eq!(stream(4096), stream(8));
}
