use std::{path::PathBuf, time::Duration};

use byte_unit::Byte;
use clap::Parser;
use ruse_prog_bank_in_mem::args::SubsumptionBankArgs;
use ruse_task_parser::BankConfig;
use serde_with::{serde_as, DurationSeconds};

#[derive(
    Copy, Clone, serde::Deserialize, serde::Serialize, Debug, clap::ValueEnum, PartialEq, Eq,
)]
#[serde(rename_all = "kebab-case")]
pub enum BankTypeArg {
    SubsumptionBank,
}

impl Default for BankTypeArg {
    fn default() -> Self {
        BankTypeArg::SubsumptionBank
    }
}

impl std::fmt::Display for BankTypeArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BankTypeArg::SubsumptionBank => write!(f, "subsumption-bank"),
        }
    }
}

#[derive(Clone, Debug)]
pub enum BankArgs {
    SubsumptionBankArgs(SubsumptionBankArgs),
}

impl BankArgs {
    pub fn parse_by_bank_type(bank_type: BankTypeArg, bank_args: &[String]) -> Self {
        match bank_type {
            BankTypeArg::SubsumptionBank => {
                BankArgs::SubsumptionBankArgs(SubsumptionBankArgs::parse_from(bank_args.iter()))
            }
        }
    }
}

impl Into<BankConfig> for BankArgs {
    fn into(self) -> BankConfig {
        match self {
            BankArgs::SubsumptionBankArgs(args) => BankConfig::SubsumptionBank(args.into()),
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
    pub max_sequence_size: usize,
    pub benchmarks: Vec<PathBuf>,
    pub max_task_mem: Byte,
    pub bank_config: BankConfig,
    #[serde(skip)]
    pub dry_run: bool,
}
