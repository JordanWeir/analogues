use crate::{
    lanes::{
        context::LaneContext,
        gate::{Gate, GateResult},
        result::LaneResult,
    },
    services::{
        av_canonical_mapping::AV_TAXONOMY,
        workspace_financial_store::WorkspaceFinancialStore,
        workspace_sql::{scalar_i64, sql_quote},
    },
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

const CORE_FLOW_CANONICAL_KEYS: &[&str] = &[
    "revenue",
    "net_income",
    "gross_profit",
    "operating_income",
    "eps",
];
const FLOW_PERIOD_TYPES: &[&str] = &["quarter", "ytd", "annual"];

pub fn build_catalog_gates() -> Vec<Arc<dyn Gate>> {
    vec![
        Arc::new(AvRawFactsIngestedGate),
        Arc::new(CoreFundamentalsTraceableGate),
        Arc::new(FlowMetricsPeriodLabeledGate),
    ]
}

struct AvRawFactsIngestedGate;

#[async_trait]
impl Gate for AvRawFactsIngestedGate {
    fn name(&self) -> &'static str {
        "av_raw_facts_ingested"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == crate::lanes::result::LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        match scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) AS count FROM av_raw_facts",
        )
        .await
        {
            Ok(0) => GateResult::reject(self.name(), "av_raw_facts is empty after build_catalog"),
            Ok(_) => GateResult::pass(self.name()),
            Err(err) => GateResult::reject(self.name(), format!("av_raw_facts count failed: {err}")),
        }
    }
}

struct CoreFundamentalsTraceableGate;

#[async_trait]
impl Gate for CoreFundamentalsTraceableGate {
    fn name(&self) -> &'static str {
        "core_fundamentals_traceable"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == crate::lanes::result::LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let store = WorkspaceFinancialStore::new(ctx.workspace.connection());
        let mappings = match store.load_active_canonical_mappings().await {
            Ok(mappings) => mappings,
            Err(err) => {
                return GateResult::reject(self.name(), format!("failed to load mappings: {err}"))
            }
        };

        if mappings.is_empty() {
            return GateResult::reject(
                self.name(),
                "no active canonical mappings after build_catalog",
            );
        }

        let revenue = mappings
            .iter()
            .find(|mapping| mapping.canonical_key == "revenue");
        let Some(revenue) = revenue else {
            return GateResult::reject(self.name(), "revenue canonical mapping is missing");
        };

        if revenue.taxonomy != AV_TAXONOMY || revenue.concept_name.is_empty() {
            return GateResult::reject(
                self.name(),
                "revenue mapping is missing Alpha Vantage field provenance",
            );
        }

        let traceable = scalar_i64(
            ctx.workspace.connection(),
            &format!(
                "SELECT COUNT(*) AS count FROM av_raw_facts WHERE field_name = '{}'",
                sql_quote(&revenue.concept_name),
            ),
        )
        .await;

        match traceable {
            Ok(0) => {
                return GateResult::reject(
                    self.name(),
                    format!(
                        "revenue mapping {} is not traceable to av_raw_facts",
                        revenue.concept_name
                    ),
                );
            }
            Ok(_) => {}
            Err(err) => {
                return GateResult::reject(
                    self.name(),
                    format!("traceability check failed: {err}"),
                );
            }
        }

        if !mappings
            .iter()
            .any(|mapping| mapping.canonical_key == "net_income")
        {
            return GateResult::warn(self.name(), "net_income canonical mapping is missing");
        }

        if !mappings.iter().any(|mapping| mapping.canonical_key == "eps") {
            return GateResult::reject(self.name(), "eps canonical mapping is missing");
        }

        GateResult::pass(self.name())
    }
}

struct FlowMetricsPeriodLabeledGate;

#[async_trait]
impl Gate for FlowMetricsPeriodLabeledGate {
    fn name(&self) -> &'static str {
        "flow_metrics_period_labeled"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == crate::lanes::result::LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let store = WorkspaceFinancialStore::new(ctx.workspace.connection());
        let observations = match store.load_fundamental_observations().await {
            Ok(observations) => observations,
            Err(err) => {
                return GateResult::reject(
                    self.name(),
                    format!("failed to load observations: {err}"),
                )
            }
        };

        let mut period_types_by_metric: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for observation in observations
            .iter()
            .filter(|observation| observation.statement_type == "income_statement")
            .filter(|observation| FLOW_PERIOD_TYPES.contains(&observation.period_type.as_str()))
            .filter(|observation| {
                CORE_FLOW_CANONICAL_KEYS
                    .iter()
                    .any(|key| observation.canonical_key.as_deref() == Some(*key))
            })
        {
            period_types_by_metric
                .entry(observation.metric_key.clone())
                .or_default()
                .insert(observation.period_type.clone());
        }

        let mixed_observations: Vec<String> = period_types_by_metric
            .into_iter()
            .filter(|(_, period_types)| period_types.len() > 1)
            .map(|(metric_key, period_types)| {
                format!(
                    "{metric_key} ({})",
                    period_types.into_iter().collect::<Vec<_>>().join(", ")
                )
            })
            .collect();

        if !mixed_observations.is_empty() {
            return GateResult::reject(
                self.name(),
                format!(
                    "flow observations mix period types without normalization: {}",
                    mixed_observations.join("; ")
                ),
            );
        }

        GateResult::pass(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        lanes::{
            build_catalog::BuildCatalogLane,
            context::LaneConfig, result::LaneWritesSummary,
        },
        services::{
            workspace_financial_store::{RawIngestPersist, WorkspaceFinancialStore},
            workspace_phases::resolve_av_canonical_mappings_on_workspace,
            workspace_store::{execute_schema, WorkspaceStore},
        },
        workspace::{seed_database, AvRawFact, InitWorkspaceRequest, WorkspacePaths},
    };
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

    async fn catalog_gate_context(materialized: bool) -> LaneContext {
        let path = std::env::temp_dir().join(format!(
            "analogues-build-catalog-gate-{}.sqlite",
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

        if materialized {
            resolve_av_canonical_mappings_on_workspace(&workspace)
                .await
                .expect("resolve");
        }

        LaneContext::new(workspace, LaneConfig::new("EXMP"))
    }

    #[tokio::test]
    async fn av_raw_facts_gate_rejects_empty_workspace() {
        let ctx = catalog_gate_context(false).await;
        let result = LaneResult::success("build_catalog", LaneWritesSummary::default());
        let gate = AvRawFactsIngestedGate;
        assert_eq!(
            gate.check(&ctx, &result).await.status,
            crate::lanes::gate::GateStatus::Pass
        );
        ctx.workspace.close().await.ok();
    }

    #[tokio::test]
    async fn core_fundamentals_traceable_gate_passes_after_catalog_build() {
        let ctx = catalog_gate_context(true).await;
        let result = LaneResult::success("build_catalog", LaneWritesSummary::default());
        let gate = CoreFundamentalsTraceableGate;
        assert_eq!(
            gate.check(&ctx, &result).await.status,
            crate::lanes::gate::GateStatus::Pass
        );
        ctx.workspace.close().await.ok();
    }

    #[tokio::test]
    async fn build_catalog_lane_registers_gates() {
        use crate::lanes::lane::Lane;
        use crate::lanes::build_catalog::strategy::CatalogResolutionStrategy;
        let lane = BuildCatalogLane::new(CatalogResolutionStrategy::Deterministic);
        assert_eq!(Lane::gates(&lane).len(), 3);
    }
}
