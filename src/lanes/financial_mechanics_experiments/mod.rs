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
pub struct FinancialMechanicsExperimentsLane;

#[async_trait]
impl Lane for FinancialMechanicsExperimentsLane {
    fn name(&self) -> &'static str {
        "financial_mechanics_experiments"
    }

    fn gates(&self) -> Vec<Arc<dyn Gate>> {
        gate::financial_mechanics_experiments_gates()
    }

    async fn run(&self, ctx: &mut LaneContext) -> Result<LaneResult> {
        let store = FinancialAnalysisStore::new(ctx.workspace.connection());
        if store.count_promoted_cruxes().await? == 0 {
            return Ok(LaneResult::skipped(
                self.name(),
                "no promoted crux_candidates; identify_crux_candidates must run first",
            ));
        }

        let sqlite_path = ctx.workspace.paths.sqlite_path.clone();
        let service =
            FinancialModelExplorerService::mechanics_experiments(sqlite_path, ctx.ticker());
        service.run().await?;

        if store.count_promoted_experiments().await? == 0 {
            return Err(Error::string(
                "financial mechanics lane finished without any promoted analysis_experiments",
            ));
        }

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
