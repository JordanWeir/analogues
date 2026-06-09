use crate::{
    agents::fundamental_catalog_manager::FundamentalCatalogManagerConfig,
    services::{
        canonical_mapping::{
            resolve_canonical_mappings, CanonicalResolutionContext, CanonicalResolutionResult,
            ConceptMappingStrategy,
        },
        concept_catalog::ConceptCatalog,
        financial_run::FinancialRun,
        fundamental_deriver::FundamentalDeriver,
        workspace_financial_store::{
            DerivedPersist, ResolutionPersist, WorkspaceFinancialStore, WorkspaceStockInfo,
        },
        workspace_ingest::{
            close_data_gap, record_financial_fetch_gap, record_financial_fetch_status,
        },
        workspace_store::WorkspaceHandle,
    },
    workspace::{
        seed_database, ConceptCatalogEntry, DerivedFundamentals, SecIngestionResult, SecRawFact,
    },
};
use chrono::Utc;
use loco_rs::prelude::*;
use std::path::PathBuf;

pub struct WorkspaceIngestLayers {
    pub stock: WorkspaceStockInfo,
    pub fetched_at: String,
    pub raw_facts: Vec<SecRawFact>,
    pub catalog_entries: Vec<ConceptCatalogEntry>,
}

pub fn resolve_sqlite_path(vars: &task::Vars) -> Result<PathBuf> {
    vars.cli
        .get("workspace")
        .map(PathBuf::from)
        .ok_or_else(|| {
            Error::string(
                "workspace phase tasks require workspace:<PATH>, for example workspace:reports/stock-narrative-research/MSFT-2026-06-04-1/run.sqlite",
            )
        })
}

pub fn mapping_strategy_from_vars(vars: &task::Vars) -> Result<ConceptMappingStrategy> {
    let Some(value) = vars
        .cli
        .get("mapping_strategy")
        .or_else(|| vars.cli.get("concept_mapping_strategy"))
    else {
        return Err(Error::string(
            "resolveCanonicalMappings requires mapping_strategy:candidate_scoring or mapping_strategy:llm_reviewed",
        ));
    };
    ConceptMappingStrategy::from_var(value)?.ok_or_else(|| {
        Error::string(
            "resolveCanonicalMappings requires mapping_strategy:candidate_scoring or mapping_strategy:llm_reviewed",
        )
    })
}

pub async fn materialize_catalog_on_workspace(handle: &WorkspaceHandle) -> Result<()> {
    let store = WorkspaceFinancialStore::new(handle.connection());
    let raw_facts = store.load_sec_raw_facts().await?;
    if raw_facts.is_empty() {
        return Err(Error::string(
            "workspace has no sec_raw_facts; run init_workspace ingest first",
        ));
    }
    let catalog_entries = ConceptCatalog::materialize_catalog_entries(&raw_facts);
    let fetched_at = raw_facts
        .iter()
        .map(|fact| fact.fetched_at.as_str())
        .max()
        .map(str::to_string)
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    store
        .persist_catalog_entries(&catalog_entries, &fetched_at)
        .await?;
    Ok(())
}

pub async fn load_ingest_layers(
    store: &WorkspaceFinancialStore<'_>,
) -> Result<WorkspaceIngestLayers> {
    let stock = store.load_stock_info().await?;
    let raw_facts = store.load_sec_raw_facts().await?;
    if raw_facts.is_empty() {
        return Err(Error::string(
            "workspace has no sec_raw_facts; run ingest (initWorkspace with mapping_strategy:none) first",
        ));
    }
    let catalog_entries = store.load_concept_catalog_entries().await?;
    if catalog_entries.is_empty() {
        return Err(Error::string(
            "workspace has no concept_catalog_entries; run ingest first",
        ));
    }
    let fetched_at = raw_facts
        .iter()
        .map(|fact| fact.fetched_at.as_str())
        .max()
        .unwrap_or(stock.updated_at.as_str())
        .to_string();
    Ok(WorkspaceIngestLayers {
        stock,
        fetched_at,
        raw_facts,
        catalog_entries,
    })
}

pub async fn resolve_canonical_mappings_on_workspace(
    handle: &WorkspaceHandle,
    strategy: ConceptMappingStrategy,
    agent_config: Option<FundamentalCatalogManagerConfig>,
) -> Result<CanonicalResolutionResult> {
    let store = WorkspaceFinancialStore::new(handle.connection());
    let layers = load_ingest_layers(&store).await?;
    let sqlite_path = handle.paths.sqlite_path.clone();
    let resolution = resolve_canonical_mappings(
        strategy,
        &CanonicalResolutionContext {
            ticker: &layers.stock.ticker,
            raw_sec_facts: &layers.raw_facts,
            catalog_entries: &layers.catalog_entries,
            fetched_at: &layers.fetched_at,
            workspace_sqlite: Some(sqlite_path),
        },
        agent_config,
    )
    .await;
    store
        .persist_canonical_resolution(&ResolutionPersist {
            fetched_at: &layers.fetched_at,
            concept_review_decisions: &resolution.review_decisions,
            canonical_mappings: &resolution.mappings,
            quality_flags: &resolution.quality_flags,
        })
        .await?;
    Ok(resolution)
}

pub async fn derive_starter_fundamentals_on_workspace(
    handle: &WorkspaceHandle,
) -> Result<FinancialRun> {
    let store = WorkspaceFinancialStore::new(handle.connection());
    let layers = load_ingest_layers(&store).await?;
    let mappings = store.load_active_canonical_mappings().await?;
    if mappings.is_empty() {
        return Err(Error::string(
            "workspace has no active canonical mappings; run resolveCanonicalMappings first",
        ));
    }

    let derived = FundamentalDeriver::derive_starter_fundamentals(
        &layers.raw_facts,
        &mappings,
        layers.stock.currency.as_deref(),
    );
    let mut run = financial_run_from_layers(&layers, derived);
    run.compute_derived_metrics();
    run.mark_gaps();

    store
        .persist_derived_fundamentals(&DerivedPersist {
            fetched_at: &layers.fetched_at,
            observations: &run.all_observations(),
            quality_flags: &run.quality_flags,
            fundamentals: &run.fundamental_metrics(),
        })
        .await?;

    let gap_message = format!("missing fields: {}", run.gaps.join(", "));
    record_financial_fetch_status(
        handle.connection(),
        if run.gaps.is_empty() {
            "succeeded"
        } else {
            "partial"
        },
        if run.gaps.is_empty() {
            None
        } else {
            Some(gap_message.as_str())
        },
    )
    .await?;

    if run.gaps.is_empty() {
        close_data_gap(handle.connection(), "starter_financials").await?;
    } else {
        record_financial_fetch_gap(
            handle.connection(),
            "partial",
            Some(gap_message.as_str()),
            &run.gaps,
        )
        .await?;
    }

    Ok(run)
}

pub async fn resolve_and_derive_on_workspace(
    handle: &WorkspaceHandle,
    strategy: ConceptMappingStrategy,
) -> Result<FinancialRun> {
    resolve_canonical_mappings_on_workspace(handle, strategy, None).await?;
    derive_starter_fundamentals_on_workspace(handle).await
}

fn financial_run_from_layers(
    layers: &WorkspaceIngestLayers,
    derived: DerivedFundamentals,
) -> FinancialRun {
    let mut run = FinancialRun::new(&layers.stock.ticker);
    run.fetched_at = layers.fetched_at.clone();
    run.currency = layers.stock.currency.clone();
    run.company_name = layers.stock.company_name.clone();
    run.fundamental_source = Some(
        layers
            .stock
            .source_note
            .as_deref()
            .filter(|note| !note.is_empty())
            .unwrap_or("SEC Company Facts")
            .to_string(),
    );
    run.ingest = Some(SecIngestionResult {
        company_name: layers.stock.company_name.clone(),
        fetched_at: layers.fetched_at.clone(),
        raw_facts: layers.raw_facts.clone(),
        catalog_entries: layers.catalog_entries.clone(),
        source_provider: run
            .fundamental_source
            .clone()
            .unwrap_or_else(|| "SEC Company Facts".to_string()),
    });
    run.apply_derived(derived);
    run
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        services::{
            concept_catalog::ConceptCatalog,
            sec_facts_provider::extract_raw_facts_from_root,
            workspace_store::{execute_schema, WorkspaceStore},
        },
        workspace::{InitWorkspaceRequest, SecRawFact, WorkspacePaths},
    };
    use sea_orm::Database;
    use serde_json::json;

    fn sample_facts() -> Vec<SecRawFact> {
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
        extract_raw_facts_from_root(&facts_root, "2026-06-08T00:00:00Z")
    }

    async fn ingest_only_workspace(dir: &std::path::Path) -> WorkspaceHandle {
        let sqlite_path = dir.join("run.sqlite");
        std::fs::create_dir_all(dir).expect("workspace dir");
        let db = Database::connect(format!(
            "sqlite://{}?mode=rwc",
            sqlite_path.to_string_lossy().replace('\\', "/")
        ))
        .await
        .expect("sqlite");
        execute_schema(&db).await.expect("schema");
        let paths = WorkspacePaths {
            run_slug: "EXMP-2026-06-08-1".to_string(),
            workspace_dir: dir.to_path_buf(),
            sqlite_path: sqlite_path.clone(),
            generated_dir: dir.join("generated"),
        };
        seed_database(
            &db,
            &InitWorkspaceRequest {
                ticker: "EXMP".to_string(),
                date: "2026-06-08".to_string(),
                base_dir: dir.parent().unwrap_or(dir).to_path_buf(),
                fetch_financials: false,
                mapping_strategy: None,
            },
            &paths,
        )
        .await
        .expect("seed");
        let facts = sample_facts();
        let entries = ConceptCatalog::materialize_catalog_entries(&facts);
        WorkspaceFinancialStore::new(&db)
            .persist_ingestion(&crate::services::workspace_financial_store::IngestPersist {
                fetched_at: "2026-06-08T00:00:00Z",
                company_name: Some("Example Corp"),
                currency: Some("USD"),
                source_note: "ingest only fixture",
                raw_sec_facts: &facts,
                concept_catalog_entries: &entries,
            })
            .await
            .expect("persist ingest");
        db.close().await.expect("close");
        WorkspaceStore
            .open_workspace(&sqlite_path)
            .await
            .expect("open")
    }

    #[tokio::test]
    async fn reruns_mapping_and_derivation_on_ingest_only_workspace() {
        let dir =
            std::env::temp_dir().join(format!("analogues-phase-rerun-{}", uuid::Uuid::new_v4()));
        let handle = ingest_only_workspace(&dir).await;
        let store = WorkspaceFinancialStore::new(handle.connection());

        assert!(store
            .load_active_canonical_mappings()
            .await
            .expect("mappings")
            .is_empty());

        resolve_canonical_mappings_on_workspace(
            &handle,
            ConceptMappingStrategy::CandidateScoring,
            None,
        )
        .await
        .expect("resolve");

        let mappings = store
            .load_active_canonical_mappings()
            .await
            .expect("mappings after resolve");
        assert!(!mappings.is_empty());

        let run = derive_starter_fundamentals_on_workspace(&handle)
            .await
            .expect("derive");

        let observations = store
            .load_fundamental_observations()
            .await
            .expect("observations");
        assert!(!observations.is_empty());
        assert!(run.starter().revenue_ttm.is_some());

        handle.close().await.expect("close");
        std::fs::remove_dir_all(dir).ok();
    }
}
