use crate::{
    agents::scenario_builder::{
        ScenarioBuilderAgent,
        types::ScenarioDetailCompleteInput,
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

pub const TOOL_NAME: &str = "complete_scenario_detail";

pub fn openrouter_tool() -> Tool {
    Tool::builder()
        .name(TOOL_NAME)
        .description(
            "Finish scenario detail after metadata and all calendar periods are submitted. \
             Validates the full scenario and ends the detail worker run. Fan-out workers set per_worker true.",
        )
        .parameters(json!({
            "type": "object",
            "properties": {
                "scenario_key": { "type": "string" },
                "per_worker": { "type": "boolean" }
            },
            "required": ["scenario_key"]
        }))
        .build()
        .expect("complete_scenario_detail tool definition should be valid")
}

pub async fn execute(sqlite_path: &PathBuf, arguments: &str) -> Result<ClientToolExecuteResult> {
    let input: ScenarioDetailCompleteInput = serde_json::from_str(arguments).map_err(|err| {
        Error::string(&format!(
            "complete_scenario_detail arguments were not valid JSON: {err}"
        ))
    })?;
    if input.scenario_key.trim().is_empty() {
        return Err(Error::string("scenario_key cannot be empty"));
    }

    let db = Database::connect(workspace_store::sqlite_uri(sqlite_path))
        .await
        .map_err(|err| Error::string(&format!("failed to open workspace sqlite: {err}")))?;
    let store = ScenarioStore::new(&db);
    store.verify_detail_complete(&input.scenario_key).await?;
    let detail = store.assemble_detail_output(&input.scenario_key).await?;
    db.close().await.ok();

    let text = serde_json::to_string(&detail).map_err(|err| {
        Error::string(&format!("failed to serialize completed scenario detail: {err}"))
    })?;
    Ok(ClientToolExecuteResult::Complete(text))
}
