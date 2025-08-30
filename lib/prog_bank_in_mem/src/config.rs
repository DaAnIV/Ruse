use ruse_synthesizer::{bank::BankConfig, bank_hasher::BankHasherBuilder};

#[derive(Default, Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SubsumptionBankConfig {
    pub hash_builder: BankHasherBuilder,
}

impl BankConfig for SubsumptionBankConfig {}
