use crate::{
    services::{
        av_canonical_mapping::{resolve_av_canonical_mappings, AvCanonicalResolution},
        av_derived_time_series::AvDerivedTimeSeries,
        av_fundamental_deriver::{AvFundamentalDeriver, AV_SOURCE_TYPE},
        concept_catalog::ConceptCatalog,
        concept_review::ConceptReviewDecisionRecord,
        financial_run::FinancialRun,
        workspace_financial_store::{
            DerivedPersist, ResolutionPersist, WorkspaceFinancialStore, WorkspaceStockInfo,
        },
        workspace_ingest::{
            close_data_gap, record_financial_fetch_gap, record_financial_fetch_status,
        },
        workspace_store::WorkspaceHandle,
    },
    workspace::{AvRawFact, DerivedFundamentals, SecRawFact},
};
use chrono::Utc;
use loco_rs::prelude::*;
use std::path::PathBuf;

pub struct WorkspaceIngestLayers {
    pub stock: WorkspaceStockInfo,
    pub fetched_at: String,
    pub av_raw_facts: Vec<AvRawFact>,
    pub sec_raw_facts: Vec<SecRawFact>,
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

pub async fn materialize_sec_catalog_on_workspace(handle: &WorkspaceHandle) -> Result<()> {
    let store = WorkspaceFinancialStore::new(handle.connection());
    let raw_facts = store.load_sec_raw_facts().await?;
    if raw_facts.is_empty() {
        return Ok(());
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
    let av_raw_facts = store.load_av_raw_facts().await?;
    if av_raw_facts.is_empty() {
        return Err(Error::string(
            "workspace has no av_raw_facts; run init_workspace ingest first",
        ));
    }
    let sec_raw_facts = store.load_sec_raw_facts().await?;
    let fetched_at = av_raw_facts
        .iter()
        .map(|fact| fact.fetched_at.as_str())
        .max()
        .unwrap_or(stock.updated_at.as_str())
        .to_string();
    Ok(WorkspaceIngestLayers {
        stock,
        fetched_at,
        av_raw_facts,
        sec_raw_facts,
    })
}

pub async fn resolve_av_canonical_mappings_on_workspace(
    handle: &WorkspaceHandle,
) -> Result<AvCanonicalResolution> {
    let store = WorkspaceFinancialStore::new(handle.connection());
    let layers = load_ingest_layers(&store).await?;
    let resolution = resolve_av_canonical_mappings(&layers.av_raw_facts)?;
    let review_decisions = Vec::<ConceptReviewDecisionRecord>::new();
    store
        .persist_canonical_resolution(&ResolutionPersist {
            fetched_at: &layers.fetched_at,
            concept_review_decisions: &review_decisions,
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
            "workspace has no active canonical mappings; run resolve_av_canonical_mappings first",
        ));
    }

    let daily_bars = store.load_daily_price_bars().await?;

    let mut derived = AvFundamentalDeriver::derive_starter_fundamentals(
        &layers.av_raw_facts,
        &mappings,
        layers.stock.currency.as_deref(),
    );

    let (derived_observations, derived_quality_flags) = AvDerivedTimeSeries::derive(
        &layers.av_raw_facts,
        &daily_bars,
        layers.stock.currency.as_deref(),
    );
    derived.observations.extend(derived_observations);
    derived.quality_flags.extend(derived_quality_flags);
    derived.source_notes.push(
        "Derived quarterly valuation bands, price HLOC, per-share metrics, and TTM windows from Alpha Vantage facts and daily_price_bars.".to_string(),
    );
    AvDerivedTimeSeries::apply_latest_ttm_to_starter(
        &mut derived.starter,
        &layers.av_raw_facts,
    );

    let mut run = financial_run_from_layers(&layers, derived);
    run.fundamental_source = Some(AV_SOURCE_TYPE.to_string());
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

pub async fn resolve_and_derive_on_workspace(handle: &WorkspaceHandle) -> Result<FinancialRun> {
    resolve_av_canonical_mappings_on_workspace(handle).await?;
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
    run.fundamental_source = Some(AV_SOURCE_TYPE.to_string());
    run.apply_derived(derived);
    run
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        services::{
            av_canonical_mapping::AV_TAXONOMY,
            workspace_store::{execute_schema, WorkspaceStore},
        },
        workspace::{seed_database, InitWorkspaceRequest, WorkspacePaths},
    };
    use sea_orm::Database;

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
                fetched_at: "2026-06-08T00:00:00Z".to_string(),
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
                fetched_at: "2026-06-08T00:00:00Z".to_string(),
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
                fetched_at: "2026-06-08T00:00:00Z".to_string(),
            },
        ]
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
                build_narrative_map: false,
                build_financial_analysis: false,
            },
            &paths,
        )
        .await
        .expect("seed");
        let facts = sample_av_facts();
        WorkspaceFinancialStore::new(&db)
            .persist_raw_ingest(&crate::services::workspace_financial_store::RawIngestPersist {
                fetched_at: "2026-06-08T00:00:00Z",
                company_name: Some("Example Corp"),
                currency: Some("USD"),
                source_note: "ingest only fixture",
                raw_av_facts: &facts,
                raw_sec_facts: &[],
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
    async fn reruns_av_mapping_and_derivation_on_ingest_only_workspace() {
        let dir =
            std::env::temp_dir().join(format!("analogues-phase-rerun-{}", uuid::Uuid::new_v4()));
        let handle = ingest_only_workspace(&dir).await;
        let store = WorkspaceFinancialStore::new(handle.connection());

        assert!(store
            .load_active_canonical_mappings()
            .await
            .expect("mappings")
            .is_empty());

        resolve_av_canonical_mappings_on_workspace(&handle)
            .await
            .expect("resolve");

        let mappings = store
            .load_active_canonical_mappings()
            .await
            .expect("mappings after resolve");
        assert!(!mappings.is_empty());
        assert!(mappings
            .iter()
            .all(|mapping| mapping.taxonomy == AV_TAXONOMY));

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
