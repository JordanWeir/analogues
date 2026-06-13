use crate::{
    agents::financial_model_explorer::{
        explorer_context::load_explorer_context, FinancialModelExplorerAgent,
        types::MechanicsExperimentsComplete,
    },
    services::{financial_analysis_store::FinancialAnalysisStore, openrouter_chat::ClientToolExecuteResult},
};
use loco_rs::prelude::*;
use openrouter_rs::types::Tool;
use serde_json::json;
use std::path::PathBuf;

pub const TOOL_NAME: &str = "submit_mechanics_experiments";

pub fn openrouter_tool() -> Tool {
    Tool::builder()
        .name(TOOL_NAME)
        .description(
            "Finish the mechanics experiment lane after at least two promoted experiments exist, \
             including forward/sensitivity work when claims include guidance. \
             Experiments should already be persisted via finalize_analysis.",
        )
        .parameters(json!({
            "type": "object",
            "properties": {
                "summary": { "type": "string" }
            }
        }))
        .build()
        .expect("submit_mechanics_experiments tool definition should be valid")
}

pub async fn execute(sqlite_path: &PathBuf, arguments: &str) -> Result<ClientToolExecuteResult> {
    let output: MechanicsExperimentsComplete = if arguments.trim().is_empty() {
        MechanicsExperimentsComplete {
            summary: String::new(),
        }
    } else {
        serde_json::from_str(arguments).map_err(|err| {
            Error::string(&format!(
                "submit_mechanics_experiments arguments were not valid JSON: {err}"
            ))
        })?
    };

    let db = sea_orm::Database::connect(crate::services::workspace_store::sqlite_uri(
        sqlite_path,
    ))
    .await?;
    let store = FinancialAnalysisStore::new(&db);
    let promoted = store.count_promoted_experiments().await?;
    let non_historical = store.count_promoted_non_historical_experiments().await?;
    db.close().await.ok();

    let ctx = load_explorer_context(sqlite_path).await?;
    FinancialModelExplorerAgent::validate_mechanics_complete(
        &output,
        promoted,
        non_historical,
        &ctx,
    )?;

    let text = serde_json::to_string(&output).map_err(|err| {
        Error::string(&format!("failed to serialize mechanics completion payload: {err}"))
    })?;
    Ok(ClientToolExecuteResult::Complete(text))
}
