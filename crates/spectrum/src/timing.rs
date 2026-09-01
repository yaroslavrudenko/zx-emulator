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
//! The table is small; the *phase* is the work, and the phase is one constant:
//! [`FIRST_CONTENDED_T_STATE`].
//!
//! # What is not verified
//!
//! [`FIRST_CONTENDED_T_STATE`] is the value the emulator community reports for an issue 3
//! 48K, and **this crate has no oracle for it.** An issue 2 machine is one T-state earlier.
//! Off-by-one here does not fail anything; it makes multicolour effects land one character
//! cell out. It is a single named constant precisely so that a future timing-test program
//! — `MACHINE.md`'s verification item 2, the only real oracle available for this — has one
//! place to correct.

use crate::screen::DISPLAY_HEIGHT;

/// T-states in one display line, border and flyback included.
pub const T_STATES_PER_LINE: u32 = 224;

/// Display lines in one frame, top and bottom border and vertical flyback included.
pub const LINES_PER_FRAME: u32 = 312;

/// T-states in one 50 Hz frame of a 48K.
///
/// A 128 runs 70908, which is why this is a constant to be moved rather than a literal
/// sprinkled through the frame loop.
pub const T_STATES_PER_FRAME: u32 = T_STATES_PER_LINE * LINES_PER_FRAME;

const _: () = assert!(T_STATES_PER_FRAME == 69888);

/// How long the ULA holds `/INT` low at the start of each frame, on a 48K.
///
/// The interrupt is not an instant. A CPU with interrupts disabled for longer than this
/// misses the frame entirely, which is a real effect and the reason this is a window
/// rather than a single moment.
pub const INTERRUPT_T_STATES: u32 = 32;

/// The first T-state of the frame at which a contended access is delayed.
///
/// See the module documentation: unverified, and deliberately a single constant.
pub const FIRST_CONTENDED_T_STATE: u32 = 14335;

/// T-states per line during which the ULA is fetching, and therefore contending.
///
/// 128 of the 224: 256 pixels at two pixels per T-state. The remaining 96 are the two
/// borders and the horizontal flyback.
const CONTENDED_T_STATES_PER_LINE: u32 = 128;

/// T-states from [`FIRST_CONTENDED_T_STATE`] to the end of the last display line.
const CONTENDED_SPAN: u32 = DISPLAY_HEIGHT as u32 * T_STATES_PER_LINE;

/// The stall, in T-states, for each position within an eight-T-state ULA group.
const DELAY_PATTERN: [u32; 8] = [6, 5, 4, 3, 2, 1, 0, 0];

const _: () = assert!(DELAY_PATTERN.len().is_power_of_two());

/// The stall a contended access starting at `frame_t_state` suffers, in T-states.
///
/// Zero outside the display's fetch window — during the borders, the flyback, and the
/// whole of the top and bottom border areas — which is the majority of a frame.
#[inline]
#[must_use]
pub const fn delay(frame_t_state: u32) -> u32 {
    if frame_t_state < FIRST_CONTENDED_T_STATE {
        return 0;
    }
    let since_first = frame_t_state - FIRST_CONTENDED_T_STATE;
    if since_first >= CONTENDED_SPAN {
        return 0;
    }
    let column = since_first % T_STATES_PER_LINE;
    if column >= CONTENDED_T_STATES_PER_LINE {
        return 0;
    }
    // INVARIANT: masked by the pattern's length, which is asserted to be a power of two —
    // so this indexes in range and the bounds check is elided.
    DELAY_PATTERN[(column & (DELAY_PATTERN.len() as u32 - 1)) as usize]
}

/// Where the machine is in the current frame, and how many frames have completed.
///
/// Frame-relative rather than absolute because everything that consults the clock —
/// contention, the interrupt window — is a function of position within a frame. The frame
/// count is what a caller uses to notice a frame boundary, and it is what makes
/// `MACHINE.md` Decision 2 a non-event: a single step that overruns the budget simply
/// lands in the next frame and increments this, rather than needing to stop on 69888.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Clock {
    frame_t_state: u32,
    frames: u64,
}

impl Clock {
    /// A clock at the start of frame zero.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            frame_t_state: 0,
            frames: 0,
        }
    }

    /// Advance by `t_states`, rolling over into the next frame as many times as needed.
    ///
    /// A loop rather than a single subtraction: nothing here bounds a caller's step to one
    /// frame, and a clock that silently ran a frame behind would be invisible.
    #[inline]
    pub fn advance(&mut self, t_states: u32) {
        self.frame_t_state += t_states;
        while self.frame_t_state >= T_STATES_PER_FRAME {
            self.frame_t_state -= T_STATES_PER_FRAME;
            self.frames += 1;
        }
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

    /// The frame-relative position `offset` T-states from now.
    ///
    /// Used to price a stall that will happen partway through a machine cycle without
    /// actually moving the clock there first.
    #[inline]
    #[must_use]
    pub const fn ahead(&self, offset: u32) -> u32 {
        (self.frame_t_state + offset) % T_STATES_PER_FRAME
    }

    /// Whether the ULA is holding `/INT` low right now.
    #[inline]
    #[must_use]
    pub const fn interrupt_asserted(&self) -> bool {
        self.frame_t_state < INTERRUPT_T_STATES
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_is_the_published_number_of_t_states() {
        assert_eq!(T_STATES_PER_FRAME, 69888);
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
    fn the_contended_span_is_the_expected_share_of_a_frame() {
        let contended: u32 = (0..T_STATES_PER_FRAME).filter(|&t| delay(t) > 0).count() as u32;
        // Six of every eight T-states stall, over 128 T-states of each of 192 lines.
        assert_eq!(contended, 6 * 16 * DISPLAY_HEIGHT as u32);
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
    fn ahead_wraps_within_the_frame() {
        let mut clock = Clock::new();
        clock.advance(T_STATES_PER_FRAME - 2);
        assert_eq!(clock.ahead(1), T_STATES_PER_FRAME - 1);
        assert_eq!(clock.ahead(3), 1);
    }
}
