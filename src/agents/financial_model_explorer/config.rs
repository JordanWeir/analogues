use super::types::ExplorerMode;
use crate::services::openrouter_chat::{
    FINANCIAL_EXPLORER_MAX_AGENT_ROUNDS, FINANCIAL_MECHANICS_MAX_AGENT_ROUNDS,
};

#[derive(Debug, Clone)]
pub struct FinancialModelExplorerConfig {
    pub model: String,
    pub mode: ExplorerMode,
    pub max_agent_rounds: usize,
    pub prompt_prefix: Option<String>,
    /// Per-crux fan-out scope for draft-hygiene prepare steps and validation hints.
    pub focus_crux_key: Option<String>,
    /// Mechanics scout worker (covers cruxes lacking promoted experiments).
    pub scout_worker: bool,
}

impl Default for FinancialModelExplorerConfig {
    fn default() -> Self {
        Self {
            model: "deepseek/deepseek-v4-flash".to_string(),
            mode: ExplorerMode::CruxTriage,
            max_agent_rounds: FINANCIAL_EXPLORER_MAX_AGENT_ROUNDS,
            prompt_prefix: None,
            focus_crux_key: None,
            scout_worker: false,
        }
    }
}

impl FinancialModelExplorerConfig {
    pub fn crux_triage() -> Self {
        Self {
            mode: ExplorerMode::CruxTriage,
            ..Self::default()
        }
    }

    pub fn mechanics_experiment() -> Self {
        Self {
            mode: ExplorerMode::MechanicsExperiment,
            max_agent_rounds: FINANCIAL_MECHANICS_MAX_AGENT_ROUNDS,
            ..Self::default()
        }
    }

    pub fn with_prompt_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prompt_prefix = Some(prefix.into());
        self
    }

    pub fn with_focus_crux_key(mut self, crux_key: impl Into<String>) -> Self {
        self.focus_crux_key = Some(crux_key.into());
        self
    }

    pub fn with_scout_worker(mut self) -> Self {
        self.scout_worker = true;
        self
    }
}
