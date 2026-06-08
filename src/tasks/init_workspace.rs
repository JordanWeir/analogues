use crate::services::{
    concept_catalog::ConceptCatalog,
    concept_review::{ConceptReviewDecisionRecord, ConceptReviewService, AGENT_REVIEW_PREAMBLE},
    market_quote_provider::YahooChartMarketDataAdapter,
    model_client::OpenRouterModelClient,
    review_workspace::{cleanup_review_workspace, materialize_review_workspace},
    sec_facts_provider::SecFactsProvider,
    workspace_financial_store::{FundamentalInsert, SnapshotPersist, WorkspaceFinancialStore},
    workspace_store::{
        normalize_ticker, validate_date, WorkspaceStore, DEFAULT_REPORT_ROOT, SCHEMA_VERSION,
    },
};
use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use std::{path::PathBuf, time::Duration};
use uuid::Uuid;

pub use crate::workspace::{
    CanonicalMapping, ConceptCatalogEntry, FundamentalObservation, SecRawFact, WorkspacePaths,
};

const REQUIRED_SECTIONS: &[&str] = &[
    "orientation",
    "business_model",
    "why_now",
    "narrative_map",
    "financial_snapshot",
    "financial_math",
    "industry_context",
    "watch_items",
    "historical_analogues",
    "final_talk_track",
    "scenario_assumptions",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitWorkspaceRequest {
    pub ticker: String,
    pub date: String,
    pub base_dir: PathBuf,
    pub fetch_financials: bool,
    pub mapping_strategy: ConceptMappingStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConceptMappingStrategy {
    CandidateScoring,
    LlmReviewed,
}

#[derive(Debug, Clone, Default)]
pub struct FinancialSnapshot {
    pub ticker: String,
    pub fetched_at: String,
    pub data_sources: Vec<String>,
    pub source_notes: Vec<String>,
    pub quality_flags: Vec<String>,
    pub observations: Vec<FundamentalObservation>,
    pub raw_sec_facts: Vec<SecRawFact>,
    pub concept_catalog_entries: Vec<ConceptCatalogEntry>,
    pub canonical_mappings: Vec<CanonicalMapping>,
    pub concept_review_decisions: Vec<ConceptReviewDecisionRecord>,
    pub gaps: Vec<String>,
    pub currency: Option<String>,
    pub company_name: Option<String>,
    pub current_price: Option<f64>,
    pub market_cap: Option<f64>,
    pub shares_outstanding: Option<f64>,
    pub revenue_ttm: Option<f64>,
    pub net_income_ttm: Option<f64>,
    pub gross_profit_ttm: Option<f64>,
    pub operating_income_ttm: Option<f64>,
    pub gross_margin: Option<f64>,
    pub operating_margin: Option<f64>,
    pub net_margin: Option<f64>,
    pub eps_ttm: Option<f64>,
    pub trailing_pe: Option<f64>,
    pub price_to_sales_ttm: Option<f64>,
    pub cash: Option<f64>,
    pub total_debt: Option<f64>,
    pub fundamental_period_end: Option<String>,
    pub fundamental_source: Option<String>,
}

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
            .map_or(Ok(ConceptMappingStrategy::CandidateScoring), |value| {
                ConceptMappingStrategy::from_var(value)
            })?;

        Ok(Self {
            ticker: normalize_ticker(ticker)?,
            date,
            base_dir,
            fetch_financials,
            mapping_strategy,
        })
    }
}

impl ConceptMappingStrategy {
    fn from_var(value: &str) -> Result<Self> {
        match value {
            "candidate" | "candidate_scoring" | "heuristic" => Ok(Self::CandidateScoring),
            "llm" | "llm_reviewed" | "model" => Ok(Self::LlmReviewed),
            _ => Err(Error::string(
                "mapping_strategy must be candidate_scoring or llm_reviewed",
            )),
        }
    }
}

pub async fn initialize_workspace(request: &InitWorkspaceRequest) -> Result<WorkspacePaths> {
    let normalized_request = request.normalized()?;
    let store = WorkspaceStore;
    let handle = store.create_workspace(&normalized_request).await?;
    let paths = handle.paths.clone();

    let ingestion_result =
        fetch_and_seed_financials(handle.connection(), &normalized_request).await;
    let close_result = handle.close().await;

    ingestion_result?;
    close_result?;

    Ok(paths)
}

impl InitWorkspaceRequest {
    fn normalized(&self) -> Result<Self> {
        validate_date(&self.date)?;
        Ok(Self {
            ticker: normalize_ticker(&self.ticker)?,
            date: self.date.clone(),
            base_dir: self.base_dir.clone(),
            fetch_financials: self.fetch_financials,
            mapping_strategy: self.mapping_strategy,
        })
    }
}

pub(crate) async fn seed_database(
    db: &sea_orm::DatabaseConnection,
    request: &InitWorkspaceRequest,
    paths: &WorkspacePaths,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    execute_sql(
        db,
        &format!(
            "INSERT INTO run_metadata (
                id, ticker, run_slug, workspace_path, sqlite_path, status, schema_version,
                created_at, financial_fetch_status, financial_fetch_error
            ) VALUES (
                1, '{}', '{}', '{}', '{}', 'initialized', {}, '{}',
                'not_attempted', NULL
            )",
            sql_quote(&request.ticker),
            sql_quote(&paths.run_slug),
            sql_quote(&paths.workspace_dir.display().to_string()),
            sql_quote(&paths.sqlite_path.display().to_string()),
            SCHEMA_VERSION,
            sql_quote(&now),
        ),
    )
    .await?;

    execute_sql(
        db,
        &format!(
            "INSERT INTO stock_info (id, ticker, source_note, updated_at)
             VALUES (1, '{}', 'Seeded by initWorkspace; agent should fill company details.', '{}')",
            sql_quote(&request.ticker),
            sql_quote(&now),
        ),
    )
    .await?;

    execute_sql(db, "INSERT INTO monte_carlo_config (id) VALUES (1)").await?;
    ConceptCatalog::seed_canonical_definitions(db, &now).await?;

    for (index, section_key) in REQUIRED_SECTIONS.iter().enumerate() {
        execute_sql(
            db,
            &format!(
                "INSERT INTO sections (section_key, section_order, status, created_at, updated_at)
                 VALUES ('{}', {}, 'pending', '{}', '{}')",
                sql_quote(section_key),
                index + 1,
                sql_quote(&now),
                sql_quote(&now),
            ),
        )
        .await?;
    }

    Ok(())
}

async fn fetch_and_seed_financials(
    db: &sea_orm::DatabaseConnection,
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

    match fetch_financial_snapshot_with_strategy(&request.ticker, request.mapping_strategy).await {
        Ok(snapshot) => {
            persist_financial_snapshot(db, &snapshot).await?;
            let status = if snapshot.gaps.is_empty() {
                "succeeded"
            } else {
                "partial"
            };
            let error = if snapshot.gaps.is_empty() {
                None
            } else {
                Some(format!("missing fields: {}", snapshot.gaps.join(", ")))
            };
            record_financial_fetch_status(db, status, error.as_deref()).await?;
            if snapshot.gaps.is_empty() {
                close_data_gap(db, "starter_financials").await?;
            } else {
                record_financial_fetch_gap(db, status, error.as_deref(), &snapshot.gaps).await?;
            }
        }
        Err(err) => {
            let message = err.to_string();
            record_financial_fetch_gap(db, "failed", Some(&message), &[message.clone()]).await?;
        }
    }

    Ok(())
}

pub async fn fetch_financial_snapshot(ticker: &str) -> Result<FinancialSnapshot> {
    fetch_financial_snapshot_with_strategy(ticker, ConceptMappingStrategy::CandidateScoring).await
}

async fn fetch_financial_snapshot_with_strategy(
    ticker: &str,
    mapping_strategy: ConceptMappingStrategy,
) -> Result<FinancialSnapshot> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("Mozilla/5.0")
        .build()
        .map_err(|err| Error::string(&format!("failed to build HTTP client: {err}")))?;
    let market_data = YahooChartMarketDataAdapter::new(client.clone());
    let sec_provider = SecFactsProvider::new(client);
    let mut snapshot = FinancialSnapshot::new(ticker);

    match market_data.fetch_snapshot(ticker).await {
        Ok(update) => snapshot.merge(update, false),
        Err(err) => snapshot
            .source_notes
            .push(format!("Yahoo chart fallback failed: {err}")),
    }

    match fetch_sec_companyfacts_snapshot(&sec_provider, ticker, mapping_strategy).await {
        Ok(update) => snapshot.merge(update, true),
        Err(err) => snapshot
            .source_notes
            .push(format!("SEC Company Facts unavailable or failed: {err}")),
    }

    snapshot.compute_derived_metrics();
    snapshot.mark_gaps();
    Ok(snapshot)
}

impl FinancialSnapshot {
    pub(crate) fn new(ticker: &str) -> Self {
        Self {
            ticker: ticker.to_string(),
            fetched_at: Utc::now().to_rfc3339(),
            ..Self::default()
        }
    }

    fn merge(&mut self, update: Self, overwrite: bool) {
        merge_string(&mut self.currency, update.currency, overwrite);
        merge_string(&mut self.company_name, update.company_name, overwrite);
        merge_number(&mut self.current_price, update.current_price, overwrite);
        merge_number(&mut self.market_cap, update.market_cap, overwrite);
        merge_number(
            &mut self.shares_outstanding,
            update.shares_outstanding,
            overwrite,
        );
        merge_number(&mut self.revenue_ttm, update.revenue_ttm, overwrite);
        merge_number(&mut self.net_income_ttm, update.net_income_ttm, overwrite);
        merge_number(
            &mut self.gross_profit_ttm,
            update.gross_profit_ttm,
            overwrite,
        );
        merge_number(
            &mut self.operating_income_ttm,
            update.operating_income_ttm,
            overwrite,
        );
        merge_number(&mut self.gross_margin, update.gross_margin, overwrite);
        merge_number(
            &mut self.operating_margin,
            update.operating_margin,
            overwrite,
        );
        merge_number(&mut self.net_margin, update.net_margin, overwrite);
        merge_number(&mut self.eps_ttm, update.eps_ttm, overwrite);
        merge_number(&mut self.trailing_pe, update.trailing_pe, overwrite);
        merge_number(
            &mut self.price_to_sales_ttm,
            update.price_to_sales_ttm,
            overwrite,
        );
        merge_number(&mut self.cash, update.cash, overwrite);
        merge_number(&mut self.total_debt, update.total_debt, overwrite);
        merge_string(
            &mut self.fundamental_period_end,
            update.fundamental_period_end,
            overwrite,
        );
        merge_string(
            &mut self.fundamental_source,
            update.fundamental_source,
            overwrite,
        );
        extend_unique(&mut self.data_sources, update.data_sources);
        extend_unique(&mut self.source_notes, update.source_notes);
        extend_unique(&mut self.quality_flags, update.quality_flags);
        self.raw_sec_facts.extend(update.raw_sec_facts);
        self.concept_catalog_entries
            .extend(update.concept_catalog_entries);
        self.canonical_mappings.extend(update.canonical_mappings);
        self.concept_review_decisions
            .extend(update.concept_review_decisions);
        self.observations.extend(update.observations);
    }

    fn compute_derived_metrics(&mut self) {
        if self.market_cap.is_none() {
            self.market_cap = multiply(self.current_price, self.shares_outstanding);
            if self.market_cap.is_some() {
                self.push_quality_flag("market_cap_derived_from_mixed_frequency_price_and_shares");
            }
        }
        if self.gross_margin.is_none() {
            self.gross_margin = ratio(self.gross_profit_ttm, self.revenue_ttm);
        }
        if self.operating_margin.is_none() {
            self.operating_margin = ratio(self.operating_income_ttm, self.revenue_ttm);
        }
        if self.net_margin.is_none() {
            self.net_margin = ratio(self.net_income_ttm, self.revenue_ttm);
        }
        if self.eps_ttm.is_none() {
            self.eps_ttm = ratio(self.net_income_ttm, self.shares_outstanding);
        }
        if self.trailing_pe.is_none() {
            self.trailing_pe = ratio(self.current_price, self.eps_ttm);
            if self.trailing_pe.is_some() {
                self.push_quality_flag(
                    "trailing_pe_uses_market_price_and_latest_filing_period_eps",
                );
            }
        }
        if self.price_to_sales_ttm.is_none() {
            self.price_to_sales_ttm = ratio(self.market_cap, self.revenue_ttm);
            if self.price_to_sales_ttm.is_some() {
                self.push_quality_flag(
                    "price_to_sales_ttm_uses_market_cap_and_latest_filing_period_revenue",
                );
            }
        }
    }

    fn push_quality_flag(&mut self, flag: &str) {
        if !self.quality_flags.iter().any(|existing| existing == flag) {
            self.quality_flags.push(flag.to_string());
        }
    }

    fn mark_gaps(&mut self) {
        let required = [
            ("current_price", "current share price", self.current_price),
            ("market_cap", "market cap", self.market_cap),
            ("shares_outstanding", "share count", self.shares_outstanding),
            ("revenue_ttm", "revenue", self.revenue_ttm),
            ("net_margin", "net margin", self.net_margin),
            ("eps_ttm", "EPS", self.eps_ttm),
        ];
        self.gaps = required
            .iter()
            .filter_map(|(_, label, value)| value.is_none().then(|| (*label).to_string()))
            .collect();
    }
}

async fn fetch_sec_companyfacts_snapshot(
    provider: &SecFactsProvider,
    ticker: &str,
    mapping_strategy: ConceptMappingStrategy,
) -> Result<FinancialSnapshot> {
    let company = provider.lookup_company(ticker).await?;
    let payload = provider.fetch_company_facts(&company).await?;
    let mut snapshot = FinancialSnapshot::new(ticker);
    snapshot.fetched_at = payload.fetched_at.clone();
    let raw_sec_facts = provider.extract_raw_facts(&payload)?;
    let concept_catalog_entries = ConceptCatalog::materialize_catalog_entries(&raw_sec_facts);
    let (canonical_mappings, concept_review_decisions, review_quality_flags) =
        canonical_mappings_for_strategy(
            mapping_strategy,
            ticker,
            &raw_sec_facts,
            &concept_catalog_entries,
            &snapshot.fetched_at,
        )
        .await;
    let observations = ConceptCatalog::build_observations(&raw_sec_facts, &canonical_mappings);
    let bundle = ConceptCatalog::select_latest_baseline_bundle(&raw_sec_facts, &canonical_mappings);
    let shares_fact = ConceptCatalog::latest_value_fact(
        &raw_sec_facts,
        &canonical_mappings,
        "shares_outstanding",
        "shares",
        bundle.as_ref().map(|bundle| bundle.period_end.as_str()),
    );
    let eps_fact = ConceptCatalog::latest_value_fact(
        &raw_sec_facts,
        &canonical_mappings,
        "eps",
        "USD/shares",
        bundle.as_ref().map(|bundle| bundle.period_end.as_str()),
    );
    let cash_fact =
        ConceptCatalog::latest_value_fact(&raw_sec_facts, &canonical_mappings, "cash", "USD", None);
    let debt = ConceptCatalog::total_latest_values(
        &raw_sec_facts,
        &canonical_mappings,
        &["debt_current", "debt_noncurrent"],
        "USD",
    );

    snapshot.company_name = company.company_title;
    snapshot.raw_sec_facts = raw_sec_facts;
    snapshot.concept_catalog_entries = concept_catalog_entries;
    snapshot.canonical_mappings = canonical_mappings;
    snapshot.concept_review_decisions = concept_review_decisions;
    snapshot.observations = observations;
    snapshot.quality_flags.extend(review_quality_flags);
    if let Some(bundle) = bundle {
        ConceptCatalog::apply_income_bundle(&mut snapshot, &bundle);
    } else {
        snapshot.push_quality_flag("sec_income_statement_no_coherent_ttm_or_annual_bundle");
    }
    snapshot.shares_outstanding = shares_fact.as_ref().map(|fact| fact.value);
    snapshot.cash = cash_fact.as_ref().map(|fact| fact.value);
    snapshot.total_debt = debt;
    snapshot.eps_ttm = eps_fact
        .as_ref()
        .and_then(|fact| (fact.end == snapshot.fundamental_period_end).then_some(fact.value))
        .or_else(|| {
            let shares_end = shares_fact.as_ref().and_then(|fact| fact.end.clone());
            (shares_end == snapshot.fundamental_period_end)
                .then(|| ratio(snapshot.net_income_ttm, snapshot.shares_outstanding))
                .flatten()
        });
    if snapshot.eps_ttm.is_none()
        && snapshot.net_income_ttm.is_some()
        && snapshot.shares_outstanding.is_some()
    {
        snapshot.push_quality_flag(
            "eps_ttm_not_derived_because_share_count_period_did_not_match_income_period",
        );
    }
    if shares_fact.as_ref().and_then(|fact| fact.end.as_deref())
        != snapshot.fundamental_period_end.as_deref()
    {
        snapshot.push_quality_flag(
            "shares_outstanding_uses_latest_available_instant_not_income_period",
        );
    }
    snapshot.fundamental_source = Some(provider.provider_name().to_string());
    snapshot
        .data_sources
        .push(provider.provider_name().to_string());
    snapshot.source_notes.push(
        "Fetched fundamentals from SEC Company Facts. Baseline values are selected from aligned income statement periods; stale or mismatched concepts are excluded from derived margins."
            .to_string(),
    );
    Ok(snapshot)
}

fn concept_review_web_search_enabled() -> bool {
    std::env::var("CONCEPT_REVIEW_WEB_SEARCH")
        .ok()
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes"))
}

async fn canonical_mappings_for_strategy(
    mapping_strategy: ConceptMappingStrategy,
    ticker: &str,
    raw_sec_facts: &[SecRawFact],
    catalog_entries: &[ConceptCatalogEntry],
    fetched_at: &str,
) -> (
    Vec<CanonicalMapping>,
    Vec<ConceptReviewDecisionRecord>,
    Vec<String>,
) {
    match mapping_strategy {
        ConceptMappingStrategy::CandidateScoring => (
            ConceptCatalog::seed_canonical_mappings(raw_sec_facts),
            Vec::new(),
            Vec::new(),
        ),
        ConceptMappingStrategy::LlmReviewed => {
            let review_db = match materialize_review_workspace(
                ticker,
                raw_sec_facts,
                catalog_entries,
                fetched_at,
            )
            .await
            {
                Ok(path) => path,
                Err(err) => {
                    return (
                        ConceptCatalog::seed_canonical_mappings(raw_sec_facts),
                        Vec::new(),
                        vec![format!(
                            "llm_concept_review_workspace_bootstrap_failed_used_candidate_scoring_{}",
                            normalize_quality_flag(&err.to_string())
                        )],
                    );
                }
            };
            let service = ConceptReviewService {
                enable_web_search: concept_review_web_search_enabled(),
                enable_workspace_sql: true,
                company_label: Some(ticker.to_string()),
                workspace_sqlite: Some(review_db.clone()),
                ..ConceptReviewService::default()
            };
            let client = OpenRouterModelClient;
            let review_result = service
                .review_workspace(&client, raw_sec_facts, AGENT_REVIEW_PREAMBLE, "")
                .await;
            cleanup_review_workspace(&review_db);
            match review_result {
                Ok((output, _response)) => {
                    let review_run_id = Uuid::new_v4().to_string();
                    let selected_by = format!("llm_agent_review:{}", service.model);
                    let decisions =
                        service.decision_records(&output, &review_run_id, &selected_by, fetched_at);
                    let promoted = service.promote_reviewed_mappings(&output, raw_sec_facts);
                    let mut quality_flags = promoted
                        .warnings
                        .into_iter()
                        .map(|warning| {
                            format!(
                                "llm_concept_review_warning_{}",
                                normalize_quality_flag(&warning)
                            )
                        })
                        .collect::<Vec<_>>();
                    let mappings = if promoted.mappings.is_empty() {
                        quality_flags.push(
                            "llm_concept_review_returned_no_promoted_mappings_used_candidate_scoring"
                                .to_string(),
                        );
                        ConceptCatalog::seed_canonical_mappings(raw_sec_facts)
                    } else {
                        promoted.mappings
                    };
                    (mappings, decisions, quality_flags)
                }
                Err(err) => (
                    ConceptCatalog::seed_canonical_mappings(raw_sec_facts),
                    Vec::new(),
                    vec![format!(
                        "llm_concept_review_failed_used_candidate_scoring_{}",
                        normalize_quality_flag(&err.to_string())
                    )],
                ),
            }
        }
    }
}

async fn persist_financial_snapshot(
    db: &sea_orm::DatabaseConnection,
    snapshot: &FinancialSnapshot,
) -> Result<()> {
    let source_note = snapshot.source_notes.join(" ");
    let fundamentals = snapshot.fundamental_metrics();
    let input = SnapshotPersist {
        fetched_at: &snapshot.fetched_at,
        company_name: snapshot.company_name.as_deref(),
        currency: snapshot.currency.as_deref(),
        source_note: &source_note,
        raw_sec_facts: &snapshot.raw_sec_facts,
        concept_catalog_entries: &snapshot.concept_catalog_entries,
        concept_review_decisions: &snapshot.concept_review_decisions,
        canonical_mappings: &snapshot.canonical_mappings,
        observations: &snapshot.observations,
        quality_flags: &snapshot.quality_flags,
        fundamentals: &fundamentals,
    };
    WorkspaceFinancialStore::new(db).persist_snapshot(&input).await
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

impl FinancialSnapshot {
    fn fundamental_metrics(&self) -> Vec<FundamentalInsert<'_>> {
        let period = self.fundamental_period_end.clone();
        let fundamental_source = self.fundamental_source.clone();
        vec![
            FundamentalInsert {
                key: "current_price",
                label: "Current price",
                value: self.current_price,
                text: None,
                unit: self.currency.as_deref(),
                period: None,
                source_note: Some("Yahoo chart endpoint".to_string()),
            },
            FundamentalInsert {
                key: "market_cap",
                label: "Market cap",
                value: self.market_cap,
                text: None,
                unit: self.currency.as_deref(),
                period: None,
                source_note: Some(
                    "Derived from price and shares when unavailable directly.".to_string(),
                ),
            },
            FundamentalInsert {
                key: "shares_outstanding",
                label: "Shares outstanding",
                value: self.shares_outstanding,
                text: None,
                unit: Some("shares"),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "revenue_ttm",
                label: "Revenue TTM",
                value: self.revenue_ttm,
                text: None,
                unit: self.currency.as_deref(),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "net_income_ttm",
                label: "Net income TTM",
                value: self.net_income_ttm,
                text: None,
                unit: self.currency.as_deref(),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "gross_profit_ttm",
                label: "Gross profit TTM",
                value: self.gross_profit_ttm,
                text: None,
                unit: self.currency.as_deref(),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "operating_income_ttm",
                label: "Operating income TTM",
                value: self.operating_income_ttm,
                text: None,
                unit: self.currency.as_deref(),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "gross_margin",
                label: "Gross margin",
                value: self.gross_margin,
                text: None,
                unit: Some("ratio"),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "operating_margin",
                label: "Operating margin",
                value: self.operating_margin,
                text: None,
                unit: Some("ratio"),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "net_margin",
                label: "Net margin",
                value: self.net_margin,
                text: None,
                unit: Some("ratio"),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "eps_ttm",
                label: "EPS TTM",
                value: self.eps_ttm,
                text: None,
                unit: self.currency.as_deref(),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "trailing_pe",
                label: "Trailing P/E",
                value: self.trailing_pe,
                text: None,
                unit: Some("multiple"),
                period: None,
                source_note: Some(
                    "Derived from current price and EPS when unavailable directly.".to_string(),
                ),
            },
            FundamentalInsert {
                key: "price_to_sales_ttm",
                label: "Price to sales TTM",
                value: self.price_to_sales_ttm,
                text: None,
                unit: Some("multiple"),
                period: None,
                source_note: Some(
                    "Derived from market cap and revenue when unavailable directly.".to_string(),
                ),
            },
            FundamentalInsert {
                key: "cash",
                label: "Cash and equivalents",
                value: self.cash,
                text: None,
                unit: self.currency.as_deref(),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "total_debt",
                label: "Total debt",
                value: self.total_debt,
                text: None,
                unit: self.currency.as_deref(),
                period,
                source_note: fundamental_source,
            },
        ]
        .into_iter()
        .filter(|metric| metric.value.is_some() || metric.text.is_some())
        .collect()
    }
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

fn normalize_quality_flag(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn merge_string(target: &mut Option<String>, update: Option<String>, overwrite: bool) {
    if update.is_some() && (overwrite || target.is_none()) {
        *target = update;
    }
}

fn merge_number(target: &mut Option<f64>, update: Option<f64>, overwrite: bool) {
    if update.is_some() && (overwrite || target.is_none()) {
        *target = update;
    }
}

fn extend_unique(target: &mut Vec<String>, updates: Vec<String>) {
    for update in updates {
        if !target.contains(&update) {
            target.push(update);
        }
    }
}

fn ratio(numerator: Option<f64>, denominator: Option<f64>) -> Option<f64> {
    match (numerator, denominator) {
        (Some(numerator), Some(denominator)) if denominator != 0.0 => Some(numerator / denominator),
        _ => None,
    }
}

fn multiply(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    Some(left? * right?)
}

pub const SCHEMA_STATEMENTS: &[&str] = &[
    "PRAGMA foreign_keys = ON",
    "CREATE TABLE IF NOT EXISTS run_metadata (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        ticker TEXT NOT NULL,
        run_slug TEXT NOT NULL UNIQUE,
        workspace_path TEXT NOT NULL,
        sqlite_path TEXT NOT NULL,
        status TEXT NOT NULL,
        schema_version INTEGER NOT NULL,
        created_at TEXT NOT NULL,
        financial_fetch_status TEXT NOT NULL,
        financial_fetch_error TEXT
    )",
    "CREATE TABLE IF NOT EXISTS stock_info (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        ticker TEXT NOT NULL,
        company_name TEXT,
        exchange TEXT,
        currency TEXT,
        sector TEXT,
        industry TEXT,
        source_note TEXT,
        updated_at TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS sec_raw_facts (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        taxonomy TEXT NOT NULL,
        concept_name TEXT NOT NULL,
        label TEXT,
        description TEXT,
        unit TEXT NOT NULL,
        form TEXT,
        period_start TEXT,
        period_end TEXT,
        filed_at TEXT,
        fiscal_year INTEGER,
        fiscal_period TEXT,
        accession TEXT,
        frame TEXT,
        metric_value REAL NOT NULL,
        raw_json TEXT NOT NULL,
        fetched_at TEXT NOT NULL,
        CHECK (json_valid(raw_json))
    )",
    "CREATE INDEX IF NOT EXISTS idx_sec_raw_facts_concept_period
        ON sec_raw_facts(taxonomy, concept_name, unit, period_end, filed_at)",
    "CREATE INDEX IF NOT EXISTS idx_sec_raw_facts_frame
        ON sec_raw_facts(frame)",
    "CREATE TABLE IF NOT EXISTS canonical_metric_definitions (
        canonical_key TEXT PRIMARY KEY,
        metric_key TEXT NOT NULL,
        metric_label TEXT NOT NULL,
        statement_type TEXT NOT NULL,
        unit_hint TEXT NOT NULL,
        display_order INTEGER NOT NULL,
        created_at TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS canonical_metric_mappings (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        canonical_key TEXT NOT NULL,
        taxonomy TEXT NOT NULL,
        concept_name TEXT NOT NULL,
        unit TEXT NOT NULL,
        confidence TEXT NOT NULL,
        rationale TEXT,
        selected_by TEXT NOT NULL,
        is_active INTEGER NOT NULL DEFAULT 1,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        UNIQUE(canonical_key, taxonomy, concept_name, unit),
        FOREIGN KEY(canonical_key) REFERENCES canonical_metric_definitions(canonical_key)
    )",
    "CREATE INDEX IF NOT EXISTS idx_canonical_metric_mappings_concept
        ON canonical_metric_mappings(taxonomy, concept_name, unit, is_active)",
    "CREATE TABLE IF NOT EXISTS supporting_metric_selections (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        selection_scope TEXT NOT NULL,
        scenario_id INTEGER,
        taxonomy TEXT NOT NULL,
        concept_name TEXT NOT NULL,
        unit TEXT NOT NULL,
        label TEXT,
        rationale TEXT NOT NULL,
        selected_by TEXT NOT NULL,
        created_at TEXT NOT NULL,
        FOREIGN KEY(scenario_id) REFERENCES scenario_assumptions(id)
    )",
    "CREATE VIEW IF NOT EXISTS raw_fact_metric_catalog AS
        SELECT
            taxonomy,
            concept_name,
            label,
            description,
            unit,
            COUNT(*) AS fact_count,
            MIN(period_end) AS earliest_period_end,
            MAX(period_end) AS latest_period_end,
            MAX(filed_at) AS latest_filed_at,
            MIN(metric_value) AS min_value,
            MAX(metric_value) AS max_value
        FROM sec_raw_facts
        GROUP BY taxonomy, concept_name, label, description, unit",
    "CREATE TABLE IF NOT EXISTS concept_catalog_entries (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        taxonomy TEXT NOT NULL,
        concept_name TEXT NOT NULL,
        label TEXT,
        description TEXT,
        unit TEXT NOT NULL,
        fact_count INTEGER NOT NULL,
        earliest_period_end TEXT,
        latest_period_end TEXT,
        latest_filed_at TEXT,
        min_value REAL,
        max_value REAL,
        period_shape_counts TEXT NOT NULL DEFAULT '{}',
        dominant_period_shape TEXT NOT NULL,
        series_usability TEXT NOT NULL,
        plot_readiness TEXT NOT NULL,
        narrative_tags TEXT NOT NULL DEFAULT '[]',
        updated_at TEXT NOT NULL,
        UNIQUE(taxonomy, concept_name, unit),
        CHECK (json_valid(period_shape_counts)),
        CHECK (json_valid(narrative_tags))
    )",
    "CREATE INDEX IF NOT EXISTS idx_concept_catalog_entries_tags
        ON concept_catalog_entries(series_usability, plot_readiness, latest_period_end)",
    "CREATE TABLE IF NOT EXISTS concept_review_decisions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        review_run_id TEXT NOT NULL,
        canonical_key TEXT,
        decision_type TEXT NOT NULL,
        taxonomy TEXT,
        concept_name TEXT,
        unit TEXT,
        confidence TEXT NOT NULL,
        rationale TEXT NOT NULL,
        selected_by TEXT NOT NULL,
        warnings_json TEXT NOT NULL DEFAULT '[]',
        payload_json TEXT NOT NULL DEFAULT '{}',
        created_at TEXT NOT NULL,
        CHECK (json_valid(warnings_json)),
        CHECK (json_valid(payload_json))
    )",
    "CREATE INDEX IF NOT EXISTS idx_concept_review_decisions_key
        ON concept_review_decisions(canonical_key, confidence, decision_type)",
    "CREATE TABLE IF NOT EXISTS fundamentals (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        metric_key TEXT NOT NULL,
        metric_label TEXT,
        metric_value REAL,
        metric_text TEXT,
        unit TEXT,
        period TEXT,
        source_id INTEGER,
        source_note TEXT,
        updated_at TEXT NOT NULL,
        UNIQUE(metric_key, period)
    )",
    "CREATE TABLE IF NOT EXISTS fundamental_observations (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        canonical_key TEXT,
        metric_key TEXT NOT NULL,
        metric_label TEXT NOT NULL,
        statement_type TEXT NOT NULL,
        period_type TEXT NOT NULL,
        period_start TEXT,
        period_end TEXT,
        as_of_date TEXT,
        filed_at TEXT,
        fiscal_year INTEGER,
        fiscal_period TEXT,
        metric_value REAL NOT NULL,
        unit TEXT,
        source_type TEXT NOT NULL,
        source_note TEXT,
        concept_name TEXT,
        form TEXT,
        accession TEXT,
        quality TEXT,
        is_derived INTEGER NOT NULL DEFAULT 0,
        updated_at TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_fundamental_observations_metric_period
        ON fundamental_observations(metric_key, period_end, period_type)",
    "CREATE INDEX IF NOT EXISTS idx_fundamental_observations_as_of
        ON fundamental_observations(as_of_date)",
    "CREATE INDEX IF NOT EXISTS idx_fundamental_observations_canonical
        ON fundamental_observations(canonical_key, period_end)",
    "CREATE VIEW IF NOT EXISTS canonical_fundamental_observations AS
        SELECT *
        FROM fundamental_observations
        WHERE canonical_key IS NOT NULL",
    "CREATE TABLE IF NOT EXISTS data_quality_flags (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        flag_key TEXT NOT NULL,
        severity TEXT NOT NULL DEFAULT 'info',
        description TEXT NOT NULL,
        metric_key TEXT,
        period TEXT,
        created_at TEXT NOT NULL,
        UNIQUE(flag_key, metric_key, period)
    )",
    "CREATE TABLE IF NOT EXISTS data_gaps (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        gap_key TEXT NOT NULL UNIQUE,
        description TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'open',
        created_at TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS sources (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        title TEXT NOT NULL,
        url TEXT,
        source_type TEXT,
        published_at TEXT,
        accessed_at TEXT,
        why_it_matters TEXT,
        notes TEXT
    )",
    "CREATE TABLE IF NOT EXISTS claims (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        claim TEXT NOT NULL,
        source_id INTEGER,
        claim_type TEXT,
        side TEXT,
        confidence TEXT,
        metric TEXT,
        notes TEXT,
        FOREIGN KEY(source_id) REFERENCES sources(id)
    )",
    "CREATE TABLE IF NOT EXISTS sections (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        section_key TEXT NOT NULL UNIQUE,
        section_order INTEGER NOT NULL,
        status TEXT NOT NULL DEFAULT 'pending',
        title TEXT,
        body TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS content_blocks (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        section_key TEXT NOT NULL,
        block_order INTEGER NOT NULL,
        block_type TEXT NOT NULL,
        title TEXT,
        body TEXT,
        source_note TEXT,
        payload TEXT NOT NULL DEFAULT '{}',
        CHECK (json_valid(payload)),
        UNIQUE(section_key, block_order),
        FOREIGN KEY(section_key) REFERENCES sections(section_key)
    )",
    "CREATE TABLE IF NOT EXISTS content_block_metrics (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        content_block_id INTEGER NOT NULL,
        metric_order INTEGER NOT NULL,
        label TEXT NOT NULL,
        value TEXT,
        note TEXT,
        FOREIGN KEY(content_block_id) REFERENCES content_blocks(id)
    )",
    "CREATE TABLE IF NOT EXISTS content_block_items (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        content_block_id INTEGER NOT NULL,
        item_order INTEGER NOT NULL,
        item_type TEXT NOT NULL DEFAULT 'item',
        body TEXT NOT NULL,
        FOREIGN KEY(content_block_id) REFERENCES content_blocks(id)
    )",
    "CREATE TABLE IF NOT EXISTS narrative_map (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        dominant TEXT,
        bull TEXT,
        bear TEXT,
        consensus TEXT,
        counter_narrative TEXT
    )",
    "CREATE TABLE IF NOT EXISTS narrative_map_items (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        item_type TEXT NOT NULL,
        item_order INTEGER NOT NULL,
        body TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS watch_items (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        item_order INTEGER NOT NULL,
        title TEXT NOT NULL,
        description TEXT,
        signal_type TEXT,
        source_id INTEGER,
        FOREIGN KEY(source_id) REFERENCES sources(id)
    )",
    "CREATE TABLE IF NOT EXISTS historical_analogues (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        analogue_order INTEGER NOT NULL,
        analogue TEXT NOT NULL,
        setup TEXT,
        lesson TEXT,
        why_it_can_mislead TEXT,
        source_id INTEGER,
        FOREIGN KEY(source_id) REFERENCES sources(id)
    )",
    "CREATE TABLE IF NOT EXISTS scenario_assumptions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        scenario_order INTEGER NOT NULL UNIQUE,
        name TEXT NOT NULL,
        stance TEXT NOT NULL CHECK (stance IN ('bullish', 'neutral', 'bearish', 'mixed')),
        probability REAL CHECK (probability IS NULL OR probability >= 0),
        description TEXT NOT NULL,
        assumption_summary TEXT,
        UNIQUE(name)
    )",
    "CREATE TABLE IF NOT EXISTS scenario_crux_assumptions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        scenario_id INTEGER NOT NULL,
        crux_order INTEGER NOT NULL,
        crux TEXT NOT NULL,
        assumption TEXT NOT NULL,
        impact TEXT,
        source_id INTEGER,
        UNIQUE(scenario_id, crux_order),
        FOREIGN KEY(scenario_id) REFERENCES scenario_assumptions(id),
        FOREIGN KEY(source_id) REFERENCES sources(id)
    )",
    "CREATE TABLE IF NOT EXISTS scenario_sensitivities (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        scenario_id INTEGER NOT NULL,
        sensitivity_order INTEGER NOT NULL,
        body TEXT NOT NULL,
        UNIQUE(scenario_id, sensitivity_order),
        FOREIGN KEY(scenario_id) REFERENCES scenario_assumptions(id)
    )",
    "CREATE TABLE IF NOT EXISTS scenario_signals (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        scenario_id INTEGER NOT NULL,
        signal_order INTEGER NOT NULL,
        signal_type TEXT NOT NULL CHECK (signal_type IN ('confirming', 'breaking')),
        body TEXT NOT NULL,
        watch_item_id INTEGER,
        source_id INTEGER,
        UNIQUE(scenario_id, signal_type, signal_order),
        FOREIGN KEY(scenario_id) REFERENCES scenario_assumptions(id),
        FOREIGN KEY(watch_item_id) REFERENCES watch_items(id),
        FOREIGN KEY(source_id) REFERENCES sources(id)
    )",
    "CREATE TABLE IF NOT EXISTS scenario_periods (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        scenario_id INTEGER NOT NULL,
        period_order INTEGER NOT NULL,
        label TEXT NOT NULL,
        revenue REAL,
        revenue_growth REAL,
        diluted_shares REAL,
        gross_margin REAL,
        operating_margin REAL,
        net_margin REAL,
        net_income REAL,
        eps REAL,
        ps_low REAL,
        ps_median REAL,
        ps_high REAL,
        pe_low REAL,
        pe_median REAL,
        pe_high REAL,
        blend_ps_weight REAL NOT NULL DEFAULT 0.5,
        blend_pe_weight REAL NOT NULL DEFAULT 0.5,
        source_note TEXT,
        UNIQUE(scenario_id, period_order),
        FOREIGN KEY(scenario_id) REFERENCES scenario_assumptions(id)
    )",
    "CREATE TABLE IF NOT EXISTS monte_carlo_config (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        iterations INTEGER NOT NULL DEFAULT 10000 CHECK (iterations > 0),
        seed INTEGER NOT NULL DEFAULT 42 CHECK (seed >= 0),
        bins INTEGER NOT NULL DEFAULT 30 CHECK (bins > 0)
    )",
    "CREATE TABLE IF NOT EXISTS monte_carlo_summary (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        iterations INTEGER NOT NULL,
        seed INTEGER NOT NULL,
        bins INTEGER NOT NULL,
        price_field TEXT,
        probability_basis TEXT,
        normal_distribution_basis TEXT,
        methodology TEXT,
        summary_min REAL,
        summary_p10 REAL,
        summary_p25 REAL,
        summary_median REAL,
        summary_mean REAL,
        summary_p75 REAL,
        summary_p90 REAL,
        summary_max REAL,
        summary_stdev REAL,
        generated_at TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS monte_carlo_histogram_bins (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        bin_order INTEGER NOT NULL UNIQUE,
        low REAL NOT NULL,
        high REAL NOT NULL,
        midpoint REAL NOT NULL,
        count INTEGER NOT NULL,
        probability REAL NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS monte_carlo_scenario_probabilities (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        scenario_id INTEGER NOT NULL UNIQUE,
        input_probability REAL,
        normalized_probability REAL NOT NULL,
        sample_count INTEGER NOT NULL,
        observed_probability REAL NOT NULL,
        FOREIGN KEY(scenario_id) REFERENCES scenario_assumptions(id)
    )",
    "CREATE TABLE IF NOT EXISTS artifacts (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        artifact_type TEXT NOT NULL,
        path TEXT NOT NULL,
        created_at TEXT NOT NULL,
        notes TEXT
    )",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{
        concept_catalog::{ttm_windows, ConceptCatalog, SecFact},
        sec_facts_provider::extract_raw_facts_from_root,
    };
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

        let bundle = ConceptCatalog::select_latest_baseline_bundle(&raw_facts, &mappings)
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
        let observations = ConceptCatalog::build_observations(&raw_facts, &mappings);

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
        assert!(ConceptCatalog::select_latest_baseline_bundle(&raw_facts, &mappings).is_some());
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
        let observations = ConceptCatalog::build_observations(&raw_facts, &mappings);
        let mut base = FinancialSnapshot::new("TEST");
        let mut update = FinancialSnapshot::new("TEST");
        update.raw_sec_facts = raw_facts.clone();
        update.canonical_mappings = mappings.clone();
        update.observations = observations.clone();

        base.merge(update, true);

        assert_eq!(base.raw_sec_facts.len(), raw_facts.len());
        assert_eq!(base.canonical_mappings.len(), mappings.len());
        assert_eq!(base.observations.len(), observations.len());
        assert!(base
            .raw_sec_facts
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
