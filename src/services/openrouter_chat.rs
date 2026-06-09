use async_trait::async_trait;
use loco_rs::prelude::*;
use openrouter_rs::{
    OpenRouterClient,
    api::chat::{ChatCompletionRequest, Message},
    error::OpenRouterError,
    types::{
        Tool, ToolChoice,
        completion::{CompletionsResponse, FinishReason},
        response_format::ResponseFormat,
    },
};
use serde::Serialize;
use serde_json::Value;
use std::{env, sync::Arc};

const OPENROUTER_CHAT_COMPLETIONS_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const HTTP_REFERER: &str = "research@example.local";
const X_TITLE: &str = "analogues";
const MAX_AGENT_ROUNDS: usize = 16;

#[async_trait]
pub trait ClientToolHandler: Send + Sync {
    async fn execute(&self, tool_name: &str, arguments: &str) -> Result<String>;
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
}

#[derive(Debug, Clone, Default)]
pub struct ChatCompletionResult {
    pub text: String,
    pub finish_reason: Option<String>,
    pub web_search_requests: u32,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
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
    if text.trim().is_empty() {
        return Err(empty_completion_error(
            &choice.finish_reason().map(finish_reason_label),
            &text,
            web_search_requests_from_payload(&raw_payload),
            0,
        ));
    }

    Ok(ChatCompletionResult {
        text,
        finish_reason: choice.finish_reason().map(finish_reason_label),
        web_search_requests: web_search_requests_from_payload(&raw_payload),
        input_tokens: response.usage.as_ref().map(|usage| usage.prompt_tokens as u64),
        output_tokens: response
            .usage
            .as_ref()
            .map(|usage| usage.completion_tokens as u64),
        agent_rounds: 1,
        client_tool_calls: 0,
    })
}

/// Client tool loop for function tools such as `workspace_sql`.
///
/// Follows the openrouter-rs typed tool agent pattern: every returned tool call
/// is executed locally and the conversation continues until the model answers.
pub async fn run_client_tool_loop(options: ChatCompletionOptions) -> Result<ChatCompletionResult> {
    let handler = options.client_tools.as_ref().ok_or_else(|| {
        Error::string("client tool loop requires a client tool handler")
    })?;

    let client = build_openrouter_client()?;
    let mut messages = options.messages;
    let mut total_web_search_requests = 0u32;
    let mut total_client_tool_calls = 0u32;
    let mut last_input_tokens = None;
    let mut last_output_tokens = None;
    let mut last_finish_reason = None;
    let mut last_text = String::new();

    for round in 0..MAX_AGENT_ROUNDS {
        let (response, raw_payload) = send_completion(
            &client,
            &options.model,
            &messages,
            options.tools.as_deref(),
            false,
        )
        .await?;

        let choice = response
            .choices
            .first()
            .ok_or_else(|| Error::string("OpenRouter response contained no choices"))?;

        total_web_search_requests += web_search_requests_from_payload(&raw_payload);
        if let Some(usage) = &response.usage {
            last_input_tokens = Some(usage.prompt_tokens as u64);
            last_output_tokens = Some(usage.completion_tokens as u64);
        }
        last_finish_reason = choice.finish_reason().map(finish_reason_label);

        if let Some(tool_calls) = choice.tool_calls().filter(|calls| !calls.is_empty()) {
            let assistant_text = choice.content().unwrap_or_default();
            messages.push(Message::assistant_with_tool_calls(
                assistant_text,
                tool_calls.to_vec(),
            ));

            for tool_call in tool_calls {
                let result = handler
                    .execute(tool_call.name(), tool_call.arguments_json())
                    .await?;
                total_client_tool_calls += 1;
                messages.push(Message::tool_response_named(
                    tool_call.id(),
                    tool_call.name(),
                    result,
                ));
            }

            if !assistant_text.trim().is_empty() {
                last_text = assistant_text.to_string();
            }
            continue;
        }

        last_text = choice.content().unwrap_or_default().to_string();
        if !last_text.trim().is_empty() {
            return Ok(ChatCompletionResult {
                text: last_text,
                finish_reason: last_finish_reason,
                web_search_requests: total_web_search_requests,
                input_tokens: last_input_tokens,
                output_tokens: last_output_tokens,
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
        total_web_search_requests,
        total_client_tool_calls,
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
        for tool in function_tools {
            builder.tool(tool);
        }
        if json_mode {
            builder.response_format(ResponseFormat::json_object());
        }
        builder.build()
    }
    .map_err(map_openrouter_error)?;
    let response = client
        .chat()
        .create(&request)
        .await
        .map_err(map_openrouter_error)?;
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

    let response = http
        .post(OPENROUTER_CHAT_COMPLETIONS_URL)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {api_key}"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header("HTTP-Referer", HTTP_REFERER)
        .header("X-Title", X_TITLE)
        .json(request)
        .send()
        .await
        .map_err(|err| Error::string(&format!("OpenRouter request failed: {err}")))?;

    let status = response.status();
    let raw_payload: Value = response.json().await.map_err(|err| {
        Error::string(&format!("OpenRouter response was not JSON: {err}"))
    })?;

    if !status.is_success() {
        let message = raw_payload
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("unknown OpenRouter error");
        return Err(Error::string(&format!(
            "OpenRouter request failed ({status}): {message}"
        )));
    }

    let parsed: CompletionsResponse = serde_json::from_value(raw_payload.clone()).map_err(|err| {
        Error::string(&format!("OpenRouter response did not match expected schema: {err}"))
    })?;

    Ok((parsed, raw_payload))
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

fn web_search_requests_from_payload(payload: &Value) -> u32 {
    payload
        .pointer("/usage/server_tool_use/web_search_requests")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32
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

fn empty_completion_error(
    finish_reason: &Option<String>,
    text: &str,
    web_search_requests: u32,
    client_tool_calls: u32,
) -> Error {
    let preview = if text.trim().is_empty() {
        "<empty>".to_string()
    } else {
        text.chars().take(240).collect()
    };
    Error::string(&format!(
        "OpenRouter returned no assistant text (finish_reason={finish_reason:?}, web_search_requests={web_search_requests}, client_tool_calls={client_tool_calls}, preview={preview})"
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
}
