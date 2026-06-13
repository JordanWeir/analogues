use crate::services::{
    tool_loop_control::{any_stop_condition, merge_prepare_step_result},
    usage_snapshot::UsageSnapshot,
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use openrouter_rs::{
    api::chat::{ChatCompletionRequest, Message},
    error::OpenRouterError,
    types::{
        completion::{CompletionsResponse, FinishReason},
        response_format::ResponseFormat,
        Tool, ToolChoice,
    },
    OpenRouterClient,
};
use reqwest::StatusCode;
use serde::Serialize;
use serde_json::Value;
use std::{env, future::Future, sync::Arc, time::Duration};

const OPENROUTER_CHAT_COMPLETIONS_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const HTTP_REFERER: &str = "research@example.local";
const X_TITLE: &str = "analogues";
pub const DEFAULT_MAX_AGENT_ROUNDS: usize = 16;
pub const CONCEPT_REVIEW_MAX_AGENT_ROUNDS: usize = 20;
pub const FINANCIAL_EXPLORER_MAX_AGENT_ROUNDS: usize = 24;
pub const NARRATIVE_RESEARCH_MAX_AGENT_ROUNDS: usize = 28;

/// Backoff delays before each retry after a transient OpenRouter failure.
const OPENROUTER_RETRY_DELAYS_SECS: &[u64] = &[5, 15, 45];

#[async_trait]
pub trait ClientToolHandler: Send + Sync {
    async fn execute(&self, tool_name: &str, arguments: &str) -> Result<ClientToolExecuteResult>;
}

/// Result of executing a client-side tool in the agent loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientToolExecuteResult {
    /// Continue the loop and return this payload to the model.
    Response(String),
    /// End the loop successfully; `text` becomes the completion result.
    Complete(String),
}

/// A chat completion tool: either a client-executed function or an OpenRouter server tool.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum CompletionTool {
    Function(Tool),
    Server(ServerTool),
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ServerTool {
    #[serde(rename = "type")]
    tool_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<Value>,
}

impl ServerTool {
    fn web_search(parameters: Value) -> Self {
        let parameters = parameters
            .as_object()
            .filter(|map| !map.is_empty())
            .map(|_| parameters.clone());
        Self {
            tool_type: "openrouter:web_search".to_string(),
            parameters,
        }
    }
}

pub fn web_search_server_tool(parameters: Value) -> CompletionTool {
    CompletionTool::Server(ServerTool::web_search(parameters))
}

#[derive(Clone)]
pub struct ChatCompletionOptions {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Option<Vec<CompletionTool>>,
    /// Only safe to enable when no tools are attached; see OpenRouter json+tools conflicts.
    pub json_mode: bool,
    pub client_tools: Option<Arc<dyn ClientToolHandler>>,
    /// Maximum model turns in the client tool loop. Defaults to [`DEFAULT_MAX_AGENT_ROUNDS`].
    pub max_agent_rounds: Option<usize>,
    /// When set, penultimate-step nudges tell the model to call this tool (e.g. submit_concept_review).
    pub submit_tool_name: Option<String>,
    /// Runs before each model turn. Defaults to [`StepBudgetPrepareStep`] when unset.
    pub prepare_step: Option<Arc<dyn PrepareStepHook>>,
    /// Evaluated after each tool-bearing step; loop stops when any condition matches.
    pub stop_when: Option<Vec<Arc<dyn StopCondition>>>,
}

pub use crate::services::tool_loop_control::{
    agent_step_budget_message, apply_step_budget_prepare, has_tool_call, step_count_is,
    AgentStep, AgentToolCall, ChainedPrepareStep, PrepareStepContext, PrepareStepHook,
    PrepareStepResult, StepBudgetPrepareStep, StopCondition, StopConditionContext,
};

#[derive(Debug, Clone, Default)]
pub struct ChatCompletionResult {
    pub text: String,
    pub finish_reason: Option<String>,
    pub usage: UsageSnapshot,
    pub agent_rounds: usize,
    pub client_tool_calls: u32,
}

#[derive(Serialize)]
struct AgentChatRequest {
    model: String,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<CompletionTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

pub fn build_openrouter_client() -> Result<OpenRouterClient> {
    let api_key = env::var("OPENROUTER_API_KEY").map_err(|_| {
        Error::string("OPENROUTER_API_KEY is required for OpenRouter chat requests")
    })?;

    OpenRouterClient::builder()
        .api_key(api_key)
        .http_referer(HTTP_REFERER)
        .x_title(X_TITLE)
        .build()
        .map_err(map_openrouter_error)
}

/// Single-request completion for server tools like `openrouter:web_search`.
///
/// OpenRouter executes server tools internally and returns the final assistant
/// message in one round trip. See the web search server tool docs.
pub async fn run_single_shot_completion(
    options: ChatCompletionOptions,
) -> Result<ChatCompletionResult> {
    let client = build_openrouter_client()?;
    let (response, raw_payload) = send_completion(
        &client,
        &options.model,
        &options.messages,
        options.tools.as_deref(),
        options.json_mode,
    )
    .await?;

    let choice = response
        .choices
        .first()
        .ok_or_else(|| Error::string("OpenRouter response contained no choices"))?;

    let text = choice.content().unwrap_or_default().to_string();
    let mut usage = UsageSnapshot::from_response_payload(&raw_payload);
    if let Some(typed_usage) = &response.usage {
        usage.merge_typed_usage(typed_usage);
    }

    if text.trim().is_empty() {
        return Err(empty_completion_error(
            &choice.finish_reason().map(finish_reason_label),
            &text,
            usage.web_search_requests.unwrap_or(0),
            0,
            1,
        ));
    }

    Ok(ChatCompletionResult {
        text,
        finish_reason: choice.finish_reason().map(finish_reason_label),
        usage,
        agent_rounds: 1,
        client_tool_calls: 0,
    })
}

/// Client tool loop for function tools such as `workspace_sql`.
///
/// Follows the openrouter-rs typed tool agent pattern: every returned tool call
/// is executed locally and the conversation continues until the model answers.
pub async fn run_client_tool_loop(options: ChatCompletionOptions) -> Result<ChatCompletionResult> {
    let handler = options
        .client_tools
        .as_ref()
        .ok_or_else(|| Error::string("client tool loop requires a client tool handler"))?;

    let client = build_openrouter_client()?;
    let mut messages = options.messages;
    let max_agent_rounds = options.max_agent_rounds.unwrap_or(DEFAULT_MAX_AGENT_ROUNDS);
    let stop_when = options.stop_when.unwrap_or_default();
    let prepare_step: Arc<dyn PrepareStepHook> = options
        .prepare_step
        .clone()
        .unwrap_or_else(|| {
            Arc::new(StepBudgetPrepareStep::new(options.submit_tool_name.clone()))
        });
    let mut model = options.model.clone();
    let mut tools = options.tools.clone();
    let mut steps: Vec<AgentStep> = Vec::new();
    let mut total_usage = UsageSnapshot::default();
    let mut total_client_tool_calls = 0u32;
    let mut last_finish_reason = None;
    let mut last_text = String::new();

    for round in 0..max_agent_rounds {
        let prepare_result = prepare_step.prepare_step(PrepareStepContext {
            step_number: round,
            max_steps: max_agent_rounds,
            steps: &steps,
            messages: &messages,
            model: &model,
        });
        merge_prepare_step_result(&mut messages, &mut model, &mut tools, prepare_result);

        let (response, raw_payload) = send_completion(
            &client,
            &model,
            &messages,
            tools.as_deref(),
            false,
        )
        .await?;

        let choice = response
            .choices
            .first()
            .ok_or_else(|| Error::string("OpenRouter response contained no choices"))?;

        let mut round_usage = UsageSnapshot::from_response_payload(&raw_payload);
        if let Some(typed_usage) = &response.usage {
            round_usage.merge_typed_usage(typed_usage);
        }
        total_usage.absorb(&round_usage);
        last_finish_reason = choice.finish_reason().map(finish_reason_label);

        if let Some(tool_calls) = choice.tool_calls().filter(|calls| !calls.is_empty()) {
            let assistant_text = choice.content().unwrap_or_default();
            messages.push(Message::assistant_with_tool_calls(
                assistant_text,
                tool_calls.to_vec(),
            ));

            let mut step_tool_calls = Vec::new();
            let mut step_tool_results = Vec::new();

            for tool_call in tool_calls {
                let arguments = tool_call.arguments_json().to_string();
                match handler
                    .execute(tool_call.name(), tool_call.arguments_json())
                    .await
                {
                    Ok(ClientToolExecuteResult::Complete(text)) => {
                        total_client_tool_calls += 1;
                        step_tool_calls.push(AgentToolCall {
                            tool_name: tool_call.name().to_string(),
                            arguments,
                            succeeded: true,
                        });
                        messages.push(Message::tool_response_named(
                            tool_call.id(),
                            tool_call.name(),
                            "Submission accepted.".to_string(),
                        ));
                        return Ok(ChatCompletionResult {
                            text,
                            finish_reason: Some("stop".to_string()),
                            usage: total_usage,
                            agent_rounds: round + 1,
                            client_tool_calls: total_client_tool_calls,
                        });
                    }
                    Ok(ClientToolExecuteResult::Response(result)) => {
                        total_client_tool_calls += 1;
                        step_tool_calls.push(AgentToolCall {
                            tool_name: tool_call.name().to_string(),
                            arguments,
                            succeeded: true,
                        });
                        step_tool_results.push(result.clone());
                        messages.push(Message::tool_response_named(
                            tool_call.id(),
                            tool_call.name(),
                            result,
                        ));
                    }
                    Err(err) => {
                        tracing::warn!(
                            tool = tool_call.name(),
                            error = %err,
                            "client tool call failed; returning error to model"
                        );
                        total_client_tool_calls += 1;
                        let payload = client_tool_error_payload(tool_call.name(), &err);
                        step_tool_calls.push(AgentToolCall {
                            tool_name: tool_call.name().to_string(),
                            arguments,
                            succeeded: false,
                        });
                        step_tool_results.push(payload.clone());
                        messages.push(Message::tool_response_named(
                            tool_call.id(),
                            tool_call.name(),
                            payload,
                        ));
                    }
                }
            }

            if !assistant_text.trim().is_empty() {
                last_text = assistant_text.to_string();
            }

            steps.push(AgentStep {
                step_number: round,
                tool_calls: step_tool_calls,
                tool_results: step_tool_results,
                usage: round_usage.clone(),
                assistant_text: if assistant_text.trim().is_empty() {
                    None
                } else {
                    Some(assistant_text.to_string())
                },
            });

            if !stop_when.is_empty() {
                let stop_ctx = StopConditionContext {
                    steps: &steps,
                    total_usage: &total_usage,
                    max_steps: max_agent_rounds,
                };
                if any_stop_condition(&stop_when, &stop_ctx) {
                    if !last_text.trim().is_empty() {
                        return Ok(ChatCompletionResult {
                            text: last_text,
                            finish_reason: last_finish_reason,
                            usage: total_usage,
                            agent_rounds: round + 1,
                            client_tool_calls: total_client_tool_calls,
                        });
                    }
                    break;
                }
            }

            continue;
        }

        last_text = choice.content().unwrap_or_default().to_string();
        if !last_text.trim().is_empty() {
            return Ok(ChatCompletionResult {
                text: last_text,
                finish_reason: last_finish_reason,
                usage: total_usage,
                agent_rounds: round + 1,
                client_tool_calls: total_client_tool_calls,
            });
        }

        if !matches!(choice.finish_reason(), Some(FinishReason::ToolCalls)) {
            break;
        }
    }

    Err(empty_completion_error(
        &last_finish_reason,
        &last_text,
        total_usage.web_search_requests.unwrap_or(0),
        total_client_tool_calls,
        max_agent_rounds,
    ))
}

pub async fn run_simple_chat_completion(
    model: &str,
    messages: Vec<Message>,
    json_mode: bool,
) -> Result<ChatCompletionResult> {
    run_single_shot_completion(ChatCompletionOptions {
        model: model.to_string(),
        messages,
        tools: None,
        json_mode,
        client_tools: None,
        max_agent_rounds: None,
        submit_tool_name: None,
        prepare_step: None,
        stop_when: None,
    })
    .await
}

async fn send_completion(
    client: &OpenRouterClient,
    model: &str,
    messages: &[Message],
    tools: Option<&[CompletionTool]>,
    json_mode: bool,
) -> Result<(CompletionsResponse, Value)> {
    if has_server_tools(tools) {
        let request = AgentChatRequest {
            model: model.to_string(),
            messages: messages.to_vec(),
            tools: tools.map(|items| items.to_vec()),
            tool_choice: tools
                .filter(|items| !items.is_empty())
                .map(|_| ToolChoice::auto()),
            parallel_tool_calls: tools.filter(|items| !items.is_empty()).map(|_| true),
            response_format: (json_mode && tools.is_none())
                .then_some(ResponseFormat::json_object()),
        };
        return post_agent_chat(&request).await;
    }

    let function_tools = tools
        .filter(|items| !items.is_empty())
        .map(function_tools)
        .unwrap_or_default();

    let request = if json_mode && function_tools.is_empty() {
        ChatCompletionRequest::builder()
            .model(model)
            .messages(messages.to_vec())
            .response_format(ResponseFormat::json_object())
            .build()
    } else if function_tools.is_empty() {
        ChatCompletionRequest::builder()
            .model(model)
            .messages(messages.to_vec())
            .build()
    } else {
        let mut builder = ChatCompletionRequest::builder();
        builder.model(model);
        builder.messages(messages.to_vec());
        builder.tool_choice_auto();
        builder.parallel_tool_calls(true);
        for tool in function_tools {
            builder.tool(tool);
        }
        if json_mode {
            builder.response_format(ResponseFormat::json_object());
        }
        builder.build()
    }
    .map_err(map_openrouter_error)?;
    let response = with_openrouter_backoff("openrouter_chat", || async {
        client
            .chat()
            .create(&request)
            .await
            .map_err(OpenRouterAttemptError::from)
    })
    .await?;
    let raw_payload = serde_json::to_value(&response).unwrap_or(Value::Null);
    Ok((response, raw_payload))
}

async fn post_agent_chat(request: &AgentChatRequest) -> Result<(CompletionsResponse, Value)> {
    let api_key = env::var("OPENROUTER_API_KEY").map_err(|_| {
        Error::string("OPENROUTER_API_KEY is required for OpenRouter chat requests")
    })?;

    let http = reqwest::Client::builder()
        .user_agent("analogues/0.1 research@example.local")
        .build()
        .map_err(|err| Error::string(&format!("failed to build HTTP client: {err}")))?;

    with_openrouter_backoff("post_agent_chat", || post_agent_chat_attempt(&http, &api_key, request))
        .await
}

async fn post_agent_chat_attempt(
    http: &reqwest::Client,
    api_key: &str,
    request: &AgentChatRequest,
) -> Result<(CompletionsResponse, Value), OpenRouterAttemptError> {
    let response = http
        .post(OPENROUTER_CHAT_COMPLETIONS_URL)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {api_key}"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header("HTTP-Referer", HTTP_REFERER)
        .header("X-Title", X_TITLE)
        .json(request)
        .send()
        .await
        .map_err(|err| {
            if reqwest_error_is_retryable(&err) {
                OpenRouterAttemptError::retryable(format!("OpenRouter request failed: {err}"))
            } else {
                OpenRouterAttemptError::fatal(format!("OpenRouter request failed: {err}"))
            }
        })?;

    let status = response.status();
    let raw_payload: Value = response.json().await.map_err(|err| {
        OpenRouterAttemptError::fatal(format!("OpenRouter response was not JSON: {err}"))
    })?;

    if !status.is_success() {
        let message = raw_payload
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("unknown OpenRouter error");
        let error = format!("OpenRouter request failed ({status}): {message}");
        if http_status_is_retryable(status) {
            return Err(OpenRouterAttemptError::retryable(error));
        }
        return Err(OpenRouterAttemptError::fatal(error));
    }

    let parsed: CompletionsResponse = serde_json::from_value(raw_payload.clone()).map_err(|err| {
        OpenRouterAttemptError::fatal(format!(
            "OpenRouter response did not match expected schema: {err}"
        ))
    })?;

    Ok((parsed, raw_payload))
}

#[derive(Debug, Clone)]
enum OpenRouterAttemptError {
    Retryable(String),
    Fatal(String),
}

impl OpenRouterAttemptError {
    fn retryable(message: impl Into<String>) -> Self {
        Self::Retryable(message.into())
    }

    fn fatal(message: impl Into<String>) -> Self {
        Self::Fatal(message.into())
    }

    fn is_retryable(&self) -> bool {
        matches!(self, Self::Retryable(_))
    }
}

impl std::fmt::Display for OpenRouterAttemptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Retryable(message) | Self::Fatal(message) => f.write_str(message),
        }
    }
}

impl From<OpenRouterError> for OpenRouterAttemptError {
    fn from(err: OpenRouterError) -> Self {
        let message = format!("OpenRouter error: {err}");
        if openrouter_error_is_retryable(&err) {
            Self::Retryable(message)
        } else {
            Self::Fatal(message)
        }
    }
}

fn http_status_is_retryable(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn reqwest_error_is_retryable(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request()
}

fn openrouter_error_is_retryable(err: &OpenRouterError) -> bool {
    match err {
        OpenRouterError::HttpRequest(_) => true,
        OpenRouterError::Api(ctx) => ctx.is_retryable(),
        _ => false,
    }
}

fn retry_delay_for_attempt(attempt: usize) -> Duration {
    let secs = OPENROUTER_RETRY_DELAYS_SECS
        .get(attempt)
        .copied()
        .unwrap_or_else(|| {
            OPENROUTER_RETRY_DELAYS_SECS
                .last()
                .copied()
                .unwrap_or(45)
        });
    Duration::from_secs(secs)
}

async fn with_openrouter_backoff<T, F, Fut>(operation_name: &str, mut operation: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, OpenRouterAttemptError>>,
{
    let max_retries = OPENROUTER_RETRY_DELAYS_SECS.len();

    for attempt in 0..=max_retries {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(err) if err.is_retryable() && attempt < max_retries => {
                let delay = retry_delay_for_attempt(attempt);
                tracing::warn!(
                    operation = operation_name,
                    attempt = attempt + 1,
                    max_retries,
                    delay_secs = delay.as_secs(),
                    error = %err,
                    "OpenRouter request failed; retrying after backoff"
                );
                tokio::time::sleep(delay).await;
            }
            Err(err) => {
                let message = match err {
                    OpenRouterAttemptError::Retryable(message)
                    | OpenRouterAttemptError::Fatal(message) => message,
                };
                return Err(Error::string(&message));
            }
        }
    }

    Err(Error::string(&format!(
        "OpenRouter request failed after {} attempts ({operation_name})",
        max_retries + 1
    )))
}

fn has_server_tools(tools: Option<&[CompletionTool]>) -> bool {
    tools.is_some_and(|items| {
        items
            .iter()
            .any(|tool| matches!(tool, CompletionTool::Server(_)))
    })
}

fn function_tools(tools: &[CompletionTool]) -> Vec<Tool> {
    tools
        .iter()
        .filter_map(|tool| match tool {
            CompletionTool::Function(tool) => Some(tool.clone()),
            CompletionTool::Server(_) => None,
        })
        .collect()
}

fn finish_reason_label(reason: &FinishReason) -> String {
    match reason {
        FinishReason::ToolCalls => "tool_calls".to_string(),
        FinishReason::Stop => "stop".to_string(),
        FinishReason::Length => "length".to_string(),
        FinishReason::ContentFilter => "content_filter".to_string(),
        FinishReason::Error => "error".to_string(),
        _ => "unknown".to_string(),
    }
}

fn map_openrouter_error(err: OpenRouterError) -> Error {
    Error::string(&format!("OpenRouter error: {err}"))
}

pub fn extract_assistant_text(message: &Value) -> Option<String> {
    let content = message.get("content")?;
    let text = match content {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| match part.get("type").and_then(Value::as_str) {
                Some("text") | Some("output_text") => {
                    part.get("text").and_then(Value::as_str).map(str::to_string)
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(""),
        Value::Null => String::new(),
        _ => return None,
    };

    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

pub fn extract_json_text(text: &str) -> &str {
    let trimmed = text.trim();
    if trimmed.starts_with("```") {
        let without_fence = trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim();
        if let Some(body) = without_fence.strip_suffix("```") {
            return body.trim();
        }
    }
    trimmed
}

pub fn extract_json_object(text: &str) -> Option<&str> {
    let trimmed = extract_json_text(text);
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end < start {
        return None;
    }
    Some(&trimmed[start..=end])
}

fn client_tool_error_payload(tool_name: &str, err: &Error) -> String {
    serde_json::json!({
        "error": true,
        "tool": tool_name,
        "message": err.to_string(),
    })
    .to_string()
}

fn empty_completion_error(
    finish_reason: &Option<String>,
    text: &str,
    web_search_requests: u32,
    client_tool_calls: u32,
    max_agent_rounds: usize,
) -> Error {
    let preview = if text.trim().is_empty() {
        "<empty>".to_string()
    } else {
        text.chars().take(240).collect()
    };
    Error::string(&format!(
        "OpenRouter returned no assistant text after {max_agent_rounds} agent steps (finish_reason={finish_reason:?}, web_search_requests={web_search_requests}, client_tool_calls={client_tool_calls}, preview={preview})"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_text_from_string_content() {
        let message = json!({"role": "assistant", "content": "{\"decisions\":[]}"});
        assert_eq!(
            extract_assistant_text(&message).as_deref(),
            Some("{\"decisions\":[]}")
        );
    }

    #[test]
    fn extracts_text_from_array_content() {
        let message = json!({
            "role": "assistant",
            "content": [
                {"type": "reasoning", "text": "thinking"},
                {"type": "text", "text": "{\"decisions\":[]}"}
            ]
        });
        assert_eq!(
            extract_assistant_text(&message).as_deref(),
            Some("{\"decisions\":[]}")
        );
    }

    #[test]
    fn extracts_json_object_from_prose_wrapper() {
        let text = "Here is the review:\n```json\n{\"decisions\":[]}\n```";
        assert_eq!(extract_json_object(text), Some("{\"decisions\":[]}"));
    }

    #[test]
    fn serializes_web_search_server_tool() {
        let tool = web_search_server_tool(json!({
            "engine": "exa",
            "max_results": 5
        }));
        let value = serde_json::to_value(tool).expect("tool should serialize");
        assert_eq!(value["type"], "openrouter:web_search");
        assert_eq!(value["parameters"]["engine"], "exa");
        assert_eq!(value["parameters"]["max_results"], 5);
    }

    #[test]
    fn serializes_web_search_server_tool_without_parameters() {
        let tool = web_search_server_tool(json!({}));
        let value = serde_json::to_value(tool).expect("tool should serialize");
        assert_eq!(value, json!({"type": "openrouter:web_search"}));
    }

    #[test]
    fn agent_step_budget_message_reports_remaining_steps() {
        let message = agent_step_budget_message(5, 20, 15, Some("submit_concept_review"));
        assert!(message.contains("Step 5/20"));
        assert!(message.contains("Steps remaining: 15"));
    }

    #[test]
    fn agent_step_budget_message_nudges_submit_on_penultimate_step() {
        let message = agent_step_budget_message(18, 20, 2, Some("submit_concept_review"));
        assert!(message.contains("Steps remaining: 2"));
        assert!(message.contains("penultimate"));
        assert!(message.contains("submit_concept_review"));
    }

    #[test]
    fn agent_step_budget_message_nudges_repair_on_final_step() {
        let message = agent_step_budget_message(19, 20, 1, Some("submit_concept_review"));
        assert!(message.contains("final turn"));
        assert!(message.contains("validation"));
    }

    #[test]
    fn client_tool_error_payload_serializes_message() {
        let payload = client_tool_error_payload(
            "workspace_sql",
            &Error::string("workspace_sql query cannot be empty"),
        );
        let value: Value = serde_json::from_str(&payload).expect("payload should be JSON");
        assert_eq!(value["error"], true);
        assert_eq!(value["tool"], "workspace_sql");
        assert!(value["message"].as_str().unwrap().contains("empty"));
    }

    #[test]
    fn agent_chat_request_enables_parallel_tool_calls_when_tools_present() {
        let request = AgentChatRequest {
            model: "test/model".to_string(),
            messages: vec![Message::new(openrouter_rs::types::Role::User, "hello")],
            tools: Some(vec![web_search_server_tool(serde_json::json!({}))]),
            tool_choice: Some(ToolChoice::auto()),
            parallel_tool_calls: Some(true),
            response_format: None,
        };
        let value = serde_json::to_value(request).expect("request should serialize");
        assert_eq!(value["parallel_tool_calls"], true);
    }

    #[test]
    fn detects_server_tools_in_mixed_tool_list() {
        let tools = [
            CompletionTool::Function(
                Tool::builder()
                    .name("workspace_sql")
                    .description("query")
                    .parameters(json!({"type": "object"}))
                    .build()
                    .expect("tool should build"),
            ),
            web_search_server_tool(json!({})),
        ];
        assert!(has_server_tools(Some(&tools)));
    }

    #[test]
    fn http_status_retryable_for_transient_failures_only() {
        assert!(http_status_is_retryable(StatusCode::TOO_MANY_REQUESTS));
        assert!(http_status_is_retryable(StatusCode::BAD_GATEWAY));
        assert!(!http_status_is_retryable(StatusCode::UNAUTHORIZED));
        assert!(!http_status_is_retryable(StatusCode::BAD_REQUEST));
    }

    #[test]
    fn retry_delay_uses_exponential_backoff() {
        assert_eq!(retry_delay_for_attempt(0), Duration::from_secs(5));
        assert_eq!(retry_delay_for_attempt(1), Duration::from_secs(15));
        assert_eq!(retry_delay_for_attempt(2), Duration::from_secs(45));
        assert_eq!(retry_delay_for_attempt(9), Duration::from_secs(45));
    }

    #[test]
    fn openrouter_http_request_errors_are_retryable() {
        use openrouter_rs::error::HttpRequestError;

        let err = OpenRouterError::HttpRequest(HttpRequestError::new(
            "error sending request for url (https://openrouter.ai/api/v1/chat/completions)",
        ));
        assert!(openrouter_error_is_retryable(&err));
    }
}
