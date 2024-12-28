use std::time::Duration;

use serde_with::{serde_as, DurationSeconds};

#[serde_as]
#[derive(serde::Deserialize)]
pub struct BenchmarkConfig {
    #[serde_as(as = "DurationSeconds<u64>")]
    pub timeout: Duration,
    pub max_iterations: u32,
    pub multi_thread: bool,
    pub max_context_depth: usize,
    pub iteration_workers_count: usize,
}
