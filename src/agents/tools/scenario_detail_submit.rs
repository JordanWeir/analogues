use crate::{
    agents::scenario_builder::{
        ScenarioBuilderAgent,
        types::ScenarioDetailMetadataOutput,
    },
    services::{
        openrouter_chat::ClientToolExecuteResult,
        scenario_store::ScenarioStore,
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
            "Submit scenario metadata (assumption summary, crux assumptions, sensitivities, signals). \
             Does not include quarterly periods — call submit_scenario_period once per calendar row, \
             then complete_scenario_detail. Fan-out workers set per_worker true.",
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
                "per_worker": { "type": "boolean" }
            },
            "required": [
                "scenario_key",
                "assumption_summary",
                "crux_assumptions",
                "sensitivities",
                "confirming_signals",
                "breaking_signals",
                "per_worker"
            ]
        }))
        .build()
        .expect("submit_scenario_detail tool definition should be valid")
}

pub async fn execute(sqlite_path: &PathBuf, arguments: &str) -> Result<ClientToolExecuteResult> {
    let output: ScenarioDetailMetadataOutput = serde_json::from_str(arguments).map_err(|err| {
        Error::string(&format!(
            "submit_scenario_detail arguments were not valid JSON: {err}"
        ))
    })?;
    let db = Database::connect(workspace_store::sqlite_uri(sqlite_path))
        .await
        .map_err(|err| Error::string(&format!("failed to open workspace sqlite: {err}")))?;
    ScenarioBuilderAgent::validate_detail_metadata(&output)?;
    ScenarioStore::validate_metadata_references(&db, &output.crux_assumptions).await?;
    let store = ScenarioStore::new(&db);
    store.persist_detail_metadata(&output).await?;
    let required = store.expected_period_count().await?;
    db.close().await.ok();
    let response = json!({
        "status": "accepted",
        "scenario_key": output.scenario_key,
        "metadata_submitted": true,
        "periods_submitted": 0,
        "periods_required": required,
        "next_step": "submit_scenario_period for period_order 1"
    });
    Ok(ClientToolExecuteResult::Response(
        serde_json::to_string(&response).map_err(|err| {
            Error::string(&format!("failed to serialize metadata submit response: {err}"))
        })?,
    ))
}
