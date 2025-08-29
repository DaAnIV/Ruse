use ruse_synthesizer::{
    bank::ProgBank,
    subsumption_bank::SubsumptionProgBank,
    bank_hasher::BankHasherBuilder,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BankType {
    SubsumptionBank,
}

pub struct BankConfig {
    pub bank_type: BankType,
    pub hash_builder: BankHasherBuilder,
}

impl BankConfig {
    pub(crate) fn new_bank(&self) -> impl ProgBank {
        match self.bank_type {
            BankType::SubsumptionBank => SubsumptionProgBank::new_with_hasher(self.hash_builder),
        }
    }
}
