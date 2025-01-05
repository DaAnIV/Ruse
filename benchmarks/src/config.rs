use core::fmt;
use std::{path::PathBuf, time::Duration};

use byte_unit::Byte;
use serde_with::{serde_as, DurationSeconds};

#[derive(Copy, Clone, serde::Deserialize, serde::Serialize, Debug, clap::ValueEnum, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BankType {
    SubsumptionBank
}

impl fmt::Display for BankType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BankType::SubsumptionBank => write!(f, "subsumption-bank"),
        }
    }
}

#[serde_as]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct BenchmarkConfig {
    #[serde_as(as = "DurationSeconds<u64>")]
    pub timeout: Duration,
    pub max_iterations: u32,
    pub multi_thread: bool,
    pub max_context_depth: usize,
    pub iteration_workers_count: usize,
    pub benchmarks: Vec<PathBuf>,
    pub max_task_mem: Byte,
    pub bank_type: BankType
}
