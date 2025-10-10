use crate::config::SubsumptionBankConfig;

#[derive(clap::Parser, Clone, Debug)]
#[command(
    name = "Subsumption Bank Options",
    no_binary_name = true,
    override_usage = "--bank-arg <arg>=<value>",
    disable_help_flag = true
)]
pub struct SubsumptionBankArgs {}

impl Into<SubsumptionBankConfig> for SubsumptionBankArgs {
    fn into(self) -> SubsumptionBankConfig {
        SubsumptionBankConfig {}
    }
}
