use crate::{
    agents::financial_model_explorer::{
        explorer_context::load_explorer_context, FinancialModelExplorerAgent, types::CruxTriageOutput,
    },
    services::openrouter_chat::ClientToolExecuteResult,
};
use loco_rs::prelude::*;
use openrouter_rs::types::Tool;
use serde_json::json;
use std::path::PathBuf;

pub const TOOL_NAME: &str = "submit_crux_triage";

pub fn openrouter_tool() -> Tool {
    Tool::builder()
        .name(TOOL_NAME)
        .description(
            "Submit final crux triage output after workspace_sql exploration. \
             quality_flags entries use flag_key (not gap_key). \
             open_questions entries use gap_key (not flag_key). \
             Per-crux focus: submit one crux with at least one supporting_metric. \
             Batch mode: 2+ promoted cruxes when narrative has 3+ crux items. \
             Validation errors are returned so you can fix and resubmit.",
        )
        .parameters(json!({
            "type": "object",
            "properties": {
                "cruxes": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "crux_key": { "type": "string" },
                            "title": { "type": "string" },
                            "statement": { "type": "string" },
                            "bridge_archetype": { "type": "string" },
                            "narrative_side": { "type": "string" },
                            "watch_condition": { "type": "string" },
                            "confirming_signal": { "type": "string" },
                            "breaking_signal": { "type": "string" },
                            "disposition": { "type": "string" },
                            "rationale": { "type": "string" },
                            "limitations": { "type": "array", "items": { "type": "string" } },
                            "cluster_members": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "taxonomy": { "type": "string" },
                                        "concept_name": { "type": "string" },
                                        "unit": { "type": "string" },
                                        "role": { "type": "string" },
                                        "dominant_period_shape": { "type": "string" }
                                    },
                                    "required": ["taxonomy", "concept_name", "unit", "role"]
                                }
                            },
                            "linked_claim_ids": {
                                "type": "array",
                                "items": { "type": "integer" }
                            }
                        },
                        "required": [
                            "crux_key", "title", "statement",
                            "watch_condition", "confirming_signal", "breaking_signal",
                            "disposition", "rationale"
                        ]
                    }
                },
                "supporting_metrics": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "selection_scope": { "type": "string" },
                            "crux_key": { "type": "string" },
                            "taxonomy": { "type": "string" },
                            "concept_name": { "type": "string" },
                            "unit": { "type": "string" },
                            "label": { "type": "string" },
                            "rationale": { "type": "string" },
                            "period_basis": { "type": "string" },
                            "quality_status": { "type": "string" }
                        },
                        "required": [
                            "selection_scope", "taxonomy", "concept_name", "unit", "rationale"
                        ]
                    }
                },
                "quality_flags": {
                    "type": "array",
                    "description": "Data quality warnings. Each item must use flag_key.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "flag_key": { "type": "string" },
                            "severity": { "type": "string" },
                            "description": { "type": "string" },
                            "metric_key": { "type": "string" },
                            "period": { "type": "string" }
                        },
                        "required": ["flag_key", "severity", "description"]
                    }
                },
                "open_questions": {
                    "type": "array",
                    "description": "Unresolved questions / data gaps. Each item must use gap_key.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "gap_key": { "type": "string" },
                            "description": { "type": "string" }
                        },
                        "required": ["gap_key", "description"]
                    }
                }
            },
            "required": ["cruxes"]
        }))
        .build()
        .expect("submit_crux_triage tool definition should be valid")
}

pub async fn execute(sqlite_path: &PathBuf, arguments: &str) -> Result<ClientToolExecuteResult> {
    let output: CruxTriageOutput = serde_json::from_str(arguments).map_err(|err| {
        Error::string(&format!(
            "submit_crux_triage arguments were not valid JSON: {err}"
        ))
    })?;
    let ctx = load_explorer_context(sqlite_path).await?;
    FinancialModelExplorerAgent::validate_crux_triage_with_workspace(&output, &ctx)?;
    let text = serde_json::to_string(&output).map_err(|err| {
        Error::string(&format!("failed to serialize accepted crux triage output: {err}"))
    })?;
    Ok(ClientToolExecuteResult::Complete(text))
}
