use crate::services::openrouter_chat::FINANCIAL_MECHANICS_VALIDATOR_MAX_AGENT_ROUNDS;

#[derive(Debug, Clone)]
pub struct FinancialMechanicsValidatorConfig {
    pub model: String,
    pub max_agent_rounds: usize,
    pub prompt_prefix: Option<String>,
    pub focus_crux_key: Option<String>,
    pub scout_worker: bool,
    pub review_round: i64,
}

impl Default for FinancialMechanicsValidatorConfig {
    fn default() -> Self {
        Self {
            model: "deepseek/deepseek-v4-flash".to_string(),
            max_agent_rounds: FINANCIAL_MECHANICS_VALIDATOR_MAX_AGENT_ROUNDS,
            prompt_prefix: None,
            focus_crux_key: None,
            scout_worker: false,
            review_round: 1,
        }
    }
}

impl FinancialMechanicsValidatorConfig {
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

    pub fn with_review_round(mut self, review_round: i64) -> Self {
        self.review_round = review_round;
        self
    }
}
