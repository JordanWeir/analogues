use crate::services::openrouter_chat::CONCEPT_REVIEW_MAX_AGENT_ROUNDS;

#[derive(Debug, Clone)]
pub struct FundamentalCatalogManagerConfig {
    pub model: String,
    pub enable_web_search: bool,
    pub max_agent_rounds: usize,
}

impl Default for FundamentalCatalogManagerConfig {
    fn default() -> Self {
        Self {
            model: "deepseek/deepseek-v4-flash".to_string(),
            enable_web_search: web_search_enabled_from_env(),
            max_agent_rounds: CONCEPT_REVIEW_MAX_AGENT_ROUNDS,
        }
    }
}

fn web_search_enabled_from_env() -> bool {
    std::env::var("CONCEPT_REVIEW_WEB_SEARCH")
        .ok()
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes"))
}
