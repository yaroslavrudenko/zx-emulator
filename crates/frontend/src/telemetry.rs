//! Opt-in, bounded timing samples for the native window; never used to pace it.

use std::time::Duration;

use page::telemetry::{self, LatencyHistogram};

const REPORT_TICKS: u64 = 250;

/// A span of wall time in seconds, as the window's clock reports it, as a latency.
fn latency(seconds: f64) -> Duration {
    Duration::try_from_secs_f64(seconds).unwrap_or(Duration::ZERO)
}

pub(super) struct Sample {
    pub(super) start: f64,
    pub(super) end: f64,
    pub(super) due: u64,
    pub(super) ran: u64,
    pub(super) dropped: u64,
    pub(super) audio_lost: u64,
}

#[derive(Default)]
pub(super) struct Telemetry {
    interval: LatencyHistogram,
    tick: LatencyHistogram,
    last_frame: Option<f64>,
    ticks: u64,
    due: u64,
    ran: u64,
    dropped: u64,
    audio_lost_at_report: u64,
}

impl Telemetry {
    pub(super) fn enabled() -> Option<Self> {
        telemetry::enabled().then(Self::default)
    }

    pub(super) fn observe(&mut self, sample: Sample) {
        if let Some(previous) = self.last_frame {
            self.interval.record(latency(sample.start - previous));
        }
        self.last_frame = Some(sample.start);
        self.tick.record(latency(sample.end - sample.start));
        self.ticks += 1;
        self.due += sample.due;
        self.ran += sample.ran;
        self.dropped += sample.dropped;
        if self.ticks.is_multiple_of(REPORT_TICKS) {
            eprintln!(
                "zx telemetry ticks={} interval {} tick {} due={} ran={} dropped={} audio_lost={}",
                self.ticks,
                self.interval.take(),
                self.tick.take(),
                self.due,
                self.ran,
                self.dropped,
                sample.audio_lost.saturating_sub(self.audio_lost_at_report),
            );
            self.due = 0;
            self.ran = 0;
            self.dropped = 0;
            self.audio_lost_at_report = sample.audio_lost;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Sample, Telemetry};
    use std::time::Duration;

    #[test]
    fn the_frame_reader_turns_clock_readings_into_intervals_and_tick_lengths() {
        let mut telemetry = Telemetry::default();
        for (index, start) in [0.0, 0.020, 0.040, 0.290].into_iter().enumerate() {
            let end = start + if index == 3 { 0.010 } else { 0.001 };
            telemetry.observe(Sample {
                start,
                end,
                due: 1,
                ran: 1,
                dropped: 0,
                audio_lost: 0,
            });
        }
        let intervals = telemetry.interval.take();
        assert_eq!(
            (intervals.samples, intervals.p50),
            (3, Duration::from_micros(19_912))
        );
        assert_eq!(intervals.p99, Duration::from_micros(246_112));
        assert_eq!(telemetry.tick.take().max, Duration::from_millis(10));
    }
}
