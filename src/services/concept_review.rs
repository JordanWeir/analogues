use crate::{
    services::{
        concept_catalog::CanonicalMappingCandidate,
        model_client::{ModelClient, ModelRequest},
    },
    tasks::init_workspace::CanonicalMapping,
};
use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEFAULT_REVIEW_PREAMBLE: &str = "You review SEC Company Facts concept catalogs. Return only valid JSON. Prefer precise audited mappings over broad name matches. If a concept needs calculation from components, do not mark it as a direct mapping.";

#[derive(Debug, Clone)]
pub struct ConceptReviewService {
    pub model: String,
    pub max_candidates_per_metric: usize,
}

impl Default for ConceptReviewService {
    fn default() -> Self {
        Self {
            model: "deepseek/deepseek-v4-flash".to_string(),
            max_candidates_per_metric: 8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptReviewOutput {
    pub decisions: Vec<ConceptReviewDecision>,
    #[serde(default)]
    pub supporting_metrics: Vec<SupportingMetricCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptReviewDecision {
    pub canonical_key: String,
    pub decision_type: String,
    pub taxonomy: Option<String>,
    pub concept_name: Option<String>,
    pub unit: Option<String>,
    pub confidence: String,
    pub rationale: String,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportingMetricCandidate {
    pub selection_scope: String,
    pub taxonomy: String,
    pub concept_name: String,
    pub unit: String,
    pub label: Option<String>,
    pub rationale: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptReviewDecisionRecord {
    pub review_run_id: String,
    pub canonical_key: Option<String>,
    pub decision_type: String,
    pub taxonomy: Option<String>,
    pub concept_name: Option<String>,
    pub unit: Option<String>,
    pub confidence: String,
    pub rationale: String,
    pub selected_by: String,
    pub warnings: Vec<String>,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct PromotedReview {
    pub mappings: Vec<CanonicalMapping>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CandidatePromptRow<'a> {
    canonical_key: &'a str,
    metric_label: &'a str,
    taxonomy: &'a str,
    concept_name: &'a str,
    unit: &'a str,
    confidence: &'a str,
    score: i64,
    fact_count: i64,
    rationale: &'a str,
}

impl ConceptReviewService {
    pub async fn review_candidates(
        &self,
        client: &dyn ModelClient,
        candidates: &[CanonicalMappingCandidate],
    ) -> Result<ConceptReviewOutput> {
        self.review_candidates_with_preamble(client, candidates, DEFAULT_REVIEW_PREAMBLE, "")
            .await
            .map(|(output, _)| output)
    }

    pub fn build_prompt(&self, candidates: &[CanonicalMappingCandidate]) -> Result<String> {
        self.prompt(candidates)
    }

    pub fn parse_output(text: &str) -> Result<ConceptReviewOutput> {
        serde_json::from_str(text).map_err(|err| {
            Error::string(&format!(
                "concept review response was not valid ConceptReviewOutput JSON: {err}"
            ))
        })
    }

    pub async fn review_candidates_with_preamble(
        &self,
        client: &dyn ModelClient,
        candidates: &[CanonicalMappingCandidate],
        preamble: &str,
        prompt_suffix: &str,
    ) -> Result<(ConceptReviewOutput, crate::services::model_client::ModelResponse)> {
        let mut prompt = self.prompt(candidates)?;
        if !prompt_suffix.is_empty() {
            prompt.push_str(prompt_suffix);
        }
        let response = client
            .complete(ModelRequest {
                model: self.model.clone(),
                preamble: preamble.to_string(),
                prompt,
                json_mode: true,
                metadata: BTreeMap::from([(
                    "worker_lane".to_string(),
                    "concept_catalog_review".to_string(),
                )]),
            })
            .await?;

        let output = Self::parse_output(&response.text)?;
        Ok((output, response))
    }

    pub fn promote_reviewed_mappings(
        &self,
        output: &ConceptReviewOutput,
        candidates: &[CanonicalMappingCandidate],
    ) -> PromotedReview {
        let mut warnings = Vec::new();
        let mut mappings = Vec::new();
        for decision in &output.decisions {
            if decision.decision_type != "direct_mapping" {
                warnings.push(format!(
                    "{} was not promoted because decision_type was {}",
                    decision.canonical_key, decision.decision_type
                ));
                continue;
            }
            if matches!(decision.confidence.as_str(), "low" | "review_required") {
                warnings.push(format!(
                    "{} was not promoted because confidence was {}",
                    decision.canonical_key, decision.confidence
                ));
                continue;
            }
            let Some(candidate) = candidates.iter().find(|candidate| {
                candidate.mapping.canonical_key == decision.canonical_key
                    && Some(candidate.mapping.taxonomy.as_str()) == decision.taxonomy.as_deref()
                    && Some(candidate.mapping.concept_name.as_str())
                        == decision.concept_name.as_deref()
                    && Some(candidate.mapping.unit.as_str()) == decision.unit.as_deref()
            }) else {
                warnings.push(format!(
                    "{} was not promoted because the selected concept was not a gated candidate",
                    decision.canonical_key
                ));
                continue;
            };

            let mut mapping = candidate.mapping.clone();
            mapping.confidence = decision.confidence.clone();
            mapping.rationale = decision.rationale.clone();
            mapping.selected_by = format!("llm_batch_review:{}", self.model);
            mapping.is_active = true;
            mappings.push(mapping);
        }

        PromotedReview { mappings, warnings }
    }

    pub fn decision_records(
        &self,
        output: &ConceptReviewOutput,
        review_run_id: &str,
        selected_by: &str,
        created_at: &str,
    ) -> Vec<ConceptReviewDecisionRecord> {
        output
            .decisions
            .iter()
            .map(|decision| ConceptReviewDecisionRecord {
                review_run_id: review_run_id.to_string(),
                canonical_key: Some(decision.canonical_key.clone()),
                decision_type: decision.decision_type.clone(),
                taxonomy: decision.taxonomy.clone(),
                concept_name: decision.concept_name.clone(),
                unit: decision.unit.clone(),
                confidence: decision.confidence.clone(),
                rationale: decision.rationale.clone(),
                selected_by: selected_by.to_string(),
                warnings: decision.warnings.clone(),
                payload_json: serde_json::to_string(decision).unwrap_or_else(|_| "{}".to_string()),
                created_at: created_at.to_string(),
            })
            .collect()
    }

    fn prompt(&self, candidates: &[CanonicalMappingCandidate]) -> Result<String> {
        let mut by_metric: BTreeMap<&str, Vec<&CanonicalMappingCandidate>> = BTreeMap::new();
        for candidate in candidates {
            by_metric
                .entry(candidate.mapping.canonical_key.as_str())
                .or_default()
                .push(candidate);
        }

        let mut rows = Vec::new();
        for candidates in by_metric.values_mut() {
            candidates.sort_by(|left, right| {
                (right.score, right.fact_count).cmp(&(left.score, left.fact_count))
            });
            rows.extend(
                candidates
                    .iter()
                    .take(self.max_candidates_per_metric)
                    .map(|candidate| CandidatePromptRow {
                        canonical_key: &candidate.mapping.canonical_key,
                        metric_label: &candidate.mapping.metric_label,
                        taxonomy: &candidate.mapping.taxonomy,
                        concept_name: &candidate.mapping.concept_name,
                        unit: &candidate.mapping.unit,
                        confidence: &candidate.mapping.confidence,
                        score: candidate.score,
                        fact_count: candidate.fact_count,
                        rationale: &candidate.mapping.rationale,
                    }),
            );
        }

        let candidate_json = serde_json::to_string_pretty(&rows)
            .map_err(|err| Error::string(&format!("failed to serialize candidates: {err}")))?;
        Ok(format!(
            "Review these SEC concept candidates and return JSON matching this shape: {{\"decisions\":[{{\"canonical_key\":\"revenue\",\"decision_type\":\"direct_mapping|calculated_from_components|unavailable|review_required\",\"taxonomy\":\"us-gaap\",\"concept_name\":\"Revenues\",\"unit\":\"USD\",\"confidence\":\"high|medium|low|review_required\",\"rationale\":\"...\",\"warnings\":[]}}],\"supporting_metrics\":[]}}.\n\nCandidates:\n{candidate_json}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(canonical_key: &str, concept_name: &str) -> CanonicalMappingCandidate {
        CanonicalMappingCandidate {
            mapping: CanonicalMapping {
                canonical_key: canonical_key.to_string(),
                metric_key: canonical_key.to_string(),
                metric_label: canonical_key.to_string(),
                statement_type: "income_statement".to_string(),
                taxonomy: "us-gaap".to_string(),
                concept_name: concept_name.to_string(),
                unit: "USD".to_string(),
                confidence: "medium".to_string(),
                rationale: "candidate".to_string(),
                selected_by: "catalog_candidate_scoring".to_string(),
                is_active: true,
            },
            score: 80,
            fact_count: 12,
            latest_period_end: None,
        }
    }

    #[test]
    fn promotes_only_direct_gated_review_decisions() {
        let service = ConceptReviewService::default();
        let candidates = vec![candidate(
            "revenue",
            "RevenueFromContractWithCustomerExcludingAssessedTax",
        )];
        let output = ConceptReviewOutput {
            decisions: vec![
                ConceptReviewDecision {
                    canonical_key: "revenue".to_string(),
                    decision_type: "direct_mapping".to_string(),
                    taxonomy: Some("us-gaap".to_string()),
                    concept_name: Some(
                        "RevenueFromContractWithCustomerExcludingAssessedTax".to_string(),
                    ),
                    unit: Some("USD".to_string()),
                    confidence: "high".to_string(),
                    rationale: "Best company revenue concept.".to_string(),
                    warnings: Vec::new(),
                },
                ConceptReviewDecision {
                    canonical_key: "net_income".to_string(),
                    decision_type: "unavailable".to_string(),
                    taxonomy: None,
                    concept_name: None,
                    unit: None,
                    confidence: "review_required".to_string(),
                    rationale: "No reliable candidate.".to_string(),
                    warnings: Vec::new(),
                },
            ],
            supporting_metrics: Vec::new(),
        };

        let promoted = service.promote_reviewed_mappings(&output, &candidates);

        assert_eq!(promoted.mappings.len(), 1);
        assert_eq!(promoted.mappings[0].confidence, "high");
        assert!(promoted.mappings[0]
            .selected_by
            .starts_with("llm_batch_review:"));
        assert_eq!(promoted.warnings.len(), 1);
    }
}
