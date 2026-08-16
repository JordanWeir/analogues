use crate::{
    agents::financial_model_explorer::types::MechanicsReviewOutput,
    services::{
        mechanics_review::MechanicsReviewService,
        openrouter_chat::ClientToolExecuteResult,
        workspace_store::sqlite_uri,
    },
};
use loco_rs::prelude::*;
use openrouter_rs::types::Tool;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

pub const TOOL_NAME: &str = "submit_mechanics_review";

#[derive(Debug, Clone)]
pub struct MechanicsReviewSubmitConfig {
    pub review_round: i64,
}

pub fn openrouter_tool() -> Tool {
    Tool::builder()
        .name(TOOL_NAME)
        .description(
            "Submit a PR-style mechanics review for the assigned scope. Stamp approved when \
             experiments are arithmetically sound and scope is clean, or changes_requested with \
             blocking findings and remediation guidance. Per-scope fan-out: set per_worker true.",
        )
        .parameters(json!({
            "type": "object",
            "properties": {
                "summary": { "type": "string" },
                "per_worker": { "type": "boolean" },
                "crux_key": { "type": "string" },
                "scout": { "type": "boolean" },
                "verdict": {
                    "type": "string",
                    "enum": ["approved", "changes_requested"]
                },
                "findings": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "category": { "type": "string" },
                            "severity": { "type": "string" },
                            "description": { "type": "string" },
                            "experiment_key": { "type": "string" },
                            "remediation": { "type": "string" }
                        },
                        "required": ["category", "severity", "description"]
                    }
                },
                "experiments_reviewed": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            },
            "required": ["summary", "verdict", "experiments_reviewed"]
        }))
        .build()
        .expect("submit_mechanics_review tool definition should be valid")
}

pub async fn execute(
    sqlite_path: &PathBuf,
    arguments: &str,
    review_round: i64,
) -> Result<ClientToolExecuteResult> {
    let output: MechanicsReviewOutput = serde_json::from_str(arguments).map_err(|err| {
        Error::string(&format!(
            "submit_mechanics_review arguments were not valid JSON: {err}"
        ))
    })?;

    let db = sea_orm::Database::connect(sqlite_uri(sqlite_path))
        .await
        .map_err(|err| Error::string(&format!("failed to open workspace db: {err}")))?;
    MechanicsReviewService::validate_with_workspace(&db, &output, review_round).await?;
    db.close().await.ok();

    let text = serde_json::to_string(&output).map_err(|err| {
        Error::string(&format!("failed to serialize mechanics review output: {err}"))
    })?;
    Ok(ClientToolExecuteResult::Complete(text))
}

pub fn handler_config(review_round: i64) -> Arc<MechanicsReviewSubmitConfig> {
    Arc::new(MechanicsReviewSubmitConfig { review_round })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        agents::financial_model_explorer::types::{
            AnalysisExperimentInput, AnalysisInputRef, AnalysisOutputRow,
        },
        services::{
            financial_analysis_store::FinancialAnalysisStore,
            workspace_store::execute_schema,
        },
    };
    use sea_orm::Database;

    async fn test_db() -> (sea_orm::DatabaseConnection, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "mechanics-review-submit-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let db = Database::connect(sqlite_uri(&path)).await.expect("connect");
        execute_schema(&db).await.expect("schema");
        (db, path)
    }

    #[tokio::test]
    async fn rejects_approved_when_orphan_draft_remains() {
        let (db, path) = test_db().await;
        db.execute_unprepared(
            "INSERT INTO crux_candidates (
                crux_key, title, statement, watch_condition, confirming_signal,
                breaking_signal, disposition, status, rationale, created_by, created_at, payload_json
            ) VALUES (
                'test_crux', 'Test', 'Statement', 'Watch', 'Confirm', 'Break',
                'promoted', 'active', 'Because', 'fixture', '2026-06-09T00:00:00Z', '{}'
            )",
        )
        .await
        .expect("insert crux");

        let store = FinancialAnalysisStore::new(&db);
        store
            .insert_analysis_run(
                "draft_run",
                store.load_crux_id_by_key("test_crux").await.expect("crux"),
                "Draft question",
                "SELECT 1",
                "annual",
                "success",
                Some(1),
                None,
                "[]",
                "[]",
                "[]",
                "2026-06-09T00:00:00Z",
                None,
            )
            .await
            .expect("insert draft");

        store
            .insert_analysis_run(
                "promoted_run",
                store.load_crux_id_by_key("test_crux").await.expect("crux"),
                "Promoted question",
                "SELECT 1",
                "annual",
                "success",
                Some(1),
                None,
                "[]",
                "[]",
                "[]",
                "2026-06-09T00:00:00Z",
                None,
            )
            .await
            .expect("insert promoted run");

        store
            .finalize_analysis_run(
                "promoted_run",
                &AnalysisExperimentInput {
                    experiment_key: "exp_a".to_string(),
                    question: "Q".to_string(),
                    purpose: "historical_investigation".to_string(),
                    crux_key: Some("test_crux".to_string()),
                    sql_body: "SELECT 1".to_string(),
                    period_basis: "annual".to_string(),
                    disposition: "promoted".to_string(),
                    rejection_reason: None,
                    source_note: None,
                    rationale: Some("Fixture".to_string()),
                    assumptions: vec![],
                    inputs: vec![AnalysisInputRef {
                        input_type: "concept".to_string(),
                        taxonomy: Some("us-gaap".to_string()),
                        concept_name: Some("Revenues".to_string()),
                        unit: Some("USD".to_string()),
                        canonical_key: None,
                        note: None,
                    }],
                    outputs: vec![
                        AnalysisOutputRow {
                            kind: "ratio".to_string(),
                            label: "Ratio".to_string(),
                            value: Some(1.0),
                            unit: Some("ratio".to_string()),
                            period_end: None,
                            formula: None,
                            text: None,
                        },
                        AnalysisOutputRow {
                            kind: "interpretation".to_string(),
                            label: "Read".to_string(),
                            value: None,
                            unit: None,
                            period_end: None,
                            formula: None,
                            text: Some("OK".to_string()),
                        },
                    ],
                    bridge: None,
                },
                "fixture",
                "2026-06-09T00:00:00Z",
                None,
            )
            .await
            .expect("finalize");

        let arguments = json!({
            "summary": "Looks good",
            "per_worker": true,
            "crux_key": "test_crux",
            "scout": false,
            "verdict": "approved",
            "findings": [],
            "experiments_reviewed": ["exp_a"]
        })
        .to_string();

        let err = execute(&path, &arguments, 1).await.expect_err("should reject");
        assert!(err.to_string().contains("cannot stamp approved"));
        db.close().await.ok();
    }
}
