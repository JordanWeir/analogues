use super::{
    normalize_quality_flag, CandidateScoringResolver, CanonicalMappingResolver,
    CanonicalResolutionContext, CanonicalResolutionResult,
};
use crate::{
    agents::fundamental_catalog_manager::FundamentalCatalogManagerAgent,
    services::concept_review::ConceptReviewService,
};
use async_trait::async_trait;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct LlmReviewedResolver {
    pub agent_config: crate::agents::fundamental_catalog_manager::FundamentalCatalogManagerConfig,
}

impl Default for LlmReviewedResolver {
    fn default() -> Self {
        Self {
            agent_config:
                crate::agents::fundamental_catalog_manager::FundamentalCatalogManagerConfig::default(
                ),
        }
    }
}

#[async_trait]
impl CanonicalMappingResolver for LlmReviewedResolver {
    fn strategy_id(&self) -> &'static str {
        "llm_reviewed"
    }

    async fn resolve(&self, ctx: &CanonicalResolutionContext<'_>) -> CanonicalResolutionResult {
        if ctx.workspace_sqlite.is_none() {
            let fallback = CandidateScoringResolver.resolve(ctx).await;
            return CanonicalResolutionResult {
                quality_flags: vec![
                    "llm_concept_review_workspace_missing_used_candidate_scoring".to_string(),
                ],
                strategy_id: fallback.strategy_id,
                ..fallback
            };
        }

        let agent = FundamentalCatalogManagerAgent::new(self.agent_config.clone());
        let review_result = agent.review_workspace_with_telemetry(ctx, "").await;

        match review_result {
            Ok((output, _response)) => {
                let model = agent.config().model.clone();
                let review_run_id = Uuid::new_v4().to_string();
                let selected_by = format!("llm_agent_review:{model}");
                let decisions = ConceptReviewService::decision_records(
                    &output,
                    &review_run_id,
                    &selected_by,
                    ctx.fetched_at,
                );
                let promoted = ConceptReviewService::promote_reviewed_mappings(
                    &model,
                    &output,
                    ctx.raw_sec_facts,
                );
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
                    strategy_id: format!("llm_agent_review:{model}"),
                }
            }
            Err(err) => {
                let fallback = CandidateScoringResolver.resolve(ctx).await;
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
