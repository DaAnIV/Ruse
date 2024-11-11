use std::time::Duration;

use serde_with::{serde_as, DurationSeconds};

#[serde_as]
#[derive(serde::Deserialize)]
pub struct BenchmarkConfig {
    #[serde_as(as = "DurationSeconds<u64>")]
    pub timeout: Duration,
    pub max_iterations: u32,
    pub multi_thread: bool,
}

impl std::fmt::Display for BenchmarkConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Timeout {:.3} seconds", self.timeout.as_secs_f32())?;
        writeln!(f, "Max iterations {}", self.max_iterations)?;

        Ok(())
    }
}
