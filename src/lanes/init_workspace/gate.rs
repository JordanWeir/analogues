use super::InitWorkspaceLane;
use crate::lanes::{
    context::LaneContext,
    gate::{Gate, GateResult},
    result::LaneResult,
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use std::sync::Arc;

pub fn init_workspace_gates() -> Vec<Arc<dyn Gate>> {
    vec![
        Arc::new(WorkspaceExistsGate),
        Arc::new(SecProvenanceGate),
        Arc::new(FetchFailuresRecordedGate),
    ]
}

struct WorkspaceExistsGate;

#[async_trait]
impl Gate for WorkspaceExistsGate {
    fn name(&self) -> &'static str {
        "workspace_exists"
    }

    async fn check(&self, ctx: &LaneContext, _result: &LaneResult) -> GateResult {
        match scalar_string(
            ctx.workspace.connection(),
            "SELECT ticker FROM run_metadata WHERE id = 1",
        )
        .await
        {
            Ok(ticker) if ticker == ctx.ticker() => GateResult::pass(self.name()),
            Ok(ticker) => GateResult::reject(
                self.name(),
                format!(
                    "run_metadata ticker mismatch: expected {}, found {ticker}",
                    ctx.ticker()
                ),
            ),
            Err(err) => GateResult::reject(self.name(), format!("workspace not readable: {err}")),
        }
    }
}

struct SecProvenanceGate;

#[async_trait]
impl Gate for SecProvenanceGate {
    fn name(&self) -> &'static str {
        "sec_provenance"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == crate::lanes::result::LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let count = scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) AS count FROM sec_raw_facts",
        )
        .await;

        let count = match count {
            Ok(count) => count,
            Err(err) => {
                return GateResult::reject(
                    self.name(),
                    format!("failed to count sec_raw_facts: {err}"),
                )
            }
        };

        if count == 0 {
            if result.error_message.is_some() {
                return GateResult::warn(
                    self.name(),
                    "no sec_raw_facts persisted after fetch failure",
                );
            }
            return GateResult::warn(self.name(), "no sec_raw_facts ingested");
        }

        let incomplete = scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) AS count FROM sec_raw_facts
             WHERE taxonomy IS NULL OR taxonomy = ''
                OR concept_name IS NULL OR concept_name = ''
                OR unit IS NULL OR unit = ''
                OR fetched_at IS NULL OR fetched_at = ''
                OR raw_json IS NULL OR raw_json = ''",
        )
        .await;

        match incomplete {
            Ok(0) => GateResult::pass(self.name()),
            Ok(missing) => GateResult::reject(
                self.name(),
                format!("{missing} sec_raw_facts rows missing required provenance fields"),
            ),
            Err(err) => GateResult::reject(self.name(), format!("provenance check failed: {err}")),
        }
    }
}

struct FetchFailuresRecordedGate;

#[async_trait]
impl Gate for FetchFailuresRecordedGate {
    fn name(&self) -> &'static str {
        "fetch_failures_recorded"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == crate::lanes::result::LaneStatus::Skipped {
            let gap_status = scalar_string(
                ctx.workspace.connection(),
                "SELECT status FROM data_gaps WHERE gap_key = 'starter_financials'",
            )
            .await;
            return match gap_status {
                Ok(status) if status == "open" => GateResult::pass(self.name()),
                Ok(status) => GateResult::warn(
                    self.name(),
                    format!(
                        "expected open starter_financials gap for skipped fetch, found {status}"
                    ),
                ),
                Err(err) => GateResult::reject(self.name(), format!("gap check failed: {err}")),
            };
        }

        let fetch_status = match scalar_string(
            ctx.workspace.connection(),
            "SELECT financial_fetch_status FROM run_metadata WHERE id = 1",
        )
        .await
        {
            Ok(status) => status,
            Err(err) => {
                return GateResult::reject(
                    self.name(),
                    format!("failed to read fetch status: {err}"),
                )
            }
        };

        if fetch_status == "failed" {
            let gap_status = scalar_string(
                ctx.workspace.connection(),
                "SELECT status FROM data_gaps WHERE gap_key = 'starter_financials'",
            )
            .await;
            return match gap_status {
                Ok(status) if status == "open" => GateResult::pass(self.name()),
                Ok(status) => GateResult::reject(
                    self.name(),
                    format!("fetch failed but starter_financials gap is {status}, expected open"),
                ),
                Err(err) => GateResult::reject(self.name(), format!("gap check failed: {err}")),
            };
        }

        if result.error_message.is_some() && fetch_status != "ingested" {
            return GateResult::warn(
                self.name(),
                "lane reported an error but fetch status is not failed",
            );
        }

        GateResult::pass(self.name())
    }
}

async fn scalar_string(db: &impl ConnectionTrait, sql: &str) -> Result<String> {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            sql.to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("query failed: {err}")))?
        .ok_or_else(|| Error::string("query returned no row"))?;
    row.try_get::<String>("", "ticker")
        .or_else(|_| row.try_get::<String>("", "status"))
        .or_else(|_| row.try_get::<String>("", "financial_fetch_status"))
        .map_err(|err| Error::string(&format!("failed to parse string column: {err}")))
}

async fn scalar_i64(db: &impl ConnectionTrait, sql: &str) -> Result<i64> {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            sql.to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("query failed: {err}")))?
        .ok_or_else(|| Error::string("query returned no row"))?;
    row.try_get::<i64>("", "count")
        .map_err(|err| Error::string(&format!("failed to parse count: {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        lanes::{context::LaneConfig, result::LaneWritesSummary},
        services::{
            sec_facts_provider::extract_raw_facts_from_root,
            workspace_financial_store::{RawIngestPersist, WorkspaceFinancialStore},
            workspace_store::{execute_schema, WorkspaceStore},
        },
        workspace::{seed_database, InitWorkspaceRequest, WorkspacePaths},
    };
    use sea_orm::Database;
    use serde_json::json;
    use std::path::PathBuf;

    async fn gate_context_with_facts(facts: bool) -> LaneContext {
        let path = std::env::temp_dir().join(format!(
            "analogues-init-gate-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let db = Database::connect(crate::services::workspace_store::sqlite_uri(&path))
            .await
            .expect("sqlite");
        execute_schema(&db).await.expect("schema");
        let paths = WorkspacePaths {
            run_slug: "EXMP-2026-06-09-1".to_string(),
            workspace_dir: path.parent().unwrap().to_path_buf(),
            sqlite_path: path.clone(),
            generated_dir: path.parent().unwrap().join("generated"),
        };
        seed_database(
            &db,
            &InitWorkspaceRequest {
                ticker: "EXMP".to_string(),
                date: "2026-06-09".to_string(),
                base_dir: PathBuf::from("reports/stock-narrative-research"),
                fetch_financials: false,
                mapping_strategy: None,
            },
            &paths,
        )
        .await
        .expect("seed");

        if facts {
            let facts_root = json!({
                "us-gaap": {
                    "Revenues": {
                        "label": "Revenues",
                        "units": { "USD": [{"form":"10-K","start":"2025-01-01","end":"2025-12-31","filed":"2026-02-15","val":100.0}]}
                    }
                }
            });
            let raw_facts = extract_raw_facts_from_root(&facts_root, "2026-06-09T00:00:00Z");
            WorkspaceFinancialStore::new(&db)
                .persist_raw_ingest(&RawIngestPersist {
                    fetched_at: "2026-06-09T00:00:00Z",
                    company_name: Some("Example Corp"),
                    currency: Some("USD"),
                    source_note: "test",
                    raw_sec_facts: &raw_facts,
                })
                .await
                .expect("persist");
        }

        db.close().await.expect("close");
        let workspace = WorkspaceStore.open_workspace(&path).await.expect("open");
        LaneContext::new(workspace, LaneConfig::new("EXMP"))
    }

    #[tokio::test]
    async fn workspace_exists_gate_passes_for_seeded_workspace() {
        let ctx = gate_context_with_facts(false).await;
        let result = LaneResult::success("init_workspace", LaneWritesSummary::default());
        let gate = WorkspaceExistsGate;
        assert_eq!(
            gate.check(&ctx, &result).await.status,
            crate::lanes::gate::GateStatus::Pass
        );
        ctx.workspace.close().await.ok();
    }

    #[tokio::test]
    async fn sec_provenance_gate_passes_when_facts_have_required_fields() {
        let ctx = gate_context_with_facts(true).await;
        let result = LaneResult::success("init_workspace", LaneWritesSummary::default());
        let gate = SecProvenanceGate;
        assert_eq!(
            gate.check(&ctx, &result).await.status,
            crate::lanes::gate::GateStatus::Pass
        );
        ctx.workspace.close().await.ok();
    }

    #[tokio::test]
    async fn init_workspace_lane_registers_gates() {
        use crate::lanes::lane::Lane;
        let lane = InitWorkspaceLane::new(&InitWorkspaceRequest {
            ticker: "EXMP".to_string(),
            date: "2026-06-09".to_string(),
            base_dir: PathBuf::from("reports/stock-narrative-research"),
            fetch_financials: true,
            mapping_strategy: None,
        });
        assert_eq!(Lane::gates(&lane).len(), 3);
    }
}
