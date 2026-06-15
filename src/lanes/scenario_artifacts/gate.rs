use crate::{
    lanes::{
        context::LaneContext,
        gate::{Gate, GateResult},
        result::{LaneResult, LaneStatus},
    },
    services::workspace_sql::scalar_i64,
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use std::{path::Path, sync::Arc};

pub fn scenario_artifacts_gates() -> Vec<Arc<dyn Gate>> {
    vec![
        Arc::new(ReportReadinessGate),
        Arc::new(ReportHtmlArtifactGate),
    ]
}

struct ReportReadinessGate;

#[async_trait]
impl Gate for ReportReadinessGate {
    fn name(&self) -> &'static str {
        "report_readiness"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }
        let db = ctx.workspace.connection();
        for (label, sql) in [
            ("sources", "SELECT COUNT(*) AS count FROM sources"),
            ("claims", "SELECT COUNT(*) AS count FROM claims"),
            (
                "revenue_ttm",
                "SELECT COUNT(*) AS count FROM fundamentals WHERE metric_key = 'revenue_ttm'",
            ),
            (
                "shares_outstanding",
                "SELECT COUNT(*) AS count FROM fundamentals WHERE metric_key = 'shares_outstanding'",
            ),
            (
                "monte_carlo_summary",
                "SELECT COUNT(*) AS count FROM monte_carlo_summary",
            ),
        ] {
            let count = scalar_i64(db, sql).await.unwrap_or(0);
            if count == 0 {
                return GateResult::reject(self.name(), format!("missing required {label} rows"));
            }
        }
        GateResult::pass(self.name())
    }
}

struct ReportHtmlArtifactGate;

#[async_trait]
impl Gate for ReportHtmlArtifactGate {
    fn name(&self) -> &'static str {
        "report_html_artifact"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }
        let db = ctx.workspace.connection();
        let artifact_count = scalar_i64(
            db,
            "SELECT COUNT(*) AS count FROM artifacts WHERE artifact_type = 'report_html'",
        )
        .await
        .unwrap_or(0);
        if artifact_count == 0 {
            return GateResult::reject(self.name(), "report_html artifact not recorded");
        }

        let report_path = ctx.workspace.paths.generated_dir.join("report.html");
        if !Path::new(&report_path).is_file() {
            return GateResult::reject(
                self.name(),
                format!("report.html missing at {}", report_path.display()),
            );
        }

        GateResult::pass(self.name())
    }
}
