use crate::{
    services::{
        market_quote_provider::YahooChartMarketDataAdapter,
        sec_facts_provider::SecFactsProvider,
        workspace_financial_store::{RawIngestPersist, WorkspaceFinancialStore},
    },
    workspace::SecRawFact,
};
use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use std::time::Duration;

/// Phase-1 SEC fetch result (raw facts only; catalog materialization is phase 2).
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
    pub market_fetched: bool,
    pub fetch_status: String,
    pub fetch_error: Option<String>,
    pub source_notes: Vec<String>,
    pub raw_fact_count: usize,
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
            market_fetched: false,
            fetch_status: "skipped".to_string(),
            fetch_error: Some("financial fetch was skipped by request".to_string()),
            source_notes: vec!["starter financial fetch skipped".to_string()],
            raw_fact_count: 0,
        });
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("Mozilla/5.0")
        .build()
        .map_err(|err| Error::string(&format!("failed to build HTTP client: {err}")))?;
    let market_data = YahooChartMarketDataAdapter::new(client.clone());
    let sec_provider = SecFactsProvider::new(client);

    let mut company_name: Option<String> = None;
    let mut currency: Option<String> = None;
    let mut source_notes = Vec::new();
    let mut fetched_at = Utc::now().to_rfc3339();
    let mut market_fetched = false;

    match market_data.fetch_snapshot(ticker).await {
        Ok(market) => {
            fetched_at = market.fetched_at.clone();
            company_name = market.company_name.clone();
            currency = market.currency.clone();
            source_notes.extend(market.source_notes.clone());
            market_fetched = true;
        }
        Err(err) => {
            source_notes.push(format!("Yahoo chart fallback failed: {err}"));
        }
    }

    match fetch_raw_sec_facts(&sec_provider, ticker).await {
        Ok(sec) => {
            if company_name.is_none() {
                company_name = sec.company_name.clone();
            }
            fetched_at = sec.fetched_at.clone();
            source_notes.push(format!(
                "Ingested {} raw SEC Company Facts observations from {}.",
                sec.raw_facts.len(),
                sec.source_provider
            ));

            let source_note = source_notes.join(" ");
            WorkspaceFinancialStore::new(db)
                .persist_raw_ingest(&RawIngestPersist {
                    fetched_at: &fetched_at,
                    company_name: company_name.as_deref(),
                    currency: currency.as_deref(),
                    source_note: &source_note,
                    raw_sec_facts: &sec.raw_facts,
                })
                .await?;

            record_financial_fetch_status(db, "ingested", None).await?;

            Ok(WorkspaceIngestOutcome {
                skipped: false,
                sec_ingested: true,
                market_fetched,
                fetch_status: "ingested".to_string(),
                fetch_error: None,
                source_notes,
                raw_fact_count: sec.raw_facts.len(),
            })
        }
        Err(err) => {
            let message = err.to_string();
            source_notes.push(format!("SEC Company Facts unavailable or failed: {message}"));

            if market_fetched {
                let source_note = source_notes.join(" ");
                WorkspaceFinancialStore::new(db)
                    .persist_raw_ingest(&RawIngestPersist {
                        fetched_at: &fetched_at,
                        company_name: company_name.as_deref(),
                        currency: currency.as_deref(),
                        source_note: &source_note,
                        raw_sec_facts: &[],
                    })
                    .await?;
            }

            record_financial_fetch_gap(db, "failed", Some(&message), &[message.clone()]).await?;

            Ok(WorkspaceIngestOutcome {
                skipped: false,
                sec_ingested: false,
                market_fetched,
                fetch_status: "failed".to_string(),
                fetch_error: Some(message),
                source_notes,
                raw_fact_count: 0,
            })
        }
    }
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
    use crate::{
        services::{
            sec_facts_provider::extract_raw_facts_from_root,
            workspace_store::execute_schema,
        },
        workspace::{seed_database, InitWorkspaceRequest, WorkspacePaths},
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
    async fn persist_raw_ingest_writes_facts_without_catalog() {
        let (db, _path) = seeded_workspace().await;
        let facts_root = json!({
            "us-gaap": {
                "Revenues": {
                    "label": "Revenues",
                    "units": { "USD": [{"form":"10-K","start":"2025-01-01","end":"2025-12-31","filed":"2026-02-15","val":100.0}]}
                }
            }
        });
        let raw_facts = extract_raw_facts_from_root(&facts_root, "2026-06-09T00:00:00Z");
        WorkspaceFinancialStore::new(&db)
            .persist_raw_ingest(&RawIngestPersist {
                fetched_at: "2026-06-09T00:00:00Z",
                company_name: Some("Example Corp"),
                currency: Some("USD"),
                source_note: "test ingest",
                raw_sec_facts: &raw_facts,
            })
            .await
            .expect("persist");

        let store = WorkspaceFinancialStore::new(&db);
        assert_eq!(
            store.load_sec_raw_facts().await.expect("facts").len(),
            raw_facts.len()
        );
        assert!(
            store
                .load_concept_catalog_entries()
                .await
                .expect("catalog")
                .is_empty()
        );
        db.close().await.ok();
    }
}
