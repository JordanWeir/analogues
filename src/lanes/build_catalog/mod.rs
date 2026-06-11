mod gate;
mod strategy;
mod writes;

use super::{
    context::LaneContext, gate::Gate, lane::Lane, result::LaneResult, result::LaneStatus,
    result::LaneWritesSummary,
};
use crate::services::{
    canonical_mapping::ConceptMappingStrategy,
    workspace_phases::{
        derive_starter_fundamentals_on_workspace, materialize_catalog_on_workspace,
        resolve_canonical_mappings_on_workspace,
    },
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use std::sync::Arc;

pub use strategy::CatalogResolutionStrategy;

pub struct BuildCatalogLane {
    resolution_strategy: CatalogResolutionStrategy,
}

impl BuildCatalogLane {
    pub fn new(mapping_strategy: ConceptMappingStrategy) -> Self {
        Self {
            resolution_strategy: CatalogResolutionStrategy::from_mapping_strategy(mapping_strategy),
        }
    }

    pub fn with_resolution_strategy(resolution_strategy: CatalogResolutionStrategy) -> Self {
        Self {
            resolution_strategy,
        }
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
        let raw_fact_count =
            crate::services::workspace_financial_store::WorkspaceFinancialStore::new(
                ctx.workspace.connection(),
            )
            .load_sec_raw_facts()
            .await?
            .len();

        if raw_fact_count == 0 {
            return Ok(LaneResult::skipped(
                self.name(),
                "no sec_raw_facts available; catalog build requires ingest first",
            ));
        }

        materialize_catalog_on_workspace(&ctx.workspace).await?;
        resolve_canonical_mappings_on_workspace(
            &ctx.workspace,
            self.resolution_strategy.mapping_strategy(),
            self.resolution_strategy.agent_config().cloned(),
        )
        .await?;
        let run = derive_starter_fundamentals_on_workspace(&ctx.workspace).await?;

        let error = if run.gaps.is_empty() {
            None
        } else {
            Some(format!("missing fields: {}", run.gaps.join(", ")))
        };

        let mut writes = LaneWritesSummary::default()
            .read("sec_raw_facts")
            .wrote("concept_catalog_entries")
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
        services::{
            concept_catalog::ConceptCatalog,
            sec_facts_provider::extract_raw_facts_from_root,
            workspace_financial_store::{RawIngestPersist, WorkspaceFinancialStore},
            workspace_store::{execute_schema, WorkspaceStore},
        },
        workspace::{seed_database, InitWorkspaceRequest, WorkspacePaths},
    };
    use sea_orm::Database;
    use serde_json::json;
    use std::path::PathBuf;

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
            },
            &paths,
        )
        .await
        .expect("seed");

        let facts_root = json!({
            "us-gaap": {
                "Revenues": {
                    "label": "Revenues",
                    "units": { "USD": [{"form":"10-K","start":"2025-01-01","end":"2025-12-31","filed":"2026-02-15","val":100.0}]}
                },
                "NetIncomeLoss": {
                    "label": "Net income",
                    "units": { "USD": [{"form":"10-K","start":"2025-01-01","end":"2025-12-31","filed":"2026-02-15","val":10.0}]}
                },
                "Assets": {
                    "label": "Assets",
                    "units": { "USD": [{"form":"10-K","end":"2025-12-31","filed":"2026-02-15","val":500.0}]}
                },
                "Liabilities": {
                    "label": "Liabilities",
                    "units": { "USD": [{"form":"10-K","end":"2025-12-31","filed":"2026-02-15","val":200.0}]}
                },
                "WeightedAverageNumberOfDilutedSharesOutstanding": {
                    "label": "Diluted shares",
                    "units": { "shares": [{"form":"10-K","start":"2025-01-01","end":"2025-12-31","filed":"2026-02-15","val":10.0}]}
                }
            }
        });
        let raw_facts = extract_raw_facts_from_root(&facts_root, "2026-06-09T00:00:00Z");
        WorkspaceFinancialStore::new(&db)
            .persist_raw_ingest(&RawIngestPersist {
                fetched_at: "2026-06-09T00:00:00Z",
                company_name: Some("Example Corp"),
                currency: Some("USD"),
                source_note: "fixture",
                raw_sec_facts: &raw_facts,
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
        let lane = BuildCatalogLane::new(ConceptMappingStrategy::CandidateScoring);
        let result = lane.run(&mut ctx).await.expect("run");

        assert_eq!(result.status, LaneStatus::Success);
        let store = WorkspaceFinancialStore::new(ctx.workspace.connection());
        assert!(!store
            .load_concept_catalog_entries()
            .await
            .expect("catalog")
            .is_empty());
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
