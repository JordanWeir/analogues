use async_trait::async_trait;
use loco_rs::prelude::*;
use rig::{
    client::{CompletionClient, ProviderClient},
    completion::Prompt,
    providers::openrouter,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub model: String,
    pub preamble: String,
    pub prompt: String,
    pub json_mode: bool,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    pub text: String,
    pub model: String,
    pub latency_ms: u128,
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
        })
    }
}
