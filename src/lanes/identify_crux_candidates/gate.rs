use crate::{
    agents::financial_model_explorer::explorer_context::{
        MIN_PROMOTED_CRUXES_WHEN_NARRATIVE_RICH, MIN_SUPPORTING_METRICS,
        NARRATIVE_CRUX_COUNT_FOR_STRICT_TRIAGE,
    },
    lanes::{
        context::LaneContext,
        gate::{Gate, GateResult},
        result::{LaneResult, LaneStatus},
    },
    services::{financial_analysis_store::FinancialAnalysisStore, workspace_sql::scalar_i64},
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use std::sync::Arc;

pub fn identify_crux_candidates_gates() -> Vec<Arc<dyn Gate>> {
    vec![
        Arc::new(NarrativeContextPresentGate),
        Arc::new(CruxCandidatesFalsifiableGate),
        Arc::new(CruxCoverageGate),
        Arc::new(SupportingMetricsPresentGate),
        Arc::new(PromotedMetricsHaveRationaleGate),
        Arc::new(PeriodShapeLabeledGate),
    ]
}

struct NarrativeContextPresentGate;

#[async_trait]
impl Gate for NarrativeContextPresentGate {
    fn name(&self) -> &'static str {
        "narrative_context_present"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let store = FinancialAnalysisStore::new(ctx.workspace.connection());
        match store.narrative_context_present().await {
            Ok(true) => GateResult::pass(self.name()),
            Ok(false) => GateResult::reject(
                self.name(),
                "narrative_map, narrative_map_items, and claims are all empty",
            ),
            Err(err) => GateResult::reject(self.name(), format!("narrative check failed: {err}")),
        }
    }
}

struct CruxCandidatesFalsifiableGate;

#[async_trait]
impl Gate for CruxCandidatesFalsifiableGate {
    fn name(&self) -> &'static str {
        "crux_candidates_falsifiable"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let rows = match query_all(
            ctx.workspace.connection(),
            "SELECT crux_key, watch_condition, breaking_signal
             FROM crux_candidates
             WHERE disposition = 'promoted' AND status = 'active'",
        )
        .await
        {
            Ok(rows) => rows,
            Err(err) => {
                return GateResult::reject(self.name(), format!("crux query failed: {err}"))
            }
        };

        if rows.is_empty() {
            return GateResult::reject(
                self.name(),
                "no promoted crux_candidates were persisted",
            );
        }

        for row in rows {
            let key = row_string(&row, 0).unwrap_or_default();
            let watch = row_string(&row, 1).unwrap_or_default();
            let breaking = row_string(&row, 2).unwrap_or_default();
            if watch.trim().is_empty() || breaking.trim().is_empty() {
                return GateResult::reject(
                    self.name(),
                    format!("crux {key} is missing watch_condition or breaking_signal"),
                );
            }
        }

        GateResult::pass(self.name())
    }
}

struct CruxCoverageGate;

#[async_trait]
impl Gate for CruxCoverageGate {
    fn name(&self) -> &'static str {
        "crux_coverage_vs_narrative"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let store = FinancialAnalysisStore::new(ctx.workspace.connection());
        let narrative_cruxes = match store.count_narrative_cruxes().await {
            Ok(count) => count,
            Err(err) => {
                return GateResult::reject(self.name(), format!("narrative crux count failed: {err}"))
            }
        };
        let promoted_cruxes = match store.count_promoted_cruxes().await {
            Ok(count) => count,
            Err(err) => {
                return GateResult::reject(self.name(), format!("promoted crux count failed: {err}"))
            }
        };

        if narrative_cruxes >= NARRATIVE_CRUX_COUNT_FOR_STRICT_TRIAGE
            && promoted_cruxes < MIN_PROMOTED_CRUXES_WHEN_NARRATIVE_RICH as i64
        {
            return GateResult::reject(
                self.name(),
                format!(
                    "narrative has {narrative_cruxes} crux items but only {promoted_cruxes} promoted crux_candidates (need {MIN_PROMOTED_CRUXES_WHEN_NARRATIVE_RICH})"
                ),
            );
        }

        GateResult::pass(self.name())
    }
}

struct SupportingMetricsPresentGate;

#[async_trait]
impl Gate for SupportingMetricsPresentGate {
    fn name(&self) -> &'static str {
        "supporting_metrics_present"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let store = FinancialAnalysisStore::new(ctx.workspace.connection());
        let promoted_cruxes = match store.count_promoted_cruxes().await {
            Ok(count) => count,
            Err(err) => {
                return GateResult::reject(self.name(), format!("promoted crux count failed: {err}"))
            }
        };
        if promoted_cruxes == 0 {
            return GateResult::pass(self.name());
        }

        let metric_count = match store.count_supporting_metrics().await {
            Ok(count) => count,
            Err(err) => {
                return GateResult::reject(self.name(), format!("metric count failed: {err}"))
            }
        };

        if metric_count < MIN_SUPPORTING_METRICS as i64 {
            return GateResult::reject(
                self.name(),
                format!(
                    "need at least {MIN_SUPPORTING_METRICS} supporting_metric_selections when cruxes are promoted (have {metric_count})"
                ),
            );
        }

        GateResult::pass(self.name())
    }
}

struct PromotedMetricsHaveRationaleGate;

#[async_trait]
impl Gate for PromotedMetricsHaveRationaleGate {
    fn name(&self) -> &'static str {
        "promoted_metrics_have_rationale"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let count = match scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) AS count FROM supporting_metric_selections
             WHERE crux_id IS NOT NULL AND TRIM(rationale) = ''",
        )
        .await
        {
            Ok(count) => count,
            Err(err) => {
                return GateResult::reject(self.name(), format!("metric query failed: {err}"))
            }
        };

        if count > 0 {
            return GateResult::reject(
                self.name(),
                "supporting_metric_selections linked to cruxes must have rationale",
            );
        }

        GateResult::pass(self.name())
    }
}

struct PeriodShapeLabeledGate;

#[async_trait]
impl Gate for PeriodShapeLabeledGate {
    fn name(&self) -> &'static str {
        "period_shape_labeled"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let count = match scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) AS count FROM supporting_metric_selections
             WHERE crux_id IS NOT NULL
               AND quality_status IN ('period_mixed', 'quarantined')",
        )
        .await
        {
            Ok(count) => count,
            Err(err) => {
                return GateResult::reject(self.name(), format!("quality query failed: {err}"))
            }
        };

        if count > 0 {
            return GateResult::warn(
                self.name(),
                "some crux-linked supporting metrics are flagged period_mixed or quarantined",
            );
        }

        GateResult::pass(self.name())
    }
}

async fn query_all(
    db: &sea_orm::DatabaseConnection,
    sql: &str,
) -> Result<Vec<sea_orm::QueryResult>> {
    db.query_all(Statement::from_string(DatabaseBackend::Sqlite, sql.to_string()))
        .await
        .map_err(|err| Error::string(&format!("query failed: {err}")))
}

fn row_string(row: &sea_orm::QueryResult, index: usize) -> Result<String> {
    row.try_get_by_index::<String>(index)
        .map_err(|err| Error::string(&format!("expected text column {index}: {err}")))
}
