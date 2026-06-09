use crate::{
    agents::tools::ToolRegistry,
    services::{
        openrouter_chat::{
            run_client_tool_loop, run_simple_chat_completion, run_single_shot_completion,
            ChatCompletionOptions, ChatCompletionResult, ClientToolHandler,
        },
        usage_snapshot::UsageSnapshot,
        worker_run_store::{
            WorkerRunRecord, WorkerRunStore, WORKER_RUN_STATUS_ERROR, WORKER_RUN_STATUS_SUCCESS,
        },
    },
};
use loco_rs::prelude::*;
use openrouter_rs::api::chat::Message;
use openrouter_rs::types::Role;
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Instant};

#[derive(Clone)]
pub struct ToolLoopRequest {
    pub worker_name: String,
    pub model: String,
    pub preamble: String,
    pub prompt: String,
    pub json_mode: bool,
    pub tools: ToolRegistry,
    pub metadata: BTreeMap<String, String>,
    pub workspace_sqlite: Option<PathBuf>,
    pub client_tools: Option<Arc<dyn ClientToolHandler>>,
    pub max_agent_rounds: Option<usize>,
    pub submit_tool_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ToolLoopResponse {
    pub text: String,
    pub model: String,
    pub worker_run_id: Option<i64>,
    pub latency_ms: u128,
    pub usage: UsageSnapshot,
    pub client_tool_calls: u32,
    pub agent_rounds: usize,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ToolLoopAgent;

impl ToolLoopAgent {
    pub async fn run(&self, request: ToolLoopRequest) -> Result<ToolLoopResponse> {
        let started_at = Instant::now();
        let messages = vec![
            Message::new(Role::System, request.preamble.as_str()),
            Message::new(Role::User, request.prompt.as_str()),
        ];

        let completion_tools = {
            let tools = request.tools.completion_tools();
            (!tools.is_empty()).then_some(tools)
        };

        let client_tools = request
            .client_tools
            .clone()
            .or_else(|| request.tools.client_handler());

        let run_result = if request.tools.needs_client_loop() {
            run_client_tool_loop(ChatCompletionOptions {
                model: request.model.clone(),
                messages,
                tools: completion_tools,
                json_mode: false,
                client_tools,
                max_agent_rounds: request.max_agent_rounds,
                submit_tool_name: request.submit_tool_name.clone(),
            })
            .await
        } else if completion_tools.is_some() {
            run_single_shot_completion(ChatCompletionOptions {
                model: request.model.clone(),
                messages,
                tools: completion_tools,
                json_mode: false,
                client_tools: None,
                max_agent_rounds: None,
                submit_tool_name: None,
            })
            .await
        } else {
            run_simple_chat_completion(&request.model, messages, request.json_mode).await
        };

        let latency_ms = started_at.elapsed().as_millis();
        match run_result {
            Ok(result) => {
                let response = completion_to_response(&request, result, latency_ms);
                let worker_run_id = self
                    .persist_run(&request, &response, WORKER_RUN_STATUS_SUCCESS, None)
                    .await?;
                Ok(ToolLoopResponse {
                    worker_run_id,
                    ..response
                })
            }
            Err(err) => {
                let _ = self
                    .persist_run(
                        &request,
                        &empty_response(&request, latency_ms),
                        WORKER_RUN_STATUS_ERROR,
                        Some(err.to_string()),
                    )
                    .await;
                Err(err)
            }
        }
    }

    async fn persist_run(
        &self,
        request: &ToolLoopRequest,
        response: &ToolLoopResponse,
        status: &str,
        error_message: Option<String>,
    ) -> Result<Option<i64>> {
        let Some(sqlite_path) = request.workspace_sqlite.as_ref() else {
            return Ok(None);
        };

        let metadata_json = build_metadata_json(request, response);
        let worker_run_id = WorkerRunStore::persist(
            sqlite_path,
            &WorkerRunRecord {
                worker_name: request.worker_name.clone(),
                model: request.model.clone(),
                status: status.to_string(),
                agent_rounds: response.agent_rounds,
                usage: response.usage.clone(),
                client_tool_calls: response.client_tool_calls,
                latency_ms: response.latency_ms,
                finish_reason: response.finish_reason.clone(),
                error_message,
                metadata_json,
            },
        )
        .await?;

        Ok(Some(worker_run_id))
    }
}

fn completion_to_response(
    request: &ToolLoopRequest,
    result: ChatCompletionResult,
    latency_ms: u128,
) -> ToolLoopResponse {
    ToolLoopResponse {
        text: result.text,
        model: request.model.clone(),
        worker_run_id: None,
        latency_ms,
        usage: result.usage,
        client_tool_calls: result.client_tool_calls,
        agent_rounds: result.agent_rounds,
        finish_reason: result.finish_reason,
    }
}

fn empty_response(request: &ToolLoopRequest, latency_ms: u128) -> ToolLoopResponse {
    ToolLoopResponse {
        text: String::new(),
        model: request.model.clone(),
        worker_run_id: None,
        latency_ms,
        usage: UsageSnapshot::default(),
        client_tool_calls: 0,
        agent_rounds: 0,
        finish_reason: None,
    }
}

fn build_metadata_json(request: &ToolLoopRequest, response: &ToolLoopResponse) -> Value {
    let mut metadata = serde_json::Map::new();
    for (key, value) in &request.metadata {
        metadata.insert(key.clone(), Value::String(value.clone()));
    }
    metadata.insert(
        "enabled_tools".to_string(),
        Value::Array(
            request
                .tools
                .completion_tools()
                .iter()
                .filter_map(|tool| match tool {
                    crate::services::openrouter_chat::CompletionTool::Function(function_tool) => {
                        Some(Value::String(function_tool.function.name.clone()))
                    }
                    crate::services::openrouter_chat::CompletionTool::Server(_) => {
                        Some(Value::String("openrouter:web_search".to_string()))
                    }
                })
                .collect(),
        ),
    );
    metadata.insert(
        "latency_ms".to_string(),
        Value::Number((response.latency_ms as u64).into()),
    );
    Value::Object(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::tools::{ToolRegistry, WebSearchConfig};

    #[test]
    fn request_metadata_includes_enabled_tools() {
        let request = ToolLoopRequest {
            worker_name: "concept_catalog_review".to_string(),
            model: "test/model".to_string(),
            preamble: "system".to_string(),
            prompt: "user".to_string(),
            json_mode: false,
            tools: ToolRegistry::new()
                .with_sql_query(PathBuf::from("/tmp/test.sqlite"))
                .with_concept_review_submit()
                .with_web_search(WebSearchConfig::default()),
            metadata: BTreeMap::from([("lane".to_string(), "build_catalog".to_string())]),
            workspace_sqlite: None,
            client_tools: None,
            max_agent_rounds: None,
            submit_tool_name: None,
        };
        let response = ToolLoopResponse {
            text: "ok".to_string(),
            model: "test/model".to_string(),
            worker_run_id: None,
            latency_ms: 10,
            usage: UsageSnapshot::default(),
            client_tool_calls: 0,
            agent_rounds: 1,
            finish_reason: Some("stop".to_string()),
        };

        let metadata = build_metadata_json(&request, &response);
        assert_eq!(metadata["lane"], "build_catalog");
        assert_eq!(metadata["enabled_tools"][0], "workspace_sql");
        assert_eq!(metadata["enabled_tools"][1], "submit_concept_review");
        assert_eq!(metadata["enabled_tools"][2], "openrouter:web_search");
    }
}
