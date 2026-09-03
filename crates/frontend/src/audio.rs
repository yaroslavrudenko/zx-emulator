//! The machine's samples, as a device's.
//!
//! # Why this is the frontend's job and not the machine's
//!
//! `crates/spectrum` emits one [`Sample`] every
//! [`SAMPLE_PERIOD_T_STATES`](spectrum::audio::SAMPLE_PERIOD_T_STATES) — a rate defined by the
//! machine's own clock — with the AY's three channels, the beeper and the tape's `EAR` signal
//! **kept apart**. Turning that into what a sound card wants is two steps, and both of them
//! belong here:
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
//! | The tape's `EAR` signal reaches the mix, and is quieter than the beeper at the same level | `a_playing_tape_is_audible_on_its_own`, `the_tape_is_quieter_than_the_beeper_at_the_same_level` | **proven** |
//! | [`Resampler::track`]'s **direction** — a deep queue is given fewer output samples and a shallow one more | `a_deep_queue_is_given_less_and_a_shallow_one_more` | **proven** |
//! | Its **magnitude** never leaves [`MAX_CORRECTION`], so the pitch shift stays under 8.6 cents | `the_correction_stays_within_its_bound` | **proven** |
//! | That the loop **converges** rather than running away, from either side | `the_loop_converges_instead_of_running_away`, closed against a simulated device | **proven** |
//! | [`queue_target`]'s arithmetic — half the ring, in samples | `the_setpoint_is_half_the_device_buffer` | **proven** |
//! | That **half** is the right fraction to steer at | **nothing.** It is a ruling, like [`HEADROOM`]: it has to leave the correction room in both directions and there is no figure to transcribe. See [`queue_target`] | **chosen** |
//! | That the depth the loop reads is **current** | **nothing here.** On the desktop it is measured at insertion and is exact; in a browser it comes back through the worklet's periodic report, so it lags by up to `REPORT_EVERY` render quanta — see [`Resampler::track`]. No test in this crate can reach a browser | **observed** |
//! | **That it sounds right** | **nothing.** There is no oracle for a tune, and this environment has no audio device and no way to capture one. A waveform written to a file and looked at is the strongest evidence available here, and it is *observation of a signal*, not of a sound | **observed** |
//!
//! **The last three rows are the ones to read twice.** Every assertion in this module's gate is
//! about numbers in a buffer. A person hearing the right tune at the right pitch is a separate
//! claim and nothing here establishes it — and the two rows above it say which halves of the new
//! correction loop are graded and which are a bet: the arithmetic is proven, the setpoint is
//! chosen, and the freshness of the signal it acts on is a property of the far side of a
//! `postMessage`.

use spectrum::Sample;
use spectrum::audio::{AMPLITUDE_MAX, SAMPLE_PERIOD_T_STATES};

/// How loud the mix is allowed to get, as a fraction of full scale.
///
/// Five sources can be at maximum together — three AY channels, the beeper and the tape's `EAR`
/// signal — and the mix divides by their weighted total, so no combination clips. This leaves
/// headroom below the device's full scale rather than running right at it.
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

/// How loud the tape's `EAR` signal is against the beeper.
///
/// **Chosen, not derived** — a ruling, and a weaker one than [`BEEPER_GAIN`], which at least has
/// two resistors behind it. There is no equivalent figure to transcribe here: what reaches the
/// amplifier from the `EAR` socket depends on the tape, the deck, and the volume the person
/// playing it set, which is exactly why loading a real Spectrum involved turning a knob until it
/// worked.
///
/// What the number has to satisfy is audibility without dominance, and **the first attempt got it
/// wrong in the quiet direction.** It was `0.5`, on the reasoning that a loading tone is
/// continuous where beeper music is not and would therefore dominate at unity. Measured, that
/// produced a peak of 2.5% of full scale against the beeper's 12.9% — a tape five times quieter
/// than the speaker, when the machine it models is remembered for being unbearably loud while
/// loading. The reasoning was sound and the magnitude it implied was not, which is what measuring
/// a ruling is for.
///
/// **The figures are transcribed here rather than cited, and that is deliberate.** This paragraph
/// first pointed at the `.wav` capture that settled it, under `.agent-workspace/` — which
/// `.gitignore` excludes, so the citation named a file no reader of this repository can open. A
/// ruling has to carry its evidence, and a path outside the tree carries none. The durable half
/// is the arithmetic, and it is reproducible from the constants above: a 0→`AMPLITUDE_MAX` square
/// through the DC blocker peaks at half its amplitude, so a lone tape reaches
/// `TAPE_GAIN / FULL_SCALE × HEADROOM / 2` of full scale and a lone beeper reaches the same with
/// `BEEPER_GAIN`. At `0.5` that was **2.44%** against the beeper's **12.92%**; at `0.9` it is
/// **4.12%** against **12.13%**. Regenerate the recording with
/// `cargo run --bin zx-shot -- --wav <out>.wav` over a loading tape if a fresh one is wanted.
///
/// `2.0` was the second attempt and **a gate refused it**, for a reason worth more than the
/// number: every source divides by the same [`FULL_SCALE`], so loudness given to one is taken
/// from the others. At `2.0` a lone beeper fell to 34.6% of the mix and
/// `five_sources_at_full_scale_do_not_exceed_the_headroom` failed on its own words — *"a 48K
/// would be thin"*. Making the tape audible had quietly made **every 48K game** quieter, which is
/// the larger defect by a wide margin.
///
/// `0.9` is what that constraint allows. `BEEPER_GAIN / FULL_SCALE > 0.4` — the gate's own
/// inequality — solves to `TAPE_GAIN < 0.97`, so this is the ceiling with a margin rather than a
/// preference. It is 1.8x the first attempt: **4.12%** of full scale against the beeper's
/// **12.13%**.
///
/// *That beeper figure said **12.9%**, which was its value at `TAPE_GAIN = 0.5`.* Raising the tape
/// raised [`FULL_SCALE`] from 6.147 to 6.547, so the beeper's share fell with everything else's —
/// which is the paragraph above's own point, arriving one paragraph later as a stale number. The
/// margin the gate leaves is genuinely narrow: `BEEPER_GAIN / FULL_SCALE` is `0.4043` against a
/// floor of `0.4`, a 1.1% margin, and any future change to [`BEEPER_GAIN`], to the AY's channel
/// count or to that floor breaks it. The failure mode is a *quieter 48K*, which no gate other
/// than `five_sources_at_full_scale_do_not_exceed_the_headroom` would notice.
///
/// **The tape is therefore still quieter than the machine it models, and the cause is structural
/// rather than this number.** A shared denominator across sources that are almost never loud
/// together spends headroom on a combination that does not occur: a tape loads *before* a game
/// makes music, not during it. Fixing it properly means normalising per source or bounding the
/// sum some other way, which is a design change and not a constant — recorded in the README's
/// post-release table rather than smuggled in here.
const TAPE_GAIN: f32 = 0.9;

/// The largest total [`mix`] can form, in units of [`AMPLITUDE_MAX`].
///
/// The beeper at its gain, the tape at its own, plus the AY's three channels at unity. Dividing
/// by this is what keeps a 128 playing an AY tune *and* clicking the beeper *and* loading a tape
/// inside the device's range — the case that clips, and the one no test that plays a single
/// source can reach.
const FULL_SCALE: f32 = BEEPER_GAIN + TAPE_GAIN + 3.0;

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

/// The furthest [`Resampler::track`] may move the output rate, as a fraction.
///
/// **Chosen, not derived** — a ruling in `docs/M6.md`'s vocabulary, like [`HEADROOM`]. Two
/// things bound it from opposite sides:
///
/// - It must exceed the drift it exists to absorb. The emulator paces to 50.08 Hz against a
///   device consuming one second per second, and host crystals are specified to ±100 ppm; the
///   worst realistic total is a few tenths of a percent, so 0.5% clears it with room.
/// - It must stay inaudible as pitch. A rate scaled by `r` shifts pitch by `1200·log₂(r)`
///   cents, so 0.5% is **8.6 cents** — under a tenth of a semitone, and reached only at the
///   extremes of the queue. The just-noticeable difference for pitch is around 25 cents for
///   sustained tones and worse for the square waves a Spectrum makes.
///
/// Raising it absorbs more drift and starts to be heard as vibrato; lowering it is quieter and
/// converges more slowly, which shows up as a queue that wanders instead of settling.
///
/// # Both bounds are about magnitude, and there is a third about *rate of change*
///
/// 8.6 cents applied once and held is inaudible. The same 8.6 cents applied and withdrawn
/// repeatedly is **vibrato**, and the ear is most sensitive to modulation in the low tens of
/// hertz — which is exactly the band a controller acting on a stale measurement produces. The
/// loop runs once per emulated frame (20 ms); if the depth it reads refreshes more slowly than
/// that, it applies the same error twice before seeing its own effect, over-corrects, and limit-
/// cycles at the measurement period.
///
/// That is why `web/zx_audio_worklet.js`'s `REPORT_EVERY` is **8** and not 16: at 16 the browser's
/// depth refreshed every ~42.7 ms against a 20 ms control period and a 50 ms setpoint — a feedback
/// period more than twice the control period, and a lag of 85% of the setpoint. At 8 it is
/// ~21.3 ms, just inside one frame. **The magnitude of the correction is gated here; its rate of
/// change is a property of the far side of a `postMessage` and nothing in this crate can see it.**
const MAX_CORRECTION: f64 = 0.005;

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

/// Milliseconds in a second, so [`queue_target`]'s unit conversion is not a bare literal.
const MILLISECONDS_PER_SECOND: u32 = 1000;

/// The queue depth [`Resampler::track`] steers towards, on a device running at `device_hz`.
///
/// **Half of [`page::BUFFER_MILLISECONDS`], and the half is a ruling.** The correction has to have
/// somewhere to go in both directions: a queue that is too shallow is as much a defect as one that
/// is too deep — it underruns, and both devices fill an underrun with silence. Steering at the
/// midpoint gives the loop the same authority either way. A quarter would bias it toward
/// underrunning; three quarters toward latency. Nothing measures which is best, so this is a
/// **choice** in `docs/M6.md`'s vocabulary, like [`HEADROOM`] and [`TAPE_GAIN`].
///
/// # Why it is a named function and not an expression in the frame loop
///
/// It was `resampler.device_hz() * page::BUFFER_MILLISECONDS / 2000`, written inline in `main`.
/// Two things were wrong with that. `2000` is a compound magic number — `1000` for the unit
/// conversion times `2` for the ruling — so the policy was fused into the arithmetic and neither
/// half was visible. And `crates/frontend/src/main.rs`'s own header holds that function *"to
/// plumbing: poll, upload, draw, await"*, because a decision inside the frame loop is one no test
/// can reach; the file already extracts `ink`, `speed_message` and `report_a_finished_tape` for
/// exactly that reason, each recording the lesson at its extraction site. This is the fourth.
/// `the_setpoint_is_half_the_device_buffer` is the test that became possible.
#[must_use]
pub const fn queue_target(device_hz: u32) -> u32 {
    device_hz * page::BUFFER_MILLISECONDS / MILLISECONDS_PER_SECOND / 2
}

/// One [`Sample`]'s five sources as a single level, from `0.0` to [`HEADROOM`].
///
/// *Five, and this line said four.* The three AY channels, the beeper and the tape's `EAR` signal
/// — [`FULL_SCALE`] sums all five and the gate beside it is named
/// `five_sources_at_full_scale_do_not_exceed_the_headroom`, so the count was wrong against two
/// neighbours in the same file. It is worth stating rather than silently correcting because the
/// number is the one a reader checks the arithmetic against.
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
    let total = f32::from(sample.beeper) * BEEPER_GAIN + f32::from(sample.tape) * TAPE_GAIN + ay;
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
    ///
    /// The figure the device actually asked for. [`Resampler::device_hz`] reports this one, and
    /// [`Resampler::track`] leaves it alone — it is the anchor a correction is expressed against.
    device_step: u64,
    /// The rate actually being resampled to, in the same units as [`Resampler::device_step`].
    ///
    /// Equal to `device_step` until [`Resampler::track`] moves it, and never further than
    /// [`MAX_CORRECTION`] away. See that constant for why a rate the device did not ask for is
    /// the right thing to resample to.
    corrected_step: u64,
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
        let device_step = device_hz as u64 * SAMPLE_PERIOD_T_STATES as u64;
        Self {
            cpu_hz: cpu_hz as u64,
            device_step,
            corrected_step: device_step,
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

    /// Nudge the output rate to steer a queue of `queued` samples toward `target`.
    ///
    /// **"Steers toward", not "converges on" — this summary said the second and it overstates
    /// what the loop does.** This is proportional-only: there is no integrator, so against a
    /// *constant* drift `d` the steady state is not `target`, it is where the correction cancels
    /// the drift — `target × (1 + d / MAX_CORRECTION)`. At the 0.24% drift actually observed in a
    /// browser that is `target × 1.48`, so the queue settles at roughly three quarters of the ring
    /// rather than at half of it. Bounded, harmless, and not what the word "converges" promises.
    /// An integral term would remove the offset and is not worth its own failure mode here: a
    /// slow-winding integrator against a clamped actuator is how a loop gets stuck at a rail.
    ///
    /// # Why the rate is corrected rather than a frame being dropped
    ///
    /// The emulator paces itself to the machine's frame rate — 50.08 Hz on a 48K — and the sound
    /// card consumes exactly one second of samples per wall second, off a crystal that agrees
    /// with nothing. The two are open-loop, so their difference **accumulates**: a fifth of a
    /// percent is half a sample per frame and 100 ms of backlog every four minutes. Something
    /// has to close the loop.
    ///
    /// This is the second answer to that. The first was to stop feeding the device once the
    /// backlog passed a ceiling, which discards a whole frame — 20 ms, ~880 samples — in one
    /// step. That is a discontinuity in the waveform, and a discontinuity is a click. It
    /// recurred on a period set by the drift, so what a person heard was a tick every few
    /// seconds through music that was otherwise correct.
    ///
    /// Correcting the *rate* removes the same backlog by resampling a fraction of a percent
    /// faster, spread across every sample instead of concentrated in one edge. Nothing is
    /// discarded, so there is no discontinuity to hear. What it costs is pitch:
    /// [`MAX_CORRECTION`] of 0.5% is **8.6 cents**, well inside the ~25 cents a trained ear
    /// resolves, and it only reaches full deflection when the queue is at the far end of its
    /// range — in steady state the correction sits near zero.
    ///
    /// This is the standard fix for the same problem everywhere it appears; it is what a
    /// DAC-clock resampler in an audio interface does with its own drift.
    ///
    /// # What `queued` has to be, and how fresh it is
    ///
    /// [`None`] is *"no device has reported a depth yet"* and leaves the rate alone. **The
    /// parameter was `i32` and carried `page::audio_push`'s `-1` sentinel straight into a public
    /// signature in a different crate** — the one place this function had a free hand, and it
    /// chose the C-shaped answer. `Option<u32>` says the same thing in the type, so the decoding
    /// of `-1` stays in the one file that owns that ABI.
    ///
    /// On the desktop the depth is measured inside the same lock that inserted the samples, so it
    /// is exact. In a browser it comes back from the worklet's periodic report and is therefore
    /// **stale by up to `REPORT_EVERY` render quanta** — see [`MAX_CORRECTION`] for why that
    /// number is 8 rather than 16, and why the staleness bound, not the correction's magnitude, is
    /// what decides whether this loop wobbles.
    pub fn track(&mut self, queued: Option<u32>, target: u32) {
        let Some(queued) = queued else {
            return;
        };
        // The error as a fraction of the target, clamped to ±1 so a queue at zero and a queue at
        // twice the target reach the same full deflection in opposite directions.
        let error = (f64::from(queued) - f64::from(target)) / f64::from(target.max(1));
        let correction = error.clamp(-1.0, 1.0) * MAX_CORRECTION;
        // **The sign, and it was inverted in the first version of this function.** The reasoning
        // that produced the error is worth keeping because it is plausible: *a deep queue is
        // drained by consuming input faster, so take a larger step.* The premise is false. The
        // input is not consumed at a rate this controls — the machine emits a fixed 2184 samples
        // per frame whatever happens here — and `feed` turns each of them into
        // `corrected_step / cpu_hz` **outputs**. A larger step therefore pushes *more* into a
        // queue that drains at a fixed device rate, and the setpoint becomes a repeller: simulated
        // over twenty minutes of browser drift the shipped sign reached **8.9 seconds** of
        // backlog where the correct one held 74 ms.
        //
        // So: a queue above target is drained by producing **less**, which is a *smaller* step.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the factor is within 0.5% of 1.0, so the product is near `device_step` and \
                      far inside `u64`; `MAX_CORRECTION` is what keeps that true"
        )]
        let corrected = (self.device_step as f64 * (1.0 - correction)) as u64;
        self.corrected_step = corrected.max(1);
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
            self.phase += self.corrected_step;

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
