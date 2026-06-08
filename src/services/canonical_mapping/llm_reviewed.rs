use super::{
    normalize_quality_flag, CandidateScoringResolver, CanonicalMappingResolver,
    CanonicalResolutionContext, CanonicalResolutionResult,
};
use crate::services::{
    concept_review::{ConceptReviewService, AGENT_REVIEW_PREAMBLE},
    model_client::OpenRouterModelClient,
};
use async_trait::async_trait;
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct LlmReviewedResolver {
    pub enable_web_search: Option<bool>,
}

impl LlmReviewedResolver {
    fn web_search_enabled(&self) -> bool {
        self.enable_web_search
            .unwrap_or_else(concept_review_web_search_enabled)
    }
}

#[async_trait]
impl CanonicalMappingResolver for LlmReviewedResolver {
    fn strategy_id(&self) -> &'static str {
        "llm_reviewed"
    }

    async fn resolve(&self, ctx: &CanonicalResolutionContext<'_>) -> CanonicalResolutionResult {
        let Some(workspace_sqlite) = ctx.workspace_sqlite.clone() else {
            let fallback = CandidateScoringResolver.resolve(ctx).await;
            return CanonicalResolutionResult {
                quality_flags: vec![
                    "llm_concept_review_workspace_missing_used_candidate_scoring".to_string(),
                ],
                strategy_id: fallback.strategy_id,
                ..fallback
            };
        };

        let service = ConceptReviewService {
            enable_web_search: self.web_search_enabled(),
            enable_workspace_sql: true,
            company_label: Some(ctx.ticker.to_string()),
            workspace_sqlite: Some(workspace_sqlite),
            ..ConceptReviewService::default()
        };
        let client = OpenRouterModelClient;
        let review_result = service
            .review_workspace(&client, ctx.raw_sec_facts, AGENT_REVIEW_PREAMBLE, "")
            .await;

        match review_result {
            Ok((output, _response)) => {
                let review_run_id = Uuid::new_v4().to_string();
                let selected_by = format!("llm_agent_review:{}", service.model);
                let decisions =
                    service.decision_records(&output, &review_run_id, &selected_by, ctx.fetched_at);
                let promoted = service.promote_reviewed_mappings(&output, ctx.raw_sec_facts);
                let mut quality_flags = promoted
                    .warnings
                    .into_iter()
                    .map(|warning| {
                        format!(
                            "llm_concept_review_warning_{}",
                            normalize_quality_flag(&warning)
                        )
                    })
                    .collect::<Vec<_>>();
                let mappings = if promoted.mappings.is_empty() {
                    quality_flags.push(
                        "llm_concept_review_returned_no_promoted_mappings_used_candidate_scoring"
                            .to_string(),
                    );
                    CandidateScoringResolver.resolve(ctx).await.mappings
                } else {
                    promoted.mappings
                };
                CanonicalResolutionResult {
                    mappings,
                    review_decisions: decisions,
                    quality_flags,
                    strategy_id: format!("llm_agent_review:{}", service.model),
                }
            }
            Err(err) => {
                let fallback = CandidateScoringResolver.resolve(ctx).await;
                // @TODO: SHouldn't this merge any quality flags from both approaches? Currently passes llm flag, without the fallback flags.
                CanonicalResolutionResult {
                    quality_flags: vec![format!(
                        "llm_concept_review_failed_used_candidate_scoring_{}",
                        normalize_quality_flag(&err.to_string())
                    )],
                    strategy_id: fallback.strategy_id,
                    mappings: fallback.mappings,
                    review_decisions: fallback.review_decisions,
                }
            }
        }
    }
}

fn concept_review_web_search_enabled() -> bool {
    // @TODO: This doesn't feel like it should be an env var.
    std::env::var("CONCEPT_REVIEW_WEB_SEARCH")
        .ok()
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes"))
}
