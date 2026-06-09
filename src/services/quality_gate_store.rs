use crate::lanes::gate::GateResult;
use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
use std::path::Path;

pub struct QualityGateStore;

impl QualityGateStore {
    pub async fn persist_batch(
        sqlite_path: &Path,
        lane_name: &str,
        gate_results: &[GateResult],
    ) -> Result<()> {
        if gate_results.is_empty() {
            return Ok(());
        }

        let db = Database::connect(crate::services::workspace_store::sqlite_uri(sqlite_path))
            .await
            .map_err(|err| Error::string(&format!("failed to open workspace db: {err}")))?;

        let created_at = Utc::now().to_rfc3339();

        for gate in gate_results {
            let message = gate
                .message
                .as_deref()
                .map(sql_quote)
                .map(|quoted| format!("'{quoted}'"))
                .unwrap_or_else(|| "NULL".to_string());

            let statement = format!(
                "INSERT INTO quality_gate_results (
                    lane_name, gate_name, status, message, created_at
                ) VALUES (
                    '{}', '{}', '{}', {}, '{}'
                )",
                sql_quote(lane_name),
                sql_quote(&gate.gate_name),
                sql_quote(gate.status.as_str()),
                message,
                sql_quote(&created_at),
            );

            db.execute(Statement::from_string(DatabaseBackend::Sqlite, statement))
                .await
                .map_err(|err| Error::string(&format!("failed to persist quality gate: {err}")))?;
        }

        Ok(())
    }

    pub async fn count_for_lane(sqlite_path: &Path, lane_name: &str) -> Result<u64> {
        let db = Database::connect(crate::services::workspace_store::sqlite_uri(sqlite_path))
            .await
            .map_err(|err| Error::string(&format!("failed to open workspace db: {err}")))?;

        let row = db
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!(
                    "SELECT COUNT(*) AS count FROM quality_gate_results WHERE lane_name = '{}'",
                    sql_quote(lane_name)
                ),
            ))
            .await
            .map_err(|err| Error::string(&format!("failed to count quality gates: {err}")))?
            .ok_or_else(|| Error::string("quality gate count query returned no row"))?;

        row.try_get::<i64>("", "count")
            .map(|count| count as u64)
            .map_err(|err| Error::string(&format!("failed to parse quality gate count: {err}")))
    }
}

fn sql_quote(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        services::workspace_store::execute_schema,
        workspace::{seed_database, InitWorkspaceRequest, WorkspacePaths},
    };
    use sea_orm::Database;
    use std::path::PathBuf;

    async fn temp_workspace_db() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "analogues-quality-gate-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let db = Database::connect(crate::services::workspace_store::sqlite_uri(&path))
            .await
            .expect("sqlite");
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
            },
            &WorkspacePaths {
                run_slug: "MSFT-2026-06-07-1".to_string(),
                workspace_dir: path.parent().unwrap().to_path_buf(),
                sqlite_path: path.clone(),
                generated_dir: path.parent().unwrap().join("generated"),
            },
        )
        .await
        .expect("seed");
        db.close().await.expect("close");
        path
    }

    #[tokio::test]
    async fn persists_gate_results_for_lane() {
        let path = temp_workspace_db().await;
        let gates = vec![
            GateResult::pass("workspace_exists"),
            GateResult::warn("sec_provenance", "partial coverage"),
        ];

        QualityGateStore::persist_batch(&path, "init_workspace", &gates)
            .await
            .expect("persist");

        let count = QualityGateStore::count_for_lane(&path, "init_workspace")
            .await
            .expect("count");
        assert_eq!(count, 2);
    }
}
