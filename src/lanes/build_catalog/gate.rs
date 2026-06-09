use super::BuildCatalogLane;
use crate::{
    lanes::{
        context::LaneContext,
        gate::{Gate, GateResult},
        result::LaneResult,
    },
    services::workspace_financial_store::WorkspaceFinancialStore,
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
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
        Arc::new(CatalogMaterializedGate),
        Arc::new(CoreFundamentalsTraceableGate),
        Arc::new(FlowMetricsPeriodLabeledGate),
    ]
}

struct CatalogMaterializedGate;

#[async_trait]
impl Gate for CatalogMaterializedGate {
    fn name(&self) -> &'static str {
        "catalog_materialized"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == crate::lanes::result::LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        match scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) AS count FROM concept_catalog_entries",
        )
        .await
        {
            Ok(0) => {
                GateResult::reject(self.name(), "concept catalog is empty after build_catalog")
            }
            Ok(_) => GateResult::pass(self.name()),
            Err(err) => GateResult::reject(self.name(), format!("catalog count failed: {err}")),
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

        if revenue.taxonomy.is_empty() || revenue.concept_name.is_empty() {
            return GateResult::reject(
                self.name(),
                "revenue mapping is missing SEC concept provenance",
            );
        }

        if revenue.rationale.trim().is_empty() && revenue.selected_by.trim().is_empty() {
            return GateResult::warn(
                self.name(),
                "revenue mapping has no rationale or selected_by audit trail",
            );
        }

        let traceable = scalar_i64(
            ctx.workspace.connection(),
            &format!(
                "SELECT COUNT(*) AS count FROM sec_raw_facts
                 WHERE taxonomy = '{}' AND concept_name = '{}'",
                sql_quote(&revenue.taxonomy),
                sql_quote(&revenue.concept_name),
            ),
        )
        .await;

        match traceable {
            Ok(0) => {
                return GateResult::reject(
                    self.name(),
                    format!(
                        "revenue mapping {}:{} is not traceable to sec_raw_facts",
                        revenue.taxonomy, revenue.concept_name
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
        let mappings = match store.load_active_canonical_mappings().await {
            Ok(mappings) => mappings,
            Err(err) => {
                return GateResult::reject(self.name(), format!("failed to load mappings: {err}"))
            }
        };
        let catalog = match store.load_concept_catalog_entries().await {
            Ok(entries) => entries,
            Err(err) => {
                return GateResult::reject(self.name(), format!("failed to load catalog: {err}"))
            }
        };

        let mut mixed_catalog_concepts = Vec::new();
        for mapping in mappings
            .iter()
            .filter(|mapping| CORE_FLOW_CANONICAL_KEYS.contains(&mapping.canonical_key.as_str()))
        {
            let Some(entry) = catalog.iter().find(|entry| {
                entry.taxonomy == mapping.taxonomy && entry.concept_name == mapping.concept_name
            }) else {
                continue;
            };

            let flow_shapes: Vec<&str> = FLOW_PERIOD_TYPES
                .iter()
                .copied()
                .filter(|shape| entry.period_shape_counts.get(*shape).copied().unwrap_or(0) > 0)
                .collect();

            if flow_shapes.len() > 1 {
                mixed_catalog_concepts.push(format!(
                    "{} (shapes: {})",
                    mapping.canonical_key,
                    flow_shapes.join(", ")
                ));
            }
        }

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

        if mixed_catalog_concepts.is_empty() {
            return GateResult::pass(self.name());
        }

        GateResult::warn(
            self.name(),
            format!(
                "mapped flow concepts expose multiple period shapes in catalog metadata: {}",
                mixed_catalog_concepts.join("; ")
            ),
        )
    }
}

async fn scalar_i64(db: &impl ConnectionTrait, sql: &str) -> Result<i64> {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            sql.to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("query failed: {err}")))?
        .ok_or_else(|| Error::string("query returned no row"))?;
    row.try_get::<i64>("", "count")
        .map_err(|err| Error::string(&format!("failed to parse count: {err}")))
}

fn sql_quote(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        lanes::{context::LaneConfig, result::LaneWritesSummary},
        services::{
            canonical_mapping::ConceptMappingStrategy,
            sec_facts_provider::extract_raw_facts_from_root,
            workspace_financial_store::{RawIngestPersist, WorkspaceFinancialStore},
            workspace_phases::{
                materialize_catalog_on_workspace, resolve_canonical_mappings_on_workspace,
            },
            workspace_store::{execute_schema, WorkspaceStore},
        },
        workspace::{seed_database, InitWorkspaceRequest, WorkspacePaths},
    };
    use sea_orm::Database;
    use serde_json::json;
    use std::path::PathBuf;

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

        if materialized {
            materialize_catalog_on_workspace(&workspace)
                .await
                .expect("catalog");
            resolve_canonical_mappings_on_workspace(
                &workspace,
                ConceptMappingStrategy::CandidateScoring,
                None,
            )
            .await
            .expect("resolve");
        }

        LaneContext::new(workspace, LaneConfig::new("EXMP"))
    }

    #[tokio::test]
    async fn catalog_materialized_gate_rejects_empty_catalog() {
        let ctx = catalog_gate_context(false).await;
        let result = LaneResult::success("build_catalog", LaneWritesSummary::default());
        let gate = CatalogMaterializedGate;
        assert_eq!(
            gate.check(&ctx, &result).await.status,
            crate::lanes::gate::GateStatus::Reject
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
        let lane = BuildCatalogLane::new(ConceptMappingStrategy::CandidateScoring);
        assert_eq!(Lane::gates(&lane).len(), 3);
    }
}
