//! Exact allocator counts for the old and current per-frame sound-depth format.
use std::fmt::Write as _;

use divan::{AllocProfiler, Bencher, black_box};
use frontend::pacing::{Speed, Tick};

#[path = "../src/queue_display.rs"]
mod queue_display;
use queue_display::QueueDisplay;

#[global_allocator]
static ALLOC: AllocProfiler = AllocProfiler::system();

fn main() {
    divan::main();
}

#[divan::bench]
fn original_queue_string() -> String {
    black_box(1024_i32).to_string()
}

#[divan::bench]
fn reused_queue_line(bencher: Bencher) {
    let mut line = String::with_capacity(128);
    bencher.bench_local(|| {
        line.clear();
        write!(
            &mut line,
            "{}",
            QueueDisplay {
                queued: black_box(1024),
                tick: Tick::Paced(Speed::REAL_TIME),
            }
        )
        .expect("String is an infallible formatter");
        black_box(&line);
    });
}
