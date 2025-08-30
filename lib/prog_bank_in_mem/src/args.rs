use std::str::FromStr;

use ruse_synthesizer::bank_hasher::BankHasherBuilder;

use crate::config::SubsumptionBankConfig;

#[derive(Clone, Copy, Debug)]
struct BankKeys(u64, u64);

impl FromStr for BankKeys {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(',');

        let first = parts.next().unwrap().to_owned();
        let second = parts.next().map(|x| x.to_owned());
        if parts.next().is_some() {
            return Err(anyhow::Error::msg("Value contains more then two ','"));
        }

        let k0: u64 = first.parse()?;
        let k1: u64 = if let Some(next) = second {
            next.parse()?
        } else {
            0
        };

        Ok(Self(k0, k1))
    }
}

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
                .map(|keys| BankHasherBuilder::new_with_keys(keys.0, keys.1))
                .unwrap_or(BankHasherBuilder::new_with_random_keys()),
        }
    }
}
