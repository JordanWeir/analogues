#[cfg(test)]
mod fixtures;
mod gate;

use super::{
    context::LaneContext,
    gate::Gate,
    lane::Lane,
    result::{LaneResult, LaneWritesSummary},
};
use gate::scenario_artifacts_gates;
use crate::services::{
    report_artifacts::render_and_persist_report,
    scenario_projection::monte_carlo_is_persisted,
    workspace_sql::scalar_i64,
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use std::sync::Arc;

pub struct ScenarioArtifactsLane {
    #[cfg(test)]
    fixture: bool,
}

impl ScenarioArtifactsLane {
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

impl Default for ScenarioArtifactsLane {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Lane for ScenarioArtifactsLane {
    fn name(&self) -> &'static str {
        "scenario_artifacts"
    }

    fn gates(&self) -> Vec<Arc<dyn Gate>> {
        scenario_artifacts_gates()
    }

    async fn run(&self, ctx: &mut LaneContext) -> Result<LaneResult> {
        let db = ctx.workspace.connection();

        #[cfg(test)]
        if self.fixture {
            fixtures::seed_minimum_report_data(db).await?;
            if !monte_carlo_is_persisted(db).await? {
                crate::services::scenario_projection::compute_and_persist_monte_carlo(db).await?;
            }
        }

        let scenario_count =
            scalar_i64(db, "SELECT COUNT(*) AS count FROM scenario_assumptions").await?;
        if scenario_count == 0 {
            return Ok(LaneResult::skipped(
                self.name(),
                "no scenario_assumptions rows",
            ));
        }

        if !monte_carlo_is_persisted(db).await? {
            return Err(Error::string(
                "monte_carlo_summary not persisted; run scenario_generation first",
            ));
        }

        let generated_dir = ctx.workspace.paths.generated_dir.clone();
        render_and_persist_report(db, &generated_dir, "scenario_artifacts").await?;

        Ok(LaneResult::success(
            self.name(),
            LaneWritesSummary::default()
                .wrote("artifacts")
                .note("rendered report.html"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::context::LaneConfig;
    use crate::services::workspace_store::execute_schema;
    use crate::workspace::{seed_database, InitWorkspaceRequest};
    use sea_orm::Database;
    use std::path::PathBuf;

    #[tokio::test]
    async fn fixture_lane_renders_report_html() {
        let path = std::env::temp_dir().join(format!(
            "analogues-scenario-artifacts-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let db = Database::connect(crate::services::workspace_store::sqlite_uri(&path))
            .await
            .expect("sqlite");
        execute_schema(&db).await.expect("schema");
        let paths = crate::workspace::WorkspacePaths {
            run_slug: "TEST-2026-06-15-1".to_string(),
            workspace_dir: path.parent().unwrap().to_path_buf(),
            sqlite_path: path.clone(),
            generated_dir: path.parent().unwrap().join("generated"),
        };
        seed_database(
            &db,
            &InitWorkspaceRequest {
                ticker: "MSFT".to_string(),
                date: "2026-06-15".to_string(),
                base_dir: PathBuf::from("reports"),
                fetch_financials: false,
                mapping_strategy: None,
                build_narrative_map: false,
                build_financial_analysis: false,
                build_scenario_generation: false,
            },
            &paths,
        )
        .await
        .expect("seed");
        db.close().await.expect("close");

        let workspace = crate::services::workspace_store::WorkspaceStore
            .open_workspace(&path)
            .await
            .expect("open");
        let mut ctx = LaneContext::new(workspace, LaneConfig::new("MSFT"));
        let lane = ScenarioArtifactsLane::fixture();
        let result = lane.run(&mut ctx).await.expect("run");
        assert_eq!(result.status, crate::lanes::result::LaneStatus::Success);
        assert!(
            ctx.workspace.paths.generated_dir.join("report.html").is_file(),
            "expected report.html"
        );
    }
}
