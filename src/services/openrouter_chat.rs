use async_trait::async_trait;
use loco_rs::prelude::*;
use serde_json::{json, Value};
use std::env;

pub const OPENROUTER_CHAT_COMPLETIONS_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const MAX_AGENT_ROUNDS: usize = 16;

#[async_trait]
pub trait ClientToolHandler: Send + Sync {
    async fn execute(&self, tool_name: &str, arguments: &str) -> Result<String>;
}

#[derive(Clone)]
pub struct ChatCompletionOptions {
    pub model: String,
    pub messages: Vec<Value>,
    pub tools: Option<Vec<Value>>,
    /// Only safe to enable when no tools are attached; see OpenRouter json+tools conflicts.
    pub json_mode: bool,
    pub client_tools: Option<std::sync::Arc<dyn ClientToolHandler>>,
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

pub async fn run_chat_completion(
    options: ChatCompletionOptions,
) -> Result<ChatCompletionResult> {
    let api_key = env::var("OPENROUTER_API_KEY").map_err(|_| {
        Error::string("OPENROUTER_API_KEY is required for OpenRouter chat requests")
    })?;

    let http = reqwest::Client::builder()
        .user_agent("analogues/0.1 research@example.local")
        .build()
        .map_err(|err| Error::string(&format!("failed to build HTTP client: {err}")))?;

    let mut messages = options.messages;
    let mut total_web_search_requests = 0u32;
    let mut total_client_tool_calls = 0u32;
    let mut last_input_tokens = None;
    let mut last_output_tokens = None;
    let mut last_finish_reason = None;
    let mut last_text = String::new();

    for round in 0..MAX_AGENT_ROUNDS {
        let body = build_request_body(
            &options.model,
            &messages,
            options.tools.as_deref(),
            options.json_mode,
        );

        let payload = post_chat_completion(&http, &api_key, &body).await?;
        let choice = payload
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .ok_or_else(|| Error::string("OpenRouter response contained no choices"))?;

        if let Some(usage) = payload.get("usage") {
            total_web_search_requests += usage
                .pointer("/server_tool_use/web_search_requests")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32;
            last_input_tokens = usage
                .get("prompt_tokens")
                .and_then(Value::as_u64)
                .or_else(|| usage.get("input_tokens").and_then(Value::as_u64));
            last_output_tokens = usage
                .get("completion_tokens")
                .and_then(Value::as_u64)
                .or_else(|| usage.get("output_tokens").and_then(Value::as_u64));
        }

        let finish_reason = choice
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(str::to_string);
        last_finish_reason = finish_reason.clone();

        let message = choice
            .get("message")
            .cloned()
            .ok_or_else(|| Error::string("OpenRouter choice contained no message"))?;

        if finish_reason.as_deref() == Some("tool_calls") {
            let tool_calls = message
                .get("tool_calls")
                .and_then(Value::as_array)
                .filter(|calls| !calls.is_empty())
                .cloned();

            let assistant_text = extract_assistant_text(&message).unwrap_or_default();
            messages.push(message);

            if let Some(tool_calls) = tool_calls {
                let handler = options.client_tools.as_ref();
                let mut executed_client_tool = false;

                for tool_call in &tool_calls {
                    if is_server_tool_call(tool_call) {
                        continue;
                    }
                    let Some(handler) = handler else {
                        return Err(Error::string(&format!(
                            "OpenRouter returned client-side tool_calls ({}) but no client tool handler is configured",
                            summarize_tool_calls(&tool_calls)
                        )));
                    };
                    let tool_name = tool_call
                        .pointer("/function/name")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            Error::string("client tool call was missing function.name")
                        })?;
                    let arguments = tool_call
                        .pointer("/function/arguments")
                        .and_then(Value::as_str)
                        .unwrap_or("{}");
                    let tool_call_id = tool_call
                        .get("id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| Error::string("client tool call was missing id"))?;

                    let result = handler.execute(tool_name, arguments).await?;
                    total_client_tool_calls += 1;
                    executed_client_tool = true;
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": tool_call_id,
                        "content": result,
                    }));
                }

                if executed_client_tool || all_server_tool_calls(&tool_calls) {
                    if !assistant_text.trim().is_empty() {
                        last_text = assistant_text;
                    }
                    continue;
                }

                return Err(Error::string(&format!(
                    "OpenRouter returned unrecognized tool_calls: {}",
                    summarize_tool_calls(&tool_calls)
                )));
            }

            if !assistant_text.trim().is_empty() {
                last_text = assistant_text;
            }
            continue;
        }

        last_text = extract_assistant_text(&message).unwrap_or_default();
        if !last_text.trim().is_empty() {
            return Ok(ChatCompletionResult {
                text: last_text,
                finish_reason,
                web_search_requests: total_web_search_requests,
                input_tokens: last_input_tokens,
                output_tokens: last_output_tokens,
                agent_rounds: round + 1,
                client_tool_calls: total_client_tool_calls,
            });
        }

        if finish_reason.as_deref() != Some("tool_calls") {
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

fn build_request_body(
    model: &str,
    messages: &[Value],
    tools: Option<&[Value]>,
    json_mode: bool,
) -> Value {
    let mut body = json!({
        "model": model,
        "messages": messages,
    });

    if let Some(tools) = tools {
        body["tools"] = json!(tools);
    }

    if json_mode && tools.is_none() {
        body["response_format"] = json!({"type": "json_object"});
    }

    body
}

async fn post_chat_completion(
    http: &reqwest::Client,
    api_key: &str,
    body: &Value,
) -> Result<Value> {
    let response = http
        .post(OPENROUTER_CHAT_COMPLETIONS_URL)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {api_key}"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(body)
        .send()
        .await
        .map_err(|err| Error::string(&format!("OpenRouter request failed: {err}")))?;

    let status = response.status();
    let payload: Value = response
        .json()
        .await
        .map_err(|err| Error::string(&format!("OpenRouter response was not JSON: {err}")))?;

    if !status.is_success() {
        let message = payload
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("unknown OpenRouter error");
        return Err(Error::string(&format!(
            "OpenRouter request failed ({status}): {message}"
        )));
    }

    Ok(payload)
}

fn is_server_tool_call(tool_call: &Value) -> bool {
    tool_call
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind.contains("web_search") || kind.starts_with("openrouter:"))
        || tool_call
            .pointer("/function/name")
            .and_then(Value::as_str)
            .is_some_and(|name| name.contains("web_search"))
}

fn all_server_tool_calls(tool_calls: &[Value]) -> bool {
    tool_calls.iter().all(is_server_tool_call)
}

fn summarize_tool_calls(tool_calls: &[Value]) -> String {
    tool_calls
        .iter()
        .filter_map(|call| call.pointer("/function/name").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join(", ")
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
    fn detects_server_tool_calls() {
        let calls = vec![json!({
            "id": "call_1",
            "type": "function",
            "function": {"name": "web_search", "arguments": "{\"query\":\"oracle revenue xbrl\"}"}
        })];
        assert!(all_server_tool_calls(&calls));
        assert!(is_server_tool_call(&calls[0]));
    }

    #[test]
    fn workspace_sql_is_client_tool() {
        let call = json!({
            "id": "call_2",
            "type": "function",
            "function": {"name": "workspace_sql", "arguments": "{\"query\":\"SELECT 1\"}"}
        });
        assert!(!is_server_tool_call(&call));
    }
}
