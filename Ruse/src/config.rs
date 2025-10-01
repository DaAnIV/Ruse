use std::{path::PathBuf, time::Duration};

use byte_unit::Byte;
use clap::{Command, CommandFactory, Parser};
use ruse_bank_in_mem::args::SubsumptionBankArgs;
use ruse_synthesizer::synthesizer::SynthesizerOptions;
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
    fn help_template() -> &'static str {
        "\
{tab}{name}:
{tab}{tab}{options}"
    }

    fn bank_commands() -> Vec<Command> {
        vec![SubsumptionBankArgs::command().help_template(Self::help_template())]
    }

    pub fn help() -> String {
        let mut help = String::new();
        help.push_str("--bank-arg <arg>=<value>\n");

        for mut command in Self::bank_commands() {
            help.push_str(&command.render_help().to_string());
        }

        help
    }
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
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct BenchmarkConfig {
    #[serde_as(as = "DurationSeconds<u64>")]
    pub timeout: Duration,
    pub max_iterations: u32,
    pub multi_thread: bool,
    pub max_mutations: u32,
    pub iteration_workers_count: usize,
    pub max_sequence_size: usize,
    pub benchmarks: Vec<PathBuf>,
    pub max_task_mem: Byte,
    pub bank_config: BankConfig,
    #[serde(skip)]
    pub dry_run: bool,
    pub fork: bool,
    pub sleep: Option<Duration>,
}

impl BenchmarkConfig {
    pub fn get_synthesizer_options(&self) -> SynthesizerOptions {
        SynthesizerOptions {
            worker_count: self.iteration_workers_count,
            max_mutations: self.max_mutations,
        }
    }
}
