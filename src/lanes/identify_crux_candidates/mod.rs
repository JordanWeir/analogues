mod gate;
mod writes;

use super::{
    context::LaneContext, gate::Gate, lane::Lane, result::LaneResult, result::LaneStatus,
    result::LaneWritesSummary,
};
use crate::{
    agents::financial_model_explorer::FinancialModelExplorerService,
    services::financial_analysis_store::FinancialAnalysisStore,
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct IdentifyCruxCandidatesLane;

#[async_trait]
impl Lane for IdentifyCruxCandidatesLane {
    fn name(&self) -> &'static str {
        "identify_crux_candidates"
    }

    fn gates(&self) -> Vec<Arc<dyn Gate>> {
        gate::identify_crux_candidates_gates()
    }

    async fn run(&self, ctx: &mut LaneContext) -> Result<LaneResult> {
        let store = FinancialAnalysisStore::new(ctx.workspace.connection());

        let catalog_count = scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) FROM concept_catalog_entries",
        )
        .await?;
        if catalog_count == 0 {
            return Ok(LaneResult::skipped(
                self.name(),
                "concept catalog is empty; build_catalog must run first",
            ));
        }

        if !store.narrative_context_present().await? {
            return Ok(LaneResult::skipped(
                self.name(),
                "narrative context is empty; build_narrative_map must run first",
            ));
        }

        let sqlite_path = ctx.workspace.paths.sqlite_path.clone();
        let service =
            FinancialModelExplorerService::crux_triage(sqlite_path, ctx.ticker());
        let (response_text, worker_run_id) = service.run().await?;
        let output = FinancialModelExplorerService::parse_crux_triage_output(&response_text)?;
        FinancialModelExplorerService::persist_crux_triage(
            ctx.workspace.connection(),
            &output,
            &service.model,
            worker_run_id,
        )
        .await?;

        let mut writes = LaneWritesSummary::default();
        for table in writes::TABLES_READ {
            writes = writes.read(*table);
        }
        for table in writes::TABLES_WRITTEN {
            writes = writes.wrote(*table);
        }

        Ok(LaneResult {
            lane_name: self.name().to_string(),
            status: LaneStatus::Success,
            writes,
            gate_results: Vec::new(),
            error_message: None,
        })
    }
}

async fn scalar_i64(db: &sea_orm::DatabaseConnection, sql: &str) -> Result<i64> {
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
    let rows = db
        .query_all(Statement::from_string(DatabaseBackend::Sqlite, sql.to_string()))
        .await
        .map_err(|err| Error::string(&format!("query failed: {err}")))?;
    if rows.is_empty() {
        return Ok(0);
    }
    rows[0]
        .try_get_by_index::<i64>(0)
        .map_err(|err| Error::string(&format!("expected integer: {err}")))
}
