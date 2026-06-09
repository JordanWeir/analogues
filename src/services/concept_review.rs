use crate::{
    services::{
        concept_catalog::ConceptCatalog,
        model_client::{extract_json_blob, ModelClient, ModelRequest, WebSearchToolConfig},
        review_workspace::{concept_review_golden_path, workspace_schema_hint},
    },
    workspace::{CanonicalMapping, SecRawFact},
};
use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

pub const DEFAULT_REVIEW_PREAMBLE: &str = "You review SEC Company Facts concept catalogs. Return only valid JSON. Prefer precise audited mappings over broad name matches. If a concept needs calculation from components, do not mark it as a direct mapping.";

pub const AGENT_REVIEW_PREAMBLE: &str = "You are the Fundamental Catalog Manager for a public company workspace. The database has raw SEC facts and a derived concept catalog, but no canonical metric mappings yet. Your job is to link each product metric in canonical_metric_definitions to the best company-specific SEC XBRL concept(s), or declare calculated_from_components / unavailable / review_required when appropriate. Use workspace_sql following the golden path: search concept_catalog_entries first (use latest_period_end, series_usability, dominant_period_shape), then spot-check sec_raw_facts. Issue multiple independent workspace_sql calls in one turn when exploring different metrics. Use web search to validate that promoted concepts and their latest values align with public filings or investor materials when web search is available. When finished exploring, call submit_concept_review with your final decisions — do not end with a plain assistant message. If submit_concept_review returns validation errors, fix the payload and call it again. Prefer precise audited balance-sheet concepts over maturity schedules, rollforwards, or flow items when a balance is required. You have a limited step budget; after each tool round the user message will report steps remaining. On the penultimate step, submit your final answer so the last step can repair validation errors.";

#[derive(Debug, Clone)]
pub struct ConceptReviewService {
    pub model: String,
    pub enable_web_search: bool,
    pub enable_workspace_sql: bool,
    pub company_label: Option<String>,
    pub workspace_sqlite: Option<PathBuf>,
}

impl Default for ConceptReviewService {
    fn default() -> Self {
        Self {
            model: "deepseek/deepseek-v4-flash".to_string(),
            enable_web_search: false,
            enable_workspace_sql: false,
            company_label: None,
            workspace_sqlite: None,
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
    #[serde(default)]
    pub online_validation: Option<OnlineValidation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineValidation {
    pub status: String,
    pub summary: String,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub search_queries: Vec<String>,
    #[serde(default)]
    pub db_latest_value: Option<f64>,
    #[serde(default)]
    pub db_latest_period_end: Option<String>,
    #[serde(default)]
    pub online_value_note: Option<String>,
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

impl ConceptReviewService {
    pub async fn review_workspace(
        &self,
        client: &dyn ModelClient,
        _raw_facts: &[SecRawFact],
        preamble: &str,
        prompt_suffix: &str,
    ) -> Result<(
        ConceptReviewOutput,
        crate::services::model_client::ModelResponse,
    )> {
        let workspace_sqlite = self.workspace_sqlite.clone().ok_or_else(|| {
            Error::string("concept review agent requires workspace_sqlite to be configured")
        })?;

        let web_search = self
            .enable_web_search
            .then(WebSearchToolConfig::concept_validation_defaults);

        let mut prompt = self.agent_prompt()?;
        if !prompt_suffix.is_empty() {
            prompt.push_str(prompt_suffix);
        }

        let response = client
            .complete(ModelRequest {
                model: self.model.clone(),
                preamble: preamble.to_string(),
                prompt,
                json_mode: false,
                metadata: BTreeMap::from([(
                    "worker_lane".to_string(),
                    "concept_catalog_review".to_string(),
                )]),
                web_search,
                workspace_sqlite: Some(workspace_sqlite),
                client_tools: None,
            })
            .await?;

        let output = Self::parse_output(&response.text).map_err(|err| {
            let preview: String = response.text.chars().take(500).collect();
            Error::string(&format!("{err}; raw model text preview: {preview}"))
        })?;
        Ok((output, response))
    }

    pub async fn review_candidates(
        &self,
        client: &dyn ModelClient,
        raw_facts: &[SecRawFact],
    ) -> Result<ConceptReviewOutput> {
        self.review_workspace(client, raw_facts, AGENT_REVIEW_PREAMBLE, "")
            .await
            .map(|(output, _)| output)
    }

    pub fn build_prompt(&self) -> Result<String> {
        self.agent_prompt()
    }

    pub fn parse_output(text: &str) -> Result<ConceptReviewOutput> {
        let json_text = extract_json_blob(text).ok_or_else(|| {
            Error::string(
                "concept review response did not contain a JSON object; call submit_concept_review or return valid JSON",
            )
        })?;
        let output: ConceptReviewOutput = serde_json::from_str(json_text).map_err(|err| {
            let preview: String = json_text.chars().take(240).collect();
            Error::string(&format!(
                "concept review response was not valid ConceptReviewOutput JSON: {err} (preview: {preview})"
            ))
        })?;
        Self::validate_output(&output)?;
        Ok(output)
    }

    pub fn validate_output(output: &ConceptReviewOutput) -> Result<()> {
        if output.decisions.is_empty() {
            return Err(Error::string(
                "submit_concept_review requires at least one decision",
            ));
        }

        let mut seen_keys = HashSet::new();
        for decision in &output.decisions {
            if !ConceptCatalog::is_known_canonical_key(&decision.canonical_key) {
                return Err(Error::string(&format!(
                    "unknown canonical_key: {}",
                    decision.canonical_key
                )));
            }
            if !seen_keys.insert(decision.canonical_key.clone()) {
                return Err(Error::string(&format!(
                    "duplicate canonical_key: {}",
                    decision.canonical_key
                )));
            }
            match decision.decision_type.as_str() {
                "direct_mapping"
                | "calculated_from_components"
                | "unavailable"
                | "review_required" => {}
                other => {
                    return Err(Error::string(&format!(
                        "invalid decision_type for {}: {other}",
                        decision.canonical_key
                    )));
                }
            }
            match decision.confidence.as_str() {
                "high" | "medium" | "low" | "review_required" => {}
                other => {
                    return Err(Error::string(&format!(
                        "invalid confidence for {}: {other}",
                        decision.canonical_key
                    )));
                }
            }
            if decision.rationale.trim().is_empty() {
                return Err(Error::string(&format!(
                    "{} requires a non-empty rationale",
                    decision.canonical_key
                )));
            }
            if decision.decision_type == "direct_mapping"
                && (decision.taxonomy.as_deref().unwrap_or("").is_empty()
                    || decision.concept_name.as_deref().unwrap_or("").is_empty()
                    || decision.unit.as_deref().unwrap_or("").is_empty())
            {
                return Err(Error::string(&format!(
                    "{} direct_mapping requires taxonomy, concept_name, and unit",
                    decision.canonical_key
                )));
            }
        }

        Ok(())
    }

    pub fn promote_reviewed_mappings(
        &self,
        output: &ConceptReviewOutput,
        raw_facts: &[SecRawFact],
    ) -> PromotedReview {
        let mut warnings = Vec::new();
        let mut mappings = Vec::new();
        let selected_by = format!("llm_agent_review:{}", self.model);

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
            let (Some(taxonomy), Some(concept_name), Some(unit)) = (
                decision.taxonomy.as_deref(),
                decision.concept_name.as_deref(),
                decision.unit.as_deref(),
            ) else {
                warnings.push(format!(
                    "{} was not promoted because taxonomy, concept_name, or unit was missing",
                    decision.canonical_key
                ));
                continue;
            };

            let Some(mapping) = ConceptCatalog::mapping_from_review_decision(
                &decision.canonical_key,
                taxonomy,
                concept_name,
                unit,
                &decision.confidence,
                &decision.rationale,
                &selected_by,
                raw_facts,
            ) else {
                warnings.push(format!(
                    "{} was not promoted because the selected concept was not found in sec_raw_facts or canonical_key is unknown",
                    decision.canonical_key
                ));
                continue;
            };

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

    fn agent_prompt(&self) -> Result<String> {
        let company_context = self
            .company_label
            .as_deref()
            .map(|label| format!("Company: {label}\n\n"))
            .unwrap_or_default();
        Ok(format!(
            r#"{company_context}{schema}

{golden_path}

Output:
When finished, call submit_concept_review with this shape:
{{"decisions":[{{"canonical_key":"revenue","decision_type":"direct_mapping|calculated_from_components|unavailable|review_required","taxonomy":"us-gaap","concept_name":"Revenues","unit":"USD","confidence":"high|medium|low|review_required","rationale":"...","warnings":[],"online_validation":{{"status":"aligned|misaligned|inconclusive","summary":"...","sources":["https://..."],"search_queries":["..."],"db_latest_value":17190000000.0,"db_latest_period_end":"2026-02-28","online_value_note":"..."}}}}],"supporting_metrics":[]}}

Emit one decision per row in canonical_metric_definitions. Validation errors from submit_concept_review should be corrected and resubmitted."#,
            schema = workspace_schema_hint(),
            golden_path = concept_review_golden_path(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::concept_catalog::CanonicalMappingCandidate;

    fn sample_fact(taxonomy: &str, concept_name: &str, unit: &str) -> SecRawFact {
        SecRawFact {
            taxonomy: taxonomy.to_string(),
            concept_name: concept_name.to_string(),
            label: None,
            description: None,
            unit: unit.to_string(),
            form: None,
            start: None,
            end: Some("2026-02-28".to_string()),
            filed: None,
            fiscal_year: None,
            fiscal_period: None,
            accession: None,
            frame: None,
            value: 100.0,
            raw_json: "{}".to_string(),
            fetched_at: "2026-06-07".to_string(),
        }
    }

    #[test]
    fn validate_output_requires_direct_mapping_fields() {
        let output = ConceptReviewOutput {
            decisions: vec![ConceptReviewDecision {
                canonical_key: "revenue".to_string(),
                decision_type: "direct_mapping".to_string(),
                taxonomy: None,
                concept_name: None,
                unit: None,
                confidence: "high".to_string(),
                rationale: "Missing concept.".to_string(),
                warnings: Vec::new(),
                online_validation: None,
            }],
            supporting_metrics: Vec::new(),
        };

        assert!(ConceptReviewService::validate_output(&output).is_err());
    }

    #[test]
    fn promotes_direct_mappings_found_in_raw_facts() {
        let service = ConceptReviewService::default();
        let raw_facts = vec![sample_fact(
            "us-gaap",
            "RevenueFromContractWithCustomerExcludingAssessedTax",
            "USD",
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
                    online_validation: None,
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
                    online_validation: None,
                },
            ],
            supporting_metrics: Vec::new(),
        };

        let promoted = service.promote_reviewed_mappings(&output, &raw_facts);

        assert_eq!(promoted.mappings.len(), 1);
        assert_eq!(promoted.mappings[0].confidence, "high");
        assert!(promoted.mappings[0]
            .selected_by
            .starts_with("llm_agent_review:"));
        assert_eq!(promoted.warnings.len(), 1);
    }

    #[allow(dead_code)]
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
}
