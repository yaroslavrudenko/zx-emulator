//! The machine's samples, as a device's.
//!
//! # Why this is the frontend's job and not the machine's
//!
//! `crates/spectrum` emits one [`Sample`] every
//! [`SAMPLE_PERIOD_T_STATES`](spectrum::audio::SAMPLE_PERIOD_T_STATES) — a rate defined by the
//! machine's own clock — with the AY's three channels and the beeper **kept apart**. Turning
//! that into what a sound card wants is two steps, and both of them belong here:
//!
//! 1. **The mix.** `docs/M8.md` Decision 9 puts it in this crate, and `docs/M7.md` Decision 6
//!    says why: the AY's frame hash must cover *"the AY's own output, not 'the machine's
//!    audio'"*, so that adding the beeper cannot falsify an M7 gate. A mixer inside
//!    `crates/spectrum` would put the beeper inside the thing the AY's gate hashes.
//! 2. **The resampling.** This one is load-bearing and is the reason the split is not a
//!    preference. If `crates/spectrum` resampled to a host rate, **its output would become a
//!    function of the machine it ran on** — 44,100 here, 48,000 there — and a frame hash that
//!    moves with the hardware is not a gate at all. The machine emits at a rate it defines;
//!    the frontend converts to a rate the device defines. Neither knows the other's.
//!
//! # The rate is not one number, and the 128 is the reason
//!
//! A 48K's Z80 runs at 3,500,000 Hz and a 128's at 3,546,900. Divided by the 32-T-state sample
//! period that is **109,375 Hz** and **110,840.625 Hz** — and the second is not a whole number,
//! so a design that stores "the machine's sample rate" as an integer is already wrong on the
//! machine that has the sound chip.
//!
//! [`Resampler`] therefore never forms that quotient. It keeps the ratio as the exact integer
//! fraction `device_hz × 32 / cpu_hz` and steps a phase accumulator in whole numbers, so there
//! is no rounding to accumulate and no drift over a long run — which is what
//! `a_long_run_does_not_drift` asserts.
//!
//! # What is gated here, and what is not
//!
//! | Property | Covered by | Class |
//! |---|---|---|
//! | Silence in produces silence out | `tests/audio_resampling.rs` | **proven** |
//! | A constant level decays to zero — no DC on the speaker | same, against the settling time | **proven** |
//! | A square wave survives with its amplitude | same, peak-to-peak against a literal | **proven** |
//! | The output rate matches the device's, over a run long enough for a rounding error to show | same, exact counts | **proven** |
//! | Nothing is allocated per frame | [`Resampler::feed`] takes `&mut Vec<f32>` and only pushes into it; the caller keeps the capacity | **proven**, structurally |
//! | **That it sounds right** | **nothing.** There is no oracle for a tune, and this environment has no audio device and no way to capture one. A waveform written to a file and looked at is the strongest evidence available here, and it is *observation of a signal*, not of a sound | **observed** |
//!
//! **The last row is the one to read twice.** Every assertion in this module's gate is about
//! numbers in a buffer. A person hearing the right tune at the right pitch is a separate claim
//! and nothing here establishes it.

use spectrum::Sample;
use spectrum::audio::{AMPLITUDE_MAX, SAMPLE_PERIOD_T_STATES};

/// How loud the mix is allowed to get, as a fraction of full scale.
///
/// Four sources can be at maximum together — three AY channels and the beeper — and the mix
/// divides by their weighted total, so no combination clips. This leaves headroom below the
/// device's full scale rather than running right at it.
///
/// Chosen, not derived. A real Spectrum's loudness depends on its speaker and its volume knob,
/// and there is no figure to transcribe here — so this is a **ruling** in the vocabulary
/// `docs/M6.md` established, and the thing to change if it is too loud or too quiet.
const HEADROOM: f32 = 0.6;

/// The resistor the 128 sums the ULA's beeper through, in ohms.
///
/// # The mix ratio is sourced, and it used to be a guess
///
/// This was a plain sum divided by four, with a paragraph explaining that the code *declined*
/// to claim the beeper and an AY channel were equally loud. Declining was the right posture
/// while there was nothing to transcribe. There is now: on the 128's own board the ULA's beeper
/// and the AY reach the MC1376 through a summing network of **R112 = 6K8** for the beeper and
/// **R132 = 18K** for the AY, from a redrawn 128 schematic.
///
/// In a summing network a source's contribution goes as `1/R`, so the beeper is
/// `18000 / 6800` ≈ **2.65×** the AY — *louder*, which is the opposite of what a naive equal
/// mix produces and the reason a 48K sounded thin. The ratio is computed from the two
/// resistances rather than written as `2.65`, so the arithmetic is visible and a corrected
/// resistor value changes one number.
///
/// **What this is not.** It is a claim about how the 128 sums two electrical signals, not about
/// how loud either *sounds*, and not about a 48K — which has no AY and no such network at all.
/// It is the only sourced figure anyone has on the question, which makes it far better than a
/// choice made by ear, and it is still one board's resistors read off one drawing.
const BEEPER_RESISTOR_OHMS: f32 = 6_800.0;

/// The resistor the 128 sums the AY through, in ohms. See [`BEEPER_RESISTOR_OHMS`].
const AY_RESISTOR_OHMS: f32 = 18_000.0;

/// How much louder the beeper is than the AY, from the two resistances.
const BEEPER_GAIN: f32 = AY_RESISTOR_OHMS / BEEPER_RESISTOR_OHMS;

/// The largest total [`mix`] can form, in units of [`AMPLITUDE_MAX`].
///
/// The beeper at its gain, plus the AY's three channels at unity. Dividing by this is what keeps
/// a 128 playing an AY tune *and* clicking the beeper inside the device's range — the case that
/// clips, and the one no test that plays a single source can reach.
const FULL_SCALE: f32 = BEEPER_GAIN + 3.0;

/// How much of the previous output a one-pole DC blocker carries forward.
///
/// # Why a DC blocker exists at all
///
/// [`Sample::beeper`] is *"the fraction of this sample's window the ULA drove the speaker
/// high"* — a number from 0 to full scale, never negative. Sound is a **deviation**, so a
/// signal that only ever goes up has a constant offset in it, and a constant offset is a
/// speaker cone held out of position: it wastes headroom, and it produces a thump whenever the
/// level changes suddenly, which on a Spectrum is every time a program stops making noise.
///
/// A real Spectrum's output is capacitor-coupled and does this in hardware. This is the same
/// filter in one line: `y = x - x₋₁ + R·y₋₁`.
///
/// `0.999` at ~44 kHz puts the corner near 7 Hz — below anything audible, so the filter removes
/// the offset and leaves the tune alone. `a_constant_level_decays_to_nothing` measures the
/// settling; `a_square_wave_keeps_its_amplitude` measures that it does not eat the signal.
const DC_POLE: f32 = 0.999;

/// T-states per second on a 48K, and on a 128.
///
/// Taken from `spectrum`'s own [`Timing`](spectrum::timing::Timing) rather than written here,
/// so the two cannot disagree. `docs/STATUS.md`'s rule about a figure copied between documents
/// applies to constants copied between crates, and it applies harder: nothing would notice.
#[must_use]
pub const fn cpu_hz(is_128: bool) -> u32 {
    if is_128 {
        spectrum::timing::Timing::SPECTRUM_128.cpu_hz()
    } else {
        spectrum::timing::Timing::SPECTRUM_48K.cpu_hz()
    }
}

/// One [`Sample`]'s four sources as a single level, from `0.0` to [`HEADROOM`].
///
/// A **weighted** sum. `crates/spectrum/src/audio.rs` says of [`AMPLITUDE_MAX`] that *"two
/// sources sharing a full scale is not a claim that they are equally loud"* — a warning that
/// this function is exactly where such a claim gets made. It makes one, and the weight comes
/// from the 128's own summing resistors rather than from taste: see [`BEEPER_RESISTOR_OHMS`].
#[must_use]
pub fn mix(sample: Sample) -> f32 {
    let ay: f32 = sample
        .channels
        .iter()
        .map(|&channel| f32::from(channel))
        .sum();
    let total = f32::from(sample.beeper) * BEEPER_GAIN + ay;
    total / (f32::from(AMPLITUDE_MAX) * FULL_SCALE) * HEADROOM
}

/// Turns the machine's samples into the device's, mixing on the way.
///
/// One per run, reused. Holds the filter's history and the resampling phase, so feeding it two
/// consecutive frames gives the same result as feeding it one buffer of both — which is what
/// `feeding_in_pieces_matches_feeding_in_one_go` asserts, and it is the property a frame loop
/// depends on without ever stating.
#[derive(Debug, Clone)]
pub struct Resampler {
    /// The machine's clock, in T-states per second.
    cpu_hz: u64,
    /// The device's rate, already multiplied by the sample period.
    device_step: u64,
    /// Fractional position between output samples, in T-states.
    phase: u64,
    /// Mixed input accumulated for the output sample being formed.
    sum: f32,
    /// How many input samples that is.
    count: u32,
    /// The DC blocker's previous input.
    last_input: f32,
    /// The DC blocker's previous output.
    last_output: f32,
}

impl Resampler {
    /// A resampler from a machine running at `cpu_hz` to a device running at `device_hz`.
    ///
    /// # The ratio is kept exact on purpose
    ///
    /// The machine's sample rate is `cpu_hz / 32`, which is **109,375** on a 48K and
    /// **110,840.625** on a 128 — not a whole number. So the quotient is never formed. Both
    /// sides are multiplied by the sample period instead, leaving `device_hz × 32` against
    /// `cpu_hz`, and every step is integer addition and subtraction.
    ///
    /// A `f32` ratio would lose about one part in ten million per step, which is inaudible for
    /// a second and is a drift of several samples a minute — the kind of error that is invisible
    /// in a test that runs for a frame and audible in a session that runs for an hour.
    #[must_use]
    pub const fn new(cpu_hz: u32, device_hz: u32) -> Self {
        Self {
            cpu_hz: cpu_hz as u64,
            device_step: device_hz as u64 * SAMPLE_PERIOD_T_STATES as u64,
            phase: 0,
            sum: 0.0,
            count: 0,
            last_input: 0.0,
            last_output: 0.0,
        }
    }

    /// The device rate this was built for.
    #[must_use]
    pub const fn device_hz(&self) -> u32 {
        (self.device_step / SAMPLE_PERIOD_T_STATES as u64) as u32
    }

    /// Mix `samples`, resample them, and append the result to `out`.
    ///
    /// **Appends rather than replaces**, and takes a `&mut Vec<f32>` rather than returning one,
    /// so a frame loop reuses one buffer and this path allocates nothing after the first few
    /// frames have grown it. `main.rs` already refuses a per-frame allocation for the screen
    /// texture — *"a leak-shaped mistake rather than a slow one"* — and the same sentence is
    /// true fifty times a second here.
    ///
    /// Each output sample is the **average** of the input samples covering its window, which is
    /// a box filter: the cheapest thing that is not simply wrong. Nearest-neighbour picking
    /// would alias a 109 kHz signal down to 44 kHz and put audible tones where the machine made
    /// none.
    pub fn feed(&mut self, samples: &[Sample], out: &mut Vec<f32>) {
        for &sample in samples {
            self.sum += mix(sample);
            self.count += 1;
            self.phase += self.device_step;

            while self.phase >= self.cpu_hz {
                self.phase -= self.cpu_hz;
                let averaged = if self.count == 0 {
                    // Reachable when the device rate is higher than the machine's, which no
                    // real device is — but upsampling must not divide by zero, and repeating
                    // the last level is what a box filter does with an empty window.
                    self.last_input
                } else {
                    self.sum / self.count as f32
                };
                out.push(self.block_dc(averaged));
                self.sum = 0.0;
                self.count = 0;
            }
        }
    }

    /// One pole of DC removal. See [`DC_POLE`].
    fn block_dc(&mut self, input: f32) -> f32 {
        let output = input - self.last_input + DC_POLE * self.last_output;
        self.last_input = input;
        self.last_output = output;
        output
    }
}
