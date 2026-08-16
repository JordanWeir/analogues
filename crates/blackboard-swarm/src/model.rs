use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub total: u64,
}

impl TokenUsage {
    pub fn record(&mut self, input: u64, output: u64) {
        self.input += input;
        self.output += output;
        self.total += input + output;
    }
}

#[derive(Clone, Debug)]
pub struct ModelRequest {
    pub prompt: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub json_mode: bool,
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug)]
pub struct ModelResponse {
    pub text: String,
    pub parsed_json: Option<serde_json::Value>,
    pub usage: TokenUsage,
    pub model: String,
    pub latency_ms: u64,
}

#[async_trait]
pub trait ModelClient: Send + Sync {
    async fn complete(&self, request: ModelRequest) -> anyhow::Result<ModelResponse>;
}
