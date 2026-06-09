use crate::agents::{
    tool_loop_agent::{ToolLoopAgent, ToolLoopRequest},
    tools::{ToolRegistry, WebSearchConfig},
};
use crate::services::usage_snapshot::UsageSnapshot;
use async_trait::async_trait;
use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Instant};

pub type WebSearchToolConfig = WebSearchConfig;

#[derive(Clone)]
pub struct ModelRequest {
    pub model: String,
    pub preamble: String,
    pub prompt: String,
    pub json_mode: bool,
    pub metadata: BTreeMap<String, String>,
    pub web_search: Option<WebSearchToolConfig>,
    pub workspace_sqlite: Option<PathBuf>,
    pub client_tools: Option<Arc<dyn crate::services::openrouter_chat::ClientToolHandler>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    pub text: String,
    pub model: String,
    pub latency_ms: u128,
    #[serde(default)]
    pub worker_run_id: Option<i64>,
    #[serde(default)]
    pub usage: UsageSnapshot,
    #[serde(default)]
    pub client_tool_calls: u32,
    #[serde(default)]
    pub agent_rounds: usize,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

impl Default for ModelResponse {
    fn default() -> Self {
        Self {
            text: String::new(),
            model: String::new(),
            latency_ms: 0,
            worker_run_id: None,
            usage: UsageSnapshot::default(),
            client_tool_calls: 0,
            agent_rounds: 0,
            finish_reason: None,
        }
    }
}

#[async_trait]
pub trait ModelClient: Send + Sync {
    async fn complete(&self, request: ModelRequest) -> Result<ModelResponse>;
}

#[derive(Debug, Clone, Default)]
pub struct OpenRouterModelClient;

#[async_trait]
impl ModelClient for OpenRouterModelClient {
    async fn complete(&self, request: ModelRequest) -> Result<ModelResponse> {
        let started_at = Instant::now();
        let mut tools = ToolRegistry::new();
        if let Some(path) = &request.workspace_sqlite {
            tools = tools.with_sql_query(path.clone());
        }
        if let Some(web_search) = &request.web_search {
            tools = tools.with_web_search(web_search.clone());
        }
        if request
            .metadata
            .get("worker_lane")
            .is_some_and(|lane| lane == "concept_catalog_review")
        {
            tools = tools.with_concept_review_submit();
        }

        let worker_name = request
            .metadata
            .get("worker_lane")
            .cloned()
            .unwrap_or_else(|| "model_completion".to_string());
        let is_concept_review = worker_name == "concept_catalog_review";

        let response = ToolLoopAgent::default()
            .run(ToolLoopRequest {
                worker_name,
                model: request.model.clone(),
                preamble: request.preamble.clone(),
                prompt: request.prompt.clone(),
                json_mode: request.json_mode,
                tools,
                metadata: request.metadata.clone(),
                workspace_sqlite: request.workspace_sqlite.clone(),
                client_tools: request.client_tools.clone(),
                max_agent_rounds: is_concept_review
                    .then_some(crate::services::openrouter_chat::CONCEPT_REVIEW_MAX_AGENT_ROUNDS),
                submit_tool_name: is_concept_review.then_some("submit_concept_review".to_string()),
            })
            .await?;

        Ok(ModelResponse {
            text: response.text,
            model: response.model,
            latency_ms: started_at.elapsed().as_millis(),
            worker_run_id: response.worker_run_id,
            usage: response.usage,
            client_tool_calls: response.client_tool_calls,
            agent_rounds: response.agent_rounds,
            finish_reason: response.finish_reason,
        })
    }
}

impl ModelRequest {
    pub fn uses_client_tool_loop(&self) -> bool {
        self.workspace_sqlite.is_some()
    }
}

pub fn extract_json_blob(text: &str) -> Option<&str> {
    crate::services::openrouter_chat::extract_json_object(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_json_from_markdown_fence() {
        let text = "```json\n{\"decisions\":[]}\n```";
        assert_eq!(extract_json_blob(text), Some("{\"decisions\":[]}"));
    }

    #[test]
    fn serializes_web_search_tool_for_openrouter() {
        let tool = WebSearchToolConfig::concept_validation_defaults().completion_tool();
        let value = serde_json::to_value(tool).expect("tool should serialize");
        assert_eq!(value["type"], "openrouter:web_search");
        assert_eq!(value["parameters"]["engine"], "exa");
        assert_eq!(value["parameters"]["max_results"], 5);
    }

    #[test]
    fn client_tool_loop_when_workspace_attached() {
        let request = ModelRequest {
            model: "test".to_string(),
            preamble: String::new(),
            prompt: String::new(),
            json_mode: false,
            metadata: BTreeMap::new(),
            web_search: None,
            workspace_sqlite: Some(PathBuf::from("/tmp/test.sqlite")),
            client_tools: None,
        };
        assert!(request.uses_client_tool_loop());
    }

    #[test]
    fn web_search_alone_does_not_use_client_tool_loop() {
        let request = ModelRequest {
            model: "test".to_string(),
            preamble: String::new(),
            prompt: String::new(),
            json_mode: false,
            metadata: BTreeMap::new(),
            web_search: Some(WebSearchToolConfig::concept_validation_defaults()),
            workspace_sqlite: None,
            client_tools: None,
        };
        assert!(!request.uses_client_tool_loop());
    }
}
