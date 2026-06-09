use crate::services::openrouter_chat::{web_search_server_tool, CompletionTool};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebSearchConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_results: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_total_results: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excluded_domains: Option<Vec<String>>,
}

impl WebSearchConfig {
    pub fn concept_validation_defaults() -> Self {
        Self {
            engine: Some("exa".to_string()),
            max_results: Some(5),
            max_total_results: Some(15),
            search_context_size: Some("medium".to_string()),
            allowed_domains: None,
            excluded_domains: Some(vec!["reddit.com".to_string(), "stocktwits.com".to_string()]),
        }
    }

    pub fn completion_tool(&self) -> CompletionTool {
        let mut parameters = Map::new();
        if let Some(engine) = &self.engine {
            parameters.insert("engine".to_string(), json!(engine));
        }
        if let Some(max_results) = self.max_results {
            parameters.insert("max_results".to_string(), json!(max_results));
        }
        if let Some(max_total_results) = self.max_total_results {
            parameters.insert("max_total_results".to_string(), json!(max_total_results));
        }
        if let Some(search_context_size) = &self.search_context_size {
            parameters.insert(
                "search_context_size".to_string(),
                json!(search_context_size),
            );
        }
        if let Some(allowed_domains) = &self.allowed_domains {
            parameters.insert("allowed_domains".to_string(), json!(allowed_domains));
        }
        if let Some(excluded_domains) = &self.excluded_domains {
            parameters.insert("excluded_domains".to_string(), json!(excluded_domains));
        }

        web_search_server_tool(Value::Object(parameters))
    }
}
