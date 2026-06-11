use super::{config::FundamentalCatalogManagerConfig, prompt, WORKER_NAME};
use crate::{
    agents::{
        tool_loop_agent::{ToolLoopAgent, ToolLoopRequest, ToolLoopResponse},
        tools::{ToolRegistry, WebSearchConfig},
    },
    services::{
        canonical_mapping::CanonicalResolutionContext,
        concept_review::{ConceptReviewOutput, ConceptReviewService},
    },
};
use loco_rs::prelude::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct FundamentalCatalogManagerAgent {
    config: FundamentalCatalogManagerConfig,
}

impl FundamentalCatalogManagerAgent {
    pub fn new(config: FundamentalCatalogManagerConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &FundamentalCatalogManagerConfig {
        &self.config
    }

    pub async fn review_workspace(
        &self,
        ctx: &CanonicalResolutionContext<'_>,
    ) -> Result<ConceptReviewOutput> {
        self.review_workspace_with_telemetry(ctx, "")
            .await
            .map(|(output, _)| output)
    }

    pub async fn review_workspace_with_telemetry(
        &self,
        ctx: &CanonicalResolutionContext<'_>,
        prompt_suffix: &str,
    ) -> Result<(ConceptReviewOutput, ToolLoopResponse)> {
        let workspace_sqlite = ctx.workspace_sqlite.clone().ok_or_else(|| {
            Error::string("fundamental_catalog_manager requires workspace_sqlite to be configured")
        })?;

        let mut tools = ToolRegistry::new()
            .with_sql_query(workspace_sqlite.clone())
            .with_concept_review_submit();
        if self.config.enable_web_search {
            tools = tools.with_web_search(WebSearchConfig::concept_validation_defaults());
        }

        let response = ToolLoopAgent::default()
            .run(ToolLoopRequest {
                worker_name: WORKER_NAME.to_string(),
                model: self.config.model.clone(),
                preamble: prompt::PREAMBLE.to_string(),
                prompt: prompt::build_user_prompt(ctx.ticker, prompt_suffix),
                json_mode: false,
                tools,
                metadata: BTreeMap::from([
                    ("lane".to_string(), "build_catalog".to_string()),
                    ("strategy".to_string(), "llm_reviewed".to_string()),
                    ("ticker".to_string(), ctx.ticker.to_string()),
                ]),
                workspace_sqlite: Some(workspace_sqlite),
                client_tools: None,
                max_agent_rounds: Some(self.config.max_agent_rounds),
                submit_tool_name: Some("submit_concept_review".to_string()),
                prepare_step: None,
                stop_when: None,
            })
            .await?;

        let output = ConceptReviewService::parse_output(&response.text).map_err(|err| {
            let preview: String = response.text.chars().take(500).collect();
            Error::string(&format!("{err}; raw model text preview: {preview}"))
        })?;
        Ok((output, response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::canonical_mapping::CanonicalResolutionContext;

    #[test]
    fn agent_request_metadata_fields() {
        let config = FundamentalCatalogManagerConfig {
            enable_web_search: true,
            ..Default::default()
        };
        let agent = FundamentalCatalogManagerAgent::new(config);
        assert_eq!(agent.config().model, "deepseek/deepseek-v4-flash");
        assert!(agent.config().enable_web_search);
    }

    #[tokio::test]
    async fn review_workspace_requires_workspace_sqlite() {
        let agent = FundamentalCatalogManagerAgent::new(FundamentalCatalogManagerConfig::default());
        let facts = Vec::new();
        let entries = Vec::new();
        let ctx = CanonicalResolutionContext {
            ticker: "EXMP",
            raw_sec_facts: &facts,
            catalog_entries: &entries,
            fetched_at: "2026-06-07T00:00:00Z",
            workspace_sqlite: None,
        };
        let err = agent
            .review_workspace(&ctx)
            .await
            .expect_err("missing sqlite");
        assert!(err.to_string().contains("workspace_sqlite"));
    }
}
