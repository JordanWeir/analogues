use crate::services::{
    usage_snapshot::UsageSnapshot,
    workspace_sql::{last_insert_rowid, sql_quote},
};
use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
use serde_json::Value;
use std::path::Path;

pub const WORKER_RUN_STATUS_SUCCESS: &str = "success";
pub const WORKER_RUN_STATUS_ERROR: &str = "error";

#[derive(Debug, Clone)]
pub struct WorkerRunRecord {
    pub worker_name: String,
    pub model: String,
    pub status: String,
    pub agent_rounds: usize,
    pub usage: UsageSnapshot,
    pub client_tool_calls: u32,
    pub latency_ms: u128,
    pub finish_reason: Option<String>,
    pub error_message: Option<String>,
    pub metadata_json: Value,
}

pub struct WorkerRunStore;

impl WorkerRunStore {
    pub async fn persist(sqlite_path: &Path, record: &WorkerRunRecord) -> Result<i64> {
        let db = Database::connect(crate::services::workspace_store::sqlite_uri(sqlite_path))
            .await
            .map_err(|err| Error::string(&format!("failed to open workspace db: {err}")))?;

        let created_at = Utc::now().to_rfc3339();
        let metadata_json = serde_json::to_string(&record.metadata_json).map_err(|err| {
            Error::string(&format!("failed to serialize worker run metadata: {err}"))
        })?;

        let statement = format!(
            "INSERT INTO worker_runs (
                worker_name, model, status, agent_rounds, input_tokens, output_tokens,
                cache_reads, cache_writes, web_search_requests, client_tool_calls, cost_usd,
                latency_ms, finish_reason, error_message, metadata_json, created_at
            ) VALUES (
                '{}', '{}', '{}', {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, '{}', '{}'
            )",
            sql_quote(&record.worker_name),
            sql_quote(&record.model),
            sql_quote(&record.status),
            record.agent_rounds,
            optional_i64(record.usage.input_tokens),
            optional_i64(record.usage.output_tokens),
            optional_i64(record.usage.cache_reads),
            optional_i64(record.usage.cache_writes),
            record.usage.web_search_requests.unwrap_or(0),
            record.client_tool_calls,
            optional_f64(record.usage.cost_usd),
            record.latency_ms,
            optional_text(record.finish_reason.as_deref()),
            optional_text(record.error_message.as_deref()),
            sql_quote(&metadata_json),
            sql_quote(&created_at),
        );

        db.execute(Statement::from_string(DatabaseBackend::Sqlite, statement))
            .await
            .map_err(|err| Error::string(&format!("failed to persist worker run: {err}")))?;

        last_insert_rowid(&db).await
    }
}

fn optional_i64(value: Option<u64>) -> String {
    value
        .map(|number| number.to_string())
        .unwrap_or_else(|| "NULL".to_string())
}

fn optional_f64(value: Option<f64>) -> String {
    value
        .map(|number| number.to_string())
        .unwrap_or_else(|| "NULL".to_string())
}

fn optional_text(value: Option<&str>) -> String {
    value
        .map(sql_quote)
        .map(|quoted| format!("'{quoted}'"))
        .unwrap_or_else(|| "NULL".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        services::workspace_store::execute_schema,
        workspace::{seed_database, InitWorkspaceRequest, WorkspacePaths},
    };
    use sea_orm::DatabaseConnection;
    use serde_json::json;
    use std::path::PathBuf;

    async fn test_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:?mode=rwc")
            .await
            .expect("in-memory sqlite");
        execute_schema(&db).await.expect("schema");
        seed_database(
            &db,
            &InitWorkspaceRequest {
                ticker: "MSFT".to_string(),
                date: "2026-06-07".to_string(),
                base_dir: PathBuf::from("reports/stock-narrative-research"),
                fetch_financials: false,
                mapping_strategy: None,
                build_narrative_map: false,
                build_financial_analysis: false,
            },
            &WorkspacePaths {
                run_slug: "MSFT-2026-06-07-1".to_string(),
                workspace_dir: PathBuf::from("/tmp/msft"),
                sqlite_path: PathBuf::from("/tmp/msft/run.sqlite"),
                generated_dir: PathBuf::from("/tmp/msft/generated"),
            },
        )
        .await
        .expect("seed");
        db
    }

    #[tokio::test]
    async fn persists_worker_run_to_workspace_schema() {
        let db = test_db().await;
        let path = std::env::temp_dir().join(format!(
            "analogues-worker-run-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        if path.exists() {
            std::fs::remove_file(&path).expect("remove old db");
        }
        {
            use sea_orm::ConnectionTrait;
            let file_db = Database::connect(crate::services::workspace_store::sqlite_uri(&path))
                .await
                .expect("file sqlite");
            for statement in crate::workspace::SCHEMA_STATEMENTS {
                file_db
                    .execute(Statement::from_string(
                        DatabaseBackend::Sqlite,
                        (*statement).to_string(),
                    ))
                    .await
                    .expect("schema statement");
            }
        }

        let worker_run_id = WorkerRunStore::persist(
            &path,
            &WorkerRunRecord {
                worker_name: "concept_catalog_review".to_string(),
                model: "test/model".to_string(),
                status: WORKER_RUN_STATUS_SUCCESS.to_string(),
                agent_rounds: 2,
                usage: UsageSnapshot {
                    input_tokens: Some(100),
                    output_tokens: Some(50),
                    cache_reads: Some(12),
                    cache_writes: Some(4),
                    cost_usd: Some(0.0025),
                    web_search_requests: Some(1),
                },
                client_tool_calls: 3,
                latency_ms: 1200,
                finish_reason: Some("stop".to_string()),
                error_message: None,
                metadata_json: json!({"worker_lane": "concept_catalog_review"}),
            },
        )
        .await
        .expect("persist worker run");

        assert!(worker_run_id > 0);

        let loaded_db = Database::connect(crate::services::workspace_store::sqlite_uri(&path))
            .await
            .expect("reopen sqlite");
        let row = loaded_db
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!("SELECT worker_name, status, agent_rounds, cache_reads, cache_writes, cost_usd FROM worker_runs WHERE id = {worker_run_id}"),
            ))
            .await
            .expect("query worker run")
            .expect("worker run row");

        assert_eq!(
            row.try_get::<String>("", "worker_name").expect("name"),
            "concept_catalog_review"
        );
        assert_eq!(
            row.try_get::<String>("", "status").expect("status"),
            WORKER_RUN_STATUS_SUCCESS
        );
        assert_eq!(row.try_get::<i64>("", "agent_rounds").expect("rounds"), 2);
        assert_eq!(
            row.try_get::<i64>("", "cache_reads").expect("cache reads"),
            12
        );
        assert_eq!(
            row.try_get::<i64>("", "cache_writes")
                .expect("cache writes"),
            4
        );
        assert!((row.try_get::<f64>("", "cost_usd").expect("cost") - 0.0025).abs() < f64::EPSILON);

        let _ = std::fs::remove_file(path);
        let _ = db;
    }
}
