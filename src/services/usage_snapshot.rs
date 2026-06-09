//! Normalized token and billing usage from OpenRouter responses.
//!
//! Chat completions deserialize to [`openrouter_rs::types::completion::ResponseUsage`],
//! but that SDK type only models `prompt_tokens`, `completion_tokens`, `total_tokens`,
//! and cost fields. OpenRouter's upstream [`ChatUsage`] schema also reports cache
//! token details under `prompt_tokens_details` (`cached_tokens`, `cache_write_tokens`),
//! and some providers surface Anthropic-style aliases (`cache_read_input_tokens`,
//! `cache_creation_input_tokens`). We parse the raw `usage` object so worker runs
//! capture the full shape.
//!
//! [`ChatUsage`]: https://openrouter.ai/docs/api-reference/chat-completion#response-usage

use openrouter_rs::types::completion::ResponseUsage;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageSnapshot {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_reads: Option<u64>,
    pub cache_writes: Option<u64>,
    pub cost_usd: Option<f64>,
    pub web_search_requests: Option<u32>,
}

impl UsageSnapshot {
    /// Parse usage from a full chat completion JSON payload (`{ usage: ... }`).
    pub fn from_response_payload(payload: &Value) -> Self {
        payload
            .get("usage")
            .map(Self::from_usage_value)
            .unwrap_or_default()
    }

    /// Parse usage from a `usage` JSON object.
    pub fn from_usage_value(usage: &Value) -> Self {
        serde_json::from_value::<OpenRouterUsageBody>(usage.clone())
            .map(OpenRouterUsageBody::into_snapshot)
            .unwrap_or_else(|_| Self::from_usage_value_fallback(usage))
    }

    /// Fill any missing fields from the SDK's typed [`ResponseUsage`].
    pub fn merge_typed_usage(&mut self, usage: &ResponseUsage) {
        if self.input_tokens.is_none() {
            self.input_tokens = Some(usage.prompt_tokens as u64);
        }
        if self.output_tokens.is_none() {
            self.output_tokens = Some(usage.completion_tokens as u64);
        }
        if self.cost_usd.is_none() {
            self.cost_usd = usage.cost;
        }
    }

    /// Sum usage across multiple completion rounds in a tool loop.
    pub fn absorb(&mut self, other: &UsageSnapshot) {
        self.input_tokens = sum_option(self.input_tokens, other.input_tokens);
        self.output_tokens = sum_option(self.output_tokens, other.output_tokens);
        self.cache_reads = sum_option(self.cache_reads, other.cache_reads);
        self.cache_writes = sum_option(self.cache_writes, other.cache_writes);
        self.cost_usd = sum_option_f64(self.cost_usd, other.cost_usd);
        self.web_search_requests = sum_option(
            self.web_search_requests.map(u64::from),
            other.web_search_requests.map(u64::from),
        )
        .map(|value| value as u32);
    }

    fn from_usage_value_fallback(usage: &Value) -> Self {
        let mut snapshot = Self {
            input_tokens: usage
                .get("prompt_tokens")
                .or_else(|| usage.get("input_tokens"))
                .and_then(Value::as_u64),
            output_tokens: usage
                .get("completion_tokens")
                .or_else(|| usage.get("output_tokens"))
                .and_then(Value::as_u64),
            cost_usd: usage.get("cost").and_then(Value::as_f64),
            web_search_requests: usage
                .pointer("/server_tool_use/web_search_requests")
                .and_then(Value::as_u64)
                .map(|value| value as u32),
            ..UsageSnapshot::default()
        };
        snapshot.cache_reads = usage
            .pointer("/prompt_tokens_details/cached_tokens")
            .or_else(|| usage.get("cache_read_input_tokens"))
            .and_then(Value::as_u64);
        snapshot.cache_writes = usage
            .pointer("/prompt_tokens_details/cache_write_tokens")
            .or_else(|| usage.get("cache_creation_input_tokens"))
            .and_then(Value::as_u64);
        snapshot
    }
}

#[derive(Debug, Deserialize)]
struct OpenRouterUsageBody {
    #[serde(alias = "input_tokens")]
    prompt_tokens: Option<u32>,
    #[serde(alias = "output_tokens")]
    completion_tokens: Option<u32>,
    #[serde(default)]
    prompt_tokens_details: Option<PromptTokenDetails>,
    cache_read_input_tokens: Option<u32>,
    cache_creation_input_tokens: Option<u32>,
    cost: Option<f64>,
    #[serde(default)]
    server_tool_use: Option<ServerToolUse>,
}

#[derive(Debug, Deserialize)]
struct PromptTokenDetails {
    cached_tokens: Option<u32>,
    cache_write_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ServerToolUse {
    web_search_requests: Option<u32>,
}

impl OpenRouterUsageBody {
    fn into_snapshot(self) -> UsageSnapshot {
        UsageSnapshot {
            input_tokens: self.prompt_tokens.map(u64::from),
            output_tokens: self.completion_tokens.map(u64::from),
            cache_reads: self
                .prompt_tokens_details
                .as_ref()
                .and_then(|details| details.cached_tokens)
                .or(self.cache_read_input_tokens)
                .map(u64::from),
            cache_writes: self
                .prompt_tokens_details
                .as_ref()
                .and_then(|details| details.cache_write_tokens)
                .or(self.cache_creation_input_tokens)
                .map(u64::from),
            cost_usd: self.cost,
            web_search_requests: self
                .server_tool_use
                .and_then(|usage| usage.web_search_requests),
        }
    }
}

fn sum_option(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(a), Some(b)) => Some(a + b),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn sum_option_f64(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(a), Some(b)) => Some(a + b),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_chat_usage_with_prompt_token_details() {
        let usage = json!({
            "prompt_tokens": 10,
            "completion_tokens": 15,
            "total_tokens": 25,
            "cost": 0.0012,
            "prompt_tokens_details": {
                "cached_tokens": 2,
                "cache_write_tokens": 3
            },
            "server_tool_use": {
                "web_search_requests": 1
            }
        });

        let snapshot = UsageSnapshot::from_usage_value(&usage);
        assert_eq!(
            snapshot,
            UsageSnapshot {
                input_tokens: Some(10),
                output_tokens: Some(15),
                cache_reads: Some(2),
                cache_writes: Some(3),
                cost_usd: Some(0.0012),
                web_search_requests: Some(1),
            }
        );
    }

    #[test]
    fn parses_anthropic_style_cache_aliases() {
        let usage = json!({
            "input_tokens": 100,
            "output_tokens": 50,
            "cache_read_input_tokens": 40,
            "cache_creation_input_tokens": 12
        });

        let snapshot = UsageSnapshot::from_usage_value(&usage);
        assert_eq!(snapshot.input_tokens, Some(100));
        assert_eq!(snapshot.output_tokens, Some(50));
        assert_eq!(snapshot.cache_reads, Some(40));
        assert_eq!(snapshot.cache_writes, Some(12));
    }

    #[test]
    fn prefers_prompt_tokens_details_over_aliases() {
        let usage = json!({
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "prompt_tokens_details": {
                "cached_tokens": 2,
                "cache_write_tokens": 3
            },
            "cache_read_input_tokens": 99,
            "cache_creation_input_tokens": 88
        });

        let snapshot = UsageSnapshot::from_usage_value(&usage);
        assert_eq!(snapshot.cache_reads, Some(2));
        assert_eq!(snapshot.cache_writes, Some(3));
    }

    #[test]
    fn merges_typed_response_usage() {
        let mut snapshot = UsageSnapshot::default();
        let typed = ResponseUsage::new(12, 7, 19);
        snapshot.merge_typed_usage(&typed);
        assert_eq!(snapshot.input_tokens, Some(12));
        assert_eq!(snapshot.output_tokens, Some(7));
    }

    #[test]
    fn absorbs_usage_across_rounds() {
        let mut total = UsageSnapshot {
            input_tokens: Some(10),
            output_tokens: Some(5),
            cache_reads: Some(2),
            cache_writes: Some(1),
            cost_usd: Some(0.001),
            web_search_requests: Some(1),
        };
        total.absorb(&UsageSnapshot {
            input_tokens: Some(8),
            output_tokens: Some(4),
            cache_reads: Some(3),
            cache_writes: Some(2),
            cost_usd: Some(0.002),
            web_search_requests: Some(2),
        });

        assert_eq!(total.input_tokens, Some(18));
        assert_eq!(total.output_tokens, Some(9));
        assert_eq!(total.cache_reads, Some(5));
        assert_eq!(total.cache_writes, Some(3));
        assert!((total.cost_usd.expect("cost") - 0.003).abs() < f64::EPSILON);
        assert_eq!(total.web_search_requests, Some(3));
    }
}
