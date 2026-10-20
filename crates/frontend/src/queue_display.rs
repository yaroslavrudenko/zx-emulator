//! The window's three-state sound depth, formatted without owning a new string.
use std::fmt;

use frontend::pacing::{Speed, Tick};

pub(super) const NO_DEVICE: &str = "--";
pub(super) const MUTED: &str = "mute";

#[derive(Clone, Copy)]
pub(super) struct QueueDisplay {
    pub(super) queued: i32,
    pub(super) tick: Tick,
}

impl fmt::Display for QueueDisplay {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.tick != Tick::Paced(Speed::REAL_TIME) {
            formatter.write_str(MUTED)
        } else if self.queued < 0 {
            formatter.write_str(NO_DEVICE)
        } else {
            write!(formatter, "{}", self.queued)
        }
    }
}
