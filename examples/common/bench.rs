//! Timing helpers shared by the benchmark examples.
use std::time::{Duration, Instant};

/// The fastest of `runs` timings of `f`.
pub fn best(runs: usize, mut f: impl FnMut()) -> Duration {
    (0..runs)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed()
        })
        .min()
        .unwrap()
}
