use crate::{
    agents::financial_model_explorer::{
        FinancialModelExplorerAgent, types::MechanicsExperimentsComplete,
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
            "Finish the mechanics experiment lane after at least one promoted experiment exists. \
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
    FinancialModelExplorerAgent::validate_mechanics_complete(&output)?;

    let db = sea_orm::Database::connect(crate::services::workspace_store::sqlite_uri(
        sqlite_path,
    ))
    .await?;
    let store = FinancialAnalysisStore::new(&db);
    let promoted = store.count_promoted_experiments().await?;
    db.close().await.ok();

    if promoted == 0 {
        return Err(Error::string(
            "submit_mechanics_experiments requires at least one promoted analysis_experiments row; use finalize_analysis first",
        ));
    }

    let text = serde_json::to_string(&output).map_err(|err| {
        Error::string(&format!("failed to serialize mechanics completion payload: {err}"))
    })?;
    Ok(ClientToolExecuteResult::Complete(text))
}

