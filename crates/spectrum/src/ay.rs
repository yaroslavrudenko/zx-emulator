//! The AY-3-8912: three tone generators, one noise generator, one envelope, and a mixer.
//!
//! # This is a state machine that happens to drive a speaker
//!
//! `docs/M7.md` Decision 6 states the verification problem plainly and it is worth repeating
//! at the top of the file it is about: **a wrong tone is audible to a human and invisible to
//! every test this repository can write.** There is no oracle for sound, there is no round
//! trip, and there is no equivalent of *"a demo tears or it does not"*, because a listener
//! cannot tell a 2 % pitch error from a correct one.
//!
//! The way through is not a better ear. **The dividing line runs between structure and
//! magnitude**, and this module is written so the line is visible in the code:
//!
//! | | Graded here | By what |
//! |---|---|---|
//! | The noise register's **period** | yes | `tests::the_noise_registers_period_is_maximal`, and the tap sweep beside it, which measures the gate's own blind spot |
//! | The envelope's **aliasing** — 16 shape values, 8 behaviours | yes | `tests::the_sixteen_envelope_shapes_are_eight_behaviours`. It is *derived* rather than tabulated: `Envelope` implements the four `CONT`/`ATT`/`ALT`/`HOLD` bits, so the aliasing **emerges** and the test grades the decode |
//! | **Counter periodicity** over all 4096 tone values | yes | `tests::every_tone_period_toggles_at_the_rate_its_register_names` |
//! | **Mixer polarity** — the enable bits are active low | yes | `tests::the_mixer_bits_are_active_low`, and it is a boolean rather than a magnitude |
//! | **Register write masks** | yes, against the transcription | `tests::the_narrow_registers_drop_the_bits_they_do_not_have` |
//! | The **volume table** | **no. Nothing here can.** | `AMPLITUDE` |
//! | The **divisors** — tone, noise, envelope | **no. Nothing here can.** | `TONE_DIVISOR` and its neighbours |
//! | **That it sounds right** | **no** | a human ear, `docs/M7.md`'s T4 |
//!
//! Blurring those two lists is how a gate comes to grade less than it appears to, and a suite
//! of green sound tests reads as *"the sound chip works"* whatever it actually asserted. So
//! every magnitude in this file carries its source in its own doc comment, and every one of
//! them says what nothing here can settle.
//!
//! # Fifteen registers, and the sixteenth that is a hole in a file format
//!
//! The `-8912` is a cut-down `-8910` with **one** 8-bit I/O port rather than two — the 128
//! wires that one to an external keypad almost nobody owned — so `R15` does not exist. The
//! World of Spectrum *128K Technical Information* reference states the range directly:
//! *"OUT (0xfffd) - Select a register 0-14"*.
//!
//! **But `.z80` version 3 reserves sixteen bytes at offsets 39–54 regardless**, so the format
//! has a slot the machine has no register for. `docs/M7.md` names the hazard exactly:
//! whatever the model puts there *"round-trips perfectly and is invisible to every round
//! trip"*. [`Ay::register`] is therefore fallible — it returns `None` for an index the chip
//! does not have — so the mismatch is **handled at the call site rather than hidden inside
//! one**, and `crate::snapshot::z80`'s decision about byte 54 is written where a reader will
//! meet it. See [`ABSENT_REGISTER`].
//!
//! # Where the chip's time comes from
//!
//! Nothing here reads a clock. `Ay::step` advances the chip by one internal step and the
//! caller decides when those happen — [`crate::audio`] owns that, and owns the reason the
//! whole of sound sits **off** `Ula::tick`'s path rather than on it.

use crate::audio::AMPLITUDE_MAX;
use crate::ula::FLOATING_BUS_BYTE;

/// Registers the `-8912` has: `R0`–`R14`.
///
/// Fifteen, not sixteen. See the module documentation.
pub const REGISTER_COUNT: usize = 15;

/// What a read of a register the chip does not have returns.
///
/// **Derived from this machine's own convention rather than transcribed.** The `-8912` does
/// not drive the data bus for a register it does not decode, so what the CPU latches is
/// whatever the bus floats at — and that is [`FLOATING_BUS_BYTE`], which `crate::ula` already
/// fixes at `0xFF` for exactly this reason and documents as *"what the data bus reads as when
/// nothing drives it"*.
///
/// This value covers two mechanisms that are distinct on the hardware and indistinguishable
/// from the CPU: selecting `R15`, which the `-8910` has and the `-8912` does not, and
/// selecting an address of 16 or more, which deselects the chip entirely so that several AYs
/// can share one bus. Both leave nothing driving the bus. They are merged here deliberately
/// and the merge is named so nobody later reads one rule as covering both by accident.
pub const ABSENT_REGISTER: u8 = FLOATING_BUS_BYTE;

/// Channels the chip mixes: A, B and C.
pub const CHANNEL_COUNT: usize = 3;

/// Register numbers, named. The layout is the `-8910`'s and the `-8912` inherits all of it
/// except `R15`.
mod register {
    /// Channel A tone period, low eight bits.
    pub(super) const A_TONE_FINE: usize = 0;
    /// Channel A tone period, high four bits.
    pub(super) const A_TONE_COARSE: usize = 1;
    /// Noise period, five bits.
    pub(super) const NOISE_PERIOD: usize = 6;
    /// Mixer and I/O direction. The six mixer bits are **active low**.
    pub(super) const MIXER: usize = 7;
    /// Channel A amplitude: four bits of level, plus bit 4 selecting the envelope.
    pub(super) const A_AMPLITUDE: usize = 8;
    /// Envelope period, low eight bits.
    pub(super) const ENVELOPE_FINE: usize = 11;
    /// Envelope period, high eight bits.
    pub(super) const ENVELOPE_COARSE: usize = 12;
    /// Envelope shape: `CONT`, `ATT`, `ALT`, `HOLD`.
    pub(super) const ENVELOPE_SHAPE: usize = 13;
}

use register::{
    A_AMPLITUDE, A_TONE_COARSE, A_TONE_FINE, ENVELOPE_COARSE, ENVELOPE_FINE, ENVELOPE_SHAPE, MIXER,
    NOISE_PERIOD,
};

/// How many bits of a write to each register the chip keeps.
///
/// **Transcribed from the General Instrument datasheet's register table.** Grading against
/// this array cannot discover that the transcription is wrong — but a wrong mask is not
/// invisible, because software *reads registers back*, and what comes back is what a guest
/// acts on. That is why the masks are graded here at all when the volume table is not: the
/// mask has an observable consequence inside the machine and the volume table does not.
///
/// The narrow ones, and why each is the width it is:
///
/// | Register | Bits | What it holds |
/// |---|---|---|
/// | 1, 3, 5 | 4 | the top four bits of a 12-bit tone period |
/// | 6 | 5 | the noise period |
/// | 8, 9, 10 | 5 | four bits of level plus the envelope-select bit |
/// | 13 | 4 | `CONT`, `ATT`, `ALT`, `HOLD` |
const WRITE_MASK: [u8; REGISTER_COUNT] = [
    0xFF, 0x0F, // 0, 1   channel A tone period
    0xFF, 0x0F, // 2, 3   channel B
    0xFF, 0x0F, // 4, 5   channel C
    0x1F, // 6      noise period
    0xFF, // 7      mixer and I/O direction
    0x1F, 0x1F, 0x1F, // 8, 9, 10   amplitudes
    0xFF, 0xFF, // 11, 12 envelope period
    0x0F, // 13     envelope shape
    0xFF, // 14     I/O port A
];

/// Bit 4 of an amplitude register: use the envelope rather than the four-bit level.
const AMPLITUDE_FROM_ENVELOPE: u8 = 0x10;

/// Bits 0–3 of an amplitude register: the fixed level.
const AMPLITUDE_LEVEL: u8 = 0x0F;

/// Levels the envelope and the amplitude registers both range over.
const LEVEL_COUNT: u8 = 16;

const _: () = assert!(
    LEVEL_COUNT.is_power_of_two(),
    "Envelope::level masks a position against LEVEL_COUNT - 1 to make its range provable to the \
     compiler, and that mask is only equivalent to the range check while the count is a power of \
     two"
);

/// The chip's sixteen output levels, as amplitudes.
///
/// # Nothing in this repository can grade a single number in this array
///
/// `docs/M7.md` Decision 6 puts it in the ungraded list and this comment does not soften it:
/// *"Nothing this project can build distinguishes a right table from a plausible one."* No
/// test below asserts a value here, and none should — a test that did would be grading the
/// transcription against itself, which `docs/STATUS.md` catalogues as *"a test whose
/// expectation is computed by the subject"*.
///
/// **What is asserted is the table's *structure***, which is a different claim and a real one:
/// it is monotonic, it starts at silence, and it ends at full scale. A transcription error
/// that reordered or truncated it fails those; a transcription error in the fourth digit of
/// one entry does not, and nothing here will ever catch it.
///
/// # Where the numbers come from
///
/// The chip's D-to-A converter is logarithmic in roughly 3 dB steps — the datasheet gives
/// *"16 levels"* on a *"logarithmic"* scale and does not tabulate the voltages, which is why
/// every emulator in existence carries a table somebody measured rather than one somebody
/// read. This one is the widely-used measured AY-3-8910 table, normalised so that level 15 is
/// [`AMPLITUDE_MAX`] and level 0 is silence.
///
/// **The `-8910` and the YM2149 differ** — the Yamaha part has 32 levels and a different
/// curve — and using one for the other is a real and common defect. This is the AY table,
/// which is the part the Sinclair 128 and the +2 (grey) carry. The +2A/+3 are out of scope
/// (`docs/M7.md` Decision 10) and are not claimed by it.
const AMPLITUDE: [u16; LEVEL_COUNT as usize] = [
    0x0000, 0x0201, 0x02FF, 0x0464, 0x0662, 0x0929, 0x0DA9, 0x1421, 0x1B8E, 0x2B23, 0x3B5D, 0x4C77,
    0x656B, 0x83AC, 0xA5D5, 0xFFFF,
];

const _: () = assert!(AMPLITUDE[0] == 0, "level 0 is silence");
const _: () = assert!(
    AMPLITUDE[LEVEL_COUNT as usize - 1] == AMPLITUDE_MAX,
    "level 15 is full scale"
);

/// Master clocks per count of a tone counter.
///
/// **A magnitude, transcribed, and nothing here can adjudicate it.** The datasheet states the
/// resulting frequency rather than the divisor: `f_tone = f_master / (16 x TP)`. A counter
/// that **toggles** its output every `TP` counts produces a full square-wave period of
/// `2 x TP` counts, so `2 x TP x d = 16 x TP` and `d = 8`. That derivation is arithmetic on a
/// transcribed formula, which makes it exactly as good as the formula and no better.
///
/// This is written as a divisor rather than as a frequency because a divisor is what the
/// implementation needs, and converting once here is one place to be wrong instead of three.
const TONE_DIVISOR: u32 = 8;

/// Master clocks per count of the noise counter.
///
/// Transcribed the same way and from the same sentence shape: `f_noise = f_master / (16 x NP)`
/// with **no** toggle — the shift register advances once per expiry — so the divisor is 16.
///
/// **The asymmetry with [`TONE_DIVISOR`] is the whole reason both are named.** They differ by
/// exactly the factor of two that a toggling output introduces, and a model that used one
/// divisor for both would put the noise an octave out while every tone stayed right. That is
/// the shape of error a listener notices and no test here would.
const NOISE_DIVISOR: u32 = 16;

/// Master clocks per count of the envelope counter.
///
/// # This was 256 and it was wrong by a factor of sixteen
///
/// The datasheet's `f_envelope = f_master / (256 x EP)` is the frequency of **one complete
/// envelope cycle** — all sixteen steps of the ramp — not of one step. Reading it as the step
/// rate makes every envelope sixteen times too slow: a note's attack that should take a
/// twentieth of a second takes most of a second, which is audible as an instrument that never
/// arrives rather than as a subtly wrong one.
///
/// Sixteen steps to a cycle, so one step is `256 / 16 = 16` master clocks per count of `EP`.
///
/// **The error is recorded rather than quietly corrected** because of how it survived. Nothing
/// in this repository could catch it: the envelope's *structure* tests are all relative — a
/// ramp of sixteen strictly monotonic levels, a period that scales with the register — and
/// every one of them passes at any divisor. It is a magnitude, and magnitudes here are exactly
/// what nothing grades. It was caught by reading the datasheet's formula again and noticing
/// that the quantity on the left is a cycle and not a step, which is the only instrument this
/// milestone has for a number of this kind.
const ENVELOPE_DIVISOR: u32 = 256 / LEVEL_COUNT as u32;

/// Master clocks in one call to [`Ay::step`].
///
/// The greatest common divisor of the three divisors above, so that every counter advances a
/// whole number of counts per step and no generator needs a fractional accumulator.
pub(crate) const STEP_MASTER_CLOCKS: u32 = TONE_DIVISOR;

const _: () = assert!(NOISE_DIVISOR.is_multiple_of(STEP_MASTER_CLOCKS));
const _: () = assert!(ENVELOPE_DIVISOR.is_multiple_of(STEP_MASTER_CLOCKS));

/// Steps between two counts of the noise counter.
const NOISE_STEPS: u32 = NOISE_DIVISOR / STEP_MASTER_CLOCKS;

/// Steps between two counts of the envelope counter.
const ENVELOPE_STEPS: u32 = ENVELOPE_DIVISOR / STEP_MASTER_CLOCKS;

/// The noise generator's shift register, in bits.
const NOISE_REGISTER_BITS: u32 = 17;

/// The noise generator's second tap. The first is bit 0.
///
/// # Primary, from the silicon — and the widely-quoted corroboration is not corroboration
///
/// The transistor-level analysis of the AY-3-8910 die
/// ([`lvd2/ay-3-8910_reverse_engineered`](https://github.com/lvd2/ay-3-8910_reverse_engineered))
/// carries a 17-bit register with taps at bits 16 and 13 and a left shift, which is this
/// register mirror-numbered. Simulated and diffed against ours: same maximal period, exact
/// bitwise complement at zero phase offset. **That is a die-level source and it settles the
/// structure.**
///
/// **What does *not* settle it, and reads exactly as though it does:** MAME's comment that
/// *"this was verified on AY-3-8910 and YM2149 chips"*. It arrived in MAME 0.72 (August 2003)
/// with **no changelog entry, no named author, and no method or data** — the algorithm itself
/// having changed from `bit0 ^ bit2` to `bit0 ^ bit3` one release earlier as a one-hex-digit
/// edit, also unlogged. Its conclusion is right, and the 2019 die work is what makes it right;
/// the sentence was never evidence. It has since been copied into **414** public repositories
/// verbatim, so a wiki or an emulator carrying it is a restatement of one unattributed comment
/// rather than a second witness — `docs/STATUS.md`'s *"a derived figure repeated across
/// documents acquires authority it never earned"*, at a scale worth naming.
///
/// The gate below is unchanged in strength by any of this, and it is still worth measuring
/// rather than claiming: [`tests::a_period_test_kills_ten_of_the_sixteen_tap_positions`] sweeps
/// every candidate and finds that a period test eliminates ten of sixteen and **cannot
/// distinguish the remaining six from each other**. A source can settle what the gate cannot,
/// and saying which did which is the point.
const NOISE_TAP: u32 = 3;

/// The shift register's power-on value.
///
/// Any non-zero seed gives the same cycle; zero is the one value a Fibonacci LFSR cannot leave.
/// `1` is chosen because it makes the first output bit deterministic and because a model that
/// seeded zero would be **silently** stuck, which is the failure this constant exists to make
/// impossible rather than unlikely.
///
/// # The silicon carries a guard this model cannot need, and that is recorded rather than copied
///
/// The die's feedback term is `(noise_reg[16] ^ noise_reg[13]) | (~|noise_reg)` — the second
/// clause forcing a `1` in whenever the whole register is zero, which is how real hardware
/// escapes an indeterminate power-on state. **This model cannot reach that state**: it seeds
/// non-zero and [`tests::the_shift_register_never_reaches_zero`] walks the entire 131071-step
/// period asserting it. Adding the clause would be code with no reachable failing case, which
/// `docs/STATUS.md` says costs more than deleting it — *"a green that cannot go red is
/// indistinguishable from a green that could"*. So the guard is named here and the test is what
/// discharges it.
const NOISE_SEED: u32 = 1;

const _: () = assert!(NOISE_SEED != 0, "a zero LFSR never leaves zero");
const _: () = assert!(NOISE_TAP < NOISE_REGISTER_BITS);

/// A period register whose value is zero behaves as one.
///
/// The counter compares against the period **after** incrementing, so a period of zero and a
/// period of one both expire on every count; writing it as a `max` rather than relying on that
/// keeps the two readings from drifting when the comparison is edited.
const fn at_least_one(period: u16) -> u32 {
    if period == 0 { 1 } else { period as u32 }
}

/// One tone generator: a counter, a period, and the square wave's current half.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Tone {
    counter: u32,
    /// The half of the square wave the output is in. A toggle is the only thing that moves it.
    high: bool,
}

impl Tone {
    /// Advance one step, toggling if the counter reached `period`.
    #[inline]
    fn step(&mut self, period: u16) {
        self.counter += 1;
        if self.counter >= at_least_one(period) {
            self.counter = 0;
            self.high = !self.high;
        }
    }
}

/// The envelope generator: a position in a sixteen-step ramp, and which way it is going.
///
/// # The four shape bits, and why they are implemented rather than tabulated
///
/// `R13` has sixteen values and eight behaviours: `0..=3` behave as `9` and `4..=7` behave as
/// `15`. **That aliasing is not written down anywhere in this file.** It emerges from the
/// four bits below, because every shape with `CONT` clear ends its first ramp at silence and
/// holds there whatever `ALT` and `HOLD` say — so the twelve remaining bit combinations
/// collapse into the two.
///
/// The alternative was a sixteen-row table of behaviours. It would have been shorter and it
/// would have made [`tests::the_sixteen_envelope_shapes_are_eight_behaviours`] a tautology:
/// a test comparing rows of a table against the table they came from grades nothing, which is
/// `docs/STATUS.md`'s *"a test whose expectation is computed by the subject"*. Implementing
/// the decode makes the same test grade the decode, and the two *targets* — that `0..=3`
/// aliases specifically to `9` and `4..=7` specifically to `15` — remain a transcription,
/// which is what `docs/M7.md` Decision 6 says they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Envelope {
    counter: u32,
    /// Position in the ramp, 0–15.
    position: u8,
    /// Whether the ramp is rising. `ALT` flips it; `ATT` sets its initial value.
    rising: bool,
    /// Whether the ramp has finished and the output is frozen.
    holding: bool,
}

impl Envelope {
    /// Bit 3 of `R13`: continue past the first ramp.
    const CONTINUE: u8 = 0x08;
    /// Bit 2: the first ramp rises.
    const ATTACK: u8 = 0x04;
    /// Bit 1: alternate direction at the end of each ramp.
    const ALTERNATE: u8 = 0x02;
    /// Bit 0: hold after the first ramp.
    const HOLD: u8 = 0x01;

    /// The state a write to `R13` puts the generator in.
    ///
    /// **Any write restarts the envelope**, including a write of the value already there.
    /// That is what music drivers use to retrigger a note, and a model that only restarted on
    /// a *change* would be silently wrong on every driver that does it.
    const fn restart(shape: u8) -> Self {
        Self {
            counter: 0,
            position: 0,
            rising: shape & Self::ATTACK != 0,
            holding: false,
        }
    }

    /// Advance one envelope count.
    fn step(&mut self, shape: u8) {
        if self.holding {
            return;
        }
        self.position += 1;
        if self.position < LEVEL_COUNT {
            return;
        }
        // One ramp is complete. Where it goes next is the whole of the shape decode.
        if shape & Self::CONTINUE == 0 {
            // Every shape with `CONT` clear is one ramp and then silence, whatever else it
            // says — which is exactly why sixteen values are eight behaviours.
            self.holding = true;
            self.rising = false;
            self.position = LEVEL_COUNT - 1;
            return;
        }
        if shape & Self::ALTERNATE != 0 {
            self.rising = !self.rising;
        }
        if shape & Self::HOLD != 0 {
            self.holding = true;
            // Freeze at the far end of the ramp just completed rather than at the start of
            // the one that will not happen.
            self.position = LEVEL_COUNT - 1;
        } else {
            self.position = 0;
        }
    }

    /// The level, 0–15.
    ///
    /// # The mask is a no-op that the compiler cannot derive, and it is worth an instruction
    ///
    /// [`Envelope::step`] keeps `position` inside `0..LEVEL_COUNT` at every exit, but that is a
    /// property of a state machine spread over four branches and LLVM does not prove it. So the
    /// subtraction below carries an **underflow check**, and the `AMPLITUDE[…]` index this feeds
    /// in [`Ay::channel_amplitude`] carries a **bounds check** — while the fixed-amplitude arm
    /// beside it, whose range *is* provable, gets neither. That asymmetry is the evidence.
    ///
    /// Masking states the invariant in a form the compiler can use. It changes no reachable
    /// value — `tests/m7_ay_stream.rs`'s frame hash is the gate on that, and it must not move —
    /// and it removes up to **1.33 M compare-and-branch pairs per second** from a loop that runs
    /// 664,763 times a second, along with three panic landing pads.
    ///
    /// The idiom is not invented here: `crates/spectrum/src/memory.rs` masks a bank index against
    /// `BANK_COUNT` for the same reason and documents it there.
    const fn level(self) -> u8 {
        let position = self.position & (LEVEL_COUNT - 1);
        if self.rising {
            position
        } else {
            LEVEL_COUNT - 1 - position
        }
    }
}

impl Default for Envelope {
    fn default() -> Self {
        Self::restart(0)
    }
}

/// The sound chip a 128 has and a 48K does not.
///
/// Holds the register file, the address latch, and the three generators. It reads no clock:
/// `Ay::step` is called by [`crate::audio`], which owns the decision that sound is generated
/// off the emulator's hot path rather than on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ay {
    registers: [u8; REGISTER_COUNT],
    /// The last value written to the address port, **whatever it was**.
    ///
    /// Stored raw rather than reduced, because `.z80` version 3 carries *"the last OUT to
    /// 0xFFFD"* at offset 38 and a value that had been masked on the way in would round-trip
    /// as a different file. Every use of it goes through [`Ay::selected_register`], which is
    /// the one place the raw byte becomes an index or an absence.
    selected: u8,
    tone: [Tone; CHANNEL_COUNT],
    noise_counter: u32,
    noise_steps: u32,
    noise_shift: u32,
    envelope: Envelope,
    envelope_steps: u32,
}

impl Default for Ay {
    /// The chip at power-on: every register zero, the shift register seeded, the envelope idle.
    ///
    /// **A register file of zeros is not silence and that is not a mistake.** `R7` is the
    /// mixer and its bits are *active low*, so all zeros enables every tone and every noise
    /// source. What makes a fresh chip silent is `R8`–`R10` being zero, which is amplitude
    /// zero. Those are two independent reasons and only one of them is about the mixer —
    /// which is exactly the confusion `tests::the_mixer_bits_are_active_low` exists to keep
    /// out of the model.
    fn default() -> Self {
        Self {
            registers: [0; REGISTER_COUNT],
            selected: 0,
            tone: [Tone::default(); CHANNEL_COUNT],
            noise_counter: 0,
            noise_steps: 0,
            noise_shift: NOISE_SEED,
            envelope: Envelope::default(),
            envelope_steps: 0,
        }
    }
}

impl Ay {
    /// A chip at power-on.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The value of register `index`, or `None` if the chip has no such register.
    ///
    /// **The `None` is the point.** The `-8912` has fifteen registers and `.z80` version 3
    /// reserves sixteen bytes, and `docs/M7.md` Decision 6 says *"the accessor's signature is
    /// where that mismatch is either handled or hidden"*. Returning an `Option` hands the
    /// decision to the caller instead of inventing an answer here, so
    /// `crate::snapshot::z80`'s choice about byte 54 is written at the site that makes it.
    ///
    /// A guest reading the same register through `IN (0xFFFD)` gets [`ABSENT_REGISTER`]
    /// instead, because a floating bus is what the hardware gives it and a CPU has no `None`.
    #[must_use]
    pub fn register(&self, index: u8) -> Option<u8> {
        self.registers.get(usize::from(index)).copied()
    }

    /// The last value written to the address port.
    #[must_use]
    pub fn selected(&self) -> u8 {
        self.selected
    }

    /// The whole register file, `R0`–`R14`.
    ///
    /// For the snapshot writer, which needs all fifteen at once and must not reach for a
    /// sixteenth. Every value here is already masked, which is what makes a snapshot round
    /// trip through [`Ay::restore`] exact rather than approximately exact.
    pub(crate) fn registers(&self) -> &[u8; REGISTER_COUNT] {
        &self.registers
    }

    /// Put the chip into the state a snapshot describes.
    ///
    /// # The generators are reset, and no format carries them
    ///
    /// `.z80` version 3 carries the register file and the address latch. It carries **none**
    /// of what the chip is doing with them: the three tone counters and their phases, the
    /// noise shift register's position, and where the envelope had reached. Nothing else does
    /// either. So a restore starts the generators from power-on, which is a **convention**
    /// chosen for the one property that matters here — it is deterministic, so a restored
    /// machine produces the same samples every time.
    ///
    /// What it costs is audible and small: a note being held across a save resumes from the
    /// start of its envelope and a fraction of a cycle out of phase. `crate::Spectrum::restore`
    /// documents three other conventions of exactly this kind, each for the same reason.
    ///
    /// Writing the registers through [`Ay::set_register`] rather than assigning them is what
    /// makes the round trip exact: masking is idempotent, so a value that came out of a chip
    /// goes back into one unchanged, and a value that came from anywhere else is reduced to
    /// one the chip could hold.
    pub(crate) fn restore(&mut self, selected: u8, registers: &[u8; REGISTER_COUNT]) {
        *self = Self::new();
        self.selected = selected;
        for (index, &value) in registers.iter().enumerate() {
            self.set_register(index, value);
        }
    }

    /// Which register the address latch selects, if it selects one.
    fn selected_register(&self) -> Option<usize> {
        let index = usize::from(self.selected);
        (index < REGISTER_COUNT).then_some(index)
    }

    /// Latch a register address — an `OUT` to `0xFFFD`.
    pub(crate) fn select(&mut self, value: u8) {
        self.selected = value;
    }

    /// Write the selected register — an `OUT` to `0xBFFD`.
    ///
    /// A write to a register the chip does not have is discarded, which is what a chip that
    /// is not listening does.
    pub(crate) fn write(&mut self, value: u8) {
        let Some(index) = self.selected_register() else {
            return;
        };
        self.set_register(index, value);
    }

    /// Read the selected register — an `IN` from `0xFFFD`.
    pub(crate) fn read(&self) -> u8 {
        self.selected_register()
            .map_or(ABSENT_REGISTER, |index| self.registers[index])
    }

    /// Put `value` in register `index`, masked to the width the chip has.
    ///
    /// The one write path: the guest's `OUT` and the snapshot applier both come through here,
    /// so a snapshot cannot install a register value the chip could never hold. That matters
    /// because a `.z80` is guest-supplied input in every sense that counts.
    ///
    /// **An out-of-range `index` is discarded, and in release it is discarded silently.** The
    /// `debug_assert!` is the whole of the noise it makes. That is deliberate and it is the same
    /// ruling `Ay::write` carries for a value a guest can actually produce, but it was stated
    /// only there: no caller can reach this with a bad index — the guest's route masks to four
    /// bits and the applier's array is `REGISTER_COUNT` long — so the `let … else` exists to
    /// keep the function total rather than to handle a case. Panicking instead would put a
    /// guest-reachable abort in a crate built with `panic = "abort"`, which is the trade
    /// `crates/spectrum/src/memory.rs` makes the same way for a bank index.
    pub(crate) fn set_register(&mut self, index: usize, value: u8) {
        debug_assert!(index < REGISTER_COUNT);
        let Some(register) = self.registers.get_mut(index) else {
            return;
        };
        *register = value & WRITE_MASK[index];
        if index == ENVELOPE_SHAPE {
            // Any write restarts the envelope, including a write of the value already there.
            self.envelope = Envelope::restart(*register);
        }
    }

    /// The 12-bit tone period of `channel`.
    fn tone_period(&self, channel: usize) -> u16 {
        let fine = self.registers[A_TONE_FINE + channel * 2];
        let coarse = self.registers[A_TONE_COARSE + channel * 2];
        u16::from(fine) | (u16::from(coarse) << 8)
    }

    /// The 16-bit envelope period.
    fn envelope_period(&self) -> u16 {
        u16::from(self.registers[ENVELOPE_FINE]) | (u16::from(self.registers[ENVELOPE_COARSE]) << 8)
    }

    /// Whether `channel`'s tone is mixed in.
    ///
    /// **Active low**, which is the classic emulator defect and is why it is a named method
    /// rather than an inline `& mask`: bit *set* means the source is *disabled*.
    fn tone_enabled(&self, channel: usize) -> bool {
        self.registers[MIXER] & (1 << channel) == 0
    }

    /// Whether `channel`'s noise is mixed in. Active low, as above; the noise bits are 3–5.
    fn noise_enabled(&self, channel: usize) -> bool {
        self.registers[MIXER] & (1 << (channel + CHANNEL_COUNT)) == 0
    }

    /// The noise generator's current output bit.
    ///
    /// # Inverted, and this is the one place a die-level source overrules the emulators
    ///
    /// The transistor-level analysis of the AY-3-8910 die
    /// ([`lvd2/ay-3-8910_reverse_engineered`](https://github.com/lvd2/ay-3-8910_reverse_engineered),
    /// *"made by reverse-engineering the AY-3-8910 die photos"*) emits `~noise_reg[16]` — the
    /// **complement** of the shift register's output bit. Its formulation shifts left and taps
    /// bits 16 and 13; ours shifts right and taps 0 and 3, which is the same LFSR
    /// mirror-numbered, and the two sequences were simulated and diffed rather than eyeballed:
    /// identical period, and bitwise complements at zero phase offset.
    ///
    /// **MAME and `ayumi` both emit it uninverted**, and this is not cosmetic. The mixer is
    /// `(tone | tone_off) & (noise | noise_off)`, so inverting the noise changes the mixed
    /// output bit for bit on every channel the noise reaches. `jt49` agrees with the silicon.
    ///
    /// So this follows the die and departs from the two most-copied implementations, which is
    /// worth stating plainly: **it is a deliberate disagreement with the majority, on primary
    /// evidence.** If a future source overturns the die analysis, this is the line to move.
    fn noise_high(&self) -> bool {
        self.noise_shift & 1 == 0
    }

    /// `channel`'s level, 0–15: the fixed one, or the envelope's.
    fn level(&self, channel: usize) -> u8 {
        let amplitude = self.registers[A_AMPLITUDE + channel];
        if amplitude & AMPLITUDE_FROM_ENVELOPE == 0 {
            amplitude & AMPLITUDE_LEVEL
        } else {
            self.envelope.level()
        }
    }

    /// Whether `channel`'s mixed output is high.
    ///
    /// The chip ORs each disable bit into its source and ANDs the two together, so a disabled
    /// source contributes a constant `1` rather than a zero. **A channel with both sources
    /// disabled therefore sits at its level and does not move** — which is silence to a
    /// speaker and is not the same statement as "the amplitude is zero". Getting that
    /// backwards produces a chip that is silent when it should sound and vice versa.
    fn mixed_high(&self, channel: usize) -> bool {
        let tone = self.tone[channel].high || !self.tone_enabled(channel);
        let noise = self.noise_high() || !self.noise_enabled(channel);
        tone && noise
    }

    /// `channel`'s output amplitude right now, 0 to [`AMPLITUDE_MAX`].
    #[inline]
    #[must_use]
    pub(crate) fn channel_amplitude(&self, channel: usize) -> u16 {
        if self.mixed_high(channel) {
            AMPLITUDE[usize::from(self.level(channel))]
        } else {
            0
        }
    }

    /// Advance the chip by [`STEP_MASTER_CLOCKS`] of its own clock.
    ///
    /// The tone counters count every step; the noise and the envelope count on their own
    /// divisors. Nothing here reads a clock or a register the caller has not already written.
    pub(crate) fn step(&mut self) {
        for channel in 0..CHANNEL_COUNT {
            self.tone[channel].step(self.tone_period(channel));
        }
        self.step_noise();
        self.step_envelope();
    }

    /// Advance the noise counter, shifting the register when it expires.
    fn step_noise(&mut self) {
        self.noise_steps += 1;
        if self.noise_steps < NOISE_STEPS {
            return;
        }
        self.noise_steps = 0;
        self.noise_counter += 1;
        if self.noise_counter < at_least_one(u16::from(self.registers[NOISE_PERIOD])) {
            return;
        }
        self.noise_counter = 0;
        self.noise_shift = shift_noise(self.noise_shift, NOISE_TAP);
    }

    /// Advance the envelope counter, stepping the ramp when it expires.
    fn step_envelope(&mut self) {
        self.envelope_steps += 1;
        if self.envelope_steps < ENVELOPE_STEPS {
            return;
        }
        self.envelope_steps = 0;
        self.envelope.counter += 1;
        if self.envelope.counter < at_least_one(self.envelope_period()) {
            return;
        }
        self.envelope.counter = 0;
        let shape = self.registers[ENVELOPE_SHAPE];
        self.envelope.step(shape);
    }
}

/// One shift of a [`NOISE_REGISTER_BITS`]-bit Fibonacci LFSR with taps at bit 0 and `tap`.
///
/// Free rather than a method so the tap sweep in the tests can run it over every candidate
/// position without a chip around it — which is what makes that sweep a measurement of the
/// *gate* rather than of this implementation.
fn shift_noise(register: u32, tap: u32) -> u32 {
    let feedback = (register ^ (register >> tap)) & 1;
    (register >> 1) | (feedback << (NOISE_REGISTER_BITS - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run `ay` for `steps` and return channel A's mixed output bit at each one.
    fn channel_a_trace(ay: &mut Ay, steps: usize) -> Vec<bool> {
        (0..steps)
            .map(|_| {
                let high = ay.mixed_high(0);
                ay.step();
                high
            })
            .collect()
    }

    /// A chip with channel A's tone at `period`, at full volume, everything else off.
    fn tone_only(period: u16) -> Ay {
        let mut ay = Ay::new();
        ay.set_register(A_TONE_FINE, (period & 0xFF) as u8);
        ay.set_register(A_TONE_COARSE, (period >> 8) as u8);
        // Noise off on every channel, tone on for A only. Active low.
        ay.set_register(MIXER, 0b0011_1110);
        ay.set_register(A_AMPLITUDE, 0x0F);
        ay
    }

    // ---- structure: the noise generator ----

    #[test]
    fn the_noise_registers_period_is_maximal() {
        // Needs no external source at all: run the register and count until it repeats.
        let mut register = NOISE_SEED;
        let mut period = 0_u32;
        loop {
            register = shift_noise(register, NOISE_TAP);
            period += 1;
            if register == NOISE_SEED {
                break;
            }
            assert!(
                period < 1 << NOISE_REGISTER_BITS,
                "the register never returned"
            );
        }
        assert_eq!(period, (1 << NOISE_REGISTER_BITS) - 1, "2^17 - 1");
    }

    #[test]
    fn a_period_test_kills_ten_of_the_sixteen_tap_positions() {
        // **The measurement of the gate above, rather than a claim about it.** `docs/M7.md`
        // reports this sweep and this test re-takes it rather than quoting it, because
        // *"copying a conclusion is the one operation that cannot detect an error in it"*.
        //
        // Sweep every second tap position and record the period each produces. A green period
        // test eliminates every position that is not maximal — and cannot tell the maximal
        // ones apart, because they all produce the same number.
        let maximal = (1_u32 << NOISE_REGISTER_BITS) - 1;
        let mut maximal_taps = Vec::new();
        let mut other_periods = Vec::new();

        for tap in 1..NOISE_REGISTER_BITS {
            let mut register = NOISE_SEED;
            let mut period = 0_u32;
            loop {
                register = shift_noise(register, tap);
                period += 1;
                if register == NOISE_SEED || period > maximal {
                    break;
                }
            }
            if period == maximal {
                maximal_taps.push(tap);
            } else {
                other_periods.push(period);
            }
        }

        // The blind spot, enumerated. This is what a green period test does *not* establish.
        assert_eq!(
            maximal_taps,
            vec![3, 5, 6, 11, 12, 14],
            "a period test cannot distinguish these six from each other"
        );
        assert_eq!(
            other_periods.len(),
            10,
            "and it does eliminate the other ten"
        );
        assert!(
            maximal_taps.contains(&NOISE_TAP),
            "the shipped tap must be one the gate cannot rule out"
        );

        // Sixteen candidates, six survivors: the gate kills five-eighths of the error space
        // and no more. Saying so is the point; a gate described as stronger than it is, is
        // how a suite comes to grade less than it appears to.
        assert_eq!(maximal_taps.len() + other_periods.len(), 16);
    }

    #[test]
    fn the_shift_register_never_reaches_zero() {
        // The one state a Fibonacci LFSR cannot leave. A model that reached it would go
        // silent forever, and would do it without failing anything.
        let mut register = NOISE_SEED;
        for _ in 0..1 << NOISE_REGISTER_BITS {
            register = shift_noise(register, NOISE_TAP);
            assert_ne!(register, 0);
        }
    }

    #[test]
    fn the_noise_counter_shifts_at_the_rate_its_register_names() {
        // Structure, not magnitude: whatever `NOISE_DIVISOR` is, doubling the period register
        // must exactly double the interval between shifts.
        let interval = |period: u8| -> u32 {
            let mut ay = Ay::new();
            ay.set_register(NOISE_PERIOD, period);
            let start = ay.noise_shift;
            let mut steps = 0;
            while ay.noise_shift == start {
                ay.step();
                steps += 1;
            }
            steps
        };
        let one = interval(1);
        assert_eq!(interval(2), one * 2);
        assert_eq!(interval(4), one * 4);
        assert_eq!(interval(0), one, "a period of zero behaves as one");
    }

    // ---- structure: the tone counters ----

    #[test]
    fn every_tone_period_toggles_at_the_rate_its_register_names() {
        // Exhaustive over all 4096 values a 12-bit tone register can hold. The claim is pure
        // arithmetic over the state machine and needs no source: a period of `n` toggles the
        // output every `n` counts, so a full square wave is `2n`.
        for period in 0..1_u16 << 12 {
            let mut tone = Tone::default();
            let mut toggles = 0;
            let expected = at_least_one(period);
            for _ in 0..expected * 2 {
                let before = tone.high;
                tone.step(period);
                if tone.high != before {
                    toggles += 1;
                }
            }
            assert_eq!(
                toggles, 2,
                "period {period} should toggle twice in 2n counts"
            );
            assert!(!tone.high, "and be back where it started");
        }
    }

    #[test]
    fn a_tone_period_of_zero_is_a_period_of_one() {
        // The two smallest values are indistinguishable on the hardware, and a model that
        // divided by the register would divide by zero.
        let mut zero = Tone::default();
        let mut one = Tone::default();
        for _ in 0..64 {
            zero.step(0);
            one.step(1);
            assert_eq!(zero, one);
        }
    }

    // ---- structure: the mixer ----

    #[test]
    fn the_mixer_bits_are_active_low() {
        // A boolean, not a magnitude. With the tone enabled the channel's output moves; with
        // it disabled the output is constant. Inverting the polarity swaps the two verdicts,
        // which is what makes this a gate rather than a description.
        let mut enabled = tone_only(4);
        let moving = channel_a_trace(&mut enabled, 32);
        assert!(
            moving.iter().any(|&high| high) && moving.iter().any(|&high| !high),
            "an enabled tone must vary"
        );

        let mut disabled = tone_only(4);
        // Every mixer bit set: every source disabled on every channel.
        disabled.set_register(MIXER, 0b0011_1111);
        let still = channel_a_trace(&mut disabled, 32);
        assert!(
            still.iter().all(|&high| high),
            "a disabled source contributes a constant 1, so the output does not move"
        );
    }

    #[test]
    fn a_channel_with_everything_disabled_is_silent_and_it_is_not_the_mixer_that_makes_it_so() {
        // The distinction the model must keep: a disabled channel sits at its *level*, and
        // what makes a fresh chip silent is the amplitude registers being zero. A model that
        // conflated the two would be right about power-on and wrong about every driver that
        // disables a channel while its volume is up.
        let mut ay = Ay::new();
        ay.set_register(MIXER, 0b0011_1111);
        assert_eq!(ay.channel_amplitude(0), 0, "amplitude zero is silence");

        ay.set_register(A_AMPLITUDE, 0x0F);
        assert_eq!(
            ay.channel_amplitude(0),
            AMPLITUDE_MAX,
            "a disabled channel at full volume is a constant, not a zero"
        );
    }

    #[test]
    fn noise_and_tone_are_anded_and_either_can_hold_the_channel_low() {
        // The mix rule itself: both sources high, or a disabled source standing in for one.
        let mut ay = tone_only(1);
        // Tone A and noise A both enabled, everything else off.
        ay.set_register(MIXER, 0b0011_0110);
        let mut seen_low_while_tone_high = false;
        for _ in 0..4096 {
            let tone_high = ay.tone[0].high;
            let mixed = ay.mixed_high(0);
            if tone_high && !mixed {
                seen_low_while_tone_high = true;
            }
            assert_eq!(mixed, tone_high && ay.noise_high());
            ay.step();
        }
        assert!(
            seen_low_while_tone_high,
            "the noise must actually hold the channel low sometimes, or this proves nothing"
        );
    }

    // ---- structure: the envelope ----

    #[test]
    fn the_sixteen_envelope_shapes_are_eight_behaviours() {
        // The aliasing is *derived* here rather than tabulated: `Envelope` implements the four
        // shape bits, so this test grades the decode. What it establishes is the structure —
        // sixteen values, eight distinct sequences, the lower eight collapsing into two blocks
        // of four. Which two upper shapes they collapse *onto* is a transcription, asserted
        // separately below so the two claims are not read as one.
        let trace = |shape: u8| -> Vec<u8> {
            let mut envelope = Envelope::restart(shape);
            (0..64)
                .map(|_| {
                    let level = envelope.level();
                    envelope.step(shape);
                    level
                })
                .collect()
        };

        let traces: Vec<Vec<u8>> = (0..16).map(trace).collect();
        let mut distinct = traces.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), 8, "sixteen values, eight behaviours");

        // Two blocks of four, and each block is internally uniform.
        for shape in 0..4 {
            assert_eq!(traces[shape], traces[0], "shapes 0-3 are one behaviour");
        }
        for shape in 4..8 {
            assert_eq!(traces[shape], traces[4], "shapes 4-7 are one behaviour");
        }
        // And the upper eight are all different from each other, which is what makes the
        // count above 8 rather than 2.
        for a in 8..16 {
            for b in 8..16 {
                assert_eq!(a == b, traces[a] == traces[b], "shapes {a} and {b}");
            }
        }
    }

    #[test]
    fn the_lower_eight_shapes_alias_onto_nine_and_fifteen() {
        // **Transcribed**, and deliberately a separate test from the structural one above.
        // The structure is provable in-repo; these two targets are not, and a single test
        // asserting both would report a transcription as if it were the proof.
        let trace = |shape: u8| -> Vec<u8> {
            let mut envelope = Envelope::restart(shape);
            (0..64)
                .map(|_| {
                    let level = envelope.level();
                    envelope.step(shape);
                    level
                })
                .collect()
        };
        for shape in 0..4 {
            assert_eq!(trace(shape), trace(9), "shape {shape} behaves as 9");
        }
        for shape in 4..8 {
            assert_eq!(trace(shape), trace(15), "shape {shape} behaves as 15");
        }
    }

    #[test]
    fn each_continuing_shape_ends_where_its_bits_say() {
        // The decode, read out one shape at a time. Every row is a consequence of `CONT`,
        // `ATT`, `ALT` and `HOLD` and none of them is a table lookup.
        //
        // | shape | first ramp | after |
        // |---|---|---|
        // | 8  `\\\\`   | falls | repeats |
        // | 9  `\___`   | falls | holds at 0 |
        // | 10 `\/\/`   | falls | alternates |
        // | 11 `\~~~`   | falls | holds at 15 |
        // | 12 `////`   | rises | repeats |
        // | 13 `/~~~`   | rises | holds at 15 |
        // | 14 `/\/\`   | rises | alternates |
        // | 15 `/___`   | rises | holds at 0 |
        let settle = |shape: u8| -> (u8, u8, u8) {
            let mut envelope = Envelope::restart(shape);
            let first = envelope.level();
            for _ in 0..15 {
                envelope.step(shape);
            }
            let end_of_ramp = envelope.level();
            for _ in 0..64 {
                envelope.step(shape);
            }
            (first, end_of_ramp, envelope.level())
        };

        assert_eq!(settle(9), (15, 0, 0), "decay then hold at silence");
        assert_eq!(settle(11), (15, 0, 15), "decay then hold at full");
        assert_eq!(settle(13), (0, 15, 15), "attack then hold at full");
        assert_eq!(settle(15), (0, 15, 0), "attack then hold at silence");
        // The four repeating shapes are back somewhere on the ramp rather than frozen.
        for shape in [8, 10, 12, 14] {
            let mut envelope = Envelope::restart(shape);
            for _ in 0..1024 {
                envelope.step(shape);
            }
            assert!(!envelope.holding, "shape {shape} must not hold");
        }
    }

    #[test]
    fn writing_the_shape_register_restarts_the_envelope_even_with_the_same_value() {
        // What a music driver uses to retrigger a note. A model that only restarted on a
        // change would be silently wrong on every driver that does it.
        let mut ay = Ay::new();
        ay.set_register(ENVELOPE_FINE, 1);
        ay.set_register(ENVELOPE_SHAPE, 0x0A);
        for _ in 0..ENVELOPE_STEPS * 4 {
            ay.step();
        }
        assert_ne!(ay.envelope.position, 0, "the ramp moved");
        ay.set_register(ENVELOPE_SHAPE, 0x0A);
        assert_eq!(ay.envelope, Envelope::restart(0x0A));
    }

    #[test]
    fn the_envelope_period_scales_the_ramp_and_zero_behaves_as_one() {
        let ramp_steps = |period: u16| -> u32 {
            let mut ay = Ay::new();
            ay.set_register(ENVELOPE_FINE, (period & 0xFF) as u8);
            ay.set_register(ENVELOPE_COARSE, (period >> 8) as u8);
            ay.set_register(ENVELOPE_SHAPE, 0x08);
            let start = ay.envelope.position;
            let mut steps = 0;
            while ay.envelope.position == start {
                ay.step();
                steps += 1;
            }
            steps
        };
        let one = ramp_steps(1);
        assert_eq!(ramp_steps(2), one * 2);
        assert_eq!(ramp_steps(0), one, "a period of zero behaves as one");
        assert_eq!(one, ENVELOPE_STEPS, "one count is one envelope divisor");
    }

    #[test]
    fn an_amplitude_register_with_bit_four_set_follows_the_envelope() {
        let mut ay = Ay::new();
        ay.set_register(MIXER, 0b0011_1110);
        ay.set_register(A_AMPLITUDE, AMPLITUDE_FROM_ENVELOPE);
        ay.set_register(ENVELOPE_SHAPE, 0x0C); // rising, repeating
        assert_eq!(ay.level(0), 0, "a rising ramp starts at silence");
        for _ in 0..ENVELOPE_STEPS * 15 {
            ay.step();
        }
        assert_eq!(ay.level(0), 15, "and reaches full scale");

        // And a fixed amplitude ignores it entirely.
        ay.set_register(A_AMPLITUDE, 0x03);
        assert_eq!(ay.level(0), 3);
    }

    // ---- structure: the register file ----

    #[test]
    fn the_narrow_registers_drop_the_bits_they_do_not_have() {
        // Graded against the transcription, and that is what it is worth. What makes it worth
        // having anyway is that software reads registers back, so a wrong mask is a wrong
        // value returned to a guest rather than merely a wrong internal state.
        for (index, &mask) in WRITE_MASK.iter().enumerate() {
            let mut ay = Ay::new();
            ay.set_register(index, 0xFF);
            assert_eq!(ay.register(index as u8), Some(mask), "register {index}");
        }
    }

    #[test]
    fn the_chip_has_fifteen_registers_and_the_sixteenth_is_absent() {
        let mut ay = Ay::new();
        assert_eq!(ay.register(14), Some(0), "R14 is the one I/O port it has");
        assert_eq!(ay.register(15), None, "R15 is port B, which it does not");
        assert_eq!(ay.register(255), None);

        // Through the guest's own route, an absent register is a floating bus.
        ay.select(15);
        ay.write(0xA5);
        assert_eq!(ay.read(), ABSENT_REGISTER);
        assert_eq!(ay.register(15), None, "and nothing was stored anywhere");

        // Selecting an address the chip does not decode at all behaves the same way, which is
        // the merge `ABSENT_REGISTER` documents.
        ay.select(200);
        assert_eq!(ay.read(), ABSENT_REGISTER);
    }

    #[test]
    fn the_address_latch_keeps_the_byte_that_was_written_to_it() {
        // `.z80` version 3 carries *"the last OUT to 0xFFFD"* at offset 38, so a value masked
        // on the way in would round-trip as a different file.
        let mut ay = Ay::new();
        for value in [0_u8, 14, 15, 16, 200, 255] {
            ay.select(value);
            assert_eq!(ay.selected(), value);
        }
    }

    #[test]
    fn a_write_lands_in_the_register_the_latch_names() {
        let mut ay = Ay::new();
        ay.select(7);
        ay.write(0x3F);
        assert_eq!(ay.register(7), Some(0x3F));
        assert_eq!(ay.read(), 0x3F);
    }

    // ---- the volume table: structure only, and the tests say so ----

    #[test]
    fn the_volume_table_is_monotonic_and_spans_the_range() {
        // **The only claim this repository can make about the table.** No test asserts a value
        // in it, because grading a transcription against itself grades nothing. What is
        // asserted is that it is ordered, starts at silence and ends at full scale — which a
        // reordered or truncated transcription fails and a wrong fourth digit does not.
        assert!(AMPLITUDE.is_sorted());
        assert_eq!(AMPLITUDE[0], 0);
        assert_eq!(AMPLITUDE[15], AMPLITUDE_MAX);
        assert_eq!(AMPLITUDE.len(), usize::from(LEVEL_COUNT));
        // Strictly increasing: two equal levels would be two register values a listener
        // cannot tell apart, which the chip's own datasheet does not claim.
        for pair in AMPLITUDE.windows(2) {
            assert!(pair[0] < pair[1]);
        }
    }

    #[test]
    fn a_channels_amplitude_is_its_level_when_high_and_zero_when_low() {
        let mut ay = tone_only(1);
        for level in 0..16_u8 {
            ay.set_register(A_AMPLITUDE, level);
            // Period 1 toggles every step, so one of the two is high.
            let first = ay.channel_amplitude(0);
            ay.step();
            let second = ay.channel_amplitude(0);
            assert_eq!(
                [first.max(second), first.min(second)],
                [AMPLITUDE[usize::from(level)], 0],
                "level {level}"
            );
        }
    }

    // ---- the divisors: structure only ----

    #[test]
    fn the_divisors_are_whole_multiples_of_one_step() {
        // The compile-time assertions above are the real gate; this names them in a failure
        // message. What it does *not* establish is that 8, 16 and 256 are the right numbers —
        // nothing here can, and the constants say so.
        assert_eq!(STEP_MASTER_CLOCKS, TONE_DIVISOR);
        assert_eq!(NOISE_STEPS, NOISE_DIVISOR / STEP_MASTER_CLOCKS);
        assert_eq!(ENVELOPE_STEPS, ENVELOPE_DIVISOR / STEP_MASTER_CLOCKS);
        assert_eq!(NOISE_STEPS, 2);
        assert_eq!(ENVELOPE_STEPS, 2, "16 master clocks per count, 8 to a step");
    }

    #[test]
    fn a_fresh_chip_is_silent_and_deterministic() {
        let mut ay = Ay::new();
        for _ in 0..10_000 {
            for channel in 0..CHANNEL_COUNT {
                assert_eq!(ay.channel_amplitude(channel), 0);
            }
            ay.step();
        }
        assert_eq!(Ay::new(), Ay::default());
    }
}
