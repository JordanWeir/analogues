use crate::services::{
    canonical_mapping::{resolve_canonical_mappings, CanonicalResolutionContext},
    concept_catalog::ConceptCatalog,
    fundamental_deriver::FundamentalDeriver,
    market_quote_provider::YahooChartMarketDataAdapter,
    sec_facts_provider::SecFactsProvider,
    workspace_financial_store::{IngestPersist, SnapshotPersist, WorkspaceFinancialStore},
    workspace_store::{
        normalize_ticker, validate_date, WorkspaceStore, DEFAULT_REPORT_ROOT,
    },
};
use crate::workspace::{SecIngestionResult, WorkspacePaths};
use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use std::{path::Path, path::PathBuf, time::Duration};

pub use crate::{
    services::{
        canonical_mapping::ConceptMappingStrategy,
        financial_run::FinancialRun,
    },
    workspace::InitWorkspaceRequest,
};

pub struct InitWorkspace;

#[async_trait]
impl Task for InitWorkspace {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "initWorkspace".to_string(),
            detail: "Initialize a stock research workspace and run SQLite database".to_string(),
        }
    }

    async fn run(&self, _app_context: &AppContext, vars: &task::Vars) -> Result<()> {
        let request = InitWorkspaceRequest::from_vars(vars)?;
        let paths = initialize_workspace(&request).await?;

        println!(
            "Created stock research workspace: {}",
            paths.workspace_dir.display()
        );
        println!("Initialized run database: {}", paths.sqlite_path.display());

        Ok(())
    }
}

impl InitWorkspaceRequest {
    pub fn from_vars(vars: &task::Vars) -> Result<Self> {
        let ticker = vars
            .cli
            .get("ticker")
            .or_else(|| vars.cli.get("symbol"))
            .map(String::as_str)
            .ok_or_else(|| {
                Error::string("initWorkspace requires ticker:<SYMBOL>, for example ticker:MSFT")
            })?;

        let date = vars
            .cli
            .get("date")
            .cloned()
            .unwrap_or_else(|| Utc::now().date_naive().format("%Y-%m-%d").to_string());

        validate_date(&date)?;

        let base_dir = vars
            .cli
            .get("base_dir")
            .map_or_else(|| PathBuf::from(DEFAULT_REPORT_ROOT), PathBuf::from);
        let fetch_financials = vars
            .cli
            .get("fetch_financials")
            .map(|value| !matches!(value.as_str(), "false" | "0" | "no" | "skip"))
            .unwrap_or(true);
        let mapping_strategy = vars
            .cli
            .get("mapping_strategy")
            .or_else(|| vars.cli.get("concept_mapping_strategy"))
            .map_or(
                Ok(Some(ConceptMappingStrategy::CandidateScoring)),
                |value| crate::services::canonical_mapping::ConceptMappingStrategy::from_var(value),
            )?;

        Ok(Self {
            ticker: normalize_ticker(ticker)?,
            date,
            base_dir,
            fetch_financials,
            mapping_strategy,
        })
    }
}

pub async fn initialize_workspace(request: &InitWorkspaceRequest) -> Result<WorkspacePaths> {
    let normalized_request = request.normalized()?;
    let store = WorkspaceStore;
    let handle = store.create_workspace(&normalized_request).await?;
    let paths = handle.paths.clone();

    let ingestion_result =
        fetch_and_seed_financials(handle.connection(), &paths.sqlite_path, &normalized_request)
            .await;
    let close_result = handle.close().await;

    ingestion_result?;
    close_result?;

    Ok(paths)
}

async fn fetch_and_seed_financials(
    db: &sea_orm::DatabaseConnection,
    sqlite_path: &Path,
    request: &InitWorkspaceRequest,
) -> Result<()> {
    if !request.fetch_financials {
        record_financial_fetch_gap(
            db,
            "skipped",
            Some("financial fetch was skipped by request"),
            &["starter financial fetch skipped".to_string()],
        )
        .await?;
        return Ok(());
    }

    let llm_review = matches!(
        request.mapping_strategy,
        Some(ConceptMappingStrategy::LlmReviewed)
    );
    let fetch_strategy = if llm_review {
        None
    } else {
        request.mapping_strategy
    };

    match fetch_financial_run_with_strategy(&request.ticker, fetch_strategy).await {
        Ok(mut run) => {
            if llm_review {
                persist_sec_ingestion(db, &run).await?;
                resolve_sec_canonical_layer(
                    &mut run,
                    &request.ticker,
                    ConceptMappingStrategy::LlmReviewed,
                    Some(sqlite_path.to_path_buf()),
                )
                .await;
                run.compute_derived_metrics();
                run.mark_gaps();
                persist_financial_run(db, &run).await?;
            } else if request.mapping_strategy.is_none() {
                persist_sec_ingestion(db, &run).await?;
            } else {
                persist_financial_run(db, &run).await?;
            }

            let status = if request.mapping_strategy.is_none() {
                "ingested"
            } else if run.gaps.is_empty() {
                "succeeded"
            } else {
                "partial"
            };
            let error = if request.mapping_strategy.is_none() {
                Some("canonical mapping and starter fundamentals deferred".to_string())
            } else if run.gaps.is_empty() {
                None
            } else {
                Some(format!("missing fields: {}", run.gaps.join(", ")))
            };
            record_financial_fetch_status(db, status, error.as_deref()).await?;
            if request.mapping_strategy.is_some() && run.gaps.is_empty() {
                close_data_gap(db, "starter_financials").await?;
            } else if request.mapping_strategy.is_some() {
                record_financial_fetch_gap(db, status, error.as_deref(), &run.gaps).await?;
            }
        }
        Err(err) => {
            let message = err.to_string();
            record_financial_fetch_gap(db, "failed", Some(&message), &[message.clone()]).await?;
        }
    }

    Ok(())
}

pub async fn fetch_financial_run(ticker: &str) -> Result<FinancialRun> {
    fetch_financial_run_with_strategy(ticker, Some(ConceptMappingStrategy::CandidateScoring)).await
}

async fn fetch_financial_run_with_strategy(
    ticker: &str,
    mapping_strategy: Option<ConceptMappingStrategy>,
) -> Result<FinancialRun> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("Mozilla/5.0")
        .build()
        .map_err(|err| Error::string(&format!("failed to build HTTP client: {err}")))?;
    let market_data = YahooChartMarketDataAdapter::new(client.clone());
    let sec_provider = SecFactsProvider::new(client);
    let mut run = FinancialRun::new(ticker);

    match market_data.fetch_snapshot(ticker).await {
        Ok(market) => run.merge_market(market),
        Err(err) => run
            .source_notes
            .push(format!("Yahoo chart fallback failed: {err}")),
    }

    match fetch_sec_layers(&sec_provider, ticker, mapping_strategy).await {
        Ok(sec_run) => run.merge_sec_layers(sec_run, true),
        Err(err) => run
            .source_notes
            .push(format!("SEC Company Facts unavailable or failed: {err}")),
    }

    run.compute_derived_metrics();
    run.mark_gaps();
    Ok(run)
}
async fn ingest_sec_facts(provider: &SecFactsProvider, ticker: &str) -> Result<SecIngestionResult> {
    let company = provider.lookup_company(ticker).await?;
    let payload = provider.fetch_company_facts(&company).await?;
    let raw_facts = provider.extract_raw_facts(&payload)?;
    let catalog_entries = ConceptCatalog::materialize_catalog_entries(&raw_facts);
    Ok(SecIngestionResult {
        company_name: company.company_title,
        fetched_at: payload.fetched_at,
        raw_facts,
        catalog_entries,
        source_provider: provider.provider_name().to_string(),
    })
}

async fn resolve_sec_canonical_layer(
    run: &mut FinancialRun,
    ticker: &str,
    mapping_strategy: ConceptMappingStrategy,
    workspace_sqlite: Option<PathBuf>,
) {
    let ingest = run
        .ingest
        .as_ref()
        .expect("resolve_sec_canonical_layer requires ingest layer");
    let resolution = resolve_canonical_mappings(
        mapping_strategy,
        &CanonicalResolutionContext {
            ticker,
            raw_sec_facts: &ingest.raw_facts,
            catalog_entries: &ingest.catalog_entries,
            fetched_at: &ingest.fetched_at,
            workspace_sqlite,
        },
    )
    .await;
    let derived = FundamentalDeriver::derive_starter_fundamentals(
        &ingest.raw_facts,
        &resolution.mappings,
        run.currency.as_deref(),
    );
    run.apply_resolution(resolution);
    run.apply_derived(derived);
    run.source_notes.push(
        "Resolved canonical mappings and derived starter fundamentals from SEC Company Facts. Baseline values are selected from aligned income statement periods; stale or mismatched concepts are excluded from derived margins."
            .to_string(),
    );
}

async fn fetch_sec_layers(
    provider: &SecFactsProvider,
    ticker: &str,
    mapping_strategy: Option<ConceptMappingStrategy>,
) -> Result<FinancialRun> {
    let ingest = ingest_sec_facts(provider, ticker).await?;
    let mut run = FinancialRun::new(ticker);
    run.apply_ingest(ingest);

    if let Some(strategy) = mapping_strategy {
        resolve_sec_canonical_layer(&mut run, ticker, strategy, None).await;
    }

    Ok(run)
}

async fn persist_sec_ingestion(db: &sea_orm::DatabaseConnection, run: &FinancialRun) -> Result<()> {
    let ingest = run
        .ingest
        .as_ref()
        .ok_or_else(|| Error::string("persist_sec_ingestion requires ingest layer"))?;
    let source_note = run.source_notes.join(" ");
    let input = IngestPersist {
        fetched_at: &run.fetched_at,
        company_name: run.company_name.as_deref(),
        currency: run.currency.as_deref(),
        source_note: &source_note,
        raw_sec_facts: &ingest.raw_facts,
        concept_catalog_entries: &ingest.catalog_entries,
    };
    WorkspaceFinancialStore::new(db)
        .persist_ingestion(&input)
        .await
}

async fn persist_financial_run(db: &sea_orm::DatabaseConnection, run: &FinancialRun) -> Result<()> {
    let ingest = run
        .ingest
        .as_ref()
        .ok_or_else(|| Error::string("persist_financial_run requires ingest layer"))?;
    let resolution = run
        .resolution
        .as_ref()
        .ok_or_else(|| Error::string("persist_financial_run requires resolution layer"))?;
    let source_note = run.source_notes.join(" ");
    let fundamentals = run.fundamental_metrics();
    let observations = run.all_observations();
    let input = SnapshotPersist {
        fetched_at: &run.fetched_at,
        company_name: run.company_name.as_deref(),
        currency: run.currency.as_deref(),
        source_note: &source_note,
        raw_sec_facts: &ingest.raw_facts,
        concept_catalog_entries: &ingest.catalog_entries,
        concept_review_decisions: &resolution.review_decisions,
        canonical_mappings: &resolution.mappings,
        observations: &observations,
        quality_flags: &run.quality_flags,
        fundamentals: &fundamentals,
    };
    WorkspaceFinancialStore::new(db)
        .persist_snapshot(&input)
        .await
}

async fn record_financial_fetch_gap(
    db: &sea_orm::DatabaseConnection,
    status: &str,
    error: Option<&str>,
    gaps: &[String],
) -> Result<()> {
    record_financial_fetch_status(db, status, error).await?;
    let now = Utc::now().to_rfc3339();
    let description = if gaps.is_empty() {
        "Starter financial fetch did not return all required fields.".to_string()
    } else {
        format!("Starter financial fetch gaps: {}", gaps.join(", "))
    };
    execute_sql(
        db,
        &format!(
            "INSERT INTO data_gaps (gap_key, description, status, created_at)
             VALUES ('starter_financials', '{}', 'open', '{}')
             ON CONFLICT(gap_key) DO UPDATE SET
                description = excluded.description,
                status = 'open'",
            sql_quote(&description),
            sql_quote(&now),
        ),
    )
    .await
}

async fn record_financial_fetch_status(
    db: &sea_orm::DatabaseConnection,
    status: &str,
    error: Option<&str>,
) -> Result<()> {
    execute_sql(
        db,
        &format!(
            "UPDATE run_metadata
             SET financial_fetch_status = '{}', financial_fetch_error = {}
             WHERE id = 1",
            sql_quote(status),
            sql_value(error),
        ),
    )
    .await
}

async fn close_data_gap(db: &sea_orm::DatabaseConnection, gap_key: &str) -> Result<()> {
    execute_sql(
        db,
        &format!(
            "UPDATE data_gaps SET status = 'closed' WHERE gap_key = '{}'",
            sql_quote(gap_key),
        ),
    )
    .await
}
async fn execute_sql(db: &impl ConnectionTrait, sql: &str) -> Result<()> {
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        sql.to_string(),
    ))
    .await
    .map_err(|err| Error::string(&format!("failed to execute SQL statement: {err}")))?;

    Ok(())
}

fn sql_quote(value: &str) -> String {
    value.replace('\'', "''")
}

fn sql_value(value: Option<&str>) -> String {
    value.map_or_else(
        || "NULL".to_string(),
        |value| format!("'{}'", sql_quote(value)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{
        concept_catalog::ConceptCatalog,
        fundamental_deriver::{ttm_windows, FundamentalDeriver, SecFact},
        sec_facts_provider::extract_raw_facts_from_root,
    };
    use crate::workspace::{DerivedFundamentals, SecRawFact};
    use serde_json::{json, Value};

    #[test]
    fn builds_ttm_from_four_contiguous_quarters() {
        let facts = vec![
            sec_test_fact("Revenue", "2026-01-01", "2026-03-31", 10.0),
            sec_test_fact("Revenue", "2025-10-01", "2025-12-31", 20.0),
            sec_test_fact("Revenue", "2025-07-01", "2025-09-30", 30.0),
            sec_test_fact("Revenue", "2025-04-01", "2025-06-30", 40.0),
        ];

        let windows = ttm_windows("revenue_ttm", &facts);

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].period_end, "2026-03-31");
        assert_eq!(windows[0].value, 100.0);
    }

    #[test]
    fn coherent_bundle_excludes_stale_mismatched_margin_inputs() {
        let facts_root = json!({
            "us-gaap": {
                "RevenueFromContractWithCustomerExcludingAssessedTax": {
                    "label": "Revenue",
                    "description": "Revenue from contracts with customers.",
                    "units": { "USD": [
                        sec_fact_json("10-Q", "2026-01-01", "2026-03-31", "2026-04-30", 10.0),
                        sec_fact_json("10-Q", "2025-10-01", "2025-12-31", "2026-02-15", 20.0),
                        sec_fact_json("10-Q", "2025-07-01", "2025-09-30", "2025-10-30", 30.0),
                        sec_fact_json("10-Q", "2025-04-01", "2025-06-30", "2025-07-30", 40.0)
                    ]}
                },
                "NetIncomeLoss": {
                    "label": "Net income",
                    "description": "Net income or loss.",
                    "units": { "USD": [
                        sec_fact_json("10-Q", "2026-01-01", "2026-03-31", "2026-04-30", 1.0),
                        sec_fact_json("10-Q", "2025-10-01", "2025-12-31", "2026-02-15", 2.0),
                        sec_fact_json("10-Q", "2025-07-01", "2025-09-30", "2025-10-30", 3.0),
                        sec_fact_json("10-Q", "2025-04-01", "2025-06-30", "2025-07-30", 4.0)
                    ]}
                },
                "GrossProfit": {
                    "label": "Gross profit",
                    "description": "Gross profit.",
                    "units": { "USD": [
                        sec_fact_json("10-K", "2016-01-01", "2016-12-31", "2017-02-15", 50.0)
                    ]}
                }
            }
        });
        let raw_facts = extract_raw_facts_from_root(&facts_root, "2026-06-04T00:00:00Z");
        let mappings = ConceptCatalog::seed_canonical_mappings(&raw_facts);

        let bundle = FundamentalDeriver::select_latest_baseline_bundle(&raw_facts, &mappings)
            .expect("coherent revenue/net income");

        assert_eq!(bundle.period_end, "2026-03-31");
        assert!(bundle.gross_profit.is_none());
        assert!(bundle
            .quality_flags
            .iter()
            .any(|flag| flag.starts_with("gross_profit_ttm_excluded_because_no_fact_matched")));
    }

    #[test]
    fn captures_unmapped_sec_facts_without_canonicalizing_them() {
        let facts_root = json!({
            "us-gaap": {
                "RevenueFromContractWithCustomerExcludingAssessedTax": {
                    "label": "Revenue",
                    "description": "Revenue from contracts with customers.",
                    "units": { "USD": [
                        sec_fact_json("10-K", "2025-01-01", "2025-12-31", "2026-02-15", 100.0)
                    ]}
                },
                "CloudRemainingPerformanceObligation": {
                    "label": "Cloud RPO",
                    "description": "Company-specific cloud backlog metric.",
                    "units": { "USD": [
                        sec_fact_json("10-K", "2025-01-01", "2025-12-31", "2026-02-15", 42.0)
                    ]}
                }
            }
        });

        let raw_facts = extract_raw_facts_from_root(&facts_root, "2026-06-04T00:00:00Z");
        let mappings = ConceptCatalog::seed_canonical_mappings(&raw_facts);
        let observations = FundamentalDeriver::build_observations(&raw_facts, &mappings);

        assert!(raw_facts
            .iter()
            .any(|fact| fact.concept_name == "CloudRemainingPerformanceObligation"));
        assert!(!mappings
            .iter()
            .any(|mapping| mapping.concept_name == "CloudRemainingPerformanceObligation"));
        assert!(!observations.iter().any(|observation| {
            observation.concept_name.as_deref() == Some("CloudRemainingPerformanceObligation")
        }));
    }

    #[test]
    fn materializes_catalog_entries_with_narrative_tags() {
        let facts_root = json!({
            "us-gaap": {
                "CloudRemainingPerformanceObligation": {
                    "label": "Cloud RPO",
                    "description": "Company-specific cloud backlog metric.",
                    "units": { "USD": [
                        sec_fact_json("10-K", "2025-01-01", "2025-12-31", "2026-02-15", 42.0)
                    ]}
                }
            }
        });

        let raw_facts = extract_raw_facts_from_root(&facts_root, "2026-06-04T00:00:00Z");
        let entries = ConceptCatalog::materialize_catalog_entries(&raw_facts);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].dominant_period_shape, "annual");
        assert_eq!(entries[0].series_usability, "event_point");
        assert!(entries[0].narrative_tags.contains(&"backlog".to_string()));
    }

    #[test]
    fn ingest_only_populates_catalog_without_canonical_layers() {
        let facts_root = json!({
            "us-gaap": {
                "Revenues": {
                    "label": "Revenues",
                    "units": { "USD": [
                        sec_fact_json("10-K", "2025-01-01", "2025-12-31", "2026-02-15", 100.0)
                    ]}
                }
            }
        });
        let raw_facts = extract_raw_facts_from_root(&facts_root, "2026-06-07T00:00:00Z");
        let ingest = SecIngestionResult {
            company_name: Some("Example Corp".to_string()),
            fetched_at: "2026-06-07T00:00:00Z".to_string(),
            raw_facts: raw_facts.clone(),
            catalog_entries: ConceptCatalog::materialize_catalog_entries(&raw_facts),
            source_provider: "SEC Company Facts".to_string(),
        };
        let mut run = FinancialRun::new("EXMP");
        run.apply_ingest(ingest);

        assert!(!run.ingest.as_ref().unwrap().raw_facts.is_empty());
        assert!(!run.ingest.as_ref().unwrap().catalog_entries.is_empty());
        assert!(run.resolution.is_none());
        assert!(run.all_observations().is_empty());
        assert!(run.starter().revenue_ttm.is_none());
    }

    #[tokio::test]
    async fn resolve_sec_canonical_layer_populates_mappings_and_observations() {
        let facts_root = json!({
            "us-gaap": {
                "Revenues": {
                    "label": "Revenues",
                    "units": { "USD": [
                        sec_fact_json("10-K", "2025-01-01", "2025-12-31", "2026-02-15", 100.0)
                    ]}
                },
                "NetIncomeLoss": {
                    "label": "Net income",
                    "units": { "USD": [
                        sec_fact_json("10-K", "2025-01-01", "2025-12-31", "2026-02-15", 10.0)
                    ]}
                }
            }
        });
        let raw_facts = extract_raw_facts_from_root(&facts_root, "2026-06-07T00:00:00Z");
        let ingest = SecIngestionResult {
            company_name: Some("Example Corp".to_string()),
            fetched_at: "2026-06-07T00:00:00Z".to_string(),
            raw_facts,
            catalog_entries: ConceptCatalog::materialize_catalog_entries(
                &extract_raw_facts_from_root(&facts_root, "2026-06-07T00:00:00Z"),
            ),
            source_provider: "SEC Company Facts".to_string(),
        };
        let mut run = FinancialRun::new("EXMP");
        run.apply_ingest(ingest);
        resolve_sec_canonical_layer(
            &mut run,
            "EXMP",
            ConceptMappingStrategy::CandidateScoring,
            None,
        )
        .await;

        assert!(!run.resolution.as_ref().unwrap().mappings.is_empty());
        assert!(!run.all_observations().is_empty());
    }

    #[test]
    fn candidate_scoring_can_select_non_seed_revenue_concept() {
        let facts_root = json!({
            "us-gaap": {
                "CustomerRevenue": {
                    "label": "Revenue from customer contracts",
                    "description": "Revenue from contracts with customers.",
                    "units": { "USD": [
                        sec_fact_json("10-Q", "2026-01-01", "2026-03-31", "2026-04-30", 10.0),
                        sec_fact_json("10-Q", "2025-10-01", "2025-12-31", "2026-02-15", 20.0),
                        sec_fact_json("10-Q", "2025-07-01", "2025-09-30", "2025-10-30", 30.0),
                        sec_fact_json("10-Q", "2025-04-01", "2025-06-30", "2025-07-30", 40.0)
                    ]}
                },
                "NetIncomeLoss": {
                    "label": "Net income",
                    "description": "Net income or loss.",
                    "units": { "USD": [
                        sec_fact_json("10-Q", "2026-01-01", "2026-03-31", "2026-04-30", 1.0),
                        sec_fact_json("10-Q", "2025-10-01", "2025-12-31", "2026-02-15", 2.0),
                        sec_fact_json("10-Q", "2025-07-01", "2025-09-30", "2025-10-30", 3.0),
                        sec_fact_json("10-Q", "2025-04-01", "2025-06-30", "2025-07-30", 4.0)
                    ]}
                }
            }
        });

        let raw_facts = extract_raw_facts_from_root(&facts_root, "2026-06-04T00:00:00Z");
        let mappings = ConceptCatalog::seed_canonical_mappings(&raw_facts);

        assert!(mappings.iter().any(|mapping| {
            mapping.canonical_key == "revenue" && mapping.concept_name == "CustomerRevenue"
        }));
        assert!(FundamentalDeriver::select_latest_baseline_bundle(&raw_facts, &mappings).is_some());
    }

    #[test]
    fn merge_preserves_raw_sec_facts_and_canonical_mappings() {
        let facts_root = json!({
            "us-gaap": {
                "RevenueFromContractWithCustomerExcludingAssessedTax": {
                    "label": "Revenue",
                    "description": "Revenue from contracts with customers.",
                    "units": { "USD": [
                        sec_fact_json("10-K", "2025-01-01", "2025-12-31", "2026-02-15", 100.0)
                    ]}
                },
                "CloudRemainingPerformanceObligation": {
                    "label": "Cloud RPO",
                    "description": "Company-specific cloud backlog metric.",
                    "units": { "USD": [
                        sec_fact_json("10-K", "2025-01-01", "2025-12-31", "2026-02-15", 42.0)
                    ]}
                }
            }
        });
        let raw_facts = extract_raw_facts_from_root(&facts_root, "2026-06-04T00:00:00Z");
        let mappings = ConceptCatalog::seed_canonical_mappings(&raw_facts);
        let observations = FundamentalDeriver::build_observations(&raw_facts, &mappings);
        let mut base = FinancialRun::new("TEST");
        let mut update = FinancialRun::new("TEST");
        update.ingest = Some(SecIngestionResult {
            company_name: None,
            fetched_at: "2026-06-04T00:00:00Z".to_string(),
            raw_facts: raw_facts.clone(),
            catalog_entries: ConceptCatalog::materialize_catalog_entries(&raw_facts),
            source_provider: "SEC Company Facts".to_string(),
        });
        update.resolution = Some(
            crate::services::canonical_mapping::CanonicalResolutionResult {
                mappings: mappings.clone(),
                review_decisions: Vec::new(),
                quality_flags: Vec::new(),
                strategy_id: "candidate_scoring".to_string(),
            },
        );
        update.derived = Some(DerivedFundamentals {
            observations: observations.clone(),
            ..DerivedFundamentals::default()
        });

        base.merge_sec_layers(update, true);

        assert_eq!(
            base.ingest.as_ref().unwrap().raw_facts.len(),
            raw_facts.len()
        );
        assert_eq!(
            base.resolution.as_ref().unwrap().mappings.len(),
            mappings.len()
        );
        assert_eq!(base.all_observations().len(), observations.len());
        assert!(base
            .ingest
            .as_ref()
            .unwrap()
            .raw_facts
            .iter()
            .any(|fact| fact.concept_name == "CloudRemainingPerformanceObligation"));
    }

    #[test]
    fn classifies_ytd_sec_facts_without_treating_them_as_annual_or_instant() {
        let q2_ytd = raw_test_fact("10-Q", Some("2025-06-01"), Some("2025-11-30"), "Q2");
        let q3_ytd = raw_test_fact("10-Q", Some("2025-06-01"), Some("2026-02-28"), "Q3");
        let q3_quarter = raw_test_fact("10-Q", Some("2025-12-01"), Some("2026-02-28"), "Q3");
        let annual = raw_test_fact("10-K", Some("2024-06-01"), Some("2025-05-31"), "FY");
        let instant = raw_test_fact("10-Q", None, Some("2026-02-28"), "Q3");

        assert_eq!(ConceptCatalog::classify_period(&q2_ytd), "ytd");
        assert_eq!(ConceptCatalog::classify_period(&q3_ytd), "ytd");
        assert_eq!(ConceptCatalog::classify_period(&q3_quarter), "quarter");
        assert_eq!(ConceptCatalog::classify_period(&annual), "annual");
        assert_eq!(ConceptCatalog::classify_period(&instant), "instant");
    }

    fn raw_test_fact(
        form: &str,
        start: Option<&str>,
        end: Option<&str>,
        fiscal_period: &str,
    ) -> SecRawFact {
        SecRawFact {
            taxonomy: "us-gaap".to_string(),
            concept_name: "RevenueFromContractWithCustomerExcludingAssessedTax".to_string(),
            label: Some("Revenue".to_string()),
            description: Some("Revenue from contracts with customers.".to_string()),
            unit: "USD".to_string(),
            form: Some(form.to_string()),
            start: start.map(str::to_string),
            end: end.map(str::to_string),
            filed: Some("2026-03-11".to_string()),
            fiscal_year: Some(2026),
            fiscal_period: Some(fiscal_period.to_string()),
            accession: Some("test".to_string()),
            frame: None,
            value: 1.0,
            raw_json: "{}".to_string(),
            fetched_at: "2026-06-04T00:00:00Z".to_string(),
        }
    }

    fn sec_test_fact(concept: &str, start: &str, end: &str, value: f64) -> SecFact {
        SecFact {
            concept: concept.to_string(),
            form: Some("10-Q".to_string()),
            start: Some(start.to_string()),
            end: Some(end.to_string()),
            filed: Some(end.to_string()),
            value,
        }
    }

    fn sec_fact_json(form: &str, start: &str, end: &str, filed: &str, value: f64) -> Value {
        json!({
            "form": form,
            "start": start,
            "end": end,
            "filed": filed,
            "fy": 2026,
            "fp": "Q1",
            "accn": "test",
            "val": value
        })
    }
}
