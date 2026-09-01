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

use core::time::Duration;

/// Nanoseconds in one frame of a 50 Hz machine.
pub const FRAME_NANOS: u128 = 20_000_000;

/// The most frames one call will run before declaring the remainder lost.
///
/// Four frames is 80 ms of emulated time. The bound exists so that a stall cannot compound:
/// past this point the loop is not going to catch up within a tick, and attempting it turns a
/// hiccup into a freeze while the backlog keeps growing. Dropping instead is a decision to
/// lose emulated time, which is why the drop is counted and shown rather than absorbed.
pub const MAX_CATCH_UP: u64 = 4;

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
}

impl Pacer {
    /// A pacer owing nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            owed: 0,
            ran: 0,
            dropped: 0,
        }
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
    pub const fn advance(&mut self, elapsed: Duration) -> u64 {
        self.owed += elapsed.as_nanos();
        let whole = self.owed / FRAME_NANOS;
        self.owed -= whole * FRAME_NANOS;

        let due = if whole > u64::MAX as u128 {
            u64::MAX
        } else {
            whole as u64
        };
        let run = if due < MAX_CATCH_UP {
            due
        } else {
            MAX_CATCH_UP
        };

        self.ran += run;
        self.dropped += due - run;
        run
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
