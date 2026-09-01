//! The frame clock, and the ULA's contention pattern as a function of position in it.
//!
//! # The machine owns the clock
//!
//! `MACHINE.md` Decision 1, which is a measurement rather than a preference: contention
//! adds T-states on the *machine's* side, and at M1 a contended bus was observed leaving
//! `Cpu::step`'s return identical to a flat run while the bus's own clock diverged. So
//! [`Clock`] is advanced by [`crate::Ula`] — once per `Bus::tick`, plus once per stall —
//! and the frame boundary is a property of this counter. Nothing in this crate adds up
//! what `step()` returns.
//!
//! # The pattern
//!
//! The ULA draws 192 lines of 32 characters. During the 128 T-states in which it is
//! fetching a line it needs the shared bus, and a CPU that wants the same bank waits. The
//! delay depends only on how far into an eight-T-state group the access falls:
//!
//! | `t` within the group | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
//! |---|---|---|---|---|---|---|---|---|
//! | T-states stalled | 6 | 5 | 4 | 3 | 2 | 1 | 0 | 0 |
//!
//! The table is small; the *phase* is the work, and the phase is one number per machine —
//! [`Timing::first_contended_t_state`].
//!
//! # Two machines, one type, and the 48K's constants do not move
//!
//! M7 makes the frame's geometry a value: [`Timing`], with [`Timing::SPECTRUM_48K`] and
//! [`Timing::SPECTRUM_128`]. **The existing constants are redefined as projections of the 48K
//! value rather than replaced**, so this module removes nothing and every gate that positions
//! itself with `FIRST_CONTENDED_T_STATE + phase` keeps compiling and keeps meaning what it
//! meant. That is not politeness: those constants are the positioning device for most of
//! `crates/spectrum/tests/`, each of those gates is *correct for a 48K* and says so in its own
//! header, and rewriting them to say a 48K fact in a more general vocabulary would be churn
//! with a defect budget and no gate to show for it.
//!
//! Because they are projections, **SSOT is enforced by the compiler**: there is no second
//! transcription of 224, 312, 69888, 32 or 14335 anywhere for the two to drift apart.
//!
//! # What is verified, and the two 128 numbers are not equally supported
//!
//! This section said, until the oracle landed:
//!
//! > [`FIRST_CONTENDED_T_STATE`] is the value the emulator community reports for an issue 3
//! > 48K, and **this crate has no oracle for it.** An issue 2 machine is one T-state earlier.
//! > Off-by-one here does not fail anything; it makes multicolour effects land one character
//! > cell out. It is a single named constant precisely so that a future timing-test program
//! > — `MACHINE.md`'s verification item 2, the only real oracle available for this — has one
//! > place to correct.
//!
//! **Two of its clauses are now wrong and the third is now satisfied.** It is kept above
//! rather than overwritten, because this project corrects loudly and because a reader who
//! acted on *"an issue 2 machine is one T-state earlier"* needs to find out.
//!
//! **The oracle exists — for the 48K.** `crates/spectrum/tests/timing_oracle.rs` grades the
//! machine against T-state counts measured on real Spectrums — `MACHINE.md`'s verification
//! item 2, which the paragraph above correctly named as the only thing that would settle this.
//! Of the values near this constant, **only 14335 is green**; 14333, 14334, 14336, 14337 and
//! 14361 all turn it red.
//!
//! **What that does and does not establish, stated precisely.** Two mutations came back
//! *green* — moving the interrupt and the window *together* — so the oracle grades the
//! **interval from `/INT` to the first contended T-state**, not this constant in isolation. The
//! constant is anchored; the frame's origin remains a convention.
//!
//! **The third green mutation was *shortening the window*, and that reading has since been
//! overturned.** It was taken as "the window's length is ungraded". It is not: a single
//! shortening lands inside a band the suite is genuinely insensitive to, and sweeping the whole
//! range 1–65 pins it to **`17..=32`** with sharp, explained edges at both ends. See
//! [`Timing::SPECTRUM_48K`]. That is a live example of this project's own recorded failure —
//! *"reporting the absence of a distinguishing test as evidence of correctness"* — in the
//! narrower form where **one** sample of a parameter was read as a verdict about the parameter.
//!
//! **The early/late difference is not a property of the board**, which is what the old
//! sentence got wrong. The suite's own authors record a *cold* machine reporting late and then
//! reporting early once warm, and board issues 3B, 4B and 6A appear in **both** classes in
//! their hardware results. So "issue 2 versus issue 3" is the wrong axis: the one T-state is
//! not a stable identity of a machine, and which of the two behaviours to reproduce may not be
//! a question with a single hardware answer. This emulator reproduces the majority class, and
//! `timing_oracle.rs` says which as a fact rather than as an intention.
//!
//! ## The 128's numbers: one is solid, one rests on a single ancestor
//!
//! **None of the 48K's hardware grading transfers.** The 128 figures here are *transcribed*,
//! and they are not transcribed from equally good sources:
//!
//! | Figure | Standing |
//! |---|---|
//! | **70908** T-states per frame | **Three independent lineages agree** — the World of Spectrum 128K reference, `MACHINE.md`'s table, and the `.z80` format description's *"17726 in 128K modes"*, whose `(17726 + 1) x 4` is 70908 exactly. `228 x 311` closes it. Corroborated a fourth time from inside `timing_tests-128k_v1.0.z80`: its detection group reads 121 where the 48K's `Late` table reads 122, and `(70908 - 69888) / 4 = 255`, which is `-1 (mod 128)` on a seven-bit `R`. That check is periodic in 512 T-states, so it separates 70908 from 69888 and not from 70396 or 71420 |
//! | **14361**, the first contended T-state | **One documentary ancestor, and better corroborated than that sounds.** The World of Spectrum 128K reference states it — *"the 6,5,4,3,2,1,0,0 pattern starts at 14361 T states after the interrupt"* — and the Sinclair Wiki repeats it citing nothing, so the *documents* are one witness. But **six independent implementations embed `14362`** as the top-left pixel and derive 14361 from it (Fuse/libspectrum, JSpeccy, rustzx, jsspeccy2, kosarev/zx, ESPectrum), the FAQ is internally self-consistent about it (its `OUT` window of 14365–14368 is +3, and 14365 − 3 = 14362), and **MAME's source carries a comment that a hardware timing-test program requires exactly this offset** — `m_base_offset = -1; // leave it one for now, but according to Timings_Test it must be -3`, and `63 x 228 - 3 = 14361`. Still **no primary measurement was found**, and it remains at a lower tier than 14335, which has a hardware oracle. *(This row read "a single ancestor seen twice" before the survey; that was true of the documents and understated the implementations.)* |
//! | **32**, the interrupt window | **A hold, not a belief, and it is the number that actually decides the 128 suite's run.** The suite's detection row is a function of the window and **not** of the contention offset; over 70908 T-states, 32 predicts a reading of 1 and `33..=43` predicts the 121 the file carries. The band is *derived* and deliberately not adopted — see [`Timing::SPECTRUM_128`]. The lesson worth carrying past this table: **the number this milestone spent its care on is not the one the first 128 measurement will be sensitive to** |
//! | **70908**, the frame | **Four independent lineages**, and the strongest figure here. The World of Spectrum reference; the `.z80` format description's *"17726 in 128K modes"*, whose `(17726 + 1) x 4` is exact; **MAME**, which derives it from separate border and retrace constants rather than carrying it (`228` and `311` reached independently); and the 128 timing suite's own detection row. `228 x 311` closes it, and the Sinclair *Servicing Manual*'s `3.54689 MHz` Z80 clock is the divisor it implies. *(One contradiction, isolated and harmless: the FAQ says the interrupt "occurs at 50.01 Hz"; 3546900 / 70908 is **50.021 Hz**. Its own 48K figure checks out, so this is an arithmetic slip in the ancestor rather than a different model.)* |
//!
//! The derivation does not transfer either: the 48K's is `64 x 224 - 1`, the 128's would be
//! `63 x 228 - 3` — a different offset from the line boundary and a different pre-display line
//! count. **A derivation whose *shape* changes between two machines is a fit, not a
//! derivation**, so it is written here as a transcription and not dressed up as arithmetic.
//! The phases differ too: `14335 mod 8 = 7` against `14361 mod 8 = 1`.
//!
//! One genuinely good property of the source, worth noticing because it is not automatic: it
//! states the figure **relative to the interrupt**, which is exactly and only the frame the 48K
//! oracle established as meaningful. So it arrives in the right coordinate system and needs no
//! reinterpretation — and reinterpretation is where a silent error would live.

use crate::screen::DISPLAY_HEIGHT;

/// The stall, in T-states, for each position within an eight-T-state ULA group.
///
/// **Shared by both machines, deliberately, and this is a judgement call worth stating.** Both
/// sources for the 128 give the identical `6,5,4,3,2,1,0,0`, differing only in the offset and
/// the line period — and a claim about *sameness* is much harder to get wrong by transcription
/// than a five-digit constant. So the pattern is not a [`Timing`] field: a field that is
/// provably the same on both models is not "everything that differs between a 48K and a 128",
/// which is what that type is for, and it would put eight more bytes in a value the hot path
/// copies.
///
/// The machine that *does* use a different pattern — the +2A/+3, with `7,6,5,4,3,2,1,0` over
/// pages 4-7 — is out of scope (`M7.md` Decision 10) and would need a mechanism change to
/// [`crate::Ula::tick`] rather than a data change, so promoting this to a field would not buy
/// its support either. If that day comes, moving it is a two-line change.
const DELAY_PATTERN: [u32; 8] = [6, 5, 4, 3, 2, 1, 0, 0];

const _: () = assert!(DELAY_PATTERN.len().is_power_of_two());

/// T-states per line during which the ULA is fetching, and therefore contending.
///
/// 128 of a 48K's 224 and of a 128's 228: 256 pixels at two pixels per T-state. The remainder
/// is the two borders and the horizontal flyback, and it is the remainder that differs between
/// the machines rather than this. Shared for the same reason [`DELAY_PATTERN`] is.
const CONTENDED_T_STATES_PER_LINE: u32 = 128;

/// A machine's frame geometry: everything about time that differs between a 48K and a 128.
///
/// # Why two of the fields are derived and stored anyway
///
/// `frame_t_states` is `t_states_per_line * lines_per_frame` and `contended_span` is
/// `DISPLAY_HEIGHT * t_states_per_line`. Both are written out in the constants below **and**
/// checked against their definitions by `Timing::is_consistent`, which is asserted at compile
/// time for both machines. Two representations that cannot disagree, because the build fails if
/// they do — the instrument `crate::PAGE_SIZE_U16` and this module's own power-of-two
/// assertions already use.
///
/// The alternative was a `const fn` constructor taking four `u32`s positionally, which is a
/// worse trade: it swaps a compiler-checked redundancy for an argument order that nothing
/// checks at all, in a type whose four numbers are interchangeable at the call site. And
/// computing them on demand instead would put a multiply inside `Clock::advance`, which runs
/// once per T-state — roughly 70,000 times a frame — for a value that never changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timing {
    t_states_per_line: u32,
    lines_per_frame: u32,
    frame_t_states: u32,
    first_contended_t_state: u32,
    contended_span: u32,
    interrupt_t_states: u32,
    cpu_hz: u32,
}

impl Timing {
    /// The 48K's geometry. **Hardware-graded**, by `crates/spectrum/tests/timing_oracle.rs`.
    ///
    /// Precisely what is graded: the first contended T-state falls exactly 14335 T-states after
    /// `/INT`, given that this machine asserts `/INT` at frame T-state 0. The frame's origin is
    /// still a convention and `224 x 312` is measured only as its product.
    ///
    /// **`interrupt_t_states` is no longer ungraded, and this comment said it was.** Sweeping
    /// the window over 1–65 against the oracle pins it to **`17..=32`**:
    ///
    /// | Window | What the suite does |
    /// |---|---|
    /// | below 17 | the contended rows disagree |
    /// | **17..=32** | green — the band this constant sits at the top of |
    /// | 33 | the **class flips**: the detection row reports 122 and 65 of 68 rows follow it onto the `TYPE2` table, **with contention untouched** |
    /// | from 44 | the suite stops terminating — its other handler reaches its own first sample at 43 and nests too |
    ///
    /// The 43/44 edge and the floor were both derived before the sweep and both measured
    /// exactly there. **One band, 14–16, was not predicted and is not explained**; it is
    /// recorded as unexplained rather than smoothed into the floor.
    ///
    /// So a value inside `17..=32` is measured and 32 is not distinguished from 17 by anything
    /// here. What *is* newly established is the far more useful negative: **33 is refuted**, and
    /// it is refuted by the row that decides the whole suite's class.
    pub const SPECTRUM_48K: Self = Self {
        t_states_per_line: 224,
        lines_per_frame: 312,
        frame_t_states: 69888,
        first_contended_t_state: 14335,
        contended_span: 192 * 224,
        interrupt_t_states: 32,
        cpu_hz: 3_500_000,
    };

    /// The 128's geometry. **Transcribed, and its two numbers are not equally supported** — see
    /// the module documentation's table. 70908 has three agreeing lineages plus an arithmetic
    /// corroboration from inside the 128 timing suite; **14361 has one ancestor seen twice and
    /// no primary source**.
    ///
    /// # `interrupt_t_states` is the highest-leverage number here, and it is **32 as a
    /// deliberate hold rather than as a belief**
    ///
    /// This field's comment used to read *"inherited from the 48K and measures nothing"*. The
    /// first half is still true and the second is now **false**: it is the single field that
    /// decides the 128 timing suite's entire run, and `first_contended_t_state` — the number
    /// this milestone spent its care on — turns out **not to be what that row is sensitive to
    /// at all.**
    ///
    /// The suite's detection group counts four-T-state `JP (HL)` iterations fitting between the
    /// start of the measurement and the next `/INT`, in **uncontended** memory. Every term comes
    /// from its disassembly:
    ///
    /// ```text
    ///   recorded R = (11 + N) mod 128,   N = (frame_t_states - entry) / 4
    ///   entry 292 = one interrupt taken      entry 324 = a second, nested one
    /// ```
    ///
    /// The 32-T-state gap between the two entries is one extra interrupt: the suite's handler
    /// opens `INC C` / `EI` / `RET C`, `EI` defers by one instruction, so the earliest the CPU
    /// can see `/INT` again is `19 + 4 + 4 + 5 = 32` T-states — **exactly the far edge of a
    /// 32-T-state window.** Whether that tie resolves as "still asserted" is what separates the
    /// suite's two 48K classes, and it is a property of the **window**, not of contention.
    ///
    /// Over this machine's 70908 T-states the same closed form gives:
    ///
    /// | Window | Detection row it predicts | Against the file's **121** |
    /// |---|---|---|
    /// | **32**, this value | **1** | **red, and red on every row** |
    /// | any of `33..=43` | **121** | matches |
    ///
    /// **33..=43 is deliberately not adopted.** It is *derived* — the handler and setup bytes
    /// are diff-identical between the 48K and 128 files, and there is no 128 here to run it on
    /// — so adopting it would promote a prediction to a fact, at the same evidence tier as
    /// 14361 and above what anything has measured. It is a **falsifiable prediction to test the
    /// first time the 128 suite is run**, and it is the first constant to move when that run
    /// goes red — before anyone touches `first_contended_t_state`, which that row cannot see.
    ///
    /// `the_128s_detection_row_is_predicted_by_the_interrupt_window` holds the arithmetic.
    ///
    /// ## The independent evidence disagrees with itself, which is why 32 stays
    ///
    /// A survey of what other implementations do splits **three to two, against** the prediction:
    ///
    /// | Value | Who |
    /// |---|---|
    /// | **32** | ZEsarUX, rustzx, MAME |
    /// | **36** | Fuse/libspectrum, JSpeccy |
    ///
    /// **No document of any kind publishes a 128 figure** — not the World of Spectrum reference,
    /// not the Sinclair Wiki, not the *Servicing Manual*. Even the 48K's 32 is hearsay at its
    /// best source, Chris Smith's zxdesign.info: *"It is documented somewhere that the ZX
    /// Spectrum holds the interrupt active for 32 T-states."*
    ///
    /// Two things cut against 36 and one cuts for it. Against: libspectrum's changelog
    /// attributes the field to the **same author** as the World of Spectrum FAQ, and JSpeccy
    /// agrees with Fuse on both the 128 and the +2A/+3, so that is one lineage with a follower
    /// rather than two witnesses — and **ZEsarUX comments that 36 is the *Pentagon's* value**
    /// (`//en spectrum, 32. en pentagon, 36`), which libspectrum also assigns to Pentagon and
    /// Scorpion. For: the SpecNext wiki records that *"the 128K machines have an RC capacitor on
    /// the pcb on the /int line from the ULA"*, which is a mechanism by which the pulse would be
    /// stretched — but it yields no number.
    ///
    /// **So two independent routes point in opposite directions**: the closed form says
    /// `33..=43`, the implementation census says 32 by majority. That disagreement is the
    /// finding, and 32 is what is shipped because it is the one value that is not a promotion —
    /// it is the 48K's, inside the 48K's measured band, inherited and labelled as such.
    pub const SPECTRUM_128: Self = Self {
        t_states_per_line: 228,
        lines_per_frame: 311,
        frame_t_states: 70908,
        first_contended_t_state: 14361,
        contended_span: 192 * 228,
        interrupt_t_states: 32,
        cpu_hz: 3_546_900,
    };

    /// Whether the derived fields agree with what they are derived from.
    ///
    /// Asserted at compile time for both constants below, which is what makes storing them
    /// safe. A `Timing` cannot exist outside this module — the fields are private and there is
    /// no constructor — so the two associated constants are the whole population.
    const fn is_consistent(self) -> bool {
        self.frame_t_states == self.t_states_per_line * self.lines_per_frame
            && self.contended_span == DISPLAY_HEIGHT as u32 * self.t_states_per_line
            && self.first_contended_t_state < self.frame_t_states
            && self.interrupt_t_states < self.first_contended_t_state
            && CONTENDED_T_STATES_PER_LINE < self.t_states_per_line
            // The clock and the frame length are transcribed from different sources, so this
            // is a genuine cross-check rather than a restatement: a transposed digit in
            // either puts the implied frame rate nowhere near 50 Hz. It does *not* establish
            // either figure — the band it admits is roughly +/-1 %.
            && self.cpu_hz / self.frame_t_states == 50
    }

    /// T-states in one display line, border and flyback included.
    #[must_use]
    pub const fn t_states_per_line(self) -> u32 {
        self.t_states_per_line
    }

    /// Display lines in one frame, both borders and the vertical flyback included.
    #[must_use]
    pub const fn lines_per_frame(self) -> u32 {
        self.lines_per_frame
    }

    /// T-states in one 50 Hz frame.
    #[must_use]
    pub const fn frame_t_states(self) -> u32 {
        self.frame_t_states
    }

    /// The first T-state of the frame at which a contended access is delayed.
    #[must_use]
    pub const fn first_contended_t_state(self) -> u32 {
        self.first_contended_t_state
    }

    /// How long the ULA holds `/INT` low at the start of each frame.
    ///
    /// The interrupt is not an instant. A CPU with interrupts disabled for longer than this
    /// misses the frame entirely, which is a real effect and the reason this is a window
    /// rather than a single moment. **Ungraded on both machines** — see [`Timing::SPECTRUM_128`].
    #[must_use]
    pub const fn interrupt_t_states(self) -> u32 {
        self.interrupt_t_states
    }

    /// The Z80's clock, in Hz.
    ///
    /// # Why a frequency is here at all, when nothing else in this crate needs one
    ///
    /// Everything about time in this emulator is counted in T-states, and until M7's sound
    /// half nothing needed to know how long a T-state *is*. [`crate::audio`] does, and only
    /// at one remove: a consumer resampling the sample stream to a host rate needs the
    /// stream's rate, which is this divided by [`crate::audio::SAMPLE_PERIOD_T_STATES`]. It
    /// is exposed rather than the rate itself because the rate is not an integer on a 128
    /// (110840.625 Hz) and a rounded frequency is worse than a ratio.
    ///
    /// # Both figures are transcriptions, and the better-sourced one is the 128's
    ///
    /// | | Hz | Standing |
    /// |---|---|---|
    /// | 48K | 3500000 | The machine's own documentation and every source since. As uncontroversial as a figure in this project gets, and still a transcription |
    /// | 128 | 3546900 | **Primary.** The Sinclair *Servicing Manual* states a `3.54689 MHz` Z80 clock, which `timing.rs` already cited as *"the divisor"* behind 70908 before anything needed the number itself. It descends from the PAL colour subcarrier: `17.734475 MHz / 5 = 3546895`, five Hz below the quoted figure and immaterial at any resolution this emulator has |
    ///
    /// `Timing::is_consistent` checks each against its own frame length — the implied frame
    /// rate must be 50 Hz — which is a real cross-check between two independently transcribed
    /// numbers and catches a transposed digit. It does **not** establish either figure: the
    /// band it admits is about a percent wide either way.
    #[must_use]
    pub const fn cpu_hz(self) -> u32 {
        self.cpu_hz
    }

    /// The stall, in T-states, a contended access starting at `frame_t_state` suffers.
    ///
    /// Zero outside the display's fetch window — during the borders, the flyback, and the
    /// whole of the top and bottom border areas — which is the majority of a frame.
    #[inline]
    #[must_use]
    pub const fn delay(&self, frame_t_state: u32) -> u32 {
        if frame_t_state < self.first_contended_t_state {
            return 0;
        }
        let since_first = frame_t_state - self.first_contended_t_state;
        if since_first >= self.contended_span {
            return 0;
        }
        let column = since_first % self.t_states_per_line;
        if column >= CONTENDED_T_STATES_PER_LINE {
            return 0;
        }
        // INVARIANT: masked by the pattern's length, which is asserted to be a power of two —
        // so this indexes in range and the bounds check is elided.
        DELAY_PATTERN[(column & (DELAY_PATTERN.len() as u32 - 1)) as usize]
    }
}

const _: () = assert!(Timing::SPECTRUM_48K.is_consistent());
const _: () = assert!(Timing::SPECTRUM_128.is_consistent());

impl Default for Timing {
    /// A 48K, matching [`Clock::new`].
    ///
    /// Written out rather than derived, and the reason is a live hazard: a derived `Default`
    /// would zero every field, and a `Clock` whose `frame_t_states` is zero makes
    /// `Clock::advance`'s rollover loop never terminate. `Clock` derives `Default`, so that
    /// value would be reachable.
    fn default() -> Self {
        Self::SPECTRUM_48K
    }
}

/// T-states in one display line of a 48K, border and flyback included.
pub const T_STATES_PER_LINE: u32 = Timing::SPECTRUM_48K.t_states_per_line();

/// Display lines in one frame of a 48K, top and bottom border and vertical flyback included.
pub const LINES_PER_FRAME: u32 = Timing::SPECTRUM_48K.lines_per_frame();

/// T-states in one 50 Hz frame of a 48K.
///
/// A 128 runs 70908, and that number is [`Timing::SPECTRUM_128`]'s rather than a second
/// constant here — which is what the sentence this replaces was asking for when it said *"a
/// constant to be moved rather than a literal sprinkled through the frame loop."* It was moved:
/// the frame loop reads it off the machine's own [`Clock`], and this constant is the 48K's
/// projection, kept because it positions most of `crates/spectrum/tests/`.
pub const T_STATES_PER_FRAME: u32 = Timing::SPECTRUM_48K.frame_t_states();

const _: () = assert!(T_STATES_PER_FRAME == 69888);
const _: () = assert!(Timing::SPECTRUM_128.frame_t_states() == 70908);

/// How long the ULA holds `/INT` low at the start of each frame, on a 48K.
pub const INTERRUPT_T_STATES: u32 = Timing::SPECTRUM_48K.interrupt_t_states();

/// The first T-state of a 48K's frame at which a contended access is delayed.
///
/// See the module documentation. Hardware-graded as an interval from `/INT`; the 128's
/// equivalent is [`Timing::SPECTRUM_128`]'s and is transcribed rather than graded.
pub const FIRST_CONTENDED_T_STATE: u32 = Timing::SPECTRUM_48K.first_contended_t_state();

/// The stall a contended access on a **48K** starting at `frame_t_state` suffers, in T-states.
///
/// Kept with its original signature — this is the 48K's projection of [`Timing::delay`], and a
/// machine reads the delay off its own [`Clock`] rather than through here.
#[inline]
#[must_use]
pub const fn delay(frame_t_state: u32) -> u32 {
    Timing::SPECTRUM_48K.delay(frame_t_state)
}

/// Where the machine is in the current frame, and how many frames have completed.
///
/// Frame-relative rather than absolute because everything that consults the clock —
/// contention, the interrupt window — is a function of position within a frame. The frame
/// count is what a caller uses to notice a frame boundary, and it is what makes
/// `MACHINE.md` Decision 2 a non-event: a single step that overruns the budget simply
/// lands in the next frame and increments this, rather than needing to stop on 69888.
///
/// It carries its machine's [`Timing`] by value, which is what makes the frame length and the
/// contention phase properties of the machine rather than of the crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Clock {
    timing: Timing,
    frame_t_state: u32,
    frames: u64,
}

impl Clock {
    /// A **48K** clock at the start of frame zero.
    ///
    /// Signature and meaning both unchanged: this already meant a 48K, because a 48K was the
    /// only machine there was.
    #[must_use]
    pub const fn new() -> Self {
        Self::with_timing(Timing::SPECTRUM_48K)
    }

    /// A clock at the start of frame zero, running `timing`.
    ///
    /// `pub(crate)` because the only thing that may decide a machine's geometry is the machine:
    /// [`crate::Ula::new`] takes it from the [`crate::Memory`] it is handed, so a 128's memory
    /// cannot end up behind a 48K's clock.
    #[must_use]
    pub(crate) const fn with_timing(timing: Timing) -> Self {
        Self {
            timing,
            frame_t_state: 0,
            frames: 0,
        }
    }

    /// This clock's machine geometry.
    #[must_use]
    pub const fn timing(&self) -> Timing {
        self.timing
    }

    /// Advance by `t_states`, rolling over into the next frame as many times as needed.
    ///
    /// A loop rather than a single subtraction: nothing here bounds a caller's step to one
    /// frame, and a clock that silently ran a frame behind would be invisible.
    ///
    /// `pub(crate)` because it was a footgun as public API. [`Clock`] is `Copy` and the only
    /// way out of the machine is [`crate::Ula::clock`], which returns it **by value** — so
    /// `machine.ula().clock().advance(1000)` auto-refs the temporary, compiles clean under
    /// `deny(warnings)`, and does nothing at all. Every real caller is in this crate.
    #[inline]
    pub(crate) fn advance(&mut self, t_states: u32) {
        self.frame_t_state += t_states;
        while self.frame_t_state >= self.timing.frame_t_states {
            self.frame_t_state -= self.timing.frame_t_states;
            self.frames += 1;
        }
    }

    /// Put the clock at `frame_t_state` **without** disturbing the frame counter.
    ///
    /// The one operation a snapshot load needs and [`Clock::advance`] cannot express: a
    /// restore moves the machine to a position in the frame, and it is not elapsed time. The
    /// frame counter is the machine's uptime since power-on — the boot gate asserts on it and
    /// the FLASH phase derives from it — so rewinding it on a load would make one number mean
    /// two things. `docs/M6.md` Decision 2 records that as a convention rather than a
    /// measurement, because no snapshot format carries a frame count.
    ///
    /// Values at or above this machine's frame length are reduced into range rather than
    /// rejected, which is the same rollover [`Clock::advance`] documents — so a position derived
    /// from a hostile file cannot leave the clock somewhere that means nothing.
    ///
    /// `pub(crate)` for exactly the reason `advance` is, and the footgun is the same one:
    /// [`Clock`] is `Copy` and [`crate::Ula::clock`] returns it **by value**, so
    /// `machine.ula().clock().set_frame_t_state(0)` would auto-ref a temporary, compile clean
    /// under `deny(warnings)`, and do nothing at all. The only caller is
    /// [`crate::Ula::set_frame_t_state`], which owns the field.
    #[inline]
    pub(crate) fn set_frame_t_state(&mut self, frame_t_state: u32) {
        self.frame_t_state = frame_t_state % self.timing.frame_t_states;
    }

    /// T-states elapsed since the start of the current frame.
    #[inline]
    #[must_use]
    pub const fn frame_t_state(&self) -> u32 {
        self.frame_t_state
    }

    /// Frames completed since the clock started.
    #[inline]
    #[must_use]
    pub const fn frames(&self) -> u64 {
        self.frames
    }

    /// T-states elapsed since the clock started, across every frame.
    ///
    /// The frame-relative pair above is what contention and the interrupt window are
    /// functions of, and this is what anything measuring an *interval* needs — an interval
    /// spanning a frame boundary looks negative in frame-relative coordinates.
    /// `crates/spectrum/tests/tape_rom_timings.rs` needed exactly this and open-coded it;
    /// [`crate::audio`] needs it too, and one expression that knows the machine's own frame
    /// length is better than two that assume a 48K's.
    ///
    /// # It is not monotonic, and the two places it moves backwards are named
    ///
    /// [`Clock::set_frame_t_state`] and a fresh clock after [`crate::Ula::reset`] both move
    /// this backwards, because neither is elapsed time. Anything integrating over it must
    /// handle that explicitly rather than assume it away; [`crate::audio::Audio::rebase`] is
    /// what does, and it exists because a restore that manufactured a frame of audio out of
    /// the jump would be the same defect as a restore that charges a machine cycle.
    #[inline]
    #[must_use]
    pub const fn t_states(&self) -> u64 {
        self.frames * self.timing.frame_t_states as u64 + self.frame_t_state as u64
    }

    /// The frame-relative position `offset` T-states from now.
    ///
    /// Used to price a stall that will happen partway through a machine cycle without
    /// actually moving the clock there first.
    ///
    /// The early return is not premature optimisation and it is worth saying why, because it
    /// is the one place M7 could have quietly made the hot path worse. The reduction used to
    /// be `% T_STATES_PER_FRAME` against a compile-time constant, which LLVM lowers to a
    /// multiply and a shift; against a **field** it lowers to a real division, and `port_delay`
    /// calls this up to four times per I/O cycle. Since `frame_t_state` is already inside the
    /// frame and every caller's `offset` is a machine cycle's worth of T-states, the sum is
    /// almost never past the boundary — so the branch is predictable and the division only
    /// happens on the frames' seam. The result is identical to `%` for every input.
    #[inline]
    #[must_use]
    pub const fn ahead(&self, offset: u32) -> u32 {
        let position = self.frame_t_state + offset;
        if position < self.timing.frame_t_states {
            return position;
        }
        position % self.timing.frame_t_states
    }

    /// The stall a contended access starting **now** would suffer.
    ///
    /// Separate from [`Clock::delay_after`] rather than `delay_after(0)`, and the difference is
    /// the reduction: the clock already stands inside its frame, so pricing the current
    /// position needs no wrap check at all. This is the hottest route to [`Timing::delay`] —
    /// every contended memory access on the machine — and it is the one that must stay free of
    /// arithmetic it does not need.
    #[inline]
    #[must_use]
    pub(crate) const fn delay_now(&self) -> u32 {
        self.timing.delay(self.frame_t_state)
    }

    /// The stall a contended access starting `offset` T-states from now would suffer.
    ///
    /// The route the I/O path takes to [`Timing::delay`], so nothing in [`crate::ula`] has to
    /// reach through an accessor and copy a `Timing` to ask.
    #[inline]
    #[must_use]
    pub(crate) const fn delay_after(&self, offset: u32) -> u32 {
        self.timing.delay(self.ahead(offset))
    }

    /// Whether the ULA is holding `/INT` low right now.
    #[inline]
    #[must_use]
    pub const fn interrupt_asserted(&self) -> bool {
        self.frame_t_state < self.timing.interrupt_t_states
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MACHINES: [(&str, Timing); 2] =
        [("48K", Timing::SPECTRUM_48K), ("128", Timing::SPECTRUM_128)];

    #[test]
    fn a_frame_is_the_published_number_of_t_states() {
        assert_eq!(T_STATES_PER_FRAME, 69888);
    }

    #[test]
    fn the_two_frame_lengths_are_the_published_ones_and_factorise() {
        // 69888 = 224 x 312 and 70908 = 228 x 311. The second identity is one of the three
        // agreeing lineages behind 70908 and it is the cheapest to re-check.
        assert_eq!(Timing::SPECTRUM_48K.frame_t_states(), 69888);
        assert_eq!(Timing::SPECTRUM_128.frame_t_states(), 70908);
        assert_eq!(224 * 312, 69888);
        assert_eq!(228 * 311, 70908);
    }

    #[test]
    fn the_frame_length_agrees_with_the_z80_formats_own_arithmetic() {
        // The `.z80` format description's independent lineage: its T-state counter counts down
        // from 17471, "17726 in 128K modes", "which make a total of 69888 (70908) T states per
        // frame". Transcribed from the format rather than from the hardware FAQ, which is what
        // makes it a second witness rather than a repetition.
        assert_eq!((17471 + 1) * 4, Timing::SPECTRUM_48K.frame_t_states());
        assert_eq!((17726 + 1) * 4, Timing::SPECTRUM_128.frame_t_states());
    }

    /// The timing suite's detection row, in closed form, every term from its disassembly.
    ///
    /// `R` is seven bits and the group is `JP (HL)` to itself in **uncontended** memory — four
    /// T-states and one refresh per iteration — so the reading is the iteration count modulo
    /// 128, offset by the 11 refreshes the setup spends. `entry` is the T-state the measurement
    /// begins at, which is 292 when one interrupt has been taken and 324 when a second has
    /// nested inside the first.
    const fn detection_row(frame_t_states: u32, entry: u32) -> u32 {
        (11 + (frame_t_states - entry) / 4) % 128
    }

    /// The measurement entry with one interrupt taken, and with a second nested inside it.
    const ONE_INTERRUPT: u32 = 292;
    const TWO_INTERRUPTS: u32 = 324;

    #[test]
    fn the_closed_form_reproduces_both_of_the_48k_suites_expected_classes() {
        // The formula is only worth predicting the 128 with if it reproduces the two readings
        // the 48K file actually carries — which are read out of its bytes and are what its own
        // BASIC lines 805 and 807 test against.
        assert_eq!(
            detection_row(Timing::SPECTRUM_48K.frame_t_states(), ONE_INTERRUPT),
            2,
            "TYPE1 (Early)"
        );
        assert_eq!(
            detection_row(Timing::SPECTRUM_48K.frame_t_states(), TWO_INTERRUPTS),
            122,
            "TYPE2 (Late)"
        );

        // And the 120 that separates them is one nested interrupt, not a phase shift: the two
        // entries differ by exactly the 32 T-states such an interrupt costs.
        assert_eq!(TWO_INTERRUPTS - ONE_INTERRUPT, 32);
        assert_eq!(19 + 4 + 4 + 5, 32, "IM2 response, INC C, EI, RET C");
    }

    #[test]
    fn the_128s_detection_row_is_predicted_by_the_interrupt_window() {
        // **The prediction this milestone hands forward, and the reason it is written as a
        // test rather than as a sentence.** The 128 file's detection row reads 121. Over 70908
        // T-states the closed form reaches 121 only from the two-interrupt entry — so the row
        // is decided by whether a second interrupt nests, which is a property of
        // `interrupt_t_states` and of nothing else.
        const FILE_READING: u32 = 121;
        let frame = Timing::SPECTRUM_128.frame_t_states();

        assert_eq!(detection_row(frame, TWO_INTERRUPTS), FILE_READING);
        assert_eq!(
            detection_row(frame, ONE_INTERRUPT),
            1,
            "one interrupt predicts 1, which is not what the file carries"
        );

        // So the shipped 32 predicts a red run, deliberately: see `SPECTRUM_128`'s comment for
        // why a derived 33..=43 is not adopted in its place. This assertion is what makes the
        // prediction falsifiable rather than a note — it goes red the moment somebody changes
        // the constant, which is exactly when they should be reading that comment.
        assert_eq!(
            Timing::SPECTRUM_128.interrupt_t_states(),
            32,
            "if this has moved, the 128 suite's detection row is what to check first"
        );
        assert_ne!(
            detection_row(frame, ONE_INTERRUPT),
            FILE_READING,
            "32 takes one interrupt, so it predicts the suite goes red on every row"
        );
    }

    #[test]
    fn the_frame_length_is_corroborated_by_the_detection_row_and_its_blind_spot_is_512() {
        // The fourth corroboration of 70908, and the only one that is not a document —
        // now stated through the closed form rather than through the shortcut it implies.
        assert_eq!(
            Timing::SPECTRUM_128.frame_t_states() - Timing::SPECTRUM_48K.frame_t_states(),
            1020
        );
        assert_eq!(1020 / 4, 255, "255 iterations, and 255 is -1 mod 128");

        // The blind spot, measured rather than waved at: the reading is periodic in 512
        // T-states of frame length, so it separates 70908 from 69888 and not from these.
        for indistinguishable in [70396_u32, 71420, 71932] {
            assert_eq!(
                detection_row(indistinguishable, TWO_INTERRUPTS),
                detection_row(Timing::SPECTRUM_128.frame_t_states(), TWO_INTERRUPTS),
                "{indistinguishable} would read the same"
            );
        }
        // And it does separate it from the 48K's, which is the whole of what it establishes.
        assert_ne!(
            detection_row(Timing::SPECTRUM_48K.frame_t_states(), TWO_INTERRUPTS),
            detection_row(Timing::SPECTRUM_128.frame_t_states(), TWO_INTERRUPTS)
        );
    }

    #[test]
    fn each_machines_clock_and_frame_length_imply_a_50_hz_frame() {
        // The cross-check `is_consistent` makes, named in a failure message. Two figures
        // transcribed from different sources have to agree about something, and this is the
        // only thing they can both be asked about.
        for (name, timing) in MACHINES {
            let millihertz = u64::from(timing.cpu_hz()) * 1000 / u64::from(timing.frame_t_states());
            assert!(
                (49_900..=50_100).contains(&millihertz),
                "{name}: {millihertz} mHz is not a 50 Hz frame"
            );
        }
        // The two are genuinely different clocks, which is why this is a field rather than a
        // constant. A model that shared one would put the 128's audio 1.3 % out — inaudible
        // on its own and a drift of a whole frame every 74 seconds against the host.
        assert_ne!(Timing::SPECTRUM_48K.cpu_hz(), Timing::SPECTRUM_128.cpu_hz());
        assert_eq!(Timing::SPECTRUM_48K.cpu_hz(), 3_500_000);
        assert_eq!(Timing::SPECTRUM_128.cpu_hz(), 3_546_900);

        // The 128's descends from the PAL colour subcarrier, which is the closest thing to a
        // derivation either figure has. Recorded as the five-Hz rounding it is rather than as
        // an exact identity it is not.
        assert_eq!(17_734_475 / 5, 3_546_895);
        assert!(Timing::SPECTRUM_128.cpu_hz().abs_diff(3_546_895) <= 5);
    }

    #[test]
    fn the_derived_fields_agree_with_their_definitions_on_both_machines() {
        // The compile-time assertions above are the real gate; this is what names them in a
        // failure message rather than as "evaluation of constant value failed".
        for (name, timing) in MACHINES {
            assert!(timing.is_consistent(), "{name}");
            assert_eq!(
                timing.frame_t_states(),
                timing.t_states_per_line() * timing.lines_per_frame(),
                "{name}"
            );
        }
    }

    #[test]
    fn the_two_machines_differ_in_every_way_the_type_exists_to_express() {
        // `timing_oracle.rs` refuted a single shared contention constant by mutation — 14361
        // on a 48K is red by 23 rows of 68 — so a `Timing` whose two values coincided on any
        // of these would be expressing something the hardware says is false.
        let (a, b) = (Timing::SPECTRUM_48K, Timing::SPECTRUM_128);
        assert_ne!(a.t_states_per_line(), b.t_states_per_line());
        assert_ne!(a.lines_per_frame(), b.lines_per_frame());
        assert_ne!(a.frame_t_states(), b.frame_t_states());
        assert_ne!(a.first_contended_t_state(), b.first_contended_t_state());
        assert_ne!(a, b);
    }

    #[test]
    fn the_128s_contention_phase_is_the_transcribed_one_and_its_derivation_does_not_transfer() {
        // 14361 is recorded as a transcription and not as arithmetic. The 48K's shape is
        // `64 x 224 - 1`; applying that shape to the 128 gives a different number, and the
        // shape that does reach 14361 is `63 x 228 - 3` — a different line count and a
        // different offset. A derivation whose shape changes between two machines is a fit.
        assert_eq!(Timing::SPECTRUM_128.first_contended_t_state(), 14361);
        assert_eq!(64 * 224 - 1, Timing::SPECTRUM_48K.first_contended_t_state());
        assert_ne!(
            64 * 228 - 1,
            Timing::SPECTRUM_128.first_contended_t_state(),
            "the 48K's derivation applied to the 128's line length does not reach 14361"
        );
        assert_eq!(63 * 228 - 3, Timing::SPECTRUM_128.first_contended_t_state());

        // And the pattern's alignment within its group is not preserved either, which is the
        // concrete reason nothing about the 48K's phase can be reused.
        assert_eq!(Timing::SPECTRUM_48K.first_contended_t_state() % 8, 7);
        assert_eq!(Timing::SPECTRUM_128.first_contended_t_state() % 8, 1);
    }

    #[test]
    fn every_public_constant_is_the_48ks_projection() {
        // The whole point of the redefinition: there is no second transcription to drift.
        assert_eq!(T_STATES_PER_LINE, Timing::SPECTRUM_48K.t_states_per_line());
        assert_eq!(LINES_PER_FRAME, Timing::SPECTRUM_48K.lines_per_frame());
        assert_eq!(T_STATES_PER_FRAME, Timing::SPECTRUM_48K.frame_t_states());
        assert_eq!(
            INTERRUPT_T_STATES,
            Timing::SPECTRUM_48K.interrupt_t_states()
        );
        assert_eq!(
            FIRST_CONTENDED_T_STATE,
            Timing::SPECTRUM_48K.first_contended_t_state()
        );
        for offset in 0..64 {
            let position = FIRST_CONTENDED_T_STATE + offset;
            assert_eq!(delay(position), Timing::SPECTRUM_48K.delay(position));
        }
    }

    #[test]
    fn nothing_is_contended_before_the_first_display_line() {
        assert_eq!(delay(0), 0);
        assert_eq!(delay(FIRST_CONTENDED_T_STATE - 1), 0);
    }

    #[test]
    fn the_pattern_starts_at_the_first_contended_t_state() {
        let start = FIRST_CONTENDED_T_STATE;
        let observed: Vec<u32> = (0..8).map(|i| delay(start + i)).collect();
        assert_eq!(observed, vec![6, 5, 4, 3, 2, 1, 0, 0]);
    }

    #[test]
    fn the_pattern_is_the_same_eight_stalls_on_both_machines() {
        // The shared-`DELAY_PATTERN` decision, as an assertion. Both sources state the 128's
        // pattern is identical to the 48K's; if that is ever falsified, this is the test that
        // has to change and it says so.
        for (name, timing) in MACHINES {
            let observed: Vec<u32> = (0..8)
                .map(|i| timing.delay(timing.first_contended_t_state() + i))
                .collect();
            assert_eq!(observed, vec![6, 5, 4, 3, 2, 1, 0, 0], "{name}");
        }
    }

    #[test]
    fn the_pattern_repeats_every_eight_t_states_across_the_fetch_window() {
        let start = FIRST_CONTENDED_T_STATE;
        for group in 0..16 {
            assert_eq!(
                delay(start + group * 8),
                6,
                "group {group} should restart the pattern"
            );
        }
    }

    #[test]
    fn the_border_and_flyback_part_of_a_line_is_not_contended() {
        let start = FIRST_CONTENDED_T_STATE;
        assert_eq!(delay(start + CONTENDED_T_STATES_PER_LINE - 8), 6);
        for offset in CONTENDED_T_STATES_PER_LINE..T_STATES_PER_LINE {
            assert_eq!(delay(start + offset), 0, "offset {offset} is off-screen");
        }
    }

    #[test]
    fn each_machine_contends_its_own_line_length_and_no_more() {
        // The 128's line is four T-states longer and every one of them is border, so the
        // uncontended tail is 100 rather than 96. A model that kept the 48K's line period
        // would start contending four T-states early on every line after the first.
        for (name, timing) in MACHINES {
            let start = timing.first_contended_t_state();
            assert_eq!(
                timing.delay(start + CONTENDED_T_STATES_PER_LINE - 8),
                6,
                "{name}"
            );
            for offset in CONTENDED_T_STATES_PER_LINE..timing.t_states_per_line() {
                assert_eq!(timing.delay(start + offset), 0, "{name} offset {offset}");
            }
            assert_eq!(
                timing.delay(start + timing.t_states_per_line()),
                6,
                "{name}: the next line restarts the pattern"
            );
        }
    }

    #[test]
    fn contention_stops_after_the_last_display_line() {
        let last_line_start =
            FIRST_CONTENDED_T_STATE + (DISPLAY_HEIGHT as u32 - 1) * T_STATES_PER_LINE;
        assert_eq!(delay(last_line_start), 6, "line 191 still contends");
        assert_eq!(
            delay(last_line_start + T_STATES_PER_LINE),
            0,
            "there is no line 192"
        );
    }

    #[test]
    fn contention_stops_after_the_last_display_line_on_both_machines() {
        for (name, timing) in MACHINES {
            let last = timing.first_contended_t_state()
                + (DISPLAY_HEIGHT as u32 - 1) * timing.t_states_per_line();
            assert_eq!(timing.delay(last), 6, "{name}: line 191 still contends");
            assert_eq!(
                timing.delay(last + timing.t_states_per_line()),
                0,
                "{name}: there is no line 192"
            );
        }
    }

    #[test]
    fn the_contended_span_is_the_expected_share_of_a_frame() {
        let contended: u32 = (0..T_STATES_PER_FRAME).filter(|&t| delay(t) > 0).count() as u32;
        // Six of every eight T-states stall, over 128 T-states of each of 192 lines.
        assert_eq!(contended, 6 * 16 * DISPLAY_HEIGHT as u32);
    }

    #[test]
    fn both_machines_stall_on_the_same_share_of_their_own_frame() {
        // The count is a property of the display, not of the frame, so it must be identical
        // even though the frames differ by 1020 T-states. A 128 whose contended span had been
        // scaled by the frame-length ratio would fail this and pass the 48K's version.
        for (name, timing) in MACHINES {
            let contended = (0..timing.frame_t_states())
                .filter(|&t| timing.delay(t) > 0)
                .count();
            assert_eq!(contended, 6 * 16 * DISPLAY_HEIGHT, "{name}");
        }
    }

    #[test]
    fn the_clock_rolls_over_at_the_frame_boundary() {
        let mut clock = Clock::new();
        clock.advance(T_STATES_PER_FRAME - 1);
        assert_eq!((clock.frames(), clock.frame_t_state()), (0, 69887));
        clock.advance(1);
        assert_eq!((clock.frames(), clock.frame_t_state()), (1, 0));
    }

    #[test]
    fn a_clock_rolls_over_at_its_own_machines_frame_boundary() {
        // The defect this guards: a 128 clock that rolled at 69888 would raise its interrupt
        // 1020 T-states early, every frame, and drift by a whole frame every 69 seconds.
        for (name, timing) in MACHINES {
            let mut clock = Clock::with_timing(timing);
            clock.advance(timing.frame_t_states() - 1);
            assert_eq!(clock.frames(), 0, "{name}");
            clock.advance(1);
            assert_eq!((clock.frames(), clock.frame_t_state()), (1, 0), "{name}");
        }

        // And the cross-check that makes it a real distinction rather than a tautology.
        let mut one_two_eight = Clock::with_timing(Timing::SPECTRUM_128);
        one_two_eight.advance(T_STATES_PER_FRAME);
        assert_eq!(
            (one_two_eight.frames(), one_two_eight.frame_t_state()),
            (0, 69888),
            "a 48K frame is not a 128 frame"
        );
    }

    #[test]
    fn a_single_advance_longer_than_a_frame_still_lands_correctly() {
        // MACHINE.md Decision 2: nothing bounds one step to one frame.
        let mut clock = Clock::new();
        clock.advance(T_STATES_PER_FRAME * 3 + 7);
        assert_eq!((clock.frames(), clock.frame_t_state()), (3, 7));
    }

    #[test]
    fn the_interrupt_is_a_window_at_the_start_of_the_frame() {
        let mut clock = Clock::new();
        assert!(clock.interrupt_asserted());
        clock.advance(INTERRUPT_T_STATES - 1);
        assert!(clock.interrupt_asserted());
        clock.advance(1);
        assert!(
            !clock.interrupt_asserted(),
            "the line drops after 32 T-states"
        );
    }

    #[test]
    fn the_interrupt_line_comes_back_on_the_next_frame() {
        let mut clock = Clock::new();
        clock.advance(T_STATES_PER_FRAME);
        assert!(clock.interrupt_asserted());
        assert_eq!(clock.frames(), 1);
    }

    #[test]
    fn setting_the_frame_position_leaves_the_frame_counter_alone() {
        // `docs/M6.md` Decision 2's convention, as an assertion rather than a sentence: a
        // restore moves the machine within the frame and does not rewind its uptime.
        let mut clock = Clock::new();
        clock.advance(T_STATES_PER_FRAME * 5 + 100);
        assert_eq!((clock.frames(), clock.frame_t_state()), (5, 100));
        clock.set_frame_t_state(12_345);
        assert_eq!(
            (clock.frames(), clock.frame_t_state()),
            (5, 12_345),
            "the position moved and the uptime did not"
        );
    }

    #[test]
    fn a_frame_position_out_of_range_is_reduced_rather_than_rejected() {
        // A hostile snapshot can name any `u32`. The rollover is the same one `advance`
        // documents, so the clock never stands somewhere that means nothing.
        let mut clock = Clock::new();
        clock.set_frame_t_state(T_STATES_PER_FRAME);
        assert_eq!((clock.frames(), clock.frame_t_state()), (0, 0));
        clock.set_frame_t_state(T_STATES_PER_FRAME * 3 + 7);
        assert_eq!(
            (clock.frames(), clock.frame_t_state()),
            (0, 7),
            "reducing is not advancing: three frames' worth of overshoot is not three frames"
        );
        clock.set_frame_t_state(u32::MAX);
        assert_eq!(clock.frame_t_state(), u32::MAX % T_STATES_PER_FRAME);
    }

    #[test]
    fn a_hostile_frame_position_is_reduced_by_the_machines_own_frame_length() {
        for (name, timing) in MACHINES {
            let mut clock = Clock::with_timing(timing);
            clock.set_frame_t_state(u32::MAX);
            assert!(clock.frame_t_state() < timing.frame_t_states(), "{name}");
            assert_eq!(
                clock.frame_t_state(),
                u32::MAX % timing.frame_t_states(),
                "{name}"
            );
        }
    }

    #[test]
    fn absolute_time_counts_whole_frames_of_the_machines_own_length() {
        for (name, timing) in MACHINES {
            let mut clock = Clock::with_timing(timing);
            assert_eq!(clock.t_states(), 0, "{name}");
            clock.advance(timing.frame_t_states() * 3 + 7);
            assert_eq!(
                clock.t_states(),
                u64::from(timing.frame_t_states()) * 3 + 7,
                "{name}"
            );
        }
        // The distinction that makes this worth having over `frames * T_STATES_PER_FRAME`:
        // the two machines' frames are different lengths, so a shared constant would put a
        // 128's absolute time 3060 T-states out after three frames.
        let mut one_two_eight = Clock::with_timing(Timing::SPECTRUM_128);
        one_two_eight.advance(Timing::SPECTRUM_128.frame_t_states() * 3);
        assert_ne!(
            one_two_eight.t_states(),
            u64::from(T_STATES_PER_FRAME) * 3,
            "a 48K frame is not a 128 frame"
        );
    }

    #[test]
    fn absolute_time_moves_backwards_at_exactly_the_two_places_that_are_not_elapsed_time() {
        // Named rather than assumed, because anything integrating over this clock has to
        // handle it. `Audio::rebase` is what does.
        let mut clock = Clock::new();
        clock.advance(T_STATES_PER_FRAME + 5_000);
        let before = clock.t_states();
        clock.set_frame_t_state(10);
        assert!(clock.t_states() < before, "a restore moves it back");
        assert_eq!(clock.t_states(), u64::from(T_STATES_PER_FRAME) + 10);
        // And a reset builds a fresh clock rather than moving this one, which is the other.
        assert_eq!(Clock::new().t_states(), 0);
    }

    #[test]
    fn ahead_wraps_within_the_frame() {
        let mut clock = Clock::new();
        clock.advance(T_STATES_PER_FRAME - 2);
        assert_eq!(clock.ahead(1), T_STATES_PER_FRAME - 1);
        assert_eq!(clock.ahead(3), 1);
    }

    #[test]
    fn ahead_agrees_with_the_modulo_it_replaced_across_the_whole_seam() {
        // The early return is an optimisation, so it needs the property it claims: identical
        // to `%` for every input, not merely for the small offsets its callers use. Swept
        // across the frame boundary in both machines, plus offsets far past a whole frame.
        for (name, timing) in MACHINES {
            let frame = timing.frame_t_states();
            for position in [0_u32, 1, frame / 2, frame - 20, frame - 1] {
                let mut clock = Clock::with_timing(timing);
                clock.advance(position);
                for offset in [0_u32, 1, 19, 20, frame - 1, frame, frame + 7, 3 * frame + 5] {
                    assert_eq!(
                        clock.ahead(offset),
                        (position + offset) % frame,
                        "{name} at {position} + {offset}"
                    );
                }
            }
        }
    }

    #[test]
    fn delay_now_is_delay_after_zero_without_the_wrap_check() {
        // The two must agree wherever both are defined, or the hot path and the I/O path
        // would price the same position differently.
        for (name, timing) in MACHINES {
            for position in [
                0,
                timing.first_contended_t_state() - 1,
                timing.first_contended_t_state(),
                timing.first_contended_t_state() + 5,
                timing.frame_t_states() - 1,
            ] {
                let mut clock = Clock::with_timing(timing);
                clock.advance(position);
                assert_eq!(
                    clock.delay_now(),
                    clock.delay_after(0),
                    "{name} at {position}"
                );
                assert_eq!(clock.delay_now(), timing.delay(position), "{name}");
            }
        }
    }

    #[test]
    fn a_default_clock_is_a_48k_clock() {
        // `Clock` derives `Default`, so this value is reachable. A derived `Timing::Default`
        // would make it a clock whose frame is zero T-states long, and `advance` would never
        // terminate — which is why `Default` is written out rather than derived.
        assert_eq!(Clock::default(), Clock::new());
        assert_eq!(Clock::default().timing(), Timing::SPECTRUM_48K);
        assert_eq!(Timing::default(), Timing::SPECTRUM_48K);
        assert!(Timing::default().frame_t_states() > 0);
    }

    #[test]
    fn a_clock_reports_the_geometry_it_was_built_with() {
        assert_eq!(Clock::new().timing(), Timing::SPECTRUM_48K);
        assert_eq!(
            Clock::with_timing(Timing::SPECTRUM_128).timing(),
            Timing::SPECTRUM_128
        );
    }

    #[test]
    fn delay_after_prices_the_position_it_names_and_not_the_current_one() {
        // The route the hot path takes to `Timing::delay`. Priced against the free function
        // so a wrong offset would be visible rather than self-consistent.
        let mut clock = Clock::new();
        clock.advance(FIRST_CONTENDED_T_STATE);
        for offset in 0..16 {
            assert_eq!(
                clock.delay_after(offset),
                delay(FIRST_CONTENDED_T_STATE + offset),
                "offset {offset}"
            );
        }
    }
}
