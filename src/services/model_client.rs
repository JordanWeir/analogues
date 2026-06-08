use crate::services::{
    agent_tools::{workspace_agent_tools, workspace_sql_openrouter_tool},
    openrouter_chat::{
        run_chat_completion, ChatCompletionOptions, ChatCompletionResult, ClientToolHandler,
    },
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use rig::{
    client::{CompletionClient, ProviderClient},
    completion::Prompt,
    providers::openrouter,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Instant};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebSearchToolConfig {
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

impl WebSearchToolConfig {
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

    fn as_openrouter_tool(&self) -> serde_json::Value {
        let mut parameters = serde_json::Map::new();
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

        json!({
            "type": "openrouter:web_search",
            "parameters": serde_json::Value::Object(parameters),
        })
    }
}

#[derive(Clone)]
pub struct ModelRequest {
    pub model: String,
    pub preamble: String,
    pub prompt: String,
    pub json_mode: bool,
    pub metadata: BTreeMap<String, String>,
    pub web_search: Option<WebSearchToolConfig>,
    pub workspace_sqlite: Option<PathBuf>,
    pub client_tools: Option<Arc<dyn ClientToolHandler>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelResponse {
    pub text: String,
    pub model: String,
    pub latency_ms: u128,
    #[serde(default)]
    pub web_search_requests: u32,
    #[serde(default)]
    pub client_tool_calls: u32,
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
    #[serde(default)]
    pub agent_rounds: usize,
    #[serde(default)]
    pub finish_reason: Option<String>,
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
        if request.uses_agent_loop() {
            return complete_with_openrouter_agent(&request).await;
        }

        let client = openrouter::Client::from_env()
            .map_err(|err| Error::string(&format!("failed to create OpenRouter client: {err}")))?;
        let agent = client
            .agent(&request.model)
            .preamble(&request.preamble)
            .build();
        let started_at = Instant::now();
        let text = agent
            .prompt(&request.prompt)
            .await
            .map_err(|err| Error::string(&format!("model completion failed: {err}")))?;

        Ok(ModelResponse {
            text,
            model: request.model,
            latency_ms: started_at.elapsed().as_millis(),
            ..ModelResponse::default()
        })
    }
}

impl ModelRequest {
    pub fn uses_agent_loop(&self) -> bool {
        self.web_search.is_some() || self.workspace_sqlite.is_some()
    }
}

async fn complete_with_openrouter_agent(request: &ModelRequest) -> Result<ModelResponse> {
    let mut tools = Vec::new();
    if request.workspace_sqlite.is_some() {
        tools.push(workspace_sql_openrouter_tool());
    }
    if let Some(web_search) = &request.web_search {
        tools.push(web_search.as_openrouter_tool());
    }

    let client_tools = request.client_tools.clone().or_else(|| {
        request
            .workspace_sqlite
            .as_ref()
            .map(|path| workspace_agent_tools(path.clone()))
    });

    let started_at = Instant::now();
    let result = run_chat_completion(ChatCompletionOptions {
        model: request.model.clone(),
        messages: vec![
            json!({"role": "system", "content": request.preamble}),
            json!({"role": "user", "content": request.prompt}),
        ],
        tools: (!tools.is_empty()).then_some(tools),
        json_mode: false,
        client_tools,
    })
    .await?;

    Ok(chat_result_to_model_response(
        request,
        result,
        started_at.elapsed().as_millis(),
    ))
}

fn chat_result_to_model_response(
    request: &ModelRequest,
    result: ChatCompletionResult,
    latency_ms: u128,
) -> ModelResponse {
    ModelResponse {
        text: result.text,
        model: request.model.clone(),
        latency_ms,
        web_search_requests: result.web_search_requests,
        client_tool_calls: result.client_tool_calls,
        input_tokens: result.input_tokens,
        output_tokens: result.output_tokens,
        agent_rounds: result.agent_rounds,
        finish_reason: result.finish_reason,
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
        let tool = WebSearchToolConfig::concept_validation_defaults().as_openrouter_tool();
        assert_eq!(tool["type"], "openrouter:web_search");
        assert_eq!(tool["parameters"]["engine"], "exa");
        assert_eq!(tool["parameters"]["max_results"], 5);
    }

    #[test]
    fn agent_loop_when_workspace_attached() {
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
        assert!(request.uses_agent_loop());
    }
}
