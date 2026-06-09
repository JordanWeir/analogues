use crate::{
    lanes::{
        context::LaneContext,
        gate::{Gate, GateResult},
        result::{LaneResult, LaneStatus},
    },
    services::financial_analysis_store::FinancialAnalysisStore,
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use std::sync::Arc;

pub fn financial_mechanics_experiments_gates() -> Vec<Arc<dyn Gate>> {
    vec![
        Arc::new(CruxCandidatesPresentGate),
        Arc::new(ExperimentsHaveQuestionsGate),
        Arc::new(InputsAndUnitsRecordedGate),
        Arc::new(ArithmeticVsInterpretationSplitGate),
        Arc::new(PromotedLinkedToSourcesGate),
        Arc::new(RejectedExperimentsExplainedGate),
    ]
}

struct CruxCandidatesPresentGate;

#[async_trait]
impl Gate for CruxCandidatesPresentGate {
    fn name(&self) -> &'static str {
        "crux_candidates_present"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let store = FinancialAnalysisStore::new(ctx.workspace.connection());
        match store.count_promoted_cruxes().await {
            Ok(0) => GateResult::reject(self.name(), "no promoted crux_candidates available"),
            Ok(_) => GateResult::pass(self.name()),
            Err(err) => GateResult::reject(self.name(), format!("crux count failed: {err}")),
        }
    }
}

struct ExperimentsHaveQuestionsGate;

#[async_trait]
impl Gate for ExperimentsHaveQuestionsGate {
    fn name(&self) -> &'static str {
        "experiments_have_questions"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let count = match scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) FROM analysis_experiments WHERE TRIM(question) = ''",
        )
        .await
        {
            Ok(count) => count,
            Err(err) => {
                return GateResult::reject(self.name(), format!("experiment query failed: {err}"))
            }
        };

        if count > 0 {
            return GateResult::reject(self.name(), "analysis_experiments rows missing question");
        }

        GateResult::pass(self.name())
    }
}

struct InputsAndUnitsRecordedGate;

#[async_trait]
impl Gate for InputsAndUnitsRecordedGate {
    fn name(&self) -> &'static str {
        "inputs_and_units_recorded"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let count = match scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) FROM analysis_experiments
             WHERE disposition IN ('promoted', 'candidate')
               AND (json_array_length(inputs_json) = 0 OR TRIM(period_basis) = '')",
        )
        .await
        {
            Ok(count) => count,
            Err(err) => {
                return GateResult::reject(self.name(), format!("inputs query failed: {err}"))
            }
        };

        if count > 0 {
            return GateResult::reject(
                self.name(),
                "promoted or candidate experiments must record inputs_json and period_basis",
            );
        }

        GateResult::pass(self.name())
    }
}

struct ArithmeticVsInterpretationSplitGate;

#[async_trait]
impl Gate for ArithmeticVsInterpretationSplitGate {
    fn name(&self) -> &'static str {
        "arithmetic_vs_interpretation_split"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let rows = match query_all(
            ctx.workspace.connection(),
            "SELECT experiment_key, outputs_json
             FROM analysis_experiments
             WHERE disposition = 'promoted'",
        )
        .await
        {
            Ok(rows) => rows,
            Err(err) => {
                return GateResult::reject(self.name(), format!("outputs query failed: {err}"))
            }
        };

        if rows.is_empty() {
            return GateResult::reject(
                self.name(),
                "no promoted analysis_experiments were persisted",
            );
        }

        for row in rows {
            let key = row_string(&row, 0).unwrap_or_default();
            let outputs_json = row_string(&row, 1).unwrap_or_default();
            if !outputs_json_contains_split(&outputs_json) {
                return GateResult::reject(
                    self.name(),
                    format!(
                        "promoted experiment {key} must include arithmetic/ratio and interpretation outputs"
                    ),
                );
            }
        }

        GateResult::pass(self.name())
    }
}

struct PromotedLinkedToSourcesGate;

#[async_trait]
impl Gate for PromotedLinkedToSourcesGate {
    fn name(&self) -> &'static str {
        "promoted_linked_to_sources"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let count = match scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) FROM analysis_experiments
             WHERE disposition = 'promoted'
               AND crux_id IS NULL
               AND (source_note IS NULL OR TRIM(source_note) = '')
               AND json_array_length(inputs_json) = 0",
        )
        .await
        {
            Ok(count) => count,
            Err(err) => {
                return GateResult::reject(self.name(), format!("source link query failed: {err}"))
            }
        };

        if count > 0 {
            return GateResult::reject(
                self.name(),
                "promoted experiments must link to a crux, source_note, or inputs_json",
            );
        }

        GateResult::pass(self.name())
    }
}

struct RejectedExperimentsExplainedGate;

#[async_trait]
impl Gate for RejectedExperimentsExplainedGate {
    fn name(&self) -> &'static str {
        "rejected_experiments_explained"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let count = match scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) FROM analysis_experiments
             WHERE disposition = 'rejected'
               AND (rejection_reason IS NULL OR TRIM(rejection_reason) = '')",
        )
        .await
        {
            Ok(count) => count,
            Err(err) => {
                return GateResult::reject(
                    self.name(),
                    format!("rejection reason query failed: {err}"),
                )
            }
        };

        if count > 0 {
            return GateResult::reject(
                self.name(),
                "rejected experiments must include rejection_reason",
            );
        }

        GateResult::pass(self.name())
    }
}

fn outputs_json_contains_split(outputs_json: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(outputs_json) else {
        return false;
    };
    let Some(rows) = value.as_array() else {
        return false;
    };

    let has_arithmetic = rows.iter().any(|row| {
        row.get("kind")
            .and_then(|v| v.as_str())
            .is_some_and(|kind| {
                matches!(
                    kind,
                    "arithmetic" | "ratio" | "series_point" | "bridge_step"
                )
            })
    });
    let has_interpretation = rows.iter().any(|row| {
        row.get("kind").and_then(|v| v.as_str()) == Some("interpretation")
            && row
                .get("text")
                .and_then(|v| v.as_str())
                .is_some_and(|text| !text.trim().is_empty())
    });
    has_arithmetic && has_interpretation
}

async fn scalar_i64(db: &sea_orm::DatabaseConnection, sql: &str) -> Result<i64> {
    let rows = query_all(db, sql).await?;
    if rows.is_empty() {
        return Ok(0);
    }
    row_i64(&rows[0], 0)
}

async fn query_all(
    db: &sea_orm::DatabaseConnection,
    sql: &str,
) -> Result<Vec<sea_orm::QueryResult>> {
    db.query_all(Statement::from_string(DatabaseBackend::Sqlite, sql.to_string()))
        .await
        .map_err(|err| Error::string(&format!("query failed: {err}")))
}

fn row_i64(row: &sea_orm::QueryResult, index: usize) -> Result<i64> {
    row.try_get_by_index::<i64>(index)
        .map_err(|err| Error::string(&format!("expected integer column {index}: {err}")))
}

fn row_string(row: &sea_orm::QueryResult, index: usize) -> Result<String> {
    row.try_get_by_index::<String>(index)
        .map_err(|err| Error::string(&format!("expected text column {index}: {err}")))
}
