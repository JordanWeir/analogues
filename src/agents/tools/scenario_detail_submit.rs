use crate::{
    agents::scenario_builder::{ScenarioBuilderAgent, types::ScenarioDetailOutput},
    services::{
        openrouter_chat::ClientToolExecuteResult,
        workspace_store,
    },
};
use loco_rs::prelude::*;
use openrouter_rs::types::Tool;
use sea_orm::Database;
use serde_json::json;
use std::path::PathBuf;

pub const TOOL_NAME: &str = "submit_scenario_detail";

pub fn openrouter_tool() -> Tool {
    Tool::builder()
        .name(TOOL_NAME)
        .description(
            "Submit quarterly projection detail for one scenario. Anchor historical quarters on \
             AlphaVantage av_raw_facts; project 12–20 forward quarters. Terminal period needs \
             ps_median. Fan-out workers set per_worker true.",
        )
        .parameters(json!({
            "type": "object",
            "properties": {
                "scenario_key": { "type": "string" },
                "assumption_summary": { "type": "string" },
                "crux_assumptions": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "crux_key": { "type": "string" },
                            "crux": { "type": "string" },
                            "assumption": { "type": "string" },
                            "impact": { "type": "string" },
                            "experiment_key": { "type": "string" },
                            "source_id": {
                                "type": "integer",
                                "description": "Optional. Reuse id from sources board; do not invent ids."
                            }
                        },
                        "required": ["crux_key", "crux", "assumption"]
                    }
                },
                "sensitivities": { "type": "array", "items": { "type": "string" } },
                "confirming_signals": { "type": "array", "items": { "type": "string" } },
                "breaking_signals": { "type": "array", "items": { "type": "string" } },
                "periods": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "period_order": { "type": "integer" },
                            "label": { "type": "string" },
                            "period_end": { "type": "string" },
                            "period_type": { "type": "string" },
                            "revenue": { "type": "number" },
                            "revenue_growth": { "type": "number" },
                            "diluted_shares": { "type": "number" },
                            "gross_margin": { "type": "number" },
                            "operating_margin": { "type": "number" },
                            "net_margin": { "type": "number" },
                            "net_income": { "type": "number" },
                            "eps": { "type": "number" },
                            "ps_low": { "type": "number" },
                            "ps_median": { "type": "number" },
                            "ps_high": { "type": "number" },
                            "pe_low": { "type": "number" },
                            "pe_median": { "type": "number" },
                            "pe_high": { "type": "number" },
                            "blend_ps_weight": { "type": "number" },
                            "blend_pe_weight": { "type": "number" },
                            "source_note": { "type": "string" }
                        },
                        "required": ["period_order", "label", "period_end"]
                    }
                },
                "per_worker": { "type": "boolean" }
            },
            "required": ["scenario_key", "assumption_summary", "periods"]
        }))
        .build()
        .expect("submit_scenario_detail tool definition should be valid")
}

pub async fn execute(sqlite_path: &PathBuf, arguments: &str) -> Result<ClientToolExecuteResult> {
    let output: ScenarioDetailOutput = serde_json::from_str(arguments).map_err(|err| {
        Error::string(&format!(
            "submit_scenario_detail arguments were not valid JSON: {err}"
        ))
    })?;
    let db = Database::connect(workspace_store::sqlite_uri(sqlite_path))
        .await
        .map_err(|err| Error::string(&format!("failed to open workspace sqlite: {err}")))?;
    ScenarioBuilderAgent::validate_detail_output_for_workspace(&db, &output).await?;
    db.close().await.ok();
    let text = serde_json::to_string(&output).map_err(|err| {
        Error::string(&format!("failed to serialize accepted detail output: {err}"))
    })?;
    Ok(ClientToolExecuteResult::Complete(text))
}
