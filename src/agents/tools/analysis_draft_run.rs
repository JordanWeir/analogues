use crate::{
    agents::financial_model_explorer::types::AnalysisDraftInput,
    services::{
        financial_analysis_store::FinancialAnalysisStore,
        workspace_query::{execute_workspace_query, WorkspaceQueryResult},
    },
};
use chrono::Utc;
use loco_rs::prelude::*;
use openrouter_rs::types::Tool;
use serde_json::json;
use std::path::PathBuf;

pub const TOOL_NAME: &str = "run_analysis_draft";

pub fn openrouter_tool() -> Tool {
    Tool::builder()
        .name(TOOL_NAME)
        .description(
            "Execute a focused read-only SQL experiment in draft mode. \
             Returns tabular results for review before finalize_analysis. \
             Does not promote or reject the experiment.",
        )
        .parameters(json!({
            "type": "object",
            "properties": {
                "run_key": { "type": "string" },
                "question": { "type": "string" },
                "sql_body": { "type": "string" },
                "period_basis": { "type": "string" },
                "crux_key": { "type": "string" },
                "assumptions": { "type": "array", "items": { "type": "object" } },
                "inputs": { "type": "array", "items": { "type": "object" } }
            },
            "required": ["run_key", "question", "sql_body", "period_basis"]
        }))
        .build()
        .expect("run_analysis_draft tool definition should be valid")
}

pub async fn execute(sqlite_path: &PathBuf, arguments: &str) -> Result<String> {
    let input: AnalysisDraftInput = serde_json::from_str(arguments).map_err(|err| {
        Error::string(&format!("run_analysis_draft arguments were not valid JSON: {err}"))
    })?;
    validate_draft_input(&input)?;

    let query_result = match execute_workspace_query(sqlite_path, &input.sql_body).await {
        Ok(result) => result,
        Err(err) => {
            persist_failed_run(sqlite_path, &input, &err.to_string()).await?;
            return Ok(error_payload(&input.run_key, &err.to_string()));
        }
    };

    let (execution_status, row_count, error_message) = classify_query_result(&query_result);
    let result_json = serde_json::to_string(&query_rows_to_json(&query_result))
        .unwrap_or_else(|_| "[]".to_string());
    let assumptions_json =
        serde_json::to_string(&input.assumptions).unwrap_or_else(|_| "[]".to_string());
    let inputs_json = serde_json::to_string(&input.inputs).unwrap_or_else(|_| "[]".to_string());
    let created_at = Utc::now().to_rfc3339();

    let db = sea_orm::Database::connect(crate::services::workspace_store::sqlite_uri(
        sqlite_path,
    ))
    .await
    .map_err(|err| Error::string(&format!("failed to open workspace db: {err}")))?;
    let store = FinancialAnalysisStore::new(&db);
    let crux_id = if let Some(key) = input.crux_key.as_deref() {
        store.load_crux_id_by_key(key).await?
    } else {
        None
    };
    store
        .insert_analysis_run(
            &input.run_key,
            crux_id,
            &input.question,
            &input.sql_body,
            &input.period_basis,
            execution_status,
            row_count,
            error_message.as_deref(),
            &result_json,
            &assumptions_json,
            &inputs_json,
            &created_at,
            None,
        )
        .await?;
    db.close().await.ok();

    Ok(success_payload(
        &input.run_key,
        execution_status,
        row_count,
        &query_result,
        error_message.as_deref(),
        &input.sql_body,
    ))
}

fn validate_draft_input(input: &AnalysisDraftInput) -> Result<()> {
    if input.run_key.trim().is_empty() {
        return Err(Error::string("run_key cannot be empty"));
    }
    if input.question.trim().is_empty() {
        return Err(Error::string("question cannot be empty"));
    }
    if input.sql_body.trim().is_empty() {
        return Err(Error::string("sql_body cannot be empty"));
    }
    if input.period_basis.trim().is_empty() {
        return Err(Error::string("period_basis cannot be empty"));
    }
    Ok(())
}

fn classify_query_result(result: &WorkspaceQueryResult) -> (&'static str, Option<i64>, Option<String>) {
    if result.rows.is_empty() {
        return ("empty", Some(0), Some("query returned zero rows".to_string()));
    }
    if result.truncated {
        return (
            "truncated",
            Some(result.row_count as i64),
            Some("results truncated at 200 rows".to_string()),
        );
    }
    ("success", Some(result.row_count as i64), None)
}

fn query_rows_to_json(result: &WorkspaceQueryResult) -> Vec<serde_json::Value> {
    result.rows.clone()
}

fn success_payload(
    run_key: &str,
    execution_status: &str,
    row_count: Option<i64>,
    result: &WorkspaceQueryResult,
    error_message: Option<&str>,
    sql_body: &str,
) -> String {
    let mut warnings: Vec<String> = Vec::new();
    if sql_body.contains("fiscal_year") {
        warnings.push(
            "SQL groups by fiscal_year; SEC may duplicate rows per period_end — prefer GROUP BY period_end only."
                .to_string(),
        );
    }
    if let Some(count) = row_count {
        if count > 8 {
            warnings.push(format!(
                "High row_count ({count}); verify deduplication before finalize_analysis."
            ));
        }
    }
    if execution_status == "truncated" {
        warnings.push("Results truncated at 200 rows.".to_string());
    }

    json!({
        "run_key": run_key,
        "execution_status": execution_status,
        "row_count": row_count,
        "error_message": error_message,
        "warnings": warnings,
        "columns": result.columns,
        "rows": result.rows,
    })
    .to_string()
}

fn error_payload(run_key: &str, error_message: &str) -> String {
    json!({
        "run_key": run_key,
        "execution_status": "error",
        "row_count": 0,
        "error_message": error_message,
        "columns": [],
        "rows": [],
    })
    .to_string()
}

async fn persist_failed_run(
    sqlite_path: &PathBuf,
    input: &AnalysisDraftInput,
    error_message: &str,
) -> Result<()> {
    let db = sea_orm::Database::connect(crate::services::workspace_store::sqlite_uri(
        sqlite_path,
    ))
    .await?;
    let store = FinancialAnalysisStore::new(&db);
    let created_at = Utc::now().to_rfc3339();
    store
        .insert_analysis_run(
            &input.run_key,
            None,
            &input.question,
            &input.sql_body,
            &input.period_basis,
            "error",
            Some(0),
            Some(error_message),
            "[]",
            "[]",
            "[]",
            &created_at,
            None,
        )
        .await?;
    db.close().await.ok();
    Ok(())
}
