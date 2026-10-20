//! Opt-in latency telemetry, shared by the desktop audio callback and the window's frame loop.
//!
//! **One `ZX_TELEMETRY` reader and one histogram**, where there were two of each: this crate
//! timed the audio callback in integer microseconds over one bucket layout, `crates/frontend`
//! timed frame intervals in floating-point milliseconds over another, and each carried its own
//! percentile walk and its own copy of the same "p99 reveals 250 ms stalls" test (DECISION.md
//! item 7b). It lives here because `frontend` already depends on this crate and the audio
//! callback cannot reach `frontend`; the file is safe code — the `forbid` below is what keeps
//! this crate's `unsafe` confined to `lib.rs`, where `tests/unsafe_inventory.rs` counts it.
//!
//! # The layout, and what moved when the two merged
//!
//! 1,024 buckets: 1 µs wide to 512 µs, 100 µs wide to 26.1 ms, then 20 ms wide to 5.13 s, and
//! everything longer in the last bucket. A percentile reports its bucket's **lower bound**; the
//! maximum is kept beside the buckets to the microsecond (a 10.5 µs sample reads 10 µs). Against
//! the two layouts this replaced (the judges' layout ruling, DECISION.md item 7b):
//! - **the audio callback** keeps its old layout to 26.1 ms and gains range: its 1 ms tail
//!   stopped at 281 ms, this 20 ms one at 5.13 s — so a 250 ms stall reads 246.1 ms where it
//!   read 249.1;
//! - **the frame loop** had 0.1 ms buckets from zero to 51.2 ms and 10 ms to 5.16 s. It keeps
//!   0.1 ms to 26.1 ms — every interval a 50, 60 or 120 Hz display produces — and is coarser
//!   above it; a 20 ms interval reads 19.912 ms where it read 20.0, because these buckets
//!   start at 512 µs rather than at zero.
//!
//! Nothing here paces anything; it is printed and read by a person.
#![forbid(unsafe_code)]

use std::fmt;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

const BUCKETS: usize = 1024;
const FINE_BUCKETS: usize = 512;
const MEDIUM_BUCKETS: usize = 256;
const MEDIUM_US: u64 = 100;
const COARSE_US: u64 = 20_000;
const MEDIUM_START_US: u64 = FINE_BUCKETS as u64;
const COARSE_START_US: u64 = MEDIUM_START_US + MEDIUM_BUCKETS as u64 * MEDIUM_US;
const COARSE_BUCKETS: u64 = (BUCKETS - FINE_BUCKETS - MEDIUM_BUCKETS) as u64;
const _: () = assert!(COARSE_START_US + (COARSE_BUCKETS - 1) * COARSE_US >= 5_000_000);

/// Whether `ZX_TELEMETRY` is set — the one place it is read, once per process.
#[must_use]
pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("ZX_TELEMETRY").is_some())
}

/// The bucket a latency of `micros` falls in.
fn bucket(micros: u64) -> usize {
    let index = if micros < MEDIUM_START_US {
        micros
    } else if micros < COARSE_START_US {
        FINE_BUCKETS as u64 + (micros - MEDIUM_START_US) / MEDIUM_US
    } else {
        let coarse = ((micros - COARSE_START_US) / COARSE_US).min(COARSE_BUCKETS - 1);
        (FINE_BUCKETS + MEDIUM_BUCKETS) as u64 + coarse
    };
    index as usize
}

/// The smallest latency, in microseconds, that lands in `bucket`.
fn lower_bound(bucket: usize) -> u64 {
    let index = bucket as u64;
    if bucket < FINE_BUCKETS {
        index
    } else if bucket < FINE_BUCKETS + MEDIUM_BUCKETS {
        MEDIUM_START_US + (index - FINE_BUCKETS as u64) * MEDIUM_US
    } else {
        COARSE_START_US + (index - (FINE_BUCKETS + MEDIUM_BUCKETS) as u64) * COARSE_US
    }
}

/// The lower bound of the bucket holding the `percent`th of `samples` latencies — the one
/// percentile walk both instruments use.
fn percentile(counts: &[u64; BUCKETS], samples: u64, percent: u64) -> Duration {
    let target = samples.saturating_mul(percent).div_ceil(100);
    let mut cumulative = counts.iter().scan(0, |seen, &count| {
        *seen += count;
        Some(*seen)
    });
    let found = cumulative.position(|seen| seen >= target);
    Duration::from_micros(found.map_or(0, lower_bound))
}

/// A latency histogram one thread records into and another takes from.
///
/// Relaxed atomics: the counts are observational, not synchronisation, so a summary taken while
/// a sample lands may count it in this period or the next. The maximum is swapped separately
/// from the counts, so such a sample can put its count in one period and its maximum in the
/// other; [`take`](Self::take) narrows that split — see there. Recording neither locks nor
/// allocates, which is what lets the audio callback use it.
#[derive(Debug)]
pub struct LatencyHistogram {
    counts: [AtomicU64; BUCKETS],
    max_us: AtomicU64,
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        Self::new()
    }
}

impl LatencyHistogram {
    /// An empty histogram.
    #[must_use]
    pub fn new() -> Self {
        Self {
            counts: std::array::from_fn(|_| AtomicU64::new(0)),
            max_us: AtomicU64::new(0),
        }
    }

    /// Count one latency.
    pub fn record(&self, latency: Duration) {
        let micros = u64::try_from(latency.as_micros()).unwrap_or(u64::MAX);
        self.counts[bucket(micros)].fetch_add(1, Ordering::Relaxed);
        self.max_us.fetch_max(micros, Ordering::Relaxed);
    }

    /// Summarise everything recorded since the last `take`, and start the next period empty.
    ///
    /// Two guards for a sample that lands mid-`take`: a period with no samples reports no maximum
    /// and leaves it for the take that counts its sample, and a maximum is never below the highest
    /// bucket counted. A split is still possible — a sample counted here whose maximum went to an
    /// earlier period, or a later sample's maximum reported here — so a summary's `max` can belong
    /// to a neighbouring period; it is never zero beside samples, nor below its own p99.
    #[must_use]
    pub fn take(&self) -> Summary {
        let counts: [u64; BUCKETS] =
            std::array::from_fn(|index| self.counts[index].swap(0, Ordering::Relaxed));
        let samples = counts.iter().sum();
        let max_us = match counts.iter().rposition(|&count| count > 0) {
            Some(top) => lower_bound(top).max(self.max_us.swap(0, Ordering::Relaxed)),
            None => 0,
        };
        let at = |percent| percentile(&counts, samples, percent);
        Summary {
            samples,
            p50: at(50),
            p95: at(95),
            p99: at(99),
            max: Duration::from_micros(max_us),
        }
    }
}

/// One period of a [`LatencyHistogram`]: percentiles at their bucket's lower bound, the
/// maximum to the microsecond.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    /// Latencies recorded in the period.
    pub samples: u64,
    /// The median.
    pub p50: Duration,
    /// The 95th percentile.
    pub p95: Duration,
    /// The 99th percentile.
    pub p99: Duration,
    /// The longest latency, to the microsecond.
    pub max: Duration,
}

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ms = |latency: Duration| latency.as_secs_f64() * 1000.0;
        write!(
            f,
            "n={} p50={:.3}ms p95={:.3}ms p99={:.3}ms max={:.3}ms",
            self.samples,
            ms(self.p50),
            ms(self.p95),
            ms(self.p99),
            ms(self.max),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{LatencyHistogram, bucket};
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    /// The clamp in `bucket` is all that stands between a resumed-from-sleep frame interval and
    /// an out-of-bounds index on the audio callback; ten seconds is twice the layout's range.
    #[test]
    fn a_latency_past_the_last_bucket_is_counted_in_it() {
        let histogram = LatencyHistogram::new();
        histogram.record(Duration::from_secs(10));
        let summary = histogram.take();
        assert_eq!(
            summary.p99,
            Duration::from_micros(5_126_112),
            "the last bucket's floor"
        );
        assert_eq!(summary.max, Duration::from_secs(10));
    }

    /// A sample recorded across a `take` — written here in the two halves `record` does — does
    /// not show as a maximum without a count, nor as a count under a maximum below its bucket.
    #[test]
    fn a_take_reports_no_maximum_without_samples_and_none_below_the_top_bucket() {
        let histogram = LatencyHistogram::new();
        histogram.max_us.store(7_000, Ordering::Relaxed);
        assert_eq!(
            histogram.take().max,
            Duration::ZERO,
            "no samples, no maximum"
        );
        histogram.counts[bucket(7_000)].fetch_add(1, Ordering::Relaxed);
        let summary = histogram.take();
        assert_eq!(
            (summary.samples, summary.max),
            (1, Duration::from_millis(7))
        );
        histogram.counts[bucket(7_000)].fetch_add(1, Ordering::Relaxed);
        let summary = histogram.take();
        assert_eq!(
            summary.max,
            Duration::from_micros(6_912),
            "never below its own bucket"
        );
    }

    #[test]
    fn p99_reveals_recurring_250ms_stalls_and_max_stays_exact() {
        let histogram = LatencyHistogram::new();
        (0..98).for_each(|_| histogram.record(Duration::from_millis(20)));
        (0..2).for_each(|_| histogram.record(Duration::from_millis(250)));
        let summary = histogram.take();
        assert_eq!(summary.samples, 100);
        assert_eq!(summary.p50, Duration::from_micros(19_912));
        assert_eq!(summary.p99, Duration::from_micros(246_112));
        assert_eq!(summary.max, Duration::from_millis(250));
        assert_eq!(
            histogram.take().samples,
            0,
            "a take starts the next period empty"
        );
    }
}
