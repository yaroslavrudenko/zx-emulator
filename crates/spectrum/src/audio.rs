//! What the machine's sound sources emit, as a stream of samples a consumer can drain.
//!
//! # The sources are kept apart, and that is a requirement rather than a convenience
//!
//! A [`Sample`] carries the AY's three channels, the beeper **and** the tape's `EAR` signal as
//! separate numbers, and nothing in `crates/spectrum` ever adds them together. Two independent
//! rulings say so:
//!
//! - `docs/M7.md` Decision 6 requires the AY's frame hash to cover *"the AY's own output, not
//!   'the machine's audio'"*, precisely so that the beeper landing cannot falsify a gate that
//!   was never wrong. A gate that goes red for a reason unrelated to what it grades is a gate
//!   that gets muted.
//! - `docs/M8.md` Decision 9 puts *"the device, the mix of the two sources, the resampling to
//!   a host rate"* in `crates/frontend`, and adds that *"whatever mixes them must live here,
//!   downstream of both, and must not be pushed into `spectrum` for convenience."*
//!
//! **Summing the AY's three channels would be a mix too**, and it is one this crate therefore
//! does not perform. That is not fastidiousness: the sum is irreversible, and stereo panning —
//! the standard way a 128 emulator presents `ABC` or `ACB` — needs the channels a mixdown
//! would have destroyed.
//!
//! # Sound is almost never on the emulator's hot path, and this is where that is decided
//!
//! **This heading read *"Sound is not on the emulator's hot path"*, and the paragraph below it
//! said M7's sound half *"does not add a branch, a load or a field"* to `Ula::tick` and ran the
//! generator at *"exactly two kinds of moment"*, both guest-initiated. M8's tape half added a
//! third and neither sentence was brought along.** They are corrected rather than replaced,
//! because a reader who took the absolute form and reasoned from it — that anything at all on
//! the per-T-state path is therefore impossible — needs to see that the shape changed rather
//! than only the wording.
//!
//! `Ula::tick` is the hottest function in the emulator. The chip's state at any moment is a pure
//! function of the registers and the time since they were last written, so the generator can be
//! run **late** — and it is, at three kinds of moment:
//!
//! 1. when something the guest does would otherwise be lost: the speaker bit **changing**, or
//!    a write to an AY register;
//! 2. when a consumer asks for the samples;
//! 3. **when the tape flips the `EAR` line** — the machine's own passage of time rather than
//!    anything the guest did, which is what makes it a different *kind* of moment from the first
//!    two and why the count was wrong rather than merely low.
//!
//! Between those it does nothing at all. The total work is proportional to emulated time either
//! way, so nothing is saved by generating eagerly.
//!
//! **What the third moment costs per T-state, measured rather than asserted.** `Ula::advance`
//! runs on every elapsed T-state and asks `crate::tape::Tape::advance` whether the level moved;
//! the answer is a `bool` that function's own loop guard already computed, so on a machine with
//! no cassette the whole per-T-state cost is that one test. The timestamp — a `u64`
//! multiply-add — sits **inside** the branch. An earlier cut passed it as an argument at the
//! call site, where it was evaluated before any guard could discard it, and that cost
//! **+21.9 % on `quiet_48k`**: a machine with no tape paying for one on every T-state.
//!
//! ```text
//! cargo bench -p spectrum --bench frame     # 2026-09-03, lowest median of three runs
//! quiet_48k          143.4 µs               # no tape in the drive
//! drained_48k        150.9 µs               # + one take_samples per frame
//! tape_playing_48k   151.8 µs               # + a pilot tone turning, ~32 EAR edges a frame
//! ```
//!
//! **+0.9 µs for a cassette actually turning**, against a 20,000 µs frame. That is the third
//! moment's whole price, and it is what the run-late design predicts: the same samples are
//! generated either way, merely across 33 calls instead of one. `benches/frame.rs`'s
//! `tape_playing_48k` is the row that says so, and it did not exist when the paragraph above was
//! written — which is why a 23 % regression on this exact path once shipped green.
//!
//! A border write that leaves the speaker bit alone costs one comparison, which is why
//! `Audio::set_beeper` takes the level and compares rather than being called only when the
//! caller thinks it changed — the comparison belongs where the state is. `Audio::set_tape` is
//! deliberately **not** that shape: its own note says why the caller filters instead.
//!
//! # The sample grid
//!
//! One sample every [`SAMPLE_PERIOD_T_STATES`] T-states, which is exactly two of the AY's own
//! internal steps. The rate is therefore the chip's rather than an invented one, and it
//! divides evenly, so no generator ever needs a fractional accumulator:
//!
//! | | T-states per frame | Samples per frame | Rate |
//! |---|---|---|---|
//! | 48K | 69888 | 2184 exactly | 109375 Hz |
//! | 128 | 70908 | 2215 or 2216 | 110840.625 Hz |
//!
//! The 128's frame is not a whole number of sample periods and **the grid does not restart at
//! a frame boundary**, so its per-frame count alternates. A consumer must read the length of
//! what it is handed rather than assuming one; that is true of any audio device anyway, and
//! `docs/M8.md` asks only for *"samples per frame at a fixed rate"*, which this is.
//!
//! The rate is not exposed as a number because it is not an integer on a 128. It is
//! [`crate::timing::Timing::cpu_hz`] divided by [`SAMPLE_PERIOD_T_STATES`], and a consumer
//! resampling to a host rate wants the ratio rather than a rounded frequency.
//!
//! # Every sample is a mean, not a reading
//!
//! A sample is the T-state-weighted mean of its source over the window it covers, not the
//! level at an instant. That costs one multiply-add per state change and it buys the property
//! that matters for a beeper: **a pulse shorter than one sample period is attenuated rather
//! than lost.** Point-sampling would drop it entirely and silently, and 48K beeper music is
//! written as exactly such loops.
//!
//! # What nothing here grades
//!
//! - **Any magnitude.** The volume table, the three divisors, the beeper's level. See
//!   [`crate::ay`], where each carries its source and says so.
//! - **The relative loudness of the beeper against the AY.** [`AMPLITUDE_MAX`] is the full
//!   scale of *each* source separately. That the two share a type does **not** assert they
//!   share a gain — the gain between them is part of the mix, and the mix is the frontend's.
//! - **When in the frame a write lands.** Music drivers write the AY from the interrupt
//!   handler and the audible result depends on it. This module reproduces the timing the
//!   machine produces; nothing checks that timing against hardware.
//! - **Whether it sounds right.** A human ear, and `docs/M7.md`'s T4.

use crate::ay::{self, Ay, CHANNEL_COUNT};
use crate::model::Model;
use crate::timing::Timing;

/// The largest value any field of a [`Sample`] can hold.
///
/// The full scale of **each source separately**. See the module documentation: sources sharing
/// a full scale is not a claim that they are equally loud.
pub const AMPLITUDE_MAX: u16 = u16::MAX;

/// T-states between samples.
///
/// Exactly two of the AY's internal steps, so the chip's own clock decides the rate rather
/// than a number chosen for convenience. The compile-time assertion below is what keeps that
/// true if either constant is ever edited.
pub const SAMPLE_PERIOD_T_STATES: u32 = 32;

/// CPU T-states per AY master clock on a 128.
///
/// **A magnitude, transcribed.** The 128's Z80 runs at 3.546900 MHz and its AY at 1.773400
/// MHz — exactly half — so one AY clock is two T-states. Nothing here grades either figure;
/// what is graded is that the ratio is a whole number, which is what makes an integer sample
/// grid possible at all.
const T_STATES_PER_AY_CLOCK: u32 = 2;

/// T-states in one call to [`Ay::step`].
const AY_STEP_T_STATES: u32 = ay::STEP_MASTER_CLOCKS * T_STATES_PER_AY_CLOCK;

const _: () = assert!(
    SAMPLE_PERIOD_T_STATES.is_multiple_of(AY_STEP_T_STATES),
    "the sample grid must be a refinement of the chip's own step grid, or a sample would \
     cover a fraction of a step and need an accumulator this module does not have"
);

/// Samples the longest frame this crate models can contain.
///
/// An interval of `n` T-states contains at most `n / period + 1` grid points, whatever its
/// alignment.
const SAMPLES_PER_LONGEST_FRAME: usize =
    (Timing::SPECTRUM_128.frame_t_states() / SAMPLE_PERIOD_T_STATES + 1) as usize;

/// Samples the machine buffers before it starts dropping them.
///
/// Two frames of the longest frame there is. **A consumer that drains once per frame never
/// drops a sample**, and one that drains less often is told how many it lost by
/// `Audio::dropped` rather than hearing an unexplained gap. The buffer is allocated once at
/// construction and filled in place, which is what `docs/M8.md` asks for: *"Buffers allocated
/// once, filled in place, handed to the device."*
pub const SAMPLE_CAPACITY: usize = SAMPLES_PER_LONGEST_FRAME * 2;

/// One instant of the machine's sound, with its sources kept apart.
///
/// Deliberately **not** `Hash`. A frame hash over these must name its own algorithm, because
/// `std`'s default hasher is not stable across releases and a gate whose expected value moves
/// with the toolchain grades the toolchain. `crates/spectrum/tests/m7_ay_stream.rs` carries
/// the one this project uses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Sample {
    /// The AY's three channels, A, B and C, each 0 to [`AMPLITUDE_MAX`].
    ///
    /// All zero on a 48K, which has no AY at all.
    pub channels: [u16; CHANNEL_COUNT],
    /// The beeper, 0 to [`AMPLITUDE_MAX`].
    ///
    /// The fraction of this sample's window the ULA drove the speaker high, so a pulse
    /// shorter than the window arrives attenuated rather than missing.
    pub beeper: u16,
    /// The `EAR` input — what the tape is driving the line to — 0 to [`AMPLITUDE_MAX`].
    ///
    /// Averaged over the window exactly like [`Sample::beeper`], and **kept apart from it for
    /// the same reason the AY is**: how loud a tape is against the speaker is a mix decision,
    /// and the mix is the frontend's.
    ///
    /// # Why the machine emits this at all
    ///
    /// On a real Spectrum the `EAR` socket does not only reach the CPU. It reaches the
    /// amplifier, which is why a loading tape screeches out of the television and why a person
    /// can hear the difference between a leader, a data block and a dropout without looking at
    /// the screen. An emulator that routes the tape to bit 6 of a `0xFE` read and nowhere else
    /// loads tapes correctly **in silence**, which is the state this field exists to end.
    ///
    /// **Not necessarily zero when the tape is stopped**, and the first version of this sentence
    /// claimed otherwise. [`crate::tape::Tape::stop`] holds *"the signal where it stands"*, and a
    /// cassette that runs out stops on whichever half-period it ended on — high half the time. So
    /// a stopped tape contributes a constant level, which the frontend's DC blocker removes
    /// rather than this field pretending it is absent.
    ///
    /// The never-played case is gated by `a_tape_that_was_never_started_is_silent` — the test
    /// that carried the broader name until its fixture was read against it — and the
    /// stopped-after-playing case by `a_tape_stopped_on_a_high_half_period_holds_the_line_high`
    /// and `a_cassette_that_runs_out_on_a_high_half_period_holds_the_line_high`, one per route
    /// into that state. The rename is argued in `crates/spectrum/tests/tape_signal.rs`.
    pub tape: u16,
}

/// The machine's sound: the chip a 128 has, the speaker both machines have, and the samples
/// they have produced since a consumer last took them.
///
/// Held by [`crate::Ula`], which owns the clock this needs and is the only thing that calls
/// the methods that move time.
#[derive(Debug)]
pub(crate) struct Audio {
    /// The chip, on a machine that has one.
    ///
    /// `Option` rather than a `Model` field and a branch, because a 48K genuinely does not
    /// have this chip and representing its absence is cheaper *and* more honest than
    /// representing a chip nobody may touch. It is also what makes [`crate::Spectrum::ay`]
    /// able to say so.
    ay: Option<Ay>,
    /// Whether the ULA is driving the speaker high.
    beeper: bool,
    /// Whether the tape is driving the `EAR` line high.
    tape: bool,
    /// T-states the generator has already rendered, since power-on or the last rebase.
    rendered: u64,
    /// T-states left in the sample window being accumulated.
    to_next_sample: u32,
    /// T-states left before the chip's next internal step.
    to_next_ay_step: u32,
    /// Each channel's amplitude integrated over the window so far, in amplitude-T-states.
    channel_accumulator: [u32; CHANNEL_COUNT],
    /// The beeper's, likewise.
    beeper_accumulator: u32,
    /// The `EAR` line's, likewise.
    tape_accumulator: u32,
    samples: Box<[Sample; SAMPLE_CAPACITY]>,
    len: usize,
    dropped: u64,
}

impl Audio {
    /// Silence, on a machine of `model`, at T-state zero.
    pub(crate) fn new(model: Model) -> Self {
        Self {
            ay: model.has_ay().then(Ay::new),
            beeper: false,
            tape: false,
            rendered: 0,
            to_next_sample: SAMPLE_PERIOD_T_STATES,
            to_next_ay_step: AY_STEP_T_STATES,
            channel_accumulator: [0; CHANNEL_COUNT],
            beeper_accumulator: 0,
            tape_accumulator: 0,
            samples: Box::new([Sample::default(); SAMPLE_CAPACITY]),
            len: 0,
            dropped: 0,
        }
    }

    /// The sound chip, on a machine that has one.
    pub(crate) fn ay(&self) -> Option<&Ay> {
        self.ay.as_ref()
    }

    /// The sound chip, mutably.
    pub(crate) fn ay_mut(&mut self) -> Option<&mut Ay> {
        self.ay.as_mut()
    }

    /// Samples lost because nobody drained the buffer in time.
    ///
    /// Zero for a consumer that takes them once per frame. It is reported rather than
    /// swallowed because an unexplained gap in audio is the kind of defect that gets blamed on
    /// everything except the buffer.
    pub(crate) fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Render everything up to `t_state`, then hand over the samples and start again.
    ///
    /// One call rather than a render/read/clear trio: the buffer is reset before the slice is
    /// returned, so taking twice in a row yields the second call nothing, which is what
    /// "take" means everywhere else.
    pub(crate) fn take(&mut self, t_state: u64) -> &[Sample] {
        self.render_to(t_state);
        let taken = self.len;
        self.len = 0;
        &self.samples[..taken]
    }

    /// Set the speaker's level as of `t_state`.
    ///
    /// **Renders first, and only when the level actually moves.** Rendering first is what puts
    /// the edge at the right place in the stream; skipping it when nothing moved is what keeps
    /// a border-only write — which shares this port and is far more frequent — down to one
    /// comparison.
    pub(crate) fn set_beeper(&mut self, high: bool, t_state: u64) {
        if high == self.beeper {
            return;
        }
        self.render_to(t_state);
        self.beeper = high;
    }

    /// Set the level the tape is driving the `EAR` line to, as of `t_state`.
    ///
    /// **Renders first, and only when the level actually moves.** Rendering first is what puts
    /// the edge at the right place in the stream, exactly as in [`Audio::set_beeper`].
    ///
    /// **Where it stops resembling `set_beeper`, and this sentence used to claim it did not.**
    /// `set_beeper`'s guard exists so that its caller need not think: a `0xFE` write is almost
    /// always a border change, and letting it call unconditionally and compare here is both
    /// cheaper and harder to get wrong. That reasoning does not transfer, because this caller is
    /// [`crate::Ula::advance`] — the one place time passes — and it runs 3.5 million times a
    /// second rather than once per `OUT`. At that rate the guard is in the wrong place: `t_state`
    /// is an **argument**, so it is evaluated at the call site before this function can discard
    /// it, and it is a `u64` multiply-add. Measured, that cost +21.9 % on `benches/frame.rs`'s
    /// `quiet_48k` — on a machine with no tape in the drive at all.
    ///
    /// So the caller filters now: `Tape::advance` returns whether the level moved, and this is
    /// reached only when it did. The guard below is kept as a second line rather than deleted,
    /// because it costs one comparison on a path that now runs ~32 times a frame instead of
    /// 69,888, and because it is what keeps the function total for any other caller.
    pub(crate) fn set_tape(&mut self, high: bool, t_state: u64) {
        if high == self.tape {
            return;
        }
        self.render_to(t_state);
        self.tape = high;
    }

    /// Latch a register address — the guest's `OUT` to `0xFFFD`.
    ///
    /// Renders nothing, because the latch changes no output: it decides where the *next*
    /// data write lands and nothing else.
    pub(crate) fn select_ay(&mut self, value: u8) {
        if let Some(ay) = &mut self.ay {
            ay.select(value);
        }
    }

    /// Write the selected register as of `t_state` — the guest's `OUT` to `0xBFFD`.
    ///
    /// **Renders first.** Everything the chip emitted under the old register value belongs to
    /// the samples before this moment, and a model that wrote first would back-date the change
    /// to whenever the generator last ran. On a machine with no chip this does nothing at all,
    /// including no rendering, because nothing observed anything.
    pub(crate) fn write_ay(&mut self, value: u8, t_state: u64) {
        if self.ay.is_none() {
            return;
        }
        self.render_to(t_state);
        if let Some(ay) = &mut self.ay {
            ay.write(value);
        }
    }

    /// Put the chip and the speaker back to power-on, at T-state zero.
    ///
    /// The reset line reaches the sound chip as much as it reaches anything else, so a reset
    /// silences a 128 that was playing. The **buffer is kept**: samples already generated
    /// describe sound the machine really made before the button was pressed.
    pub(crate) fn reset(&mut self) {
        self.ay = self.ay.as_ref().map(|_| Ay::new());
        self.beeper = false;
        // The tape is **not** reset: `crate::Ula::reset` leaves the cassette in the drive and
        // wound where it stood, so the line is still being driven by whatever the head is over.
        self.rebase(0);
    }

    /// Move the time base to `t_state` without rendering anything.
    ///
    /// For the two operations that move the clock without time passing — a reset and a
    /// snapshot restore. `crate::Ula` documents the rule they share: *setting state is not
    /// elapsed time*, so a restore must not manufacture a frame of audio out of the jump.
    ///
    /// The buffer is **not** cleared. Samples already generated describe sound the machine
    /// genuinely made, and discarding a consumer's undrained audio because the machine was
    /// reloaded would lose real work.
    pub(crate) fn rebase(&mut self, t_state: u64) {
        self.rendered = t_state;
        self.to_next_sample = SAMPLE_PERIOD_T_STATES;
        self.to_next_ay_step = AY_STEP_T_STATES;
        self.channel_accumulator = [0; CHANNEL_COUNT];
        self.beeper_accumulator = 0;
        self.tape_accumulator = 0;
    }

    /// Generate every sample whose window closes at or before `t_state`.
    ///
    /// A backwards `t_state` renders nothing and does not move the base: the two callers that
    /// can move the clock backwards go through [`Audio::rebase`] instead, and silently
    /// re-basing here would hide it if one of them ever stopped.
    fn render_to(&mut self, t_state: u64) {
        let Some(mut remaining) = t_state.checked_sub(self.rendered) else {
            return;
        };
        while remaining > 0 {
            let limit = self.to_next_sample.min(self.to_next_ay_step);
            // INVARIANT: `limit` is at most `SAMPLE_PERIOD_T_STATES`, so the minimum with a
            // `u64` fits a `u32` and the cast cannot lose anything.
            let step = u64::from(limit).min(remaining) as u32;

            self.accumulate(step);
            remaining -= u64::from(step);
            self.rendered += u64::from(step);

            self.to_next_sample -= step;
            if self.to_next_sample == 0 {
                self.emit();
                self.to_next_sample = SAMPLE_PERIOD_T_STATES;
            }
            self.to_next_ay_step -= step;
            if self.to_next_ay_step == 0 {
                if let Some(ay) = &mut self.ay {
                    ay.step();
                }
                self.to_next_ay_step = AY_STEP_T_STATES;
            }
        }
    }

    /// Add `t_states` of the current output to the window being accumulated.
    #[inline]
    fn accumulate(&mut self, t_states: u32) {
        if let Some(ay) = &self.ay {
            for (channel, accumulator) in self.channel_accumulator.iter_mut().enumerate() {
                *accumulator += u32::from(ay.channel_amplitude(channel)) * t_states;
            }
        }
        if self.beeper {
            self.beeper_accumulator += u32::from(AMPLITUDE_MAX) * t_states;
        }
        if self.tape {
            self.tape_accumulator += u32::from(AMPLITUDE_MAX) * t_states;
        }
    }

    /// Close the current window into a sample, or count it as dropped.
    fn emit(&mut self) {
        let mean = |accumulated: u32| -> u16 {
            // INVARIANT: the accumulator holds at most `AMPLITUDE_MAX` per T-state of one
            // window, so the mean is at most `AMPLITUDE_MAX` and the cast is lossless.
            (accumulated / SAMPLE_PERIOD_T_STATES) as u16
        };
        let sample = Sample {
            channels: self.channel_accumulator.map(mean),
            beeper: mean(self.beeper_accumulator),
            tape: mean(self.tape_accumulator),
        };
        self.channel_accumulator = [0; CHANNEL_COUNT];
        self.beeper_accumulator = 0;
        self.tape_accumulator = 0;

        match self.samples.get_mut(self.len) {
            Some(slot) => {
                *slot = sample;
                self.len += 1;
            }
            // The buffer is full and the sample is lost. The chip's *state* is not: it has
            // already been advanced, so what a late consumer hears is a gap and not a
            // different machine.
            None => self.dropped += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 128's audio, which is the model that has the chip as well as the speaker.
    fn audio() -> Audio {
        Audio::new(Model::Spectrum128)
    }

    #[test]
    fn a_48k_has_no_sound_chip_and_a_128_does() {
        assert!(Audio::new(Model::Spectrum48K).ay().is_none());
        assert!(audio().ay().is_some());
    }

    #[test]
    fn the_sample_grid_is_the_period_and_nothing_is_emitted_early() {
        let mut audio = audio();
        assert!(audio.take(SAMPLE_PERIOD_T_STATES as u64 - 1).is_empty());
        // Rendering resumes where it stopped rather than restarting, so the first sample
        // still closes at the period.
        assert_eq!(audio.take(SAMPLE_PERIOD_T_STATES as u64).len(), 1);
        assert_eq!(audio.take(SAMPLE_PERIOD_T_STATES as u64 * 10).len(), 9);
    }

    #[test]
    fn taking_twice_yields_the_second_call_nothing() {
        let mut audio = audio();
        assert!(!audio.take(SAMPLE_PERIOD_T_STATES as u64 * 4).is_empty());
        assert!(audio.take(SAMPLE_PERIOD_T_STATES as u64 * 4).is_empty());
    }

    #[test]
    fn a_frame_of_either_machine_fits_in_half_the_buffer() {
        // The property `SAMPLE_CAPACITY` is sized for: a consumer draining once per frame
        // never drops. Taken over both machines and at every alignment of the grid against
        // the frame, because the 128's frame is not a whole number of periods.
        for timing in [Timing::SPECTRUM_48K, Timing::SPECTRUM_128] {
            let frame = u64::from(timing.frame_t_states());
            for offset in 0..u64::from(SAMPLE_PERIOD_T_STATES) {
                let mut audio = audio();
                audio.render_to(offset);
                audio.len = 0;
                let produced = audio.take(offset + frame).len();
                assert!(
                    produced <= SAMPLES_PER_LONGEST_FRAME,
                    "{produced} samples in one frame of {} T-states",
                    timing.frame_t_states()
                );
            }
        }
    }

    #[test]
    fn a_48ks_frame_is_a_whole_number_of_samples_and_a_128s_is_not() {
        // Recorded because a consumer must read the length it is handed. A 48K's frame
        // divides exactly and a 128's does not, so a consumer that hard-coded either count
        // would be right on one machine and wrong on the other.
        assert_eq!(
            Timing::SPECTRUM_48K.frame_t_states() % SAMPLE_PERIOD_T_STATES,
            0
        );
        assert_eq!(
            Timing::SPECTRUM_48K.frame_t_states() / SAMPLE_PERIOD_T_STATES,
            2184
        );
        assert_ne!(
            Timing::SPECTRUM_128.frame_t_states() % SAMPLE_PERIOD_T_STATES,
            0
        );
    }

    #[test]
    fn the_beeper_sample_is_the_share_of_the_window_the_speaker_was_high() {
        // The property point-sampling would not have: a pulse shorter than a window arrives
        // attenuated rather than missing.
        let mut audio = audio();
        audio.set_beeper(true, 0);
        audio.set_beeper(false, u64::from(SAMPLE_PERIOD_T_STATES) / 4);
        let samples = audio.take(u64::from(SAMPLE_PERIOD_T_STATES));
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].beeper, AMPLITUDE_MAX / 4);
    }

    #[test]
    fn a_beeper_pulse_shorter_than_a_sample_survives_as_an_attenuated_one() {
        // One T-state high in a 32 T-state window. Point-sampling loses this entirely,
        // whatever instant it picks, unless it happens to pick the one T-state.
        let mut audio = audio();
        audio.set_beeper(true, 0);
        audio.set_beeper(false, 1);
        let samples = audio.take(u64::from(SAMPLE_PERIOD_T_STATES));
        assert_eq!(
            samples[0].beeper,
            AMPLITUDE_MAX / SAMPLE_PERIOD_T_STATES as u16
        );
        assert_ne!(samples[0].beeper, 0, "and it is not silence");
    }

    #[test]
    fn a_speaker_held_high_reads_full_scale_and_held_low_reads_silence() {
        let mut audio = audio();
        audio.set_beeper(true, 0);
        for sample in audio.take(u64::from(SAMPLE_PERIOD_T_STATES) * 4) {
            assert_eq!(sample.beeper, AMPLITUDE_MAX);
        }
        audio.set_beeper(false, u64::from(SAMPLE_PERIOD_T_STATES) * 4);
        for sample in audio.take(u64::from(SAMPLE_PERIOD_T_STATES) * 8) {
            assert_eq!(sample.beeper, 0);
        }
    }

    #[test]
    fn setting_the_speaker_to_the_level_it_already_holds_renders_nothing() {
        // The comparison that keeps a border-only write off the generator. Measured through
        // the one thing that can see it: the generator's own position.
        let mut audio = audio();
        audio.set_beeper(false, 10_000);
        assert_eq!(audio.rendered, 0, "an unchanged level must not render");
        audio.set_beeper(true, 10_000);
        assert_eq!(audio.rendered, 10_000, "a changed one must");
    }

    #[test]
    fn samples_beyond_the_buffer_are_counted_rather_than_silently_lost() {
        let mut audio = audio();
        let full = u64::from(SAMPLE_PERIOD_T_STATES) * SAMPLE_CAPACITY as u64;
        audio.render_to(full);
        assert_eq!(audio.len, SAMPLE_CAPACITY);
        assert_eq!(audio.dropped(), 0);

        audio.render_to(full + u64::from(SAMPLE_PERIOD_T_STATES) * 5);
        assert_eq!(audio.dropped(), 5, "five windows closed with nowhere to go");
        assert_eq!(audio.len, SAMPLE_CAPACITY, "and the buffer did not grow");
    }

    #[test]
    fn an_overrun_loses_samples_and_not_the_chips_state() {
        // What makes dropping tolerable: a late consumer hears a gap rather than a different
        // machine. The chip is advanced whether or not there is room for its output.
        let mut ahead = audio();
        let mut drained = audio();
        let full = u64::from(SAMPLE_PERIOD_T_STATES) * (SAMPLE_CAPACITY as u64 + 100);

        ahead.render_to(full);
        let mut position = 0;
        while position < full {
            position += u64::from(SAMPLE_PERIOD_T_STATES) * 16;
            let _ = drained.take(position.min(full));
        }
        assert!(ahead.dropped() > 0 && drained.dropped() == 0);
        assert_eq!(ahead.ay, drained.ay, "the chip ran the same either way");
    }

    #[test]
    fn rendering_is_the_same_however_it_is_split_up() {
        // The property that makes the whole late-generation design defensible: when the
        // generator runs cannot change what it produces. If it could, a frame hash would be
        // grading the consumer's call pattern.
        let target = u64::from(SAMPLE_PERIOD_T_STATES) * 200;
        let mut whole = audio();
        whole.set_beeper(true, 0);
        let in_one_go: Vec<Sample> = whole.take(target).to_vec();

        let mut split = audio();
        split.set_beeper(true, 0);
        let mut piecewise = Vec::new();
        for chunk in 1..=200_u64 {
            piecewise.extend_from_slice(split.take(chunk * 7));
        }
        piecewise.extend_from_slice(split.take(target));
        assert_eq!(in_one_go, piecewise);
    }

    #[test]
    fn a_backwards_time_renders_nothing_and_does_not_move_the_base() {
        let mut audio = audio();
        audio.render_to(10_000);
        audio.render_to(5_000);
        assert_eq!(audio.rendered, 10_000);
    }

    #[test]
    fn a_rebase_moves_the_clock_without_manufacturing_audio() {
        // What a reset and a snapshot restore need: the clock jumps and no time passed.
        let mut audio = audio();
        audio.set_beeper(true, 0);
        audio.render_to(u64::from(SAMPLE_PERIOD_T_STATES) * 3);
        let before = audio.len;

        audio.rebase(1_000_000);
        assert_eq!(audio.len, before, "a rebase generates nothing");
        assert_eq!(audio.rendered, 1_000_000);
        // And the samples already produced survive, because they describe sound the machine
        // really made.
        assert_eq!(audio.take(1_000_000).len(), before);
    }

    #[test]
    fn a_48k_emits_silence_on_every_ay_channel() {
        let mut audio = Audio::new(Model::Spectrum48K);
        audio.set_beeper(true, 0);
        for sample in audio.take(u64::from(SAMPLE_PERIOD_T_STATES) * 64) {
            assert_eq!(sample.channels, [0; CHANNEL_COUNT], "a 48K has no AY");
            assert_eq!(sample.beeper, AMPLITUDE_MAX, "and does have a speaker");
        }
    }

    #[test]
    fn the_chip_reaches_the_samples() {
        // The wiring, as a failing case rather than a description: a tone at full volume must
        // make one channel move and leave the other two alone.
        let mut audio = audio();
        let ay = audio.ay_mut().expect("a 128 has a chip");
        ay.select(0);
        ay.write(0x10); // channel A tone period, low byte
        ay.select(7);
        ay.write(0b0011_1110); // tone A only. Active low.
        ay.select(8);
        ay.write(0x0F); // channel A at full volume

        let samples = audio.take(u64::from(SAMPLE_PERIOD_T_STATES) * 256).to_vec();
        assert!(samples.iter().any(|s| s.channels[0] > 0));
        assert!(samples.iter().any(|s| s.channels[0] == 0));
        assert!(
            samples
                .iter()
                .all(|s| s.channels[1] == 0 && s.channels[2] == 0)
        );
    }
}
