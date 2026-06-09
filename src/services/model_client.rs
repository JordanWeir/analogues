use crate::services::{
    agent_tools::{workspace_agent_tools, workspace_sql_tool},
    openrouter_chat::{
        run_client_tool_loop, run_single_shot_completion, run_simple_chat_completion,
        web_search_server_tool, ChatCompletionOptions, ChatCompletionResult, ClientToolHandler,
        CompletionTool,
    },
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use openrouter_rs::api::chat::Message;
use openrouter_rs::types::Role;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
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

    fn to_completion_tool(&self) -> CompletionTool {
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
        let started_at = Instant::now();
        let result = if request.uses_client_tool_loop() {
            complete_with_client_tools(&request).await?
        } else if request.web_search.is_some() {
            complete_with_web_search(&request).await?
        } else {
            run_simple_chat_completion(
                &request.model,
                vec![
                    Message::new(Role::System, request.preamble.as_str()),
                    Message::new(Role::User, request.prompt.as_str()),
                ],
                request.json_mode,
            )
            .await?
        };

        Ok(chat_result_to_model_response(
            &request,
            result,
            started_at.elapsed().as_millis(),
        ))
    }
}

impl ModelRequest {
    pub fn uses_client_tool_loop(&self) -> bool {
        self.workspace_sqlite.is_some()
    }
}

async fn complete_with_web_search(request: &ModelRequest) -> Result<ChatCompletionResult> {
    let tools = request
        .web_search
        .as_ref()
        .map(|config| vec![config.to_completion_tool()]);

    run_single_shot_completion(ChatCompletionOptions {
        model: request.model.clone(),
        messages: vec![
            Message::new(Role::System, request.preamble.as_str()),
            Message::new(Role::User, request.prompt.as_str()),
        ],
        tools,
        json_mode: false,
        client_tools: None,
    })
    .await
}

async fn complete_with_client_tools(request: &ModelRequest) -> Result<ChatCompletionResult> {
    let mut tools = vec![CompletionTool::Function(workspace_sql_tool())];
    if let Some(web_search) = &request.web_search {
        tools.push(web_search.to_completion_tool());
    }

    let client_tools = request.client_tools.clone().or_else(|| {
        request
            .workspace_sqlite
            .as_ref()
            .map(|path| workspace_agent_tools(path.clone()))
    });

    run_client_tool_loop(ChatCompletionOptions {
        model: request.model.clone(),
        messages: vec![
            Message::new(Role::System, request.preamble.as_str()),
            Message::new(Role::User, request.prompt.as_str()),
        ],
        tools: Some(tools),
        json_mode: false,
        client_tools,
    })
    .await
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
        let tool = WebSearchToolConfig::concept_validation_defaults().to_completion_tool();
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
