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
    agents::financial_model_explorer::explorer_context::MIN_PROMOTED_EXPERIMENTS,
    agents::financial_model_explorer::FinancialModelExplorerAgent,
    services::financial_analysis_store::FinancialAnalysisStore,
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use std::sync::Arc;
use strategy::FinancialMechanicsExperimentsStrategy;

pub struct FinancialMechanicsExperimentsLane {
    strategy: FinancialMechanicsExperimentsStrategy,
}

impl FinancialMechanicsExperimentsLane {
    pub fn new(strategy: FinancialMechanicsExperimentsStrategy) -> Self {
        Self { strategy }
    }

    pub fn default_lane_name() -> &'static str {
        "financial_mechanics_experiments"
    }

    #[cfg(test)]
    pub fn fixture() -> Self {
        Self::new(FinancialMechanicsExperimentsStrategy::Fixture)
    }
}

#[async_trait]
impl Lane for FinancialMechanicsExperimentsLane {
    fn name(&self) -> &'static str {
        Self::default_lane_name()
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

        match &self.strategy {
            FinancialMechanicsExperimentsStrategy::Agent(config) => {
                let sqlite_path = ctx.workspace.paths.sqlite_path.clone();
                let agent = FinancialModelExplorerAgent::new(config.clone())
                    .with_company_label(ctx.ticker());
                agent.run(sqlite_path, ctx.ticker()).await?;
            }
            #[cfg(test)]
            FinancialMechanicsExperimentsStrategy::Fixture => {
                fixtures::persist_fixture_experiment(ctx).await?;
            }
        }

        if store.count_promoted_experiments().await? < MIN_PROMOTED_EXPERIMENTS {
            return Err(Error::string(&format!(
                "financial mechanics lane finished with fewer than {MIN_PROMOTED_EXPERIMENTS} promoted analysis_experiments"
            )));
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
