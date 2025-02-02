use core::fmt;
use std::{path::PathBuf, time::Duration};

use byte_unit::Byte;
use ruse_synthesizer::bank_hasher::BankHasherBuilder;
use serde_with::{serde_as, DurationSeconds};

#[derive(
    Copy, Clone, serde::Deserialize, serde::Serialize, Debug, clap::ValueEnum, PartialEq, Eq,
)]
#[serde(rename_all = "kebab-case")]
pub enum BankType {
    SubsumptionBank,
}

impl fmt::Display for BankType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BankType::SubsumptionBank => write!(f, "subsumption-bank"),
        }
    }
}

impl Into<ruse_task_parser::BankType> for BankType {
    fn into(self) -> ruse_task_parser::BankType {
        match self {
            BankType::SubsumptionBank => ruse_task_parser::BankType::SubsumptionBank,
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
    pub bank_type: BankType,
    pub bank_hash_builder: BankHasherBuilder,
}
