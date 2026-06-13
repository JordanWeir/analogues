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
    agents::narrative_researcher::NarrativeResearcherAgent, services::workspace_sql::scalar_i64,
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use std::sync::Arc;
use strategy::NarrativeMapStrategy;

pub struct BuildNarrativeMapLane {
    strategy: NarrativeMapStrategy,
}

impl BuildNarrativeMapLane {
    pub fn new(strategy: NarrativeMapStrategy) -> Self {
        Self { strategy }
    }

    pub fn default_lane_name() -> &'static str {
        "build_narrative_map"
    }

    #[cfg(test)]
    pub fn fixture() -> Self {
        Self::new(NarrativeMapStrategy::Fixture)
    }
}

#[async_trait]
impl Lane for BuildNarrativeMapLane {
    fn name(&self) -> &'static str {
        Self::default_lane_name()
    }

    fn gates(&self) -> Vec<Arc<dyn Gate>> {
        gate::build_narrative_map_gates()
    }

    async fn run(&self, ctx: &mut LaneContext) -> Result<LaneResult> {
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

        match &self.strategy {
            NarrativeMapStrategy::Agent(config) => {
                NarrativeResearcherAgent::new(config.clone())
                    .run_on_workspace(&ctx.workspace)
                    .await?;
            }
            #[cfg(test)]
            NarrativeMapStrategy::Fixture => {
                crate::agents::tools::narrative_research::execute(
                    &ctx.workspace.paths.sqlite_path,
                    crate::agents::tools::narrative_research::TOOL_FINALIZE,
                    "{}",
                )
                .await
                .map_err(|err| {
                    Error::string(&format!(
                        "fixture strategy requires pre-populated narrative state: {err}"
                    ))
                })?;
            }
        }

        let source_count = scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) AS count FROM sources",
        )
        .await
        .unwrap_or(0);

        let writes = LaneWritesSummary::default()
            .read("run_metadata")
            .read("stock_info")
            .read("fundamentals")
            .read("concept_catalog_entries")
            .wrote("sources")
            .wrote("claims")
            .wrote("narrative_map")
            .wrote("narrative_map_items")
            .wrote("sections")
            .wrote("data_gaps");

        Ok(LaneResult {
            lane_name: self.name().to_string(),
            status: LaneStatus::Success,
            writes,
            gate_results: Vec::new(),
            error_message: if source_count == 0 {
                Some("narrative lane finished without sources".to_string())
            } else {
                None
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        lanes::{context::LaneConfig, runner::LinearRunner},
        services::workspace_store::{execute_schema, WorkspaceStore},
        workspace::{seed_database, InitWorkspaceRequest, WorkspacePaths},
    };
    use sea_orm::Database;
    use std::path::PathBuf;

    use super::fixtures::{catalog_lane_context, populate_fixture_narrative};

    async fn catalog_context_with_fixture_narrative() -> LaneContext {
        let (_ctx, sqlite_path) = catalog_lane_context().await;
        populate_fixture_narrative(&sqlite_path).await;
        let workspace = WorkspaceStore
            .open_workspace(&sqlite_path)
            .await
            .expect("reopen");
        LaneContext::new(workspace, LaneConfig::new("EXMP"))
    }

    #[tokio::test]
    async fn build_narrative_map_lane_skips_without_catalog() {
        let path = std::env::temp_dir().join(format!(
            "analogues-narrative-skip-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let db = Database::connect(crate::services::workspace_store::sqlite_uri(&path))
            .await
            .expect("sqlite");
        execute_schema(&db).await.expect("schema");
        let paths = WorkspacePaths {
            run_slug: "EXMP-2026-06-09-1".to_string(),
            workspace_dir: path.parent().unwrap().to_path_buf(),
            sqlite_path: path.clone(),
            generated_dir: path.parent().unwrap().join("generated"),
        };
        seed_database(
            &db,
            &InitWorkspaceRequest {
                ticker: "EXMP".to_string(),
                date: "2026-06-09".to_string(),
                base_dir: PathBuf::from("reports/stock-narrative-research"),
                fetch_financials: false,
                mapping_strategy: None,
                build_narrative_map: false,
                build_financial_analysis: false,
            },
            &paths,
        )
        .await
        .expect("seed");
        db.close().await.ok();

        let workspace = WorkspaceStore.open_workspace(&path).await.expect("open");
        let mut ctx = LaneContext::new(workspace, LaneConfig::new("EXMP"));
        let lane = BuildNarrativeMapLane::fixture();
        let result = lane.run(&mut ctx).await.expect("run");
        assert_eq!(result.status, LaneStatus::Skipped);
        ctx.workspace.close().await.ok();
    }

    #[tokio::test]
    async fn build_narrative_map_lane_passes_gates_with_fixture() {
        let mut ctx = catalog_context_with_fixture_narrative().await;
        let lane = BuildNarrativeMapLane::fixture();
        let report = LinearRunner::new(vec![Arc::new(lane)])
            .run(&mut ctx)
            .await
            .expect("runner");
        assert!(!report.stopped_early, "{:?}", report.stop_reason);
        ctx.workspace.close().await.ok();
    }
}
