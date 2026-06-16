use crate::{
    agents::financial_model_explorer::{
        explorer_context::{enforce_mechanics_draft_hygiene, load_explorer_context},
        FinancialModelExplorerAgent,
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
            "Finish the mechanics experiment lane. Experiments should already be persisted via finalize_analysis. \
             Per-crux fan-out workers: set per_worker true (lane checks minimums after all workers). \
             Lane-complete submit: omit per_worker or set false; requires ≥2 promoted experiments and \
             forward/sensitivity work when claims include guidance.",
        )
        .parameters(json!({
            "type": "object",
            "properties": {
                "summary": { "type": "string" },
                "per_worker": {
                    "type": "boolean",
                    "description": "True for per-crux fan-out workers; skips lane-level minimum checks."
                },
                "crux_key": {
                    "type": "string",
                    "description": "Assigned promoted crux_key for this fan-out worker (required when per_worker is true unless scout is true)."
                },
                "scout": {
                    "type": "boolean",
                    "description": "True for the mechanics scout worker covering cruxes that still lack promoted experiments."
                }
            }
        }))
        .build()
        .expect("submit_mechanics_experiments tool definition should be valid")
}

pub async fn execute(sqlite_path: &PathBuf, arguments: &str) -> Result<ClientToolExecuteResult> {
    let output: MechanicsExperimentsComplete = if arguments.trim().is_empty() {
        MechanicsExperimentsComplete {
            summary: String::new(),
            per_worker: false,
            crux_key: None,
            scout: false,
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
    enforce_mechanics_draft_hygiene(&store, &output).await?;
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
