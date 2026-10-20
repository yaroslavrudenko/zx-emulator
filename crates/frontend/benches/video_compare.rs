//! Measurement-only equality cost for a declined GPU-upload guard. The running window
//! currently uploads every frame; this benchmark records the candidate's 80 KB comparison
//! cost, not work paid by the production renderer.
//!
//! *(2026-10-20: the window now renders and uploads only on a display tick where an emulated
//! frame ran — `Pacer::ran` moved — which skips the redundant upload without paying this
//! comparison; DECISION.md item 6. The guard stays refused and this bench is its record.)*
use divan::{Bencher, black_box};
use spectrum::Frame;

fn main() {
    divan::main();
}

#[divan::bench]
fn equal_frames(bencher: Bencher) {
    bencher
        .with_inputs(|| (Frame::new(), Frame::new()))
        .bench_local_values(|(current, previous)| black_box(current == previous));
}
