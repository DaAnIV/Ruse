use std::fmt;

use ruse_prog_bank_in_mem::{config::SubsumptionBankConfig, subsumption_bank::SubsumptionProgBank};
use ruse_synthesizer::bank::ProgBank;

#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum BankType {
    SubsumptionBank,
}

impl fmt::Display for BankType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum BankConfig {
    SubsumptionBank(SubsumptionBankConfig),
}

pub enum Bank {
    SubsumptionBank(SubsumptionProgBank),
}

impl BankConfig {
    pub fn new_bank(&self) -> Bank {
        match self {
            BankConfig::SubsumptionBank(config) => {
                Bank::SubsumptionBank(SubsumptionProgBank::new_with_config(config.clone()))
            }
        }
    }
}

impl fmt::Display for BankConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
