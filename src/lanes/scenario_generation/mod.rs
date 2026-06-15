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
        openrouter_chat::is_empty_completion_error,
        scenario_projection::compute_and_persist_monte_carlo,
        scenario_store::ScenarioStore,
        workspace_sql::scalar_i64,
    },
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use std::{path::PathBuf, sync::Arc, time::Duration};

/// Initial attempt plus up to two retries after empty-completion failures.
const SCENARIO_DETAIL_WORKER_MAX_ATTEMPTS: usize = 3;
const SCENARIO_DETAIL_RETRY_DELAYS_SECS: &[u64] = &[5, 15];

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

        let sqlite = ctx.workspace.paths.sqlite_path.clone();
        let ticker = ctx.ticker().to_string();
        let scenario_count = store.count_scenarios().await?;
        let with_periods = store.count_scenarios_with_periods().await?;

        if scenario_count == 0 {
            store.clear_scenario_workspace().await?;
            run_blueprint_phase(&store, &sqlite, &ticker).await?;
        } else {
            tracing::info!(
                scenario_count,
                with_periods,
                "scenario_generation resuming existing blueprint"
            );
        }

        let scenarios_needing_detail = store.load_scenarios_needing_detail().await?;
        if scenarios_needing_detail.is_empty() {
            tracing::info!(
                scenario_count,
                "all scenarios already have quarterly periods; skipping detail fan-out"
            );
        } else {
            tracing::info!(
                pending = scenarios_needing_detail.len(),
                scenario_count,
                "running scenario detail fan-out for scenarios missing periods"
            );
            let (detail_ok, detail_failed) = run_detail_fan_out(
                &store,
                &sqlite,
                &ticker,
                scenarios_needing_detail,
            )
            .await?;

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
        }

        if store.count_scenarios_with_periods().await? < store.count_scenarios().await? {
            return Err(Error::string(
                "not every scenario has persisted quarterly periods",
            ));
        }

        compute_and_persist_monte_carlo(db).await?;

        Ok(ok(self.name()))
    }
}

async fn run_blueprint_phase(
    store: &ScenarioStore<'_>,
    sqlite: &PathBuf,
    ticker: &str,
) -> Result<()> {
    let blueprint_response = ScenarioBuilderAgent::new(ScenarioBuilderConfig::blueprint())
        .with_company_label(ticker)
        .run(sqlite.clone(), ticker)
        .await?;
    let blueprint = ScenarioBuilderAgent::parse_blueprint_output(&blueprint_response.0)?;
    store.persist_blueprint(&blueprint).await
}

async fn run_detail_fan_out(
    store: &ScenarioStore<'_>,
    sqlite: &PathBuf,
    ticker: &str,
    scenarios: Vec<(String, String, String)>,
) -> Result<(usize, usize)> {
    let mut detail_tasks: Vec<(
        String,
        tokio::task::JoinHandle<Result<(String, Option<i64>)>>,
    )> = Vec::new();

    for (scenario_key, name, description) in scenarios {
        let (sqlite, ticker) = (sqlite.clone(), ticker.to_string());
        let label = format!("scenario_{scenario_key}");
        detail_tasks.push((
            label,
            tokio::spawn(async move {
                run_scenario_detail_with_retry(&sqlite, &ticker, &scenario_key, &name, &description)
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

    Ok((detail_ok, detail_failed))
}

async fn run_scenario_detail_with_retry(
    sqlite: &PathBuf,
    ticker: &str,
    scenario_key: &str,
    name: &str,
    description: &str,
) -> Result<(String, Option<i64>)> {
    let label = format!("scenario_{scenario_key}");
    let prefix = detail_prompt_prefix(scenario_key, name, description);
    let mut last_err: Option<Error> = None;

    for attempt in 0..SCENARIO_DETAIL_WORKER_MAX_ATTEMPTS {
        if attempt > 0 {
            let delay = scenario_detail_retry_delay(attempt - 1);
            tracing::warn!(
                worker = %label,
                attempt = attempt + 1,
                max_attempts = SCENARIO_DETAIL_WORKER_MAX_ATTEMPTS,
                delay_secs = delay.as_secs(),
                error = %last_err.as_ref().expect("retry requires prior error"),
                "retrying scenario detail worker after empty completion"
            );
            tokio::time::sleep(delay).await;
        }

        match run_scenario_detail_agent(sqlite, ticker, &prefix, scenario_key).await {
            Ok(result) => return Ok(result),
            Err(err) if is_empty_completion_error(&err)
                && attempt + 1 < SCENARIO_DETAIL_WORKER_MAX_ATTEMPTS =>
            {
                last_err = Some(err);
            }
            Err(err) => return Err(err),
        }
    }

    Err(last_err.unwrap_or_else(|| Error::string("scenario detail worker failed")))
}

async fn run_scenario_detail_agent(
    sqlite: &PathBuf,
    ticker: &str,
    prefix: &str,
    scenario_key: &str,
) -> Result<(String, Option<i64>)> {
    ScenarioBuilderAgent::new(
        ScenarioBuilderConfig::detail()
            .with_prompt_prefix(prefix)
            .with_focus_scenario_key(scenario_key),
    )
    .with_company_label(ticker)
    .run(sqlite.clone(), ticker)
    .await
}

fn detail_prompt_prefix(scenario_key: &str, name: &str, description: &str) -> String {
    format!(
        "FOCUS: Build quarterly projection detail for scenario `{scenario_key}` only.\n\
         Name: {name}\nDescription: {description}\n\n\
         Anchor ~4 historical quarters on av_raw_facts; project 12–20 forward quarters. \
         Finish with submit_scenario_detail, per_worker true.\n\n"
    )
}

fn scenario_detail_retry_delay(retry_index: usize) -> Duration {
    let secs = SCENARIO_DETAIL_RETRY_DELAYS_SECS
        .get(retry_index)
        .copied()
        .unwrap_or_else(|| {
            SCENARIO_DETAIL_RETRY_DELAYS_SECS
                .last()
                .copied()
                .unwrap_or(15)
        });
    Duration::from_secs(secs)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{
        openrouter_chat::{is_empty_completion_error, EMPTY_COMPLETION_ERROR_MARKER},
        scenario_store::ScenarioStore,
        workspace_store::execute_schema,
        workspace_sql::execute_sql,
    };
    use sea_orm::Database;

    #[test]
    fn scenario_detail_retry_delay_uses_configured_backoff() {
        assert_eq!(scenario_detail_retry_delay(0), Duration::from_secs(5));
        assert_eq!(scenario_detail_retry_delay(1), Duration::from_secs(15));
        assert_eq!(scenario_detail_retry_delay(9), Duration::from_secs(15));
    }

    #[test]
    fn empty_completion_errors_are_detected_for_retry() {
        let err = Error::string(&format!(
            "{EMPTY_COMPLETION_ERROR_MARKER} 32 agent steps (finish_reason=None, web_search_requests=0, client_tool_calls=3, preview=<empty>)"
        ));
        assert!(is_empty_completion_error(&err));
        assert!(!is_empty_completion_error(&Error::string(
            "invalid scenario detail JSON"
        )));
    }

    #[tokio::test]
    async fn load_scenarios_needing_detail_skips_scenarios_with_periods() {
        let path = std::env::temp_dir().join(format!(
            "analogues-scenario-resume-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let db = Database::connect(crate::services::workspace_store::sqlite_uri(&path))
            .await
            .expect("sqlite");
        execute_schema(&db).await.expect("schema");

        for (order, key) in [(1, "complete"), (2, "pending")] {
            execute_sql(
                &db,
                &format!(
                    "INSERT INTO scenario_assumptions (
                        scenario_order, scenario_key, name, stance, probability, description, assumption_summary
                     ) VALUES ({order}, '{key}', '{key}', 'neutral', 0.5, 'desc', 'summary')"
                ),
            )
            .await
            .expect("scenario");
        }
        execute_sql(
            &db,
            "INSERT INTO scenario_periods (
                scenario_id, period_order, label, period_end, period_type, revenue_growth
             ) VALUES (1, 1, 'Q1', '2026-05-31', 'quarter', 0.1)",
        )
        .await
        .expect("period");

        let store = ScenarioStore::new(&db);
        let pending = store
            .load_scenarios_needing_detail()
            .await
            .expect("load");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].0, "pending");
    }
}
