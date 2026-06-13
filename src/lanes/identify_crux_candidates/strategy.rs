use crate::agents::financial_model_explorer::FinancialModelExplorerConfig;

#[derive(Debug, Clone)]
pub enum IdentifyCruxCandidatesStrategy {
    Agent(FinancialModelExplorerConfig),
    #[cfg(test)]
    Fixture,
}

impl IdentifyCruxCandidatesStrategy {
    pub fn agent_defaults() -> Self {
        Self::Agent(FinancialModelExplorerConfig::crux_triage())
    }
}
