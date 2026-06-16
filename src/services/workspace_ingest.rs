use crate::{
    services::{
        alpha_vantage_fundamentals_provider::AlphaVantageFundamentalsProvider,
        market_quote_provider::YahooChartMarketDataAdapter,
        sec_facts_provider::SecFactsProvider,
        workspace_financial_store::{RawIngestPersist, WorkspaceFinancialStore},
        workspace_phases::materialize_sec_catalog_on_workspace,
        workspace_sql::{execute_sql, sql_quote, sql_value},
        workspace_store::WorkspaceHandle,
    },
    workspace::{MarketQuoteSnapshot, SecRawFact},
};
use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::ConnectionTrait;
use std::time::Duration;

/// Phase-1 SEC fetch result (raw facts only; catalog materialization is optional).
#[derive(Debug, Clone)]
pub struct RawSecIngestResult {
    pub company_name: Option<String>,
    pub fetched_at: String,
    pub raw_facts: Vec<SecRawFact>,
    pub source_provider: String,
}

/// Outcome of the workspace ingest lane (phase 1).
#[derive(Debug, Clone)]
pub struct WorkspaceIngestOutcome {
    pub skipped: bool,
    pub sec_ingested: bool,
    pub market_persisted: bool,
    pub alpha_vantage_persisted: bool,
    pub fetch_status: String,
    pub fetch_error: Option<String>,
    pub source_notes: Vec<String>,
    pub av_raw_fact_count: usize,
    pub sec_raw_fact_count: usize,
}

pub fn build_financial_http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("Mozilla/5.0")
        .build()
        .map_err(|err| Error::string(&format!("failed to build HTTP client: {err}")))
}

pub async fn fetch_raw_sec_facts(
    provider: &SecFactsProvider,
    ticker: &str,
) -> Result<RawSecIngestResult> {
    let company = provider.lookup_company(ticker).await?;
    let payload = provider.fetch_company_facts(&company).await?;
    let raw_facts = provider.extract_raw_facts(&payload)?;
    Ok(RawSecIngestResult {
        company_name: company.company_title,
        fetched_at: payload.fetched_at,
        raw_facts,
        source_provider: provider.provider_name().to_string(),
    })
}

pub async fn run_workspace_ingest(
    db: &sea_orm::DatabaseConnection,
    ticker: &str,
    fetch_financials: bool,
) -> Result<WorkspaceIngestOutcome> {
    if !fetch_financials {
        record_financial_fetch_gap(
            db,
            "skipped",
            Some("financial fetch was skipped by request"),
            &["starter financial fetch skipped".to_string()],
        )
        .await?;
        return Ok(WorkspaceIngestOutcome {
            skipped: true,
            sec_ingested: false,
            market_persisted: false,
            alpha_vantage_persisted: false,
            fetch_status: "skipped".to_string(),
            fetch_error: Some("financial fetch was skipped by request".to_string()),
            source_notes: vec!["starter financial fetch skipped".to_string()],
            av_raw_fact_count: 0,
            sec_raw_fact_count: 0,
        });
    }

    let client = build_financial_http_client()?;
    let market_data = YahooChartMarketDataAdapter::new(client.clone());
    let sec_provider = SecFactsProvider::new(client.clone());

    let mut company_name: Option<String> = None;
    let mut currency: Option<String> = None;
    let mut source_notes = Vec::new();
    let mut fetched_at = Utc::now().to_rfc3339();
    let mut market_snapshot: Option<MarketQuoteSnapshot> = None;

    match market_data.fetch_snapshot(ticker).await {
        Ok(market) => {
            fetched_at = market.fetched_at.clone();
            company_name = market.company_name.clone();
            currency = market.currency.clone();
            source_notes.extend(market.source_notes.clone());
            market_snapshot = Some(market);
        }
        Err(err) => {
            source_notes.push(format!("Yahoo chart fallback failed: {err}"));
        }
    }

    let Some(alpha_vantage) = AlphaVantageFundamentalsProvider::from_env(client.clone()) else {
        let message = "Alpha Vantage fundamentals required: ALPHA_VANTAGE_API_KEY not set";
        source_notes.push(message.to_string());
        record_financial_fetch_gap(db, "failed", Some(message), &[message.to_string()]).await?;
        return Ok(WorkspaceIngestOutcome {
            skipped: false,
            sec_ingested: false,
            market_persisted: false,
            alpha_vantage_persisted: false,
            fetch_status: "failed".to_string(),
            fetch_error: Some(message.to_string()),
            source_notes,
            av_raw_fact_count: 0,
            sec_raw_fact_count: 0,
        });
    };

    let av_ingest = match alpha_vantage.fetch_raw_time_series(ticker).await {
        Ok(ingest) => ingest,
        Err(err) => {
            let message = err.to_string();
            source_notes.push(format!("Alpha Vantage fundamentals fetch failed: {message}"));
            record_financial_fetch_gap(db, "failed", Some(&message), &[message.clone()]).await?;
            return Ok(WorkspaceIngestOutcome {
                skipped: false,
                sec_ingested: false,
                market_persisted: false,
                alpha_vantage_persisted: false,
                fetch_status: "failed".to_string(),
                fetch_error: Some(message),
                source_notes,
                av_raw_fact_count: 0,
                sec_raw_fact_count: 0,
            });
        }
    };

    if company_name.is_none() {
        company_name = av_ingest.company_name.clone();
    }
    if currency.is_none() {
        currency = av_ingest.currency.clone();
    }
    fetched_at = av_ingest.fetched_at.clone();
    source_notes.extend(av_ingest.source_notes.clone());

    let store = WorkspaceFinancialStore::new(db);
    let include_implied_price = market_snapshot
        .as_ref()
        .and_then(|market| market.headlines.current_price)
        .is_none();
    let source_note = source_notes.join(" ");
    store
        .persist_av_raw_ingest(
            &av_ingest,
            company_name.as_deref(),
            currency.as_deref(),
            &source_note,
            include_implied_price,
        )
        .await?;
    source_notes.push(format!(
        "Ingested {} Alpha Vantage av_raw_facts observations.",
        av_ingest.raw_facts.len()
    ));
    if !av_ingest.daily_prices.is_empty() {
        source_notes.push(format!(
            "Persisted {} daily OHLC bars from Alpha Vantage.",
            av_ingest.daily_prices.len()
        ));
    }
    let alpha_vantage_persisted = true;
    let av_raw_fact_count = av_ingest.raw_facts.len();

    let sec_result = fetch_raw_sec_facts(&sec_provider, ticker).await;
    let (sec_ingested, sec_raw_fact_count) = match sec_result {
        Ok(sec) => {
            if company_name.is_none() {
                company_name = sec.company_name.clone();
            }
            source_notes.push(format!(
                "Ingested {} raw SEC Company Facts observations from {} for niche agent research.",
                sec.raw_facts.len(),
                sec.source_provider
            ));
            store
                .persist_raw_ingest(&RawIngestPersist {
                    fetched_at: &fetched_at,
                    company_name: company_name.as_deref(),
                    currency: currency.as_deref(),
                    source_note: &source_notes.join(" "),
                    raw_av_facts: &[],
                    raw_sec_facts: &sec.raw_facts,
                })
                .await?;
            (true, sec.raw_facts.len())
        }
        Err(err) => {
            source_notes.push(format!(
                "SEC Company Facts unavailable or failed (niche catalog only): {}",
                err
            ));
            (false, 0)
        }
    };

    let market_persisted = persist_market_if_available(&store, market_snapshot.as_ref()).await?;

    record_financial_fetch_status(db, "ingested", None).await?;

    Ok(WorkspaceIngestOutcome {
        skipped: false,
        sec_ingested,
        market_persisted,
        alpha_vantage_persisted,
        fetch_status: "ingested".to_string(),
        fetch_error: None,
        source_notes,
        av_raw_fact_count,
        sec_raw_fact_count,
    })
}

pub async fn finalize_sec_catalog_if_present(handle: &WorkspaceHandle) -> Result<()> {
    materialize_sec_catalog_on_workspace(handle).await
}

async fn persist_market_if_available(
    store: &WorkspaceFinancialStore<'_>,
    market: Option<&MarketQuoteSnapshot>,
) -> Result<bool> {
    if let Some(market) = market {
        store.persist_market_snapshot(market).await?;
        return Ok(true);
    }
    Ok(false)
}

pub async fn record_financial_fetch_gap(
    db: &impl ConnectionTrait,
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

pub async fn record_financial_fetch_status(
    db: &impl ConnectionTrait,
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

pub async fn close_data_gap(db: &impl ConnectionTrait, gap_key: &str) -> Result<()> {
    execute_sql(
        db,
        &format!(
            "UPDATE data_gaps SET status = 'closed' WHERE gap_key = '{}'",
            sql_quote(gap_key),
        ),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        services::{
            sec_facts_provider::extract_raw_facts_from_root, workspace_store::execute_schema,
        },
        workspace::{seed_database, InitWorkspaceRequest, MarketHeadlines, WorkspacePaths},
    };
    use sea_orm::Database;
    use serde_json::json;
    use std::path::PathBuf;

    async fn seeded_workspace() -> (sea_orm::DatabaseConnection, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "analogues-workspace-ingest-{}.sqlite",
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
        (db, path)
    }

    #[tokio::test]
    async fn skipped_ingest_records_gap_without_fetching() {
        let (db, _path) = seeded_workspace().await;
        let outcome = run_workspace_ingest(&db, "EXMP", false)
            .await
            .expect("ingest");

        assert!(outcome.skipped);
        assert!(!outcome.sec_ingested);
        db.close().await.ok();
    }

    #[tokio::test]
    async fn persist_raw_ingest_writes_av_and_sec_facts_without_catalog() {
        let (db, _path) = seeded_workspace().await;
        let facts_root = json!({
            "us-gaap": {
                "Revenues": {
                    "label": "Revenues",
                    "units": { "USD": [{"form":"10-K","start":"2025-01-01","end":"2025-12-31","filed":"2026-02-15","val":100.0}]}
                }
            }
        });
        let raw_sec_facts = extract_raw_facts_from_root(&facts_root, "2026-06-09T00:00:00Z");
        let raw_av_facts = vec![crate::workspace::AvRawFact {
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
        }];
        WorkspaceFinancialStore::new(&db)
            .persist_raw_ingest(&RawIngestPersist {
                fetched_at: "2026-06-09T00:00:00Z",
                company_name: Some("Example Corp"),
                currency: Some("USD"),
                source_note: "test ingest",
                raw_av_facts: &raw_av_facts,
                raw_sec_facts: &raw_sec_facts,
            })
            .await
            .expect("persist");

        let store = WorkspaceFinancialStore::new(&db);
        assert_eq!(
            store.load_av_raw_facts().await.expect("av facts").len(),
            raw_av_facts.len()
        );
        assert_eq!(
            store.load_sec_raw_facts().await.expect("sec facts").len(),
            raw_sec_facts.len()
        );
        assert!(store
            .load_concept_catalog_entries()
            .await
            .expect("catalog")
            .is_empty());
        db.close().await.ok();
    }

    #[tokio::test]
    async fn persist_market_snapshot_writes_price_and_observations() {
        let (db, _path) = seeded_workspace().await;
        let market = MarketQuoteSnapshot {
            ticker: "EXMP".to_string(),
            fetched_at: "2026-06-09T00:00:00Z".to_string(),
            currency: Some("USD".to_string()),
            company_name: Some("Example Corp".to_string()),
            headlines: MarketHeadlines {
                current_price: Some(123.45),
                ..MarketHeadlines::default()
            },
            observations: vec![crate::workspace::FundamentalObservation {
                canonical_key: Some("current_price".to_string()),
                metric_key: "current_price".to_string(),
                metric_label: "Current price".to_string(),
                statement_type: "market".to_string(),
                period_type: "instant".to_string(),
                period_start: None,
                period_end: None,
                as_of_date: Some("2026-06-09T00:00:00Z".to_string()),
                filed_at: None,
                fiscal_year: None,
                fiscal_period: None,
                value: 123.45,
                unit: Some("USD".to_string()),
                source_type: "Yahoo chart endpoint".to_string(),
                source_note: Some("test".to_string()),
                concept_name: None,
                form: None,
                accession: None,
                quality: Some("market_quote".to_string()),
                is_derived: false,
            }],
            data_sources: vec!["Yahoo chart endpoint".to_string()],
            source_notes: vec!["test market fetch".to_string()],
        };

        WorkspaceFinancialStore::new(&db)
            .persist_market_snapshot(&market)
            .await
            .expect("persist market");

        let store = WorkspaceFinancialStore::new(&db);
        let observations = store
            .load_fundamental_observations()
            .await
            .expect("observations");
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].metric_key, "current_price");

        let row = db
            .query_one(sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT metric_value FROM fundamentals WHERE metric_key = 'current_price'"
                    .to_string(),
            ))
            .await
            .expect("query")
            .expect("row");
        let price: f64 = row.try_get("", "metric_value").expect("price");
        assert_eq!(price, 123.45);

        db.close().await.ok();
    }
}
