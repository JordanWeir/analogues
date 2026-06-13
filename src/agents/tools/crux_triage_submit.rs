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
             Requires 2+ promoted cruxes when narrative has 3+ crux items, and 2+ supporting_metrics. \
             Validation errors are returned so you can fix and resubmit.",
        )
        .parameters(json!({
            "type": "object",
            "properties": {
                "cruxes": {
                    "type": "array",
                    "items": { "type": "object" }
                },
                "supporting_metrics": {
                    "type": "array",
                    "items": { "type": "object" }
                },
                "quality_flags": {
                    "type": "array",
                    "items": { "type": "object" }
                },
                "open_questions": {
                    "type": "array",
                    "items": { "type": "object" }
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
