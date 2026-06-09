use crate::services::{
    concept_review::{ConceptReviewOutput, ConceptReviewService},
    openrouter_chat::ClientToolExecuteResult,
};
use loco_rs::prelude::*;
use openrouter_rs::types::Tool;
use serde_json::json;

pub const TOOL_NAME: &str = "submit_concept_review";

pub fn openrouter_tool() -> Tool {
    Tool::builder()
        .name(TOOL_NAME)
        .description(
            "Submit final canonical metric mapping decisions after workspace_sql exploration. \
             Call once when ready. Validation errors are returned so you can fix and resubmit. \
             Do not finish with a plain assistant message — use this tool.",
        )
        .parameters(json!({
            "type": "object",
            "properties": {
                "decisions": {
                    "type": "array",
                    "description": "One decision per row in canonical_metric_definitions",
                    "items": {
                        "type": "object",
                        "properties": {
                            "canonical_key": { "type": "string" },
                            "decision_type": {
                                "type": "string",
                                "enum": [
                                    "direct_mapping",
                                    "calculated_from_components",
                                    "unavailable",
                                    "review_required"
                                ]
                            },
                            "taxonomy": { "type": "string" },
                            "concept_name": { "type": "string" },
                            "unit": { "type": "string" },
                            "confidence": {
                                "type": "string",
                                "enum": ["high", "medium", "low", "review_required"]
                            },
                            "rationale": { "type": "string" },
                            "warnings": {
                                "type": "array",
                                "items": { "type": "string" }
                            },
                            "online_validation": { "type": "object" }
                        },
                        "required": [
                            "canonical_key",
                            "decision_type",
                            "confidence",
                            "rationale"
                        ]
                    }
                },
                "supporting_metrics": {
                    "type": "array",
                    "items": { "type": "object" }
                }
            },
            "required": ["decisions"]
        }))
        .build()
        .expect("submit_concept_review tool definition should be valid")
}

pub fn execute(arguments: &str) -> Result<ClientToolExecuteResult> {
    let output: ConceptReviewOutput = serde_json::from_str(arguments).map_err(|err| {
        Error::string(&format!(
            "submit_concept_review arguments were not valid JSON: {err}. \
             Pass an object with a decisions array matching the schema."
        ))
    })?;
    ConceptReviewService::validate_output(&output)?;
    let text = serde_json::to_string(&output).map_err(|err| {
        Error::string(&format!("failed to serialize accepted concept review output: {err}"))
    })?;
    Ok(ClientToolExecuteResult::Complete(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_minimal_valid_submission() {
        let arguments = json!({
            "decisions": [{
                "canonical_key": "revenue",
                "decision_type": "direct_mapping",
                "taxonomy": "us-gaap",
                "concept_name": "Revenues",
                "unit": "USD",
                "confidence": "high",
                "rationale": "Best revenue concept.",
                "warnings": []
            }],
            "supporting_metrics": []
        })
        .to_string();

        let result = execute(&arguments).expect("submission should validate");
        match result {
            ClientToolExecuteResult::Complete(text) => {
                assert!(text.contains("\"canonical_key\":\"revenue\""));
            }
            ClientToolExecuteResult::Response(_) => {
                panic!("expected Complete result");
            }
        }
    }

    #[test]
    fn rejects_direct_mapping_without_concept_fields() {
        let arguments = json!({
            "decisions": [{
                "canonical_key": "revenue",
                "decision_type": "direct_mapping",
                "confidence": "high",
                "rationale": "Missing concept fields."
            }]
        })
        .to_string();

        assert!(execute(&arguments).is_err());
    }

    #[test]
    fn rejects_unknown_canonical_key() {
        let arguments = json!({
            "decisions": [{
                "canonical_key": "not_a_metric",
                "decision_type": "unavailable",
                "confidence": "medium",
                "rationale": "Unknown key."
            }]
        })
        .to_string();

        assert!(execute(&arguments).is_err());
    }
}
