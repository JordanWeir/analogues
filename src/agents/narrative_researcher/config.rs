use crate::services::openrouter_chat::NARRATIVE_RESEARCH_MAX_AGENT_ROUNDS;

#[derive(Debug, Clone)]
pub struct NarrativeResearcherConfig {
    pub model: String,
    pub enable_web_search: bool,
    pub max_agent_rounds: usize,
}

impl Default for NarrativeResearcherConfig {
    fn default() -> Self {
        Self {
            model: "deepseek/deepseek-v4-flash".to_string(),
            enable_web_search: true,
            max_agent_rounds: NARRATIVE_RESEARCH_MAX_AGENT_ROUNDS,
        }
    }
}
