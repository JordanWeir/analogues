mod candidate_scoring;
mod llm_reviewed;

use crate::{
    services::concept_review::ConceptReviewDecisionRecord,
    workspace::{CanonicalMapping, ConceptCatalogEntry, SecRawFact},
};
use loco_rs::prelude::*;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConceptMappingStrategy {
    CandidateScoring,
    LlmReviewed,
}

impl ConceptMappingStrategy {
    pub fn from_var(value: &str) -> Result<Option<Self>> {
        match value {
            "none" | "skip" | "skip_mapping" => Ok(None),
            "candidate" | "candidate_scoring" | "heuristic" => Ok(Some(Self::CandidateScoring)),
            "llm" | "llm_reviewed" | "model" => Ok(Some(Self::LlmReviewed)),
            _ => Err(Error::string(
                "mapping_strategy must be none, candidate_scoring, or llm_reviewed",
            )),
        }
    }
}

pub use candidate_scoring::CandidateScoringResolver;
pub use llm_reviewed::LlmReviewedResolver;

#[derive(Debug, Clone)]
pub struct CanonicalResolutionContext<'a> {
    pub ticker: &'a str,
    pub raw_sec_facts: &'a [SecRawFact],
    pub catalog_entries: &'a [ConceptCatalogEntry],
    pub fetched_at: &'a str,
    /// Required for LLM review; should point at a workspace DB with ingest layers persisted.
    pub workspace_sqlite: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct CanonicalResolutionResult {
    pub mappings: Vec<CanonicalMapping>,
    pub review_decisions: Vec<ConceptReviewDecisionRecord>,
    pub quality_flags: Vec<String>,
    pub strategy_id: String,
}

#[async_trait]
pub trait CanonicalMappingResolver {
    fn strategy_id(&self) -> &'static str;
    async fn resolve(&self, ctx: &CanonicalResolutionContext<'_>) -> CanonicalResolutionResult;
}

pub async fn resolve_canonical_mappings(
    strategy: ConceptMappingStrategy,
    ctx: &CanonicalResolutionContext<'_>,
) -> CanonicalResolutionResult {
    match strategy {
        ConceptMappingStrategy::CandidateScoring => CandidateScoringResolver.resolve(ctx).await,
        ConceptMappingStrategy::LlmReviewed => LlmReviewedResolver::default().resolve(ctx).await,
    }
}

pub(crate) fn normalize_quality_flag(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::sec_facts_provider::extract_raw_facts_from_root;
    use serde_json::json;

    fn sample_facts() -> Vec<SecRawFact> {
        let facts_root = json!({
            "us-gaap": {
                "Revenues": {
                    "label": "Revenues",
                    "units": { "USD": [{"form":"10-K","start":"2025-01-01","end":"2025-12-31","filed":"2026-02-15","val":100.0}]}
                },
                "NetIncomeLoss": {
                    "label": "Net income",
                    "units": { "USD": [{"form":"10-K","start":"2025-01-01","end":"2025-12-31","filed":"2026-02-15","val":10.0}]}
                }
            }
        });
        extract_raw_facts_from_root(&facts_root, "2026-06-07T00:00:00Z")
    }

    #[tokio::test]
    async fn candidate_scoring_resolver_returns_mappings() {
        let facts = sample_facts();
        let entries =
            crate::services::concept_catalog::ConceptCatalog::materialize_catalog_entries(&facts);
        let ctx = CanonicalResolutionContext {
            ticker: "EXMP",
            raw_sec_facts: &facts,
            catalog_entries: &entries,
            fetched_at: "2026-06-07T00:00:00Z",
            workspace_sqlite: None,
        };
        let result = CandidateScoringResolver.resolve(&ctx).await;
        assert_eq!(result.strategy_id, "candidate_scoring");
        assert!(!result.mappings.is_empty());
        assert!(result.review_decisions.is_empty());
    }

    #[tokio::test]
    async fn llm_resolver_falls_back_without_workspace_sqlite() {
        let facts = sample_facts();
        let entries =
            crate::services::concept_catalog::ConceptCatalog::materialize_catalog_entries(&facts);
        let ctx = CanonicalResolutionContext {
            ticker: "EXMP",
            raw_sec_facts: &facts,
            catalog_entries: &entries,
            fetched_at: "2026-06-07T00:00:00Z",
            workspace_sqlite: None,
        };
        let result = LlmReviewedResolver::default().resolve(&ctx).await;
        assert_eq!(result.strategy_id, "candidate_scoring");
        assert!(!result.mappings.is_empty());
        assert!(result
            .quality_flags
            .iter()
            .any(|flag| flag.contains("llm_concept_review_workspace_missing")));
    }
}
