mod gate;
mod strategy;
mod writes;

use super::{
    context::LaneContext, gate::Gate, lane::Lane, result::LaneResult, result::LaneStatus,
    result::LaneWritesSummary,
};
use crate::services::workspace_phases::{
    derive_starter_fundamentals_on_workspace, resolve_av_canonical_mappings_on_workspace,
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use std::sync::Arc;

pub use strategy::CatalogResolutionStrategy;

pub struct BuildCatalogLane {
    _resolution_strategy: CatalogResolutionStrategy,
}

impl BuildCatalogLane {
    pub fn new(resolution_strategy: CatalogResolutionStrategy) -> Self {
        Self {
            _resolution_strategy: resolution_strategy,
        }
    }

    pub fn with_resolution_strategy(resolution_strategy: CatalogResolutionStrategy) -> Self {
        Self::new(resolution_strategy)
    }
}

#[async_trait]
impl Lane for BuildCatalogLane {
    fn name(&self) -> &'static str {
        "build_catalog"
    }

    fn gates(&self) -> Vec<Arc<dyn Gate>> {
        gate::build_catalog_gates()
    }

    async fn run(&self, ctx: &mut LaneContext) -> Result<LaneResult> {
        let av_raw_fact_count =
            crate::services::workspace_financial_store::WorkspaceFinancialStore::new(
                ctx.workspace.connection(),
            )
            .load_av_raw_facts()
            .await?
            .len();

        if av_raw_fact_count == 0 {
            return Ok(LaneResult::skipped(
                self.name(),
                "no av_raw_facts available; catalog build requires Alpha Vantage ingest first",
            ));
        }

        resolve_av_canonical_mappings_on_workspace(&ctx.workspace).await?;
        let run = derive_starter_fundamentals_on_workspace(&ctx.workspace).await?;

        let error = if run.gaps.is_empty() {
            None
        } else {
            Some(format!("missing fields: {}", run.gaps.join(", ")))
        };

        let mut writes = LaneWritesSummary::default()
            .read("av_raw_facts")
            .wrote("canonical_metric_mappings")
            .wrote("fundamental_observations")
            .wrote("fundamentals")
            .wrote("run_metadata");

        if !run.gaps.is_empty() {
            writes = writes.wrote("data_gaps");
        }
        if !run.quality_flags.is_empty() {
            writes = writes.wrote("data_quality_flags");
        }

        Ok(LaneResult {
            lane_name: self.name().to_string(),
            status: LaneStatus::Success,
            writes,
            gate_results: Vec::new(),
            error_message: error,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        lanes::context::LaneConfig,
        services::workspace_financial_store::{RawIngestPersist, WorkspaceFinancialStore},
        workspace::{seed_database, AvRawFact, InitWorkspaceRequest, WorkspacePaths},
    };
    use crate::services::workspace_store::{execute_schema, WorkspaceStore};
    use sea_orm::Database;
    use std::path::PathBuf;

    fn sample_av_facts() -> Vec<AvRawFact> {
        vec![
            AvRawFact {
                endpoint: "INCOME_STATEMENT".to_string(),
                report_type: "annual".to_string(),
                field_name: "totalRevenue".to_string(),
                label: None,
                period_end: "2025-12-31".to_string(),
                period_type: "annual".to_string(),
                unit: "USD".to_string(),
                currency: Some("USD".to_string()),
                value: 100.0,
                raw_json: "{}".to_string(),
                fetched_at: "2026-06-09T00:00:00Z".to_string(),
            },
            AvRawFact {
                endpoint: "INCOME_STATEMENT".to_string(),
                report_type: "annual".to_string(),
                field_name: "netIncome".to_string(),
                label: None,
                period_end: "2025-12-31".to_string(),
                period_type: "annual".to_string(),
                unit: "USD".to_string(),
                currency: Some("USD".to_string()),
                value: 10.0,
                raw_json: "{}".to_string(),
                fetched_at: "2026-06-09T00:00:00Z".to_string(),
            },
            AvRawFact {
                endpoint: "OVERVIEW".to_string(),
                report_type: "overview".to_string(),
                field_name: "DilutedEPSTTM".to_string(),
                label: None,
                period_end: "2025-12-31".to_string(),
                period_type: "ttm".to_string(),
                unit: "USD".to_string(),
                currency: Some("USD".to_string()),
                value: 1.25,
                raw_json: "{}".to_string(),
                fetched_at: "2026-06-09T00:00:00Z".to_string(),
            },
        ]
    }

    async fn ingest_only_lane_context() -> LaneContext {
        let path = std::env::temp_dir().join(format!(
            "analogues-build-catalog-{}.sqlite",
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
                checkpoints: false,
            },
            &paths,
        )
        .await
        .expect("seed");

        WorkspaceFinancialStore::new(&db)
            .persist_raw_ingest(&RawIngestPersist {
                fetched_at: "2026-06-09T00:00:00Z",
                company_name: Some("Example Corp"),
                currency: Some("USD"),
                source_note: "fixture",
                raw_av_facts: &sample_av_facts(),
                raw_sec_facts: &[],
            })
            .await
            .expect("persist");
        db.close().await.expect("close");

        let workspace = WorkspaceStore.open_workspace(&path).await.expect("open");
        LaneContext::new(workspace, LaneConfig::new("EXMP"))
    }

    #[tokio::test]
    async fn build_catalog_lane_materializes_mappings_and_fundamentals() {
        let mut ctx = ingest_only_lane_context().await;
        let lane = BuildCatalogLane::new(CatalogResolutionStrategy::Deterministic);
        let result = lane.run(&mut ctx).await.expect("run");

        assert_eq!(result.status, LaneStatus::Success);
        let store = WorkspaceFinancialStore::new(ctx.workspace.connection());
        assert!(!store
            .load_active_canonical_mappings()
            .await
            .expect("mappings")
            .is_empty());
        assert!(!store
            .load_fundamental_observations()
            .await
            .expect("observations")
            .is_empty());

        ctx.workspace.close().await.ok();
    }
}
