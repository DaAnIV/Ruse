use ruse_synthesizer::bank_hasher::{BankHasherBuilder, BankKeys};

use crate::config::SubsumptionBankConfig;

#[derive(clap::Parser, Clone, Debug)]
#[command(
    name = "subsumption-bank args",
    no_binary_name = true,
    override_usage = "--bank-arg <arg>=<value>"
)]
pub struct SubsumptionBankArgs {
    #[arg(long, value_parser = clap::value_parser!(BankKeys))]
    bank_keys: Option<BankKeys>,
}

impl Into<SubsumptionBankConfig> for SubsumptionBankArgs {
    fn into(self) -> SubsumptionBankConfig {
        SubsumptionBankConfig {
            hash_builder: self
                .bank_keys
                .map(|keys| BankHasherBuilder::new_with_keys(keys))
                .unwrap_or(BankHasherBuilder::new_with_random_keys()),
        }
    }
}
