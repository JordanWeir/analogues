use crate::{
    agents::scenario_builder::types::{
        SCENARIO_BLUEPRINT_MIN, SCENARIOS_TARGET, MIN_TOTAL_QUARTERLY_PERIODS,
    },
    lanes::{
        context::LaneContext,
        gate::{Gate, GateResult},
        result::{LaneResult, LaneStatus},
    },
    services::{scenario_store::ScenarioStore, workspace_sql::scalar_i64},
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use std::sync::Arc;

pub fn scenario_generation_gates() -> Vec<Arc<dyn Gate>> {
    vec![
        Arc::new(ScenariosPresentGate),
        Arc::new(ScenarioStanceCoverageGate),
        Arc::new(ScenarioProjectionCalendarGate),
        Arc::new(ScenarioPeriodsPresentGate),
        Arc::new(QuarterlyCadenceGate),
        Arc::new(MonteCarloPersistedGate),
    ]
}

struct ScenariosPresentGate;

#[async_trait]
impl Gate for ScenariosPresentGate {
    fn name(&self) -> &'static str {
        "scenarios_present"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }
        let count = ScenarioStore::new(ctx.workspace.connection())
            .count_scenarios()
            .await
            .unwrap_or(0);
        if count < SCENARIO_BLUEPRINT_MIN as i64 {
            return GateResult::reject(
                self.name(),
                format!("need at least {SCENARIO_BLUEPRINT_MIN} scenarios, got {count}"),
            );
        }
        GateResult::pass(self.name())
    }
}

struct ScenarioStanceCoverageGate;

#[async_trait]
impl Gate for ScenarioStanceCoverageGate {
    fn name(&self) -> &'static str {
        "scenario_stance_coverage"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }
        let db = ctx.workspace.connection();
        for stance in ["bullish", "neutral", "bearish"] {
            let count = scalar_i64(
                db,
                &format!(
                    "SELECT COUNT(*) AS count FROM scenario_assumptions WHERE stance = '{stance}'"
                ),
            )
            .await
            .unwrap_or(0);
            if count == 0 {
                return GateResult::reject(
                    self.name(),
                    format!("no scenario with stance '{stance}'"),
                );
            }
        }
        GateResult::pass(self.name())
    }
}

struct ScenarioProjectionCalendarGate;

#[async_trait]
impl Gate for ScenarioProjectionCalendarGate {
    fn name(&self) -> &'static str {
        "scenario_projection_calendar_present"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }
        let store = ScenarioStore::new(ctx.workspace.connection());
        if !store.has_projection_calendar().await.unwrap_or(false) {
            return GateResult::reject(
                self.name(),
                "scenario_projection_config not persisted from blueprint",
            );
        }
        let calendar = store
            .load_projection_calendar()
            .await
            .ok()
            .flatten();
        let Some(calendar) = calendar else {
            return GateResult::reject(self.name(), "scenario_projection_periods empty");
        };
        if calendar.periods.is_empty() {
            return GateResult::reject(self.name(), "scenario_projection_periods empty");
        }
        let terminal = scalar_i64(
            ctx.workspace.connection(),
            &format!(
                "SELECT COUNT(*) AS count FROM (
                    SELECT MAX(period_end) AS terminal_end
                    FROM scenario_periods
                    GROUP BY scenario_id
                 ) WHERE terminal_end != {}",
                sql_quote(&calendar.terminal_period_end),
            ),
        )
        .await
        .unwrap_or(0);
        if terminal > 0 {
            return GateResult::reject(
                self.name(),
                format!(
                    "{terminal} scenario(s) have a terminal period_end other than {}",
                    calendar.terminal_period_end
                ),
            );
        }
        GateResult::pass(self.name())
    }
}

fn sql_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

struct ScenarioPeriodsPresentGate;

#[async_trait]
impl Gate for ScenarioPeriodsPresentGate {
    fn name(&self) -> &'static str {
        "scenario_periods_present"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }
        let db = ctx.workspace.connection();
        let scenarios = scalar_i64(
            db,
            "SELECT COUNT(*) AS count FROM scenario_assumptions",
        )
        .await
        .unwrap_or(0);
        let with_periods = scalar_i64(
            db,
            "SELECT COUNT(DISTINCT scenario_id) AS count FROM scenario_periods",
        )
        .await
        .unwrap_or(0);
        if with_periods < scenarios {
            return GateResult::reject(
                self.name(),
                format!("{with_periods}/{scenarios} scenarios have scenario_periods rows"),
            );
        }
        GateResult::pass(self.name())
    }
}

struct QuarterlyCadenceGate;

#[async_trait]
impl Gate for QuarterlyCadenceGate {
    fn name(&self) -> &'static str {
        "quarterly_cadence_labeled"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }
        let db = ctx.workspace.connection();
        let non_quarter = scalar_i64(
            db,
            "SELECT COUNT(*) AS count FROM scenario_periods
             WHERE period_type IS NOT NULL AND period_type != 'quarter'",
        )
        .await
        .unwrap_or(0);
        if non_quarter > 0 {
            return GateResult::reject(
                self.name(),
                "scenario_periods must use period_type quarter",
            );
        }
        let missing_end = scalar_i64(
            db,
            "SELECT COUNT(*) AS count FROM scenario_periods
             WHERE period_end IS NULL OR TRIM(period_end) = ''",
        )
        .await
        .unwrap_or(0);
        if missing_end > 0 {
            return GateResult::reject(self.name(), "scenario_periods need period_end dates");
        }
        let min_periods = scalar_i64(
            db,
            "SELECT MIN(cnt) FROM (
                SELECT COUNT(*) AS cnt FROM scenario_periods GROUP BY scenario_id
             )",
        )
        .await
        .unwrap_or(0);
        if min_periods < MIN_TOTAL_QUARTERLY_PERIODS as i64 {
            return GateResult::warn(
                self.name(),
                format!(
                    "target {MIN_TOTAL_QUARTERLY_PERIODS}+ quarterly periods per scenario; minimum found {min_periods}"
                ),
            );
        }
        let _ = SCENARIOS_TARGET;
        GateResult::pass(self.name())
    }
}

struct MonteCarloPersistedGate;

#[async_trait]
impl Gate for MonteCarloPersistedGate {
    fn name(&self) -> &'static str {
        "monte_carlo_persisted"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }
        let summary = scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) AS count FROM monte_carlo_summary",
        )
        .await
        .unwrap_or(0);
        if summary == 0 {
            return GateResult::reject(self.name(), "monte_carlo_summary not persisted");
        }
        let bins = scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) AS count FROM monte_carlo_histogram_bins",
        )
        .await
        .unwrap_or(0);
        if bins == 0 {
            return GateResult::reject(self.name(), "monte_carlo_histogram_bins empty");
        }
        GateResult::pass(self.name())
    }
}
