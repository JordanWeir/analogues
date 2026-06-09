use crate::{
    agents::financial_model_explorer::{service::FinancialModelExplorerService, types::CruxTriageOutput},
    services::openrouter_chat::ClientToolExecuteResult,
};
use loco_rs::prelude::*;
use openrouter_rs::types::Tool;
use serde_json::json;

pub const TOOL_NAME: &str = "submit_crux_triage";

pub fn openrouter_tool() -> Tool {
    Tool::builder()
        .name(TOOL_NAME)
        .description(
            "Submit final crux triage output after workspace_sql exploration. \
             Call once when ready. Validation errors are returned so you can fix and resubmit.",
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

pub fn execute(arguments: &str) -> Result<ClientToolExecuteResult> {
    let output: CruxTriageOutput = serde_json::from_str(arguments).map_err(|err| {
        Error::string(&format!(
            "submit_crux_triage arguments were not valid JSON: {err}"
        ))
    })?;
    FinancialModelExplorerService::validate_crux_triage_output(&output)?;
    let text = serde_json::to_string(&output).map_err(|err| {
        Error::string(&format!("failed to serialize accepted crux triage output: {err}"))
    })?;
    Ok(ClientToolExecuteResult::Complete(text))
}
