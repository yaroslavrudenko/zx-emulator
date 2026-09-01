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
