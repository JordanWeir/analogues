use super::types::ScenarioBuilderMode;
use crate::services::openrouter_chat::{
    SCENARIO_BLUEPRINT_MAX_AGENT_ROUNDS, SCENARIO_DETAIL_MAX_AGENT_ROUNDS,
};

#[derive(Debug, Clone)]
pub struct ScenarioBuilderConfig {
    pub model: String,
    pub mode: ScenarioBuilderMode,
    pub max_agent_rounds: usize,
    pub prompt_prefix: Option<String>,
    pub focus_scenario_key: Option<String>,
}

impl Default for ScenarioBuilderConfig {
    fn default() -> Self {
        Self {
            model: "deepseek/deepseek-v4-flash".to_string(),
            mode: ScenarioBuilderMode::Blueprint,
            max_agent_rounds: SCENARIO_BLUEPRINT_MAX_AGENT_ROUNDS,
            prompt_prefix: None,
            focus_scenario_key: None,
        }
    }
}

impl ScenarioBuilderConfig {
    pub fn blueprint() -> Self {
        Self {
            mode: ScenarioBuilderMode::Blueprint,
            max_agent_rounds: SCENARIO_BLUEPRINT_MAX_AGENT_ROUNDS,
            ..Self::default()
        }
    }

    pub fn detail() -> Self {
        Self {
            mode: ScenarioBuilderMode::Detail,
            max_agent_rounds: SCENARIO_DETAIL_MAX_AGENT_ROUNDS,
            ..Self::default()
        }
    }

    pub fn with_prompt_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prompt_prefix = Some(prefix.into());
        self
    }

    pub fn with_focus_scenario_key(mut self, scenario_key: impl Into<String>) -> Self {
        self.focus_scenario_key = Some(scenario_key.into());
        self
    }
}
