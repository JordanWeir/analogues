use crate::{
    agents::financial_model_explorer::{
        service::FinancialModelExplorerService,
        types::AnalysisFinalizeInput,
    },
    services::{financial_analysis_store::FinancialAnalysisStore, openrouter_chat::ClientToolExecuteResult},
};
use chrono::Utc;
use loco_rs::prelude::*;
use openrouter_rs::types::Tool;
use serde_json::json;
use std::path::PathBuf;

pub const TOOL_NAME: &str = "finalize_analysis";

pub fn openrouter_tool() -> Tool {
    Tool::builder()
        .name(TOOL_NAME)
        .description(
            "Judge a draft analysis run and persist a finalized experiment record. \
             Use only after reviewing run_analysis_draft results.",
        )
        .parameters(json!({
            "type": "object",
            "properties": {
                "run_key": { "type": "string" },
                "experiment": { "type": "object" }
            },
            "required": ["run_key", "experiment"]
        }))
        .build()
        .expect("finalize_analysis tool definition should be valid")
}

pub async fn execute(sqlite_path: &PathBuf, arguments: &str) -> Result<String> {
    let input: AnalysisFinalizeInput = serde_json::from_str(arguments).map_err(|err| {
        Error::string(&format!("finalize_analysis arguments were not valid JSON: {err}"))
    })?;
    if input.run_key.trim().is_empty() {
        return Err(Error::string("run_key cannot be empty"));
    }
    FinancialModelExplorerService::validate_experiment_input(&input.experiment)?;

    let db = sea_orm::Database::connect(crate::services::workspace_store::sqlite_uri(
        sqlite_path,
    ))
    .await?;
    let store = FinancialAnalysisStore::new(&db);
    let run = store
        .load_analysis_run(&input.run_key)
        .await?
        .ok_or_else(|| Error::string(&format!("unknown analysis run_key: {}", input.run_key)))?;
    if run.status != "draft" {
        return Err(Error::string(&format!(
            "analysis run {} is not in draft status",
            input.run_key
        )));
    }
    if run.execution_status == "error" && input.experiment.disposition == "promoted" {
        return Err(Error::string(
            "cannot promote an experiment when the draft run errored",
        ));
    }

    let created_at = Utc::now().to_rfc3339();
    store
        .finalize_analysis_run(
            &input.run_key,
            &input.experiment,
            "financial_model_explorer",
            &created_at,
            None,
        )
        .await?;
    db.close().await.ok();

    Ok(json!({
        "run_key": input.run_key,
        "experiment_key": input.experiment.experiment_key,
        "disposition": input.experiment.disposition,
        "status": "finalized"
    })
    .to_string())
}

pub fn execute_sync_for_handler(
    sqlite_path: &PathBuf,
    arguments: &str,
) -> Result<ClientToolExecuteResult> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| Error::string(&format!("failed to start tool runtime: {err}")))?;
    let payload = runtime.block_on(execute(sqlite_path, arguments))?;
    Ok(ClientToolExecuteResult::Response(payload))
}
