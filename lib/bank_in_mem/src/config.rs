use ruse_synthesizer::bank::BankConfig;

#[derive(Default, Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SubsumptionBankConfig {}

impl BankConfig for SubsumptionBankConfig {}
