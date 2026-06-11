#[cfg(test)]
mod fixtures;
mod gate;
pub mod strategy;
mod writes;

use super::{
    context::LaneContext, gate::Gate, lane::Lane, result::LaneResult, result::LaneStatus,
    result::LaneWritesSummary,
};
use crate::{
    agents::financial_model_explorer::FinancialModelExplorerAgent,
    services::{financial_analysis_store::FinancialAnalysisStore, workspace_sql::scalar_i64},
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use std::sync::Arc;
use strategy::IdentifyCruxCandidatesStrategy;

pub struct IdentifyCruxCandidatesLane {
    strategy: IdentifyCruxCandidatesStrategy,
}

impl IdentifyCruxCandidatesLane {
    pub fn new(strategy: IdentifyCruxCandidatesStrategy) -> Self {
        Self { strategy }
    }

    pub fn default_lane_name() -> &'static str {
        "identify_crux_candidates"
    }

    #[cfg(test)]
    pub fn fixture() -> Self {
        Self::new(IdentifyCruxCandidatesStrategy::Fixture)
    }
}

#[async_trait]
impl Lane for IdentifyCruxCandidatesLane {
    fn name(&self) -> &'static str {
        Self::default_lane_name()
    }

    fn gates(&self) -> Vec<Arc<dyn Gate>> {
        gate::identify_crux_candidates_gates()
    }

    async fn run(&self, ctx: &mut LaneContext) -> Result<LaneResult> {
        let store = FinancialAnalysisStore::new(ctx.workspace.connection());

        let catalog_count = scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) AS count FROM concept_catalog_entries",
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

        match &self.strategy {
            IdentifyCruxCandidatesStrategy::Agent(config) => {
                let sqlite_path = ctx.workspace.paths.sqlite_path.clone();
                let agent = FinancialModelExplorerAgent::new(config.clone())
                    .with_company_label(ctx.ticker());
                let (response_text, worker_run_id) = agent.run(sqlite_path, ctx.ticker()).await?;
                let output = FinancialModelExplorerAgent::parse_crux_triage_output(&response_text)?;
                FinancialModelExplorerAgent::persist_crux_triage(
                    ctx.workspace.connection(),
                    &output,
                    &agent.config().model,
                    worker_run_id,
                )
                .await?;
            }
            #[cfg(test)]
            IdentifyCruxCandidatesStrategy::Fixture => {
                fixtures::persist_fixture_cruxes(ctx).await?;
            }
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
