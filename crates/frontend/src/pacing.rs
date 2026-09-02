//! Keeping 50 Hz, and saying so when it does not.
//!
//! # The failure this module exists to make visible
//!
//! A frame loop that runs one emulated frame per displayed frame is silently wrong on every
//! monitor that is not exactly 50 Hz — at 60 Hz the machine runs 20 % fast, its own `FRAMES`
//! counter drifts against the wall clock, and nothing anywhere reports it. A loop that
//! instead runs however many frames the elapsed time owes is right until something stalls,
//! and then it tries to run the whole backlog in one tick, which takes longer than a tick,
//! which grows the backlog. The second failure is worse than the first because it is
//! self-amplifying: one slow frame becomes a freeze.
//!
//! [`Pacer`] does neither. It converts elapsed time into whole frames owed, runs at most
//! [`MAX_CATCH_UP`] of them, and **counts the rest as lost** rather than carrying them. The
//! count is the point. `docs/STATUS.md` spends several sections on tools that answered a
//! narrower question than the caller asked in a form indistinguishable from the wider one; a
//! frame loop that quietly drops work is exactly that shape, and the remedy is the same one
//! this project uses everywhere else — *make the tool state what it covered, and assert on
//! that, rather than on its verdict.*
//!
//! # What is gated here and what is not
//!
//! The **arithmetic** is gated: `tests/pacing_accounting.rs` drives literal `Duration`
//! sequences and asserts literal `(run, dropped)` pairs, including the ones that cross the
//! catch-up bound and the one that would overflow. Whether the result *looks* smooth is
//! observation — a run reporting `50.0 Hz, 0 dropped` that still stutters would be a vsync
//! interaction happening below this module, and nothing here could see it.
//!
//! The **indicator** is gated too, and separately, because it turned out to be a different
//! question from the count. [`LossMeter::keeping_up`] is what colours the status bar, so it lives
//! here where a test can drive it rather than in the shell where it cannot — and the same file
//! asserts both directions: a burst of losses followed by clean frames must go back to normal,
//! and a stutter that keeps going must stay red across a window boundary. The second is the one
//! that is easy to forget, and it is the one a naive fix breaks.
//!
//! # Running faster than a real Spectrum, and why it belongs *here*
//!
//! A tape is a **signal** — `docs/M6.md` Decision 4 rules out the ROM trap that would skip it, and
//! `crates/spectrum/tests/tape_rom_load.rs`'s `no_shortcut_exists_past_the_ear_bit` keeps it ruled
//! out — so a three-minute load takes three minutes of emulated time and there is no way round
//! that which is not the shortcut. The way round it that is **not** a shortcut is to run the same
//! machine against a faster clock: [`Speed`] multiplies the elapsed wall time this module converts
//! into frames owed, and nothing below that conversion is told. Every loader works, including the
//! turbo loaders a trap would have missed, because from the machine's side nothing has happened.
//!
//! **What that costs, and where it is paid.** Two things in a frontend are functions of real time
//! rather than of T-states, and they are the two the multiplier has to be reasoned about at:
//!
//! - **The catch-up bound.** [`MAX_CATCH_UP`] is a frame count standing in for a wall-clock
//!   duration, and it stops being one the moment a tick is *expected* to owe more than four
//!   frames. It is therefore scaled by the multiplier — see its own documentation — so that a lost
//!   frame keeps meaning the same physical event at every speed and fast-forward cannot masquerade
//!   as a stall.
//! - **Audio.** A device consumes one second per second whatever this module does, so eight
//!   seconds of samples a second is a backlog rather than a sound. That decision is the shell's
//!   and is made in `src/main.rs`, at the one place a device is written to.
//!
//! # And then the rung that picks the multiplier for you
//!
//! Every rung above is a number somebody has to choose, and the owner's request was not for a
//! number: *"I want the tape to load at once and not wait"*. [`Rung::Automatic`] is the answer —
//! flat out while the machine is decoding a tape, real time when it is not — and it is a **fifth
//! rung of the same key** rather than an override, so nobody watching a load is overtaken by it
//! and one press still comes home.
//!
//! **That trigger was wrong once, in a way worth keeping written down.** It read the *motor*, and
//! a turning motor is not a machine that is loading. Pressing PLAY before typing `LOAD ""` had
//! always been free — the ROM's five-second pilot leader is five seconds of grace — and keying a
//! 90× fast-forward off the drive spends that grace in **0.055 s**, so the cassette is gone two
//! seconds later and the loader that eventually asks for it finds silence. It cost the owner an
//! evening on 2026-09-01. [`EarMeter`] is the signal that replaced it: a rate rather than a
//! state, read from the machine rather than from the drive, and true *before* anybody presses
//! PLAY — which is the half no delay and no bigger leader could ever have reached.
//!
//! It needed a second mechanism rather than a bigger number, and the reason is the bullet
//! directly above: a multiplier's cost per tick is `MAX_CATCH_UP × factor × the cost of a frame`,
//! so raising the factor to reach *"as fast as this host manages"* raises the **freeze** with it.
//! [`FLAT_OUT_BUDGET`] is the bound in the unit that cost is paid in, and [`Pacer::run_flat_out`]
//! spends it. The machine still executes every T-state of every frame; what changes is how many
//! frames a tick asks for and how it decides.

use core::fmt;
use core::num::NonZeroU32;
use core::time::Duration;

/// Nanoseconds in one frame of a 50 Hz machine.
pub const FRAME_NANOS: u128 = 20_000_000;

/// The most frames one call will run before declaring the remainder lost, **at real time**.
///
/// Four frames is 80 ms of emulated time. The bound exists so that a stall cannot compound:
/// past this point the loop is not going to catch up within a tick, and attempting it turns a
/// hiccup into a freeze while the backlog keeps growing. Dropping instead is a decision to
/// lose emulated time, which is why the drop is counted and shown rather than absorbed.
///
/// # It is a wall-clock bound wearing a frame count, and [`Speed`] is what exposes that
///
/// *"Four frames"* and *"eighty milliseconds"* are the same sentence only while one emulated
/// second takes one wall second. At eight times real time an ordinary 20 ms tick owes **eight**
/// frames, so a fixed ceiling of four would clip every single tick: the machine would run at 4×
/// however high the multiplier went, and [`Pacer::dropped`] would climb by four a frame while
/// nothing whatever was wrong. The status bar would then be red for the entire fast-forward — a
/// false alarm indistinguishable from the real one, which is the exact defect [`LossMeter`] was
/// written to remove, arriving from the other direction.
///
/// So [`Pacer::advance`] uses `MAX_CATCH_UP × factor`, and the property that survives is the one
/// worth having: **a frame is lost only when a tick took more than 80 ms of wall clock**, at every
/// speed. The meter needs no exception, the colour needs no suppression, and a host that genuinely
/// cannot sustain the multiplier it was asked for still says so.
pub const MAX_CATCH_UP: u64 = 4;

/// Seconds after which a person stops reading a response as instantaneous.
///
/// The oldest number in interaction design and still the one that matters: past about a tenth of
/// a second a response is no longer felt as *caused by* the key, it is felt as *arriving after*
/// it. It is here rather than in the shell because it is what [`FLAT_OUT_BUDGET`] is derived
/// from, and a derivation whose premise lives in another file is a derivation nobody re-checks.
const INSTANTANEOUS: f64 = 0.1;

/// Seconds of wall clock one tick hands to the machine while it is running flat out.
///
/// # What a budget is for, and why it is not a very large multiplier
///
/// [`RUNGS`] already sets out why an unbounded factor is unsafe here, and the sentence worth
/// repeating is the mechanism: [`MAX_CATCH_UP`] is a **frame count scaled by the factor**, so one
/// tick costs `4 × factor × 204 µs` of wall clock with nothing drawn and no key read — 52 ms at
/// 64×, and fourteen hours at [`u32::MAX`]. *"As fast as this host manages"* therefore cannot be
/// spelled as a multiplier at all: it needs a bound in the unit the cost is actually paid in,
/// which is the wall clock, and it needs it read **inside** the loop rather than computed before
/// it. That is this constant, and [`Pacer::run_flat_out`] is where it is spent.
///
/// # Where the number comes from, which is a person and not a round figure
///
/// Two things about a fast-forwarding window are perceptual, and both are properties of the
/// **tick period** rather than of the machine: a key pressed during a load waits at most one tick
/// to be read, and the picture on the screen is at most one tick old.
///
/// - **Response.** `INSTANTANEOUS` is the ceiling. A tick at that ceiling would be a window
///   whose every keystroke lands exactly at the edge of being noticed, which is not a design so
///   much as a dare. A **third** of it leaves the other two thirds for the draw, for a missed
///   vsync, and for a host slower than the one this was measured on.
/// - **Motion.** A third of a tenth of a second is 33 ms, which redraws **thirty times a
///   second** — comfortably clear of the twenty-four a cinema projector settled on, so the
///   loading stripes read as movement rather than as a slideshow. This is the constraint that
///   stops the budget simply growing until the duty cycle approaches one: at 100 ms a tick the
///   machine would be fractionally faster and the screen would be a slide show.
///
/// So the tick is 33.3 ms, and the budget is that **minus what producing a picture costs**,
/// because the two share the tick. Half of that subtrahend is measurable and was measured, and
/// half of it is not — which is what the rounding is for:
///
/// | | |
/// |---|---|
/// | a tick a person still reads as instant, and as motion | 33.3 ms |
/// | this crate's half of a picture — `Spectrum::render` into a [`spectrum::Frame`], then [`crate::palette::write_rgba`] into the buffer the window uploads | **0.016 ms**, measured |
/// | the GPU's half — a 320 KB texture upload, one draw call, the vsync wait | not measurable from a headless test |
/// | what is left, rounded **down** | **30 ms** |
///
/// `cargo test --release -p frontend --test speed_multiplier -- --ignored --nocapture` re-takes
/// the measured row; `the_cost_of_one_picture` is the test, and it carries its own warning about
/// what the number was before the optimiser was told not to delete the loop.
///
/// **The rounding is the headroom, not a tidy figure.** Rounding 33.3 down to 30 leaves 3.3 ms
/// for the row that could not be measured — two hundred times the row that could — so the
/// derivation degrades gracefully on a host whose GPU is slower than this one's rather than
/// silently overrunning the tick it was derived from.
///
/// # What that delivers, and why the readout still has the last word
///
/// **Measured headless, on the real cassette, through this constant and
/// [`Pacer::run_flat_out`]:** *Manic Miner* — about 9,630 frames, 192.6 seconds of emulated time —
/// in **2.07 s to 2.31 s of wall clock, 83× to 93× real time**, over four runs on one machine.
/// `a_real_cassette_end_to_end_under_automatic` is the run and prints every one of those figures.
///
/// The spread is **the machine, not this loop**, and the two figures that move say which: the
/// frame count is stable to a tenth of a percent while the cost of a frame goes from 215 µs to
/// 240 µs depending on what else the host is doing. A burst is a fixed 30 ms either way, so a
/// dearer frame simply means fewer of them in it — which is the whole point of budgeting in wall
/// clock rather than in frames, and is what a multiplier cannot do.
///
/// **In a window it will be less, and nothing here can say how much less.** A tick there also has
/// to draw, and the tick period is quantised by vsync — so a display whose frame does not divide
/// neatly into 33 ms delivers a lower share, and a slower host delivers less again. That is
/// exactly why nothing reports a multiplier for [`Rung::Automatic`]: `Hz` on the status bar is
/// what was **delivered**, measured rather than claimed, and the gap between it and 50 is the
/// reading.
pub const FLAT_OUT_BUDGET: f64 = 0.030;

const _: () = assert!(
    FLAT_OUT_BUDGET < INSTANTANEOUS,
    "a tick that runs longer than the delay a person can notice is a window that stops answering \
     the keyboard during a load — which is the freeze this module refuses in its first paragraph, \
     reached by choosing a budget instead of by failing to keep one"
);

/// How much faster than a real Spectrum the machine is being run.
///
/// # Why this is a type and not a `u32`
///
/// Zero is not a slow machine, it is a **stopped** one: a factor of nought makes
/// [`Pacer::advance`] owe nothing however long the wall clock takes, so the screen freezes while
/// the readout insists everything is on time. That is this module's own recurring failure —
/// an indicator answering a question nobody asked — so the value that produces it is made
/// unrepresentable rather than guarded against at each use.
///
/// [`Default`] is [`Speed::REAL_TIME`], which is what lets [`Pacer`] keep its derived `Default`
/// and stay the pacer this crate has always had.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Speed(NonZeroU32);

impl Speed {
    /// One emulated second per wall second: a real Spectrum.
    pub const REAL_TIME: Self = Self(NonZeroU32::MIN);

    /// `factor` emulated seconds per wall second, or `None` when `factor` is zero.
    ///
    /// Total rather than panicking, so the one input that is not a speed is a value a caller
    /// has to answer for rather than an abort. Every caller in this crate passes a literal, and
    /// in a `const` item [`Option::expect`] turns a mistake there into a build failure.
    #[must_use]
    pub const fn new(factor: u32) -> Option<Self> {
        match NonZeroU32::new(factor) {
            Some(factor) => Some(Self(factor)),
            None => None,
        }
    }

    /// The multiplier, as the number a readout prints.
    #[must_use]
    pub const fn factor(self) -> u32 {
        self.0.get()
    }
}

impl Default for Speed {
    fn default() -> Self {
        Self::REAL_TIME
    }
}

/// One rung of the speed key's cycle: a multiplier somebody chose, or the one that decides.
///
/// # Why automatic is a rung of this cycle and not a mode laid over it
///
/// The alternative was a separate control — a flag, a second key, a rule that quietly took over
/// whenever a tape started. Every version of that answers *"what happens if I am at 1× watching
/// the loading stripes"* badly: something the person did not ask for happens to them, and the
/// only way back is to find out that an override exists. As a **rung** the question does not
/// arise. Somebody who wants to watch a tape load never selects it, somebody at 1× is never
/// overridden, and the key that turned it on is the key that turns it off.
///
/// It is also why the readout names it. A machine running flat out and a machine with a broken
/// clock look identical from outside, so [`Rung::Automatic`] is drawn as `auto` at all times and
/// as `auto (loading)` while it is actually doing something — see [`Rung::note`]. Automatic that
/// is not legible **as** automatic is the same defect as a colour that latches: a readout
/// answering a question nobody asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rung {
    /// A fixed multiple of real time, whatever the machine is doing.
    Fixed(Speed),
    /// Flat out while the machine is decoding a tape; real time when it is not.
    ///
    /// # The trigger is the machine, and it used to be the drive
    ///
    /// It read [`spectrum::tape::Tape::is_playing`] — *the motor is turning* — which is a fact
    /// about the **cassette player** and not about the **computer**, and the two come apart in
    /// exactly the case a person reaches for first. Pressing PLAY and then typing `LOAD ""` had
    /// always worked, because the leader is five seconds of pilot tone and typing takes two; at
    /// 90× those five seconds are **0.055 s**, and the tape is off its end before the loader
    /// exists. Nothing on the bar could say so either, because at that point nothing was wrong
    /// with the drive.
    ///
    /// So the trigger is now [`EarMeter::decoding`]: how often the machine reads the `EAR` line,
    /// against a threshold [`decoding_threshold`] derives from the frame length. That signal
    /// covers the same ground as the motor and one strip more — **a loader waiting for a tape
    /// reads the line hardest of all**, so the documented order accelerates from the first tick
    /// rather than from the press, which is a thing no amount of delay could buy.
    ///
    /// It is still read every tick rather than shadowed, for the reason
    /// `crates/frontend/src/keymap.rs` records about shadow copies, and the end of a cassette
    /// still keys it off by itself — a beat later than the motor did, once the guest stops
    /// asking rather than the moment the train runs out.
    Automatic,
}

/// What a [`Rung`] asks of the tick about to run.
///
/// A second type rather than a resolved [`Rung`], because these are two different questions and
/// the module already keeps that kind of pair apart — [`Pacer`] and [`LossMeter`] are separate
/// *"because they answer different questions and only one of them decides anything"*. A `Rung` is
/// what somebody **selected**; a `Tick` is what that selection **means right now**, once the
/// drive has been consulted. Collapsing them would leave `Rung::Automatic` meaning *"the
/// automatic rung"* in one position and *"flat out"* in another, which is one word doing two
/// jobs at the exact place a reader most needs it to do one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tick {
    /// Owe frames from the elapsed wall clock at this multiplier. See [`Pacer::advance`].
    Paced(Speed),
    /// Run frames until the wall-clock budget is spent. See [`Pacer::run_flat_out`].
    FlatOut,
}

impl Rung {
    /// What this rung asks of the tick about to run, given whether the machine is decoding a tape.
    ///
    /// `decoding` is [`EarMeter::decoding`]'s verdict — *the machine is reading the `EAR` line at
    /// a rate only a loader reaches* — and **not** whether the drive is turning. Those are
    /// different questions and this rung asks the second one badly; [`Rung::Automatic`] carries
    /// what that cost.
    ///
    /// **A [`Rung::Fixed`] rung answers itself**, and that is the whole of why a multiplier stays
    /// a thing somebody chose rather than something that happens to them: the machine is not
    /// consulted, so putting a cassette in cannot move a machine the person parked at 1×.
    #[must_use]
    pub const fn this_tick(self, decoding: bool) -> Tick {
        match self {
            Self::Fixed(speed) => Tick::Paced(speed),
            Self::Automatic if decoding => Tick::FlatOut,
            Self::Automatic => Tick::Paced(Speed::REAL_TIME),
        }
    }

    /// What the readout adds after this rung's name, and nothing when there is nothing to add.
    ///
    /// The half of the label that is a *state* rather than a *setting*: `auto` says what the key
    /// selected and this says whether it is currently doing anything. Both are drawn, because
    /// automatic with nothing to be automatic about is exactly the case a person would otherwise
    /// read as the feature being broken.
    ///
    /// **It became true when [`Rung::this_tick`]'s argument did, and that is worth naming.** The
    /// word it draws is `loading`, and it used to be written from the motor — so it appeared over
    /// a turning drive nobody was reading, which is the same lie `crate::drive` was written to
    /// take out of the *tape* row one row up. It is derived from [`Rung::this_tick`] rather than
    /// from its own condition, so the label cannot say `loading` about a tick that is not running
    /// flat out however either is later changed.
    #[must_use]
    pub const fn note(self, decoding: bool) -> &'static str {
        match self.this_tick(decoding) {
            Tick::FlatOut => LOADING,
            Tick::Paced(_) => "",
        }
    }
}

impl fmt::Display for Rung {
    /// `1x`, `64x`, `auto` — the name only, with [`Rung::note`] carrying the state.
    ///
    /// A `Display` rather than a `String`-returning method so that [`Status::draw`] keeps
    /// formatting the whole readout in one `write!` into its reused buffer, allocating nothing
    /// fifty times a second.
    ///
    /// [`Status::draw`]: ../zx/index.html
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fixed(speed) => write!(f, "{}x", speed.factor()),
            Self::Automatic => f.write_str(AUTOMATIC),
        }
    }
}

/// What the readout calls [`Rung::Automatic`].
///
/// Private, with the pair below, because nothing outside this module reads either: [`Rung`]'s
/// `Display` and [`Rung::note`] are the whole of how a caller gets at them, which is the point of
/// having those two rather than a pair of strings to concatenate. A `pub const` with no consumer
/// is public surface bought for nobody — and it would be surface on a *string*, where the next
/// person to reword the label would be making a semver change without noticing.
const AUTOMATIC: &str = "auto";

/// What the readout adds to [`AUTOMATIC`] while the machine is decoding a tape. See
/// [`Rung::note`].
const LOADING: &str = " (loading)";

/// The rungs the window cycles through, in the order it reaches them.
///
/// # The old top was a round number, and the ceiling turned out to be an order of magnitude away
///
/// This table stopped at eight, and the argument for eight was that *"past it the win is seconds
/// while the cost — an unwatchable screen and a machine that has to sustain 400 emulated frames a
/// second — is not"*. The second half of that sentence was the load-bearing one and **it was never
/// measured**: 400 frames a second was written as though it were a lot, and nobody had asked what
/// this host actually manages. It manages roughly five thousand.
///
/// **Measured 2026-09-01, two independent ways, both in `--release`.**
///
/// `cargo bench -p spectrum --bench frame` puts one emulated frame at **150.9 µs** idle
/// (`quiet_48k`) and **170.3 µs** with the border being rewritten (`border_48k`), which is what a
/// loading stripe is. And end to end, through the frame loop a person actually runs:
/// `zx-shot --media testdata/games/ManicMiner.tap --play-tape` ran a whole cassette — **9,445
/// frames, 189 seconds of emulated time** — in **1.93 s of wall clock**, which is **204 µs per
/// frame** and **98× real time**. The two agree: a tape-loading frame is a shade dearer than a
/// beeper frame (`beeper_48k`, 200.2 µs), because the loader spins on a contended `IN` and paints
/// the border, and that is the honest upper bound rather than the idle figure.
///
/// So the ceiling is **about 98×**, not eight, and it is the *host* that sets it.
///
/// # Why the top is 64 and not the ceiling, and not uncapped either
///
/// **Uncapped was weighed and is unsafe here, for a reason specific to this module.**
/// [`MAX_CATCH_UP`] is a *frame count* scaled by the factor, and it only stands in for eighty
/// milliseconds while the host keeps up. Its real cost is `4 × factor × 204 µs` of wall clock in a
/// single tick, and that is the window frozen with nothing drawn and no key read: 52 ms at 64×,
/// 105 ms at 128×, and **fourteen hours** at [`u32::MAX`]. An unbounded multiplier therefore does
/// not buy *"as fast as this host manages"* — it converts the one bound that stops a hiccup
/// compounding into an unbounded one, which is the self-amplifying freeze this module's own header
/// refuses in its first paragraph. *"As fast as this host manages"* needs a **wall-clock** budget
/// read inside the frame loop, which is a different mechanism from a multiplier and is not this
/// table.
///
/// > **That prediction was cashed in, and it held.** [`FLAT_OUT_BUDGET`] is the wall-clock budget
/// > and [`Pacer::run_flat_out`] is the loop that reads it, and neither is a multiplier: this
/// > table gained a fifth entry rather than a fifth *number*, [`Speed`] gained no variant, and
/// > [`Pacer::advance`] — the one place a factor is applied — was not touched. What the budget
/// > buys over the top rung here is the difference between 64× and whatever the host is actually
/// > good for, which on this one is about 98×; what it costs is that the figure is no longer
/// > predictable from the table, which is why [`Rung::Automatic`] prints no multiplier at all.
///
/// **64 rather than 98** because a rung the host cannot sustain is a rung that lies. At 64× a
/// 20 ms tick owes 64 frames and costs 13 ms, so it fits inside a display frame with room to
/// spare, the picture keeps moving, and [`LossMeter`] stays grey because nothing is being lost.
/// A rung *above* saturation still works and still tells the truth — the ceiling clips it, the
/// readout's `Hz` falls short of the multiplier, and the bar goes red — but red for the whole of
/// every load, on a machine that is doing its best, is an alarm nobody would ever act on. That is
/// the same defect [`LossMeter`] exists to remove, reached from the other side.
///
/// This is one host on one day. A slower machine saturates lower and its top rungs will colour the
/// bar, which is the designed behaviour and not a bug: the multiplier is what was *asked for* and
/// `Hz` is what was *delivered*, and the gap between them is the reading.
///
/// # Quartering rather than doubling, so the table stays short
///
/// The old argument for doubling was keystrokes — *"three keystrokes where `1..8` would need
/// seven"* — and it survives the wider range only by widening the step with it. `1, 2, 4, 8, 16,
/// 32, 64` is seven rungs and six presses to get home; `1, 4, 16, 64` is four rungs, three presses
/// to the top and **one press back to real time**, which is exactly the ergonomics this table had
/// before and the state somebody wants back in a hurry.
///
/// What that buys, in the unit the owner actually asked in: a three-minute cassette that took
/// **twenty-five seconds** at the old top takes **three** at this one.
///
/// # And then a fifth rung, because *three seconds* was still an answer to the wrong question
///
/// The owner's words were *"I want the tape to load at once and not wait"*, and every multiplier
/// in this table answers that with a number somebody has to pick — three presses to reach it, one
/// to leave, and a figure that is right on the host it was measured on and wrong on every other.
/// [`Rung::Automatic`] answers it with the machine instead: flat out while the machine is decoding
/// a tape, real time when it is not, and no figure to be wrong. It is **last** in the cycle rather
/// than first, so the table still opens at real time and `F8` still returns there in one press
/// from wherever it stands.
///
/// **Nothing above it is redundant.** A fixed rung is what somebody watching a load reaches for —
/// 4× is a tape you can still see happening — and it is also the only thing to fall back on when
/// the automatic rung's trigger is wrong for a particular tape. The gaps are where that shows:
/// after a pilot is detected the ROM parks at `LD-WAIT` and reads the line **not at all** for
/// about a second, so [`EarMeter`] correctly reports a machine that is not decoding and the tick
/// is paced. See [`EarMeter`] for why that is the signal working and not chatter to be smoothed
/// away.
pub const RUNGS: &[Rung] = &[
    Rung::Fixed(Speed::REAL_TIME),
    // `expect` on a literal, inside a `const`: a zero written here does not panic at run time,
    // it fails to compile. Pattern P's permitted case, in its strongest form.
    Rung::Fixed(Speed::new(4).expect("4 is not zero")),
    Rung::Fixed(Speed::new(16).expect("16 is not zero")),
    Rung::Fixed(Speed::new(64).expect("64 is not zero")),
    Rung::Automatic,
];

/// Frames lost inside a [`LossMeter`]'s window before the readout calls it a stall.
///
/// **One is not enough, and the reason is [`MAX_CATCH_UP`].** A frame is only ever lost when a
/// single call to [`Pacer::advance`] owed more than four of them — that is, when one tick took
/// over 100 ms — and a windowing system hands any program one of those from time to time: a
/// resize, a tab regaining focus, a page fault, a device opening. It is a real event, which is
/// why [`Pacer::dropped`] counts it for ever and this module refuses to absorb it.
///
/// It is not a *condition*. Two inside one window means it is recurring, and recurring is the
/// difference between *"something happened"* and *"something is wrong"* — the first is what a
/// running total is for, the second is what a colour is for, and the defect this constant exists
/// to close was a colour answering the first question.
pub const LOSS_ALARM: u64 = 2;

const _: () = assert!(FRAME_NANOS * 50 == 1_000_000_000);

/// Converts elapsed wall time into frames to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pacer {
    /// Emulated time owed and not yet run, always less than one frame after [`Pacer::advance`].
    owed: u128,
    ran: u64,
    dropped: u64,
    /// Emulated seconds owed per wall second. See [`Speed`].
    speed: Speed,
}

impl Pacer {
    /// A pacer owing nothing, running at real time.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            owed: 0,
            ran: 0,
            dropped: 0,
            speed: Speed::REAL_TIME,
        }
    }

    /// Owe frames at `speed` from the next [`Pacer::advance`] onward.
    ///
    /// The sub-frame remainder is **kept** across the change rather than cleared. It is emulated
    /// time already owed and not yet delivered, and a machine that silently lost up to 20 ms of
    /// itself every time somebody pressed the speed key would be doing exactly what
    /// [`Pacer::advance`] keeps the remainder to avoid.
    pub const fn set_speed(&mut self, speed: Speed) {
        self.speed = speed;
    }

    /// The multiplier frames are currently owed at.
    #[must_use]
    pub const fn speed(self) -> Speed {
        self.speed
    }

    /// Account for `elapsed` and return how many frames to run now.
    ///
    /// The sub-frame remainder is kept, so a 60 Hz display alternates between one frame and
    /// two and averages exactly 50 — dropping the remainder instead would lose 3 ms every
    /// tick and run the machine 15 % slow with nothing to show for it.
    ///
    /// Saturating rather than panicking on the way out of `u128`: a debugger pause or a
    /// suspended laptop can hand this an elapsed time of hours, and a frontend that aborts
    /// because the machine was asleep is a worse outcome than one that reports a very large
    /// number of dropped frames.
    ///
    /// [`Speed`] is applied here and **only** here, which is the whole of what makes the machine
    /// identical at every multiplier: what changes is how much time this function decides has
    /// passed, and every frame it then asks for is the frame it would have asked for anyway. The
    /// widest case does not overflow with room to spare — `Duration::MAX` is about 1.8 × 10²⁸
    /// nanoseconds and the largest multiplier is 4.3 × 10⁹, whose product is 7.9 × 10³⁷ against a
    /// `u128` ceiling of 3.4 × 10³⁸ — and
    /// `a_suspended_machine_at_the_highest_multiplier_does_not_overflow` is the assertion rather
    /// than this sentence.
    pub const fn advance(&mut self, elapsed: Duration) -> u64 {
        // `as` rather than `u32::into`, because `From` is not `const` and this function is.
        // Both casts widen; neither can lose a bit.
        let factor = self.speed.factor();
        self.owed += elapsed.as_nanos() * factor as u128;
        let whole = self.owed / FRAME_NANOS;
        self.owed -= whole * FRAME_NANOS;

        let due = if whole > u64::MAX as u128 {
            u64::MAX
        } else {
            whole as u64
        };
        // Scaled, because the bound is a wall-clock one written as a frame count — see
        // [`MAX_CATCH_UP`]. Unscaled it would clip every ordinary tick above real time and
        // report the fast-forward as a permanent stall.
        let ceiling = MAX_CATCH_UP * factor as u64;
        let run = if due < ceiling { due } else { ceiling };

        self.ran += run;
        self.dropped += due - run;
        run
    }

    /// Hand over frames until this tick's wall-clock budget is spent, and say how many that was.
    ///
    /// [`Tick::FlatOut`]'s half of the loop, and the counterpart to [`Pacer::advance`]: that one
    /// computes a count from time already elapsed, and this one **discovers** a count by spending
    /// [`FLAT_OUT_BUDGET`] of time that has not elapsed yet. There is no way to know how many
    /// frames fit in thirty milliseconds without running them, so the frames are run from here —
    /// which is why this takes a `frame` to call rather than returning a number the way its
    /// sibling does.
    ///
    /// It stays on [`Pacer`] rather than becoming a free function so that the count reaches
    /// [`Pacer::ran`] in the same place `advance`'s does. A frontend that had to remember a
    /// second call to credit the frames would have a readout whose `Hz` read 50 while the machine
    /// ran at five thousand, and a rate that is only right on some rungs is worse than no rate.
    ///
    /// **[`Pacer::dropped`] is untouched, and that is the point rather than an omission.** A
    /// dropped frame means *the machine was owed emulated time and did not get it*; here nothing
    /// is owed — the budget is a decision to run for a while, not a debt — so there is nothing to
    /// lose and the status bar stays grey through a load. The sub-frame remainder in `owed` is
    /// likewise left alone, so the tick after the tape stops resumes from wherever real time had
    /// got to instead of paying back a burst.
    ///
    /// **`clock` is read after every frame and must advance**, which is a property of a wall
    /// clock and is worth naming because a frontend could hand over the wrong one: macroquad's
    /// `get_time` is documented to change *"as real world time progresses during computation"*,
    /// which is exactly what this needs, where its `get_frame_time` is a per-tick delta and would
    /// spin here for ever.
    ///
    /// One frame always runs, whatever the clock says. A budget so tight that a tick could run
    /// **none** would be a stopped machine reported as a fast one, which is the failure
    /// [`Speed`]'s own `NonZeroU32` exists to make unrepresentable, arriving through the door
    /// next to it.
    pub fn run_flat_out(&mut self, mut clock: impl FnMut() -> f64, mut frame: impl FnMut()) -> u64 {
        let deadline = clock() + FLAT_OUT_BUDGET;
        let mut run = 0;
        loop {
            frame();
            run += 1;
            if clock() >= deadline {
                self.ran += run;
                return run;
            }
        }
    }

    /// Frames run since power-on.
    #[must_use]
    pub const fn ran(self) -> u64 {
        self.ran
    }

    /// Frames the machine owed and never got.
    #[must_use]
    pub const fn dropped(self) -> u64 {
        self.dropped
    }
}

/// Emulated frames per wall second, over a closed window.
///
/// Separate from [`Pacer`] because they answer different questions and only one of them
/// decides anything: the pacer chooses how much work to do, and this reports how much got
/// done. Fusing them would make a measuring instrument part of the thing it measures.
///
/// The clock is passed in as a `f64` of seconds rather than read from [`std::time::Instant`],
/// which is the single choice that keeps this module compiling for `wasm32-unknown-unknown` —
/// see [`crate::host`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RateMeter {
    window_seconds: f64,
    opened_at: f64,
    frames_at_open: u64,
    hz: f64,
}

impl RateMeter {
    /// A meter reporting over `window_seconds`, opened at `now`.
    ///
    /// One second is the shortest window that reads steadily: shorter and the figure jitters
    /// with each display frame, longer and a stall has stopped by the time it is visible.
    #[must_use]
    pub const fn new(window_seconds: f64, now: f64) -> Self {
        Self {
            window_seconds,
            opened_at: now,
            frames_at_open: 0,
            hz: 0.0,
        }
    }

    /// Offer the current time and cumulative frame count; closes the window when it is due.
    pub const fn sample(&mut self, now: f64, frames: u64) {
        let elapsed = now - self.opened_at;
        if elapsed < self.window_seconds {
            return;
        }
        self.hz = (frames - self.frames_at_open) as f64 / elapsed;
        self.opened_at = now;
        self.frames_at_open = frames;
    }

    /// The rate over the last closed window; zero until the first one closes.
    #[must_use]
    pub const fn hz(self) -> f64 {
        self.hz
    }
}

/// Frames lost *recently*, as opposed to ever.
///
/// # The defect this exists to fix
///
/// [`Pacer::dropped`] is a lifetime total. It answers *"has anything ever gone wrong"*, it is
/// honest, and it is the wrong number to colour a status bar with — which is what the shell did
/// until it was pointed out: `if pacer.dropped() == 0 { LIGHTGRAY } else { RED }`. One lost frame
/// at any moment latched the readout red for the rest of the run, however perfectly it went
/// afterwards. And a loss at start-up is close to certain — the window opens, the audio device
/// initialises, the first `elapsed` is enormous and [`MAX_CATCH_UP`] clips the rest — so the
/// indicator was very nearly *always on*, which is the same as being off.
///
/// A cumulative counter was driving a state indicator. The count is not the mistake; the count is
/// a real total and stays. The mistake was answering *"has anything ever gone wrong"* in a place a
/// person reads as *"is something wrong now"*.
///
/// # Why the losses age out rather than start-up being excluded
///
/// Skipping the first *n* frames would fix the symptom and hide a class of real problem: an
/// emulator that takes two seconds to reach speed **is** stuttering, a person watching would see
/// it, and a rule that looked away from exactly that interval would be a measurement lying by
/// omission. A window that simply forgets old losses needs no special case — start-up colours the
/// bar red, honestly, for as long as it is actually stalling, and then it clears.
///
/// # Why this is not [`RateMeter`] with a different argument
///
/// The shape is deliberately the same, and the difference is one line: `RateMeter` reports **only
/// closed windows**, because a partial window divides a few frames by a little time and reads as
/// noise. That is right for a rate and wrong for an alarm. Losses are events rather than a rate —
/// one is one, immediately — so the window now open is counted too, and a stall colours the bar
/// on the frame it happens rather than up to a second later, by which time a short one is over
/// and nobody saw it. `tests/pacing_accounting.rs` pins that property so the two cannot be
/// merged by someone who notices they look alike.
///
/// The count spans the open window plus the one before it, so it covers between one and two
/// windows. That is what stops the figure falling to zero for an instant every time a window
/// turns over — a sustained stutter must stay red, and a bar that flickers grey while frames are
/// still being lost is the same defect as one that latches, pointing the other way.
// No `Eq` and no `Default`, matching [`RateMeter`] rather than [`Pacer`]: `f64` is not `Eq`, and a
// meter defaulted to a zero-second window would close on every sample and quietly report only the
// last one. `new` is the only way to build one because a window length is not a thing to default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LossMeter {
    window_seconds: f64,
    opened_at: f64,
    dropped_at_open: u64,
    /// Losses in the last window to close.
    closed: u64,
    /// Losses so far in the window still open.
    open: u64,
}

impl LossMeter {
    /// A meter forgetting losses older than `window_seconds`, opened at `now`.
    #[must_use]
    pub const fn new(window_seconds: f64, now: f64) -> Self {
        Self {
            window_seconds,
            opened_at: now,
            dropped_at_open: 0,
            closed: 0,
            open: 0,
        }
    }

    /// Offer the current time and [`Pacer::dropped`]; closes the window when it is due.
    ///
    /// Saturating on the way in and out, for the same reason [`Pacer::advance`] saturates: a
    /// suspended laptop reports a loss near [`u64::MAX`], and a readout that panicked because the
    /// machine had been asleep would be a worse outcome than a large number.
    pub const fn sample(&mut self, now: f64, dropped: u64) {
        self.open = dropped.saturating_sub(self.dropped_at_open);
        if now - self.opened_at < self.window_seconds {
            return;
        }
        self.closed = self.open;
        self.open = 0;
        self.opened_at = now;
        self.dropped_at_open = dropped;
    }

    /// Frames lost over the last window or two.
    #[must_use]
    pub const fn recent(self) -> u64 {
        self.closed.saturating_add(self.open)
    }

    /// Whether the machine is holding 50 Hz *at the moment*.
    ///
    /// This is the readout's colour, and it is a decision rather than an observation, which is
    /// why it lives here where a test can reach it rather than in the shell where it cannot.
    #[must_use]
    pub const fn keeping_up(self) -> bool {
        self.recent() < LOSS_ALARM
    }
}

/// Half-period of the widest pulse a standard loader has to resolve, in T-states.
///
/// The ROM's pilot tone, which is the slowest edge a `.tap` ever asks a loader to see.
/// `crates/spectrum/src/tape/tap.rs`'s `PILOT_PULSE` is where the figure comes from and it is
/// written out again here rather than borrowed, because that constant is `pub(super)` — one
/// crate away and two modules down — and widening a machine's public surface to hand a frontend
/// a number the frontend can state for itself would be a semver change bought for nothing. Every
/// test in this workspace that needs it writes it down too.
const WIDEST_HALF_PERIOD: u64 = 2168;

/// Reads of the `EAR` line per frame at or above which a machine counts as decoding a tape.
///
/// # Derived, not chosen, which is the only reason a number this load-bearing is allowed
///
/// A loader cannot see an edge it does not sample, so it must read the line at least **twice**
/// inside the widest half-period it has to resolve. That is the floor, and it is arithmetic:
/// `2 × frame_t_states / WIDEST_HALF_PERIOD`, which is **64** on a 48K's 69,888-T-state frame
/// and **65** on a 128's 70,908. Reading the frame length off the machine rather than writing 64
/// down is what stops one model quietly borrowing the other's arithmetic — the same move
/// `crates/frontend/tests/speed_multiplier.rs`'s `pilot_tone` makes when it sizes a cassette.
///
/// # The margin, which is what makes it safe rather than merely principled
///
/// Measured on a 48K, per frame: an idle BASIC prompt reads the port **8** times — the ROM's
/// `KEY-SCAN` walks one half-row per interrupt, so *"only a loader reads this port"* is simply
/// false and was believed here until it was measured — a running loader reads it **682** times,
/// and `LOAD ""` *waiting* for a cassette reads it **1122**. The two populations are 85 times
/// apart and the threshold sits between them with eight times' clearance below and ten times'
/// above, so nothing plausible is within an order of magnitude of it in either direction.
const fn decoding_threshold(frame_t_states: u32) -> u64 {
    2 * frame_t_states as u64 / WIDEST_HALF_PERIOD
}

/// Whether the machine is decoding a tape, from how hard it is reading the `EAR` line.
///
/// # Why a rate, and why it is measured here rather than answered by the machine
///
/// [`spectrum::Spectrum::ear_reads`] is a running total and nothing else — the machine owns the
/// fact and this owns the policy, which is the split [`RateMeter`] and [`LossMeter`] already make
/// against [`Pacer::ran`] and [`Pacer::dropped`]. What arrives here is two readings; what leaves
/// is one verdict, and [`decoding_threshold`] is the whole of what turns one into the other.
///
/// The window is **one display tick**, because that is the granularity the decision is made at:
/// [`Rung::this_tick`] is asked once a tick, so a rate averaged over anything longer would be
/// answering about a moment that has passed. It is therefore the *previous* tick's rate, which is
/// the same one-tick lag [`LossMeter`] carries and for the same reason — there is no other kind
/// of measurement of something that has not happened yet.
///
/// # A tick that ran no frames is not evidence, and is not treated as any
///
/// [`Pacer::advance`] can owe zero frames, and a rate over zero frames is not a small rate — it
/// is no reading at all. [`EarMeter::sample`] therefore returns without touching anything, and
/// the verdict from the last tick that *did* run a frame stands. Dividing by the elapsed frames
/// rather than by the elapsed ticks is the same choice: a flat-out tick runs a hundred frames and
/// a paced one runs a single frame, and the number that means the same thing in both is per-frame.
///
/// # The quiet second between blocks, which is the signal being right
///
/// Once the ROM has detected a pilot it parks at `LD-WAIT` — `PC = 0x0574`, the post-pilot delay
/// — and reads this port **zero** times per frame for about a second. So a real load is not one
/// unbroken burst: it is bursts with a second of real time between them, and the readout says
/// `auto` rather than `auto (loading)` while that second passes.
///
/// **That is correct and it is deliberately not smoothed.** The machine genuinely is not decoding
/// anything there, and pacing it at real time costs about a second a block on a cassette whose
/// blocks are tens of seconds each. Hysteresis — a hold-on window, a decaying average — would buy
/// back that second and would do it by asserting a load is in progress when the measurement says
/// otherwise, which is this module's own recurring defect: a readout answering a question nobody
/// asked. If a future measurement shows the gaps dominating a real load, the fix is a stated
/// reason and a number derived from that measurement, not a smoothing constant chosen to make the
/// figure look better.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EarMeter {
    /// Reads per frame at or above which this machine is decoding. See [`decoding_threshold`].
    threshold: u64,
    reads_at_open: u64,
    frames_at_open: u64,
    /// Reads per frame over the last tick that ran a frame.
    rate: u64,
}

impl EarMeter {
    /// A meter for a machine whose frame is `frame_t_states` long, having seen nothing yet.
    ///
    /// The frame length rather than a [`spectrum::Spectrum`], so this module keeps taking
    /// numbers instead of machines and `tests/pacing_accounting.rs` can drive it with literals
    /// the way it drives [`Pacer`]. A caller reads it from
    /// `machine.ula().clock().timing().frame_t_states()`, which is where every other frame-length
    /// consumer in this workspace reads it.
    ///
    /// A machine with no reads yet is a machine that is **not** decoding, which is what a
    /// zero rate against any positive threshold already says — so there is no start-up case and
    /// no special value. A frontend that opened at `Rung::Automatic` therefore starts paced,
    /// which is the honest answer for a machine that has not run a frame.
    #[must_use]
    pub const fn new(frame_t_states: u32) -> Self {
        Self {
            threshold: decoding_threshold(frame_t_states),
            reads_at_open: 0,
            frames_at_open: 0,
            rate: 0,
        }
    }

    /// Offer [`spectrum::Spectrum::ear_reads`] and [`spectrum::Spectrum::frames`], both as
    /// running totals; closes the window over the frames since the last call.
    ///
    /// Saturating on the way in for the same reason [`LossMeter::sample`] saturates: both
    /// counters are documented monotonic, and a frontend that met one that was not should report
    /// a machine doing nothing rather than abort — `overflow-checks` is on in this workspace's
    /// release profile, so a plain subtraction here would be a panic in a shipped window.
    pub const fn sample(&mut self, reads: u64, frames: u64) {
        let ran = frames.saturating_sub(self.frames_at_open);
        if ran == 0 {
            return;
        }
        self.rate = reads.saturating_sub(self.reads_at_open) / ran;
        self.reads_at_open = reads;
        self.frames_at_open = frames;
    }

    /// Whether the machine was decoding a tape over the last tick that ran a frame.
    ///
    /// [`Rung::this_tick`]'s argument, and the one place [`decoding_threshold`]'s number is
    /// applied — so a test can drive the rate either side of it and the shell has no second copy
    /// of the comparison to drift from.
    #[must_use]
    pub const fn decoding(self) -> bool {
        self.rate >= self.threshold
    }
}
