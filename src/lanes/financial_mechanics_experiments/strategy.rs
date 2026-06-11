use crate::agents::financial_model_explorer::FinancialModelExplorerConfig;

#[derive(Debug, Clone)]
pub enum FinancialMechanicsExperimentsStrategy {
    Agent(FinancialModelExplorerConfig),
    #[cfg(test)]
    Fixture,
}

impl FinancialMechanicsExperimentsStrategy {
    pub fn agent_defaults() -> Self {
        Self::Agent(FinancialModelExplorerConfig::mechanics_experiment())
    }
}
