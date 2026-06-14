#[cfg(test)]
pub(crate) mod fixtures;
mod gate;

use super::{
    context::LaneContext,
    gate::Gate,
    lane::Lane,
    result::{LaneResult, LaneStatus, LaneWritesSummary},
};
use gate::scenario_generation_gates;
use crate::{
    agents::scenario_builder::{ScenarioBuilderAgent, ScenarioBuilderConfig},
    services::{
        financial_analysis_store::FinancialAnalysisStore,
        scenario_projection::compute_and_persist_monte_carlo,
        scenario_store::ScenarioStore,
        workspace_sql::scalar_i64,
    },
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use std::sync::Arc;

pub struct ScenarioGenerationLane {
    #[cfg(test)]
    fixture: bool,
}

impl ScenarioGenerationLane {
    pub fn new() -> Self {
        Self {
            #[cfg(test)]
            fixture: false,
        }
    }

    #[cfg(test)]
    pub fn fixture() -> Self {
        Self { fixture: true }
    }
}

impl Default for ScenarioGenerationLane {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Lane for ScenarioGenerationLane {
    fn name(&self) -> &'static str {
        "scenario_generation"
    }

    fn gates(&self) -> Vec<Arc<dyn Gate>> {
        scenario_generation_gates()
    }

    async fn run(&self, ctx: &mut LaneContext) -> Result<LaneResult> {
        #[cfg(test)]
        if self.fixture {
            fixtures::persist_fixture_scenarios(ctx).await?;
            compute_and_persist_monte_carlo(ctx.workspace.connection()).await?;
            return Ok(ok(self.name()));
        }

        let db = ctx.workspace.connection();
        let store = ScenarioStore::new(db);
        let analysis = FinancialAnalysisStore::new(db);

        if scalar_i64(db, "SELECT COUNT(*) AS count FROM av_raw_facts").await? == 0 {
            return Ok(LaneResult::skipped(self.name(), "av_raw_facts empty"));
        }
        if analysis.count_promoted_cruxes().await? == 0 {
            return Ok(LaneResult::skipped(
                self.name(),
                "no promoted crux_candidates",
            ));
        }
        if analysis.count_promoted_experiments().await? == 0 {
            return Ok(LaneResult::skipped(
                self.name(),
                "no promoted analysis_experiments",
            ));
        }

        store.clear_scenario_workspace().await?;

        // Phase 1 — blueprint
        let sqlite = ctx.workspace.paths.sqlite_path.clone();
        let ticker = ctx.ticker().to_string();
        let blueprint_response = ScenarioBuilderAgent::new(ScenarioBuilderConfig::blueprint())
            .with_company_label(&ticker)
            .run(sqlite.clone(), &ticker)
            .await?;
        let blueprint =
            ScenarioBuilderAgent::parse_blueprint_output(&blueprint_response.0)?;
        store.persist_blueprint(&blueprint).await?;

        // Phase 2 — per-scenario detail fan-out
        let scenarios = store.load_scenario_keys().await?;
        let mut detail_tasks: Vec<(
            String,
            tokio::task::JoinHandle<Result<(String, Option<i64>)>>,
        )> = Vec::new();

        for (scenario_key, name, description) in scenarios {
            let (sqlite, ticker) = (sqlite.clone(), ticker.clone());
            let label = format!("scenario_{scenario_key}");
            let prefix = format!(
                "FOCUS: Build quarterly projection detail for scenario `{scenario_key}` only.\n\
                 Name: {name}\nDescription: {description}\n\n\
                 Anchor ~4 historical quarters on av_raw_facts; project 12–20 forward quarters. \
                 Finish with submit_scenario_detail, per_worker true.\n\n"
            );
            let focus_key = scenario_key.clone();
            detail_tasks.push((
                label,
                tokio::spawn(async move {
                    ScenarioBuilderAgent::new(
                        ScenarioBuilderConfig::detail()
                            .with_prompt_prefix(prefix)
                            .with_focus_scenario_key(focus_key),
                    )
                    .with_company_label(&ticker)
                    .run(sqlite, &ticker)
                    .await
                }),
            ));
        }

        let mut detail_ok = 0usize;
        let mut detail_failed = 0usize;
        for (label, task) in detail_tasks {
            match task.await {
                Ok(Ok((text, _))) => {
                    match ScenarioBuilderAgent::parse_detail_output(&text) {
                        Ok(detail) => {
                            if let Err(err) = store.persist_detail(&detail).await {
                                detail_failed += 1;
                                tracing::warn!(
                                    worker = %label,
                                    error = %err,
                                    "scenario detail persist failed"
                                );
                            } else {
                                detail_ok += 1;
                            }
                        }
                        Err(err) => {
                            detail_failed += 1;
                            tracing::warn!(
                                worker = %label,
                                error = %err,
                                "invalid scenario detail output"
                            );
                        }
                    }
                }
                Ok(Err(err)) => {
                    detail_failed += 1;
                    tracing::warn!(worker = %label, error = %err, "scenario detail agent failed");
                }
                Err(err) => {
                    detail_failed += 1;
                    tracing::warn!(worker = %label, error = %err, "scenario detail join failed");
                }
            }
        }

        if detail_ok == 0 {
            return Err(Error::string(&format!(
                "all {detail_failed} scenario detail workers failed"
            )));
        }
        if detail_failed > 0 {
            tracing::warn!(
                detail_ok,
                detail_failed,
                "scenario_generation detail phase completed with worker failures"
            );
        }

        if store.count_scenarios_with_periods().await? < store.count_scenarios().await? {
            return Err(Error::string(
                "not every scenario has persisted quarterly periods",
            ));
        }

        // Phase 3 — deterministic valuation bands + Monte Carlo
        compute_and_persist_monte_carlo(db).await?;

        Ok(ok(self.name()))
    }
}

fn ok(name: &str) -> LaneResult {
    LaneResult {
        lane_name: name.to_string(),
        status: LaneStatus::Success,
        writes: LaneWritesSummary::default()
            .read("crux_candidates")
            .read("analysis_experiments")
            .read("av_raw_facts")
            .wrote("scenario_assumptions")
            .wrote("scenario_periods")
            .wrote("monte_carlo_summary"),
        gate_results: Vec::new(),
        error_message: None,
    }
}
