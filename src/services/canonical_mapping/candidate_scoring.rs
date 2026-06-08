use super::{CanonicalMappingResolver, CanonicalResolutionContext, CanonicalResolutionResult};
use crate::services::concept_catalog::ConceptCatalog;
use async_trait::async_trait;

pub struct CandidateScoringResolver;

#[async_trait]
impl CanonicalMappingResolver for CandidateScoringResolver {
    fn strategy_id(&self) -> &'static str {
        "candidate_scoring"
    }

    async fn resolve(&self, ctx: &CanonicalResolutionContext<'_>) -> CanonicalResolutionResult {
        CanonicalResolutionResult {
            mappings: ConceptCatalog::seed_canonical_mappings(ctx.raw_sec_facts),
            review_decisions: Vec::new(),
            quality_flags: Vec::new(),
            strategy_id: self.strategy_id().to_string(),
        }
    }
}
