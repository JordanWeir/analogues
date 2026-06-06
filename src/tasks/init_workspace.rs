use chrono::{NaiveDate, Utc};
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

const DEFAULT_REPORT_ROOT: &str = "reports/stock-narrative-research";
const RUN_DB_FILENAME: &str = "run.sqlite";
const SCHEMA_VERSION: i64 = 1;
const SEC_USER_AGENT: &str = "stock-agent-2/0.1 research@example.local";
const SEC_TICKERS_URL: &str = "https://www.sec.gov/files/company_tickers.json";
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
}

#[derive(Debug, Clone, Default)]
pub struct FinancialSnapshot {
    pub ticker: String,
    pub fetched_at: String,
    pub data_sources: Vec<String>,
    pub source_notes: Vec<String>,
    pub quality_flags: Vec<String>,
    pub observations: Vec<FundamentalObservation>,
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

#[derive(Debug, Clone)]
pub struct FundamentalObservation {
    pub metric_key: String,
    pub metric_label: String,
    pub statement_type: String,
    pub period_type: String,
    pub period_start: Option<String>,
    pub period_end: Option<String>,
    pub as_of_date: Option<String>,
    pub filed_at: Option<String>,
    pub fiscal_year: Option<i64>,
    pub fiscal_period: Option<String>,
    pub value: f64,
    pub unit: Option<String>,
    pub source_type: String,
    pub source_note: Option<String>,
    pub concept_name: Option<String>,
    pub form: Option<String>,
    pub accession: Option<String>,
    pub quality: Option<String>,
    pub is_derived: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePaths {
    pub run_slug: String,
    pub workspace_dir: PathBuf,
    pub sqlite_path: PathBuf,
    pub generated_dir: PathBuf,
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

        Ok(Self {
            ticker: normalize_ticker(ticker)?,
            date,
            base_dir,
            fetch_financials,
        })
    }
}

pub async fn initialize_workspace(request: &InitWorkspaceRequest) -> Result<WorkspacePaths> {
    fs::create_dir_all(&request.base_dir).map_err(|err| {
        Error::string(&format!(
            "failed to create report root {}: {err}",
            request.base_dir.display()
        ))
    })?;

    let paths = next_workspace_paths(request)?;
    fs::create_dir(&paths.workspace_dir).map_err(|err| {
        Error::string(&format!(
            "failed to create workspace {}: {err}",
            paths.workspace_dir.display()
        ))
    })?;
    fs::create_dir(&paths.generated_dir).map_err(|err| {
        Error::string(&format!(
            "failed to create generated directory {}: {err}",
            paths.generated_dir.display()
        ))
    })?;

    initialize_run_database(request, &paths).await?;
    Ok(paths)
}

fn next_workspace_paths(request: &InitWorkspaceRequest) -> Result<WorkspacePaths> {
    for index in 1..10_000 {
        let run_slug = format!("{}-{}-{}", request.ticker, request.date, index);
        let workspace_dir = request.base_dir.join(&run_slug);
        if !workspace_dir.exists() {
            let sqlite_path = workspace_dir.join(RUN_DB_FILENAME);
            let generated_dir = workspace_dir.join("generated");
            return Ok(WorkspacePaths {
                run_slug,
                workspace_dir,
                sqlite_path,
                generated_dir,
            });
        }
    }

    Err(Error::string(&format!(
        "could not allocate a workspace for {} on {}",
        request.ticker, request.date
    )))
}

async fn initialize_run_database(
    request: &InitWorkspaceRequest,
    paths: &WorkspacePaths,
) -> Result<()> {
    let db = Database::connect(sqlite_uri(&paths.sqlite_path))
        .await
        .map_err(|err| Error::string(&format!("failed to open run SQLite database: {err}")))?;

    let initialization_result = async {
        execute_schema(&db).await?;
        seed_database(&db, request, paths).await?;
        fetch_and_seed_financials(&db, request).await
    }
    .await;
    let close_result = db
        .close()
        .await
        .map_err(|err| Error::string(&format!("failed to close run SQLite database: {err}")));

    initialization_result?;
    close_result?;

    Ok(())
}

async fn execute_schema(db: &sea_orm::DatabaseConnection) -> Result<()> {
    for statement in SCHEMA_STATEMENTS {
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            (*statement).to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("failed to apply run schema: {err}")))?;
    }

    Ok(())
}

async fn seed_database(
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

    match fetch_financial_snapshot(&request.ticker).await {
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
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("Mozilla/5.0")
        .build()
        .map_err(|err| Error::string(&format!("failed to build HTTP client: {err}")))?;
    let mut snapshot = FinancialSnapshot::new(ticker);

    match fetch_yahoo_chart_snapshot(&client, ticker).await {
        Ok(update) => snapshot.merge(update, false),
        Err(err) => snapshot
            .source_notes
            .push(format!("Yahoo chart fallback failed: {err}")),
    }

    match fetch_sec_companyfacts_snapshot(&client, ticker).await {
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
    fn new(ticker: &str) -> Self {
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

async fn fetch_yahoo_chart_snapshot(
    client: &reqwest::Client,
    ticker: &str,
) -> Result<FinancialSnapshot> {
    let url =
        format!("https://query1.finance.yahoo.com/v8/finance/chart/{ticker}?range=1d&interval=1d");
    let payload = fetch_json(client, &url, None).await?;
    let meta = payload
        .pointer("/chart/result/0/meta")
        .ok_or_else(|| Error::string("Yahoo chart response did not include quote metadata"))?;

    let mut snapshot = FinancialSnapshot::new(ticker);
    snapshot.currency = string_at(meta, "currency");
    snapshot.company_name = string_at(meta, "shortName").or_else(|| string_at(meta, "longName"));
    snapshot.current_price =
        number_at(meta, "regularMarketPrice").or_else(|| number_at(meta, "previousClose"));
    if let Some(price) = snapshot.current_price {
        snapshot.observations.push(FundamentalObservation {
            metric_key: "current_price".to_string(),
            metric_label: "Current price".to_string(),
            statement_type: "market".to_string(),
            period_type: "instant".to_string(),
            period_start: None,
            period_end: None,
            as_of_date: Some(snapshot.fetched_at.clone()),
            filed_at: None,
            fiscal_year: None,
            fiscal_period: None,
            value: price,
            unit: snapshot.currency.clone(),
            source_type: "Yahoo chart endpoint".to_string(),
            source_note: Some("Yahoo chart endpoint quote metadata.".to_string()),
            concept_name: None,
            form: None,
            accession: None,
            quality: Some("market_quote".to_string()),
            is_derived: false,
        });
    }
    snapshot
        .data_sources
        .push("Yahoo chart endpoint".to_string());
    snapshot.source_notes.push(
        "Fetched limited price metadata from Yahoo chart endpoint. Fundamental fields require SEC Company Facts or manual input."
            .to_string(),
    );
    Ok(snapshot)
}

async fn fetch_sec_companyfacts_snapshot(
    client: &reqwest::Client,
    ticker: &str,
) -> Result<FinancialSnapshot> {
    let company = lookup_sec_company(client, ticker).await?;
    let cik = company
        .get("cik_str")
        .and_then(Value::as_i64)
        .ok_or_else(|| Error::string("SEC ticker record did not include cik_str"))?;
    let company_name = company
        .get("title")
        .and_then(Value::as_str)
        .map(str::to_string);
    let url = format!("https://data.sec.gov/api/xbrl/companyfacts/CIK{cik:010}.json");
    let payload = fetch_json(client, &url, Some(SEC_USER_AGENT)).await?;
    let us_gaap = payload
        .pointer("/facts/us-gaap")
        .ok_or_else(|| Error::string("SEC Company Facts response did not include us-gaap facts"))?;

    let observations = sec_observations(us_gaap);
    let bundle = select_latest_income_bundle(us_gaap);
    let shares_fact = latest_value_fact(
        us_gaap,
        &[
            "WeightedAverageNumberOfDilutedSharesOutstanding",
            "CommonStockSharesOutstanding",
        ],
        "shares",
        bundle.as_ref().map(|bundle| bundle.period_end.as_str()),
    );
    let eps_fact = latest_value_fact(
        us_gaap,
        &["EarningsPerShareDiluted"],
        "USD/shares",
        bundle.as_ref().map(|bundle| bundle.period_end.as_str()),
    );
    let cash_fact = latest_value_fact(
        us_gaap,
        &[
            "CashAndCashEquivalentsAtCarryingValue",
            "CashCashEquivalentsRestrictedCashAndRestrictedCashEquivalents",
        ],
        "USD",
        None,
    );
    let debt = total_latest_values(
        us_gaap,
        &[
            &[
                "DebtCurrent",
                "LongTermDebtAndFinanceLeaseObligationsCurrent",
            ][..],
            &[
                "LongTermDebtAndFinanceLeaseObligationsNoncurrent",
                "LongTermDebtAndCapitalLeaseObligations",
            ][..],
        ],
        "USD",
    );

    let mut snapshot = FinancialSnapshot::new(ticker);
    snapshot.company_name = company_name;
    snapshot.observations = observations;
    if let Some(bundle) = bundle {
        append_bundle_observations(&mut snapshot, &bundle);
        snapshot.revenue_ttm = bundle.revenue.as_ref().map(|metric| metric.value);
        snapshot.net_income_ttm = bundle.net_income.as_ref().map(|metric| metric.value);
        snapshot.gross_profit_ttm = bundle.gross_profit.as_ref().map(|metric| metric.value);
        snapshot.operating_income_ttm = bundle.operating_income.as_ref().map(|metric| metric.value);
        snapshot.gross_margin = ratio(snapshot.gross_profit_ttm, snapshot.revenue_ttm);
        snapshot.operating_margin = ratio(snapshot.operating_income_ttm, snapshot.revenue_ttm);
        snapshot.net_margin = ratio(snapshot.net_income_ttm, snapshot.revenue_ttm);
        snapshot.fundamental_period_end = Some(bundle.period_end.clone());
        snapshot.source_notes.extend(bundle.source_notes);
        snapshot.quality_flags.extend(bundle.quality_flags);
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
    snapshot.fundamental_source = Some("SEC Company Facts".to_string());
    snapshot.data_sources.push("SEC Company Facts".to_string());
    snapshot.source_notes.push(
        "Fetched fundamentals from SEC Company Facts. Baseline values are selected from aligned income statement periods; stale or mismatched concepts are excluded from derived margins."
            .to_string(),
    );
    Ok(snapshot)
}

async fn lookup_sec_company(client: &reqwest::Client, ticker: &str) -> Result<Value> {
    let payload = fetch_json(client, SEC_TICKERS_URL, Some(SEC_USER_AGENT)).await?;
    let ticker_upper = ticker.to_uppercase();
    payload
        .as_object()
        .and_then(|companies| {
            companies.values().find(|company| {
                company
                    .get("ticker")
                    .and_then(Value::as_str)
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(&ticker_upper))
            })
        })
        .cloned()
        .ok_or_else(|| Error::string(&format!("Ticker {ticker} was not found in SEC tickers")))
}

async fn fetch_json(
    client: &reqwest::Client,
    url: &str,
    user_agent: Option<&str>,
) -> Result<Value> {
    let mut request = client.get(url);
    if let Some(user_agent) = user_agent {
        request = request.header(reqwest::header::USER_AGENT, user_agent);
    }

    let response = request
        .send()
        .await
        .map_err(|err| Error::string(&format!("request failed for {url}: {err}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(Error::string(&format!(
            "request failed for {url}: {status}"
        )));
    }
    response
        .json::<Value>()
        .await
        .map_err(|err| Error::string(&format!("invalid JSON response from {url}: {err}")))
}

async fn persist_financial_snapshot(
    db: &sea_orm::DatabaseConnection,
    snapshot: &FinancialSnapshot,
) -> Result<()> {
    let source_note = snapshot.source_notes.join(" ");
    execute_sql(
        db,
        &format!(
            "UPDATE stock_info
             SET company_name = {}, currency = {}, source_note = {}, updated_at = '{}'
             WHERE id = 1",
            sql_value(snapshot.company_name.as_deref()),
            sql_value(snapshot.currency.as_deref()),
            sql_value(Some(&source_note)),
            sql_quote(&snapshot.fetched_at),
        ),
    )
    .await?;

    for observation in &snapshot.observations {
        insert_observation(db, observation, &snapshot.fetched_at).await?;
    }
    for flag in &snapshot.quality_flags {
        insert_data_quality_flag(db, flag, &snapshot.fetched_at).await?;
    }
    for metric in snapshot.fundamental_metrics() {
        insert_fundamental(db, &metric, &snapshot.fetched_at).await?;
    }

    Ok(())
}

async fn insert_observation(
    db: &sea_orm::DatabaseConnection,
    observation: &FundamentalObservation,
    updated_at: &str,
) -> Result<()> {
    execute_sql(
        db,
        &format!(
            "INSERT INTO fundamental_observations (
                metric_key, metric_label, statement_type, period_type, period_start, period_end,
                as_of_date, filed_at, fiscal_year, fiscal_period, metric_value, unit,
                source_type, source_note, concept_name, form, accession, quality, is_derived,
                updated_at
            ) VALUES (
                '{}', '{}', '{}', '{}', {}, {}, {}, {}, {}, {}, {}, {}, '{}', {}, {}, {}, {}, {}, {}, '{}'
            )",
            sql_quote(&observation.metric_key),
            sql_quote(&observation.metric_label),
            sql_quote(&observation.statement_type),
            sql_quote(&observation.period_type),
            sql_value(observation.period_start.as_deref()),
            sql_value(observation.period_end.as_deref()),
            sql_value(observation.as_of_date.as_deref()),
            sql_value(observation.filed_at.as_deref()),
            sql_i64(observation.fiscal_year),
            sql_value(observation.fiscal_period.as_deref()),
            observation.value,
            sql_value(observation.unit.as_deref()),
            sql_quote(&observation.source_type),
            sql_value(observation.source_note.as_deref()),
            sql_value(observation.concept_name.as_deref()),
            sql_value(observation.form.as_deref()),
            sql_value(observation.accession.as_deref()),
            sql_value(observation.quality.as_deref()),
            if observation.is_derived { 1 } else { 0 },
            sql_quote(updated_at),
        ),
    )
    .await
}

async fn insert_data_quality_flag(
    db: &sea_orm::DatabaseConnection,
    flag: &str,
    created_at: &str,
) -> Result<()> {
    execute_sql(
        db,
        &format!(
            "INSERT INTO data_quality_flags (flag_key, severity, description, created_at)
             VALUES ('{}', 'info', '{}', '{}')
             ON CONFLICT(flag_key, metric_key, period) DO UPDATE SET
                severity = excluded.severity,
                description = excluded.description,
                created_at = excluded.created_at",
            sql_quote(flag),
            sql_quote(&flag.replace('_', " ")),
            sql_quote(created_at),
        ),
    )
    .await
}

async fn insert_fundamental(
    db: &sea_orm::DatabaseConnection,
    metric: &FinancialMetric<'_>,
    updated_at: &str,
) -> Result<()> {
    execute_sql(
        db,
        &format!(
            "INSERT INTO fundamentals (
                metric_key, metric_label, metric_value, metric_text, unit, period, source_note, updated_at
            ) VALUES (
                '{}', '{}', {}, {}, {}, {}, {}, '{}'
            )
            ON CONFLICT(metric_key, period) DO UPDATE SET
                metric_label = excluded.metric_label,
                metric_value = excluded.metric_value,
                metric_text = excluded.metric_text,
                unit = excluded.unit,
                source_note = excluded.source_note,
                updated_at = excluded.updated_at",
            sql_quote(metric.key),
            sql_quote(metric.label),
            sql_number(metric.value),
            sql_value(metric.text.as_deref()),
            sql_value(metric.unit),
            sql_value(metric.period.as_deref()),
            sql_value(metric.source_note.as_deref()),
            sql_quote(updated_at),
        ),
    )
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

struct FinancialMetric<'a> {
    key: &'a str,
    label: &'a str,
    value: Option<f64>,
    text: Option<String>,
    unit: Option<&'a str>,
    period: Option<String>,
    source_note: Option<String>,
}

impl FinancialSnapshot {
    fn fundamental_metrics(&self) -> Vec<FinancialMetric<'_>> {
        let period = self.fundamental_period_end.clone();
        let fundamental_source = self.fundamental_source.clone();
        vec![
            FinancialMetric {
                key: "current_price",
                label: "Current price",
                value: self.current_price,
                text: None,
                unit: self.currency.as_deref(),
                period: None,
                source_note: Some("Yahoo chart endpoint".to_string()),
            },
            FinancialMetric {
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
            FinancialMetric {
                key: "shares_outstanding",
                label: "Shares outstanding",
                value: self.shares_outstanding,
                text: None,
                unit: Some("shares"),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FinancialMetric {
                key: "revenue_ttm",
                label: "Revenue TTM",
                value: self.revenue_ttm,
                text: None,
                unit: self.currency.as_deref(),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FinancialMetric {
                key: "net_income_ttm",
                label: "Net income TTM",
                value: self.net_income_ttm,
                text: None,
                unit: self.currency.as_deref(),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FinancialMetric {
                key: "gross_profit_ttm",
                label: "Gross profit TTM",
                value: self.gross_profit_ttm,
                text: None,
                unit: self.currency.as_deref(),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FinancialMetric {
                key: "operating_income_ttm",
                label: "Operating income TTM",
                value: self.operating_income_ttm,
                text: None,
                unit: self.currency.as_deref(),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FinancialMetric {
                key: "gross_margin",
                label: "Gross margin",
                value: self.gross_margin,
                text: None,
                unit: Some("ratio"),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FinancialMetric {
                key: "operating_margin",
                label: "Operating margin",
                value: self.operating_margin,
                text: None,
                unit: Some("ratio"),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FinancialMetric {
                key: "net_margin",
                label: "Net margin",
                value: self.net_margin,
                text: None,
                unit: Some("ratio"),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FinancialMetric {
                key: "eps_ttm",
                label: "EPS TTM",
                value: self.eps_ttm,
                text: None,
                unit: self.currency.as_deref(),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FinancialMetric {
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
            FinancialMetric {
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
            FinancialMetric {
                key: "cash",
                label: "Cash and equivalents",
                value: self.cash,
                text: None,
                unit: self.currency.as_deref(),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FinancialMetric {
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

#[derive(Debug, Clone)]
struct SecFact {
    concept: String,
    unit: String,
    form: Option<String>,
    start: Option<String>,
    end: Option<String>,
    filed: Option<String>,
    fiscal_year: Option<i64>,
    fiscal_period: Option<String>,
    accession: Option<String>,
    value: f64,
}

#[derive(Debug, Clone, Copy)]
struct ConceptSpec {
    metric_key: &'static str,
    metric_label: &'static str,
    statement_type: &'static str,
    concepts: &'static [&'static str],
    unit_hint: &'static str,
}

#[derive(Debug, Clone)]
struct TtmMetric {
    metric_key: &'static str,
    value: f64,
    period_start: Option<String>,
    period_end: String,
    source_note: String,
    quality_flags: Vec<String>,
}

#[derive(Debug, Clone)]
struct IncomeBundle {
    period_end: String,
    revenue: Option<TtmMetric>,
    net_income: Option<TtmMetric>,
    gross_profit: Option<TtmMetric>,
    operating_income: Option<TtmMetric>,
    source_notes: Vec<String>,
    quality_flags: Vec<String>,
}

const REVENUE_CONCEPTS: &[&str] = &[
    "RevenueFromContractWithCustomerExcludingAssessedTax",
    "Revenues",
    "SalesRevenueNet",
];
const NET_INCOME_CONCEPTS: &[&str] = &["NetIncomeLoss"];
const GROSS_PROFIT_CONCEPTS: &[&str] = &["GrossProfit"];
const OPERATING_INCOME_CONCEPTS: &[&str] = &["OperatingIncomeLoss"];
const DILUTED_SHARES_CONCEPTS: &[&str] = &[
    "WeightedAverageNumberOfDilutedSharesOutstanding",
    "CommonStockSharesOutstanding",
];
const EPS_CONCEPTS: &[&str] = &["EarningsPerShareDiluted"];
const CASH_CONCEPTS: &[&str] = &[
    "CashAndCashEquivalentsAtCarryingValue",
    "CashCashEquivalentsRestrictedCashAndRestrictedCashEquivalents",
];
const DEBT_CURRENT_CONCEPTS: &[&str] = &[
    "DebtCurrent",
    "LongTermDebtAndFinanceLeaseObligationsCurrent",
];
const DEBT_NONCURRENT_CONCEPTS: &[&str] = &[
    "LongTermDebtAndFinanceLeaseObligationsNoncurrent",
    "LongTermDebtAndCapitalLeaseObligations",
];
const SEC_CONCEPT_SPECS: &[ConceptSpec] = &[
    ConceptSpec {
        metric_key: "revenue",
        metric_label: "Revenue",
        statement_type: "income_statement",
        concepts: REVENUE_CONCEPTS,
        unit_hint: "USD",
    },
    ConceptSpec {
        metric_key: "net_income",
        metric_label: "Net income",
        statement_type: "income_statement",
        concepts: NET_INCOME_CONCEPTS,
        unit_hint: "USD",
    },
    ConceptSpec {
        metric_key: "gross_profit",
        metric_label: "Gross profit",
        statement_type: "income_statement",
        concepts: GROSS_PROFIT_CONCEPTS,
        unit_hint: "USD",
    },
    ConceptSpec {
        metric_key: "operating_income",
        metric_label: "Operating income",
        statement_type: "income_statement",
        concepts: OPERATING_INCOME_CONCEPTS,
        unit_hint: "USD",
    },
    ConceptSpec {
        metric_key: "diluted_shares",
        metric_label: "Diluted shares",
        statement_type: "income_statement",
        concepts: DILUTED_SHARES_CONCEPTS,
        unit_hint: "shares",
    },
    ConceptSpec {
        metric_key: "eps",
        metric_label: "Diluted EPS",
        statement_type: "income_statement",
        concepts: EPS_CONCEPTS,
        unit_hint: "USD/shares",
    },
    ConceptSpec {
        metric_key: "cash",
        metric_label: "Cash and equivalents",
        statement_type: "balance_sheet",
        concepts: CASH_CONCEPTS,
        unit_hint: "USD",
    },
    ConceptSpec {
        metric_key: "debt_current",
        metric_label: "Current debt",
        statement_type: "balance_sheet",
        concepts: DEBT_CURRENT_CONCEPTS,
        unit_hint: "USD",
    },
    ConceptSpec {
        metric_key: "debt_noncurrent",
        metric_label: "Noncurrent debt",
        statement_type: "balance_sheet",
        concepts: DEBT_NONCURRENT_CONCEPTS,
        unit_hint: "USD",
    },
];

fn sec_observations(us_gaap: &Value) -> Vec<FundamentalObservation> {
    SEC_CONCEPT_SPECS
        .iter()
        .flat_map(|spec| {
            statement_facts(us_gaap, spec.concepts)
                .into_iter()
                .filter(|fact| unit_matches(&fact.unit, spec.unit_hint))
                .map(move |fact| sec_observation(spec, &fact))
        })
        .collect()
}

fn sec_observation(spec: &ConceptSpec, fact: &SecFact) -> FundamentalObservation {
    FundamentalObservation {
        metric_key: spec.metric_key.to_string(),
        metric_label: spec.metric_label.to_string(),
        statement_type: spec.statement_type.to_string(),
        period_type: fact_period_type(fact).to_string(),
        period_start: fact.start.clone(),
        period_end: fact.end.clone(),
        as_of_date: fact.end.clone(),
        filed_at: fact.filed.clone(),
        fiscal_year: fact.fiscal_year,
        fiscal_period: fact.fiscal_period.clone(),
        value: fact.value,
        unit: Some(fact.unit.clone()),
        source_type: "SEC Company Facts".to_string(),
        source_note: Some(format!(
            "{} from SEC concept {} filed {}.",
            spec.metric_label,
            fact.concept,
            fact.filed
                .clone()
                .unwrap_or_else(|| "unknown date".to_string())
        )),
        concept_name: Some(fact.concept.clone()),
        form: fact.form.clone(),
        accession: fact.accession.clone(),
        quality: None,
        is_derived: false,
    }
}

fn select_latest_income_bundle(us_gaap: &Value) -> Option<IncomeBundle> {
    let revenue = ttm_series_for_metric("revenue_ttm", us_gaap, REVENUE_CONCEPTS, "USD");
    let net_income = ttm_series_for_metric("net_income_ttm", us_gaap, NET_INCOME_CONCEPTS, "USD");
    let gross_profit =
        ttm_series_for_metric("gross_profit_ttm", us_gaap, GROSS_PROFIT_CONCEPTS, "USD");
    let operating_income = ttm_series_for_metric(
        "operating_income_ttm",
        us_gaap,
        OPERATING_INCOME_CONCEPTS,
        "USD",
    );

    let mut period_ends: Vec<String> = revenue
        .iter()
        .map(|metric| metric.period_end.clone())
        .collect();
    period_ends.sort();
    period_ends.dedup();
    period_ends.reverse();

    for period_end in period_ends {
        let revenue_metric = metric_for_period(&revenue, &period_end);
        let net_income_metric = metric_for_period(&net_income, &period_end);
        if revenue_metric.is_none() || net_income_metric.is_none() {
            continue;
        }
        let gross_profit_metric = metric_for_period(&gross_profit, &period_end);
        let operating_income_metric = metric_for_period(&operating_income, &period_end);
        let mut source_notes = Vec::new();
        let mut quality_flags = Vec::new();
        for metric in [
            revenue_metric.as_ref(),
            net_income_metric.as_ref(),
            gross_profit_metric.as_ref(),
            operating_income_metric.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            extend_unique(&mut source_notes, vec![metric.source_note.clone()]);
            extend_unique(&mut quality_flags, metric.quality_flags.clone());
        }
        for (metric_key, candidates) in [
            ("gross_profit_ttm", &gross_profit),
            ("operating_income_ttm", &operating_income),
        ] {
            if !candidates.is_empty() && metric_for_period(candidates, &period_end).is_none() {
                quality_flags.push(format!(
                    "{metric_key}_excluded_because_no_fact_matched_baseline_period_{period_end}"
                ));
            }
        }
        return Some(IncomeBundle {
            period_end,
            revenue: revenue_metric,
            net_income: net_income_metric,
            gross_profit: gross_profit_metric,
            operating_income: operating_income_metric,
            source_notes,
            quality_flags,
        });
    }

    None
}

fn metric_for_period(metrics: &[TtmMetric], period_end: &str) -> Option<TtmMetric> {
    metrics
        .iter()
        .find(|metric| metric.period_end == period_end)
        .cloned()
}

fn append_bundle_observations(snapshot: &mut FinancialSnapshot, bundle: &IncomeBundle) {
    for metric in [
        bundle.revenue.as_ref(),
        bundle.net_income.as_ref(),
        bundle.gross_profit.as_ref(),
        bundle.operating_income.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        let label = ttm_label(metric.metric_key);
        snapshot.observations.push(FundamentalObservation {
            metric_key: metric.metric_key.to_string(),
            metric_label: label.to_string(),
            statement_type: "income_statement".to_string(),
            period_type: "ttm".to_string(),
            period_start: metric.period_start.clone(),
            period_end: Some(metric.period_end.clone()),
            as_of_date: Some(metric.period_end.clone()),
            filed_at: None,
            fiscal_year: None,
            fiscal_period: None,
            value: metric.value,
            unit: snapshot.currency.clone(),
            source_type: "SEC Company Facts".to_string(),
            source_note: Some(metric.source_note.clone()),
            concept_name: None,
            form: None,
            accession: None,
            quality: Some(if metric.quality_flags.is_empty() {
                "aligned".to_string()
            } else {
                metric.quality_flags.join(",")
            }),
            is_derived: true,
        });
    }

    for (metric_key, label, numerator) in [
        ("gross_margin", "Gross margin", bundle.gross_profit.as_ref()),
        (
            "operating_margin",
            "Operating margin",
            bundle.operating_income.as_ref(),
        ),
        ("net_margin", "Net margin", bundle.net_income.as_ref()),
    ] {
        let Some(revenue) = &bundle.revenue else {
            continue;
        };
        let Some(numerator) = numerator else {
            continue;
        };
        let Some(value) = ratio(Some(numerator.value), Some(revenue.value)) else {
            continue;
        };
        snapshot.observations.push(FundamentalObservation {
            metric_key: metric_key.to_string(),
            metric_label: label.to_string(),
            statement_type: "income_statement".to_string(),
            period_type: "ttm".to_string(),
            period_start: revenue.period_start.clone(),
            period_end: Some(bundle.period_end.clone()),
            as_of_date: Some(bundle.period_end.clone()),
            filed_at: None,
            fiscal_year: None,
            fiscal_period: None,
            value,
            unit: Some("ratio".to_string()),
            source_type: "derived".to_string(),
            source_note: Some(format!(
                "{label} derived only from observations aligned to {}.",
                bundle.period_end
            )),
            concept_name: None,
            form: None,
            accession: None,
            quality: Some("aligned".to_string()),
            is_derived: true,
        });
    }
}

fn ttm_label(metric_key: &str) -> &'static str {
    match metric_key {
        "revenue_ttm" => "Revenue TTM",
        "net_income_ttm" => "Net income TTM",
        "gross_profit_ttm" => "Gross profit TTM",
        "operating_income_ttm" => "Operating income TTM",
        _ => "TTM metric",
    }
}

fn ttm_series_for_metric(
    metric_key: &'static str,
    us_gaap: &Value,
    concepts: &[&str],
    unit_hint: &str,
) -> Vec<TtmMetric> {
    let facts = statement_facts(us_gaap, concepts)
        .into_iter()
        .filter(|fact| unit_matches(&fact.unit, unit_hint))
        .collect::<Vec<_>>();
    let mut metrics = ttm_windows(metric_key, &facts);
    if metrics.is_empty() {
        if let Some(annual) = latest_duration_fact(&facts, &["10-K", "10-K/A"], 250, 380, None) {
            if let Some(period_end) = annual.end.clone() {
                metrics.push(TtmMetric {
                    metric_key,
                    value: annual.value,
                    period_start: annual.start.clone(),
                    period_end: period_end.clone(),
                    source_note: format!(
                        "{} used latest annual value through {period_end} because a contiguous TTM bridge was unavailable.",
                        annual.concept
                    ),
                    quality_flags: vec![format!("{metric_key}_annual_fallback_used")],
                });
            }
        }
    }
    metrics.sort_by(|left, right| right.period_end.cmp(&left.period_end));
    metrics
}

fn ttm_windows(metric_key: &'static str, facts: &[SecFact]) -> Vec<TtmMetric> {
    let quarters = latest_quarter_facts(facts);
    let mut windows = Vec::new();
    for window in quarters.windows(4) {
        if !is_contiguous_ttm_window(window) {
            continue;
        }
        let Some(latest) = window.first() else {
            continue;
        };
        let Some(earliest) = window.last() else {
            continue;
        };
        let Some(period_end) = latest.end.clone() else {
            continue;
        };
        let value = window.iter().map(|fact| fact.value).sum();
        windows.push(TtmMetric {
            metric_key,
            value,
            period_start: earliest.start.clone(),
            period_end: period_end.clone(),
            source_note: format!(
                "{} TTM summed from four contiguous quarterly facts through {period_end}.",
                latest.concept
            ),
            quality_flags: Vec::new(),
        });
    }
    windows
}

fn is_contiguous_ttm_window(facts: &[SecFact]) -> bool {
    if facts.len() != 4 {
        return false;
    }
    let Some(start) = facts.last().and_then(|fact| fact.start.as_deref()) else {
        return false;
    };
    let Some(end) = facts.first().and_then(|fact| fact.end.as_deref()) else {
        return false;
    };
    let Some(span_days) = days_between(start, end) else {
        return false;
    };
    (300..=390).contains(&span_days)
}

fn days_between(start: &str, end: &str) -> Option<i64> {
    let start = NaiveDate::parse_from_str(start, "%Y-%m-%d").ok()?;
    let end = NaiveDate::parse_from_str(end, "%Y-%m-%d").ok()?;
    Some((end - start).num_days())
}

fn statement_facts(us_gaap: &Value, concepts: &[&str]) -> Vec<SecFact> {
    concepts
        .iter()
        .filter_map(|concept| us_gaap.get(*concept).map(|payload| (*concept, payload)))
        .flat_map(|(concept, payload)| {
            payload
                .get("units")
                .and_then(Value::as_object)
                .into_iter()
                .flat_map(move |units| {
                    units.iter().flat_map(move |(unit, values)| {
                        values
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(move |value| sec_fact(concept, unit, value))
                    })
                })
        })
        .collect()
}

fn sec_fact(concept: &str, unit: &str, value: &Value) -> Option<SecFact> {
    Some(SecFact {
        concept: concept.to_string(),
        unit: unit.to_string(),
        form: string_at(value, "form"),
        start: string_at(value, "start"),
        end: string_at(value, "end"),
        filed: string_at(value, "filed"),
        fiscal_year: value.get("fy").and_then(Value::as_i64),
        fiscal_period: string_at(value, "fp"),
        accession: string_at(value, "accn"),
        value: number_at(value, "val")?,
    })
}

fn latest_value(us_gaap: &Value, concepts: &[&str], unit_hint: &str) -> Option<f64> {
    latest_value_fact(us_gaap, concepts, unit_hint, None).map(|fact| fact.value)
}

fn latest_value_fact(
    us_gaap: &Value,
    concepts: &[&str],
    unit_hint: &str,
    prefer_period_end: Option<&str>,
) -> Option<SecFact> {
    concepts
        .iter()
        .filter_map(|concept| us_gaap.get(*concept).map(|payload| (*concept, payload)))
        .flat_map(|(concept, payload)| {
            payload
                .get("units")
                .and_then(Value::as_object)
                .into_iter()
                .flat_map(move |units| {
                    units
                        .iter()
                        .filter(move |(unit, _)| unit_matches(unit, unit_hint))
                        .flat_map(move |(unit, values)| {
                            values
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(move |value| sec_fact(concept, unit, value))
                        })
                })
        })
        .filter(|fact| {
            prefer_period_end.is_none_or(|period_end| {
                fact.end.as_deref() <= Some(period_end)
                    || fact
                        .start
                        .as_deref()
                        .is_some_and(|start| start <= period_end)
            })
        })
        .max_by(|left, right| {
            (
                left.end.as_deref().unwrap_or(""),
                left.filed.as_deref().unwrap_or(""),
            )
                .cmp(&(
                    right.end.as_deref().unwrap_or(""),
                    right.filed.as_deref().unwrap_or(""),
                ))
        })
}

fn total_latest_values(
    us_gaap: &Value,
    concept_groups: &[&[&str]],
    unit_hint: &str,
) -> Option<f64> {
    let values: Vec<f64> = concept_groups
        .iter()
        .filter_map(|concepts| latest_value(us_gaap, concepts, unit_hint))
        .collect();
    (!values.is_empty()).then(|| values.iter().sum())
}

fn latest_duration_fact(
    facts: &[SecFact],
    forms: &[&str],
    min_days: i64,
    max_days: i64,
    end_after: Option<&str>,
) -> Option<SecFact> {
    facts
        .iter()
        .filter(|fact| {
            fact.form
                .as_deref()
                .is_some_and(|form| forms.contains(&form))
                && duration_days(fact).is_some_and(|days| min_days <= days && days <= max_days)
                && end_after.is_none_or(|end_after| fact.end.as_deref().unwrap_or("") > end_after)
        })
        .max_by(|left, right| {
            (
                left.end.as_deref().unwrap_or(""),
                left.filed.as_deref().unwrap_or(""),
            )
                .cmp(&(
                    right.end.as_deref().unwrap_or(""),
                    right.filed.as_deref().unwrap_or(""),
                ))
        })
        .cloned()
}

fn latest_quarter_facts(facts: &[SecFact]) -> Vec<SecFact> {
    let mut by_end: BTreeMap<String, SecFact> = BTreeMap::new();
    for fact in facts.iter().filter(|fact| {
        fact.form
            .as_deref()
            .is_some_and(|form| matches!(form, "10-Q" | "10-K"))
            && duration_days(fact).is_some_and(|days| (60..=120).contains(&days))
    }) {
        if let Some(end) = &fact.end {
            by_end
                .entry(end.clone())
                .and_modify(|existing| {
                    if fact.filed > existing.filed {
                        *existing = fact.clone();
                    }
                })
                .or_insert_with(|| fact.clone());
        }
    }

    let mut facts: Vec<SecFact> = by_end.into_values().collect();
    facts.sort_by(|left, right| {
        (
            right.end.as_deref().unwrap_or(""),
            right.filed.as_deref().unwrap_or(""),
        )
            .cmp(&(
                left.end.as_deref().unwrap_or(""),
                left.filed.as_deref().unwrap_or(""),
            ))
    });
    facts
}

fn unit_matches(unit: &str, unit_hint: &str) -> bool {
    unit.to_lowercase().contains(&unit_hint.to_lowercase())
}

fn fact_period_type(fact: &SecFact) -> &'static str {
    if duration_days(fact).is_some_and(|days| (60..=120).contains(&days)) {
        "quarter"
    } else if duration_days(fact).is_some_and(|days| (250..=380).contains(&days)) {
        "annual"
    } else {
        "instant"
    }
}

fn duration_days(fact: &SecFact) -> Option<i64> {
    let start = NaiveDate::parse_from_str(fact.start.as_deref()?, "%Y-%m-%d").ok()?;
    let end = NaiveDate::parse_from_str(fact.end.as_deref()?, "%Y-%m-%d").ok()?;
    Some((end - start).num_days())
}

async fn execute_sql(db: &sea_orm::DatabaseConnection, sql: &str) -> Result<()> {
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        sql.to_string(),
    ))
    .await
    .map_err(|err| Error::string(&format!("failed to execute SQL statement: {err}")))?;

    Ok(())
}

fn normalize_ticker(raw: &str) -> Result<String> {
    let ticker = raw.trim().to_uppercase();
    if ticker.is_empty() {
        return Err(Error::string("ticker cannot be empty"));
    }

    let valid = ticker
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'));

    if !valid {
        return Err(Error::string(
            "ticker can only contain ASCII letters, numbers, dots, and hyphens",
        ));
    }

    Ok(ticker)
}

fn validate_date(date: &str) -> Result<()> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| Error::string("date must use YYYY-MM-DD format, for example date:2026-06-04"))
}

fn sqlite_uri(path: &Path) -> String {
    let normalized_path = path.to_string_lossy().replace('\\', "/");
    format!("sqlite://{normalized_path}?mode=rwc")
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

fn sql_number(value: Option<f64>) -> String {
    value.map_or_else(|| "NULL".to_string(), |value| value.to_string())
}

fn sql_i64(value: Option<i64>) -> String {
    value.map_or_else(|| "NULL".to_string(), |value| value.to_string())
}

fn string_at(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn number_at(value: &Value, key: &str) -> Option<f64> {
    match value.get(key)? {
        Value::Number(number) => number.as_f64(),
        _ => None,
    }
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

const SCHEMA_STATEMENTS: &[&str] = &[
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
    use serde_json::json;

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
        let us_gaap = json!({
            "RevenueFromContractWithCustomerExcludingAssessedTax": {
                "units": { "USD": [
                    sec_fact_json("10-Q", "2026-01-01", "2026-03-31", "2026-04-30", 10.0),
                    sec_fact_json("10-Q", "2025-10-01", "2025-12-31", "2026-02-15", 20.0),
                    sec_fact_json("10-Q", "2025-07-01", "2025-09-30", "2025-10-30", 30.0),
                    sec_fact_json("10-Q", "2025-04-01", "2025-06-30", "2025-07-30", 40.0)
                ]}
            },
            "NetIncomeLoss": {
                "units": { "USD": [
                    sec_fact_json("10-Q", "2026-01-01", "2026-03-31", "2026-04-30", 1.0),
                    sec_fact_json("10-Q", "2025-10-01", "2025-12-31", "2026-02-15", 2.0),
                    sec_fact_json("10-Q", "2025-07-01", "2025-09-30", "2025-10-30", 3.0),
                    sec_fact_json("10-Q", "2025-04-01", "2025-06-30", "2025-07-30", 4.0)
                ]}
            },
            "GrossProfit": {
                "units": { "USD": [
                    sec_fact_json("10-K", "2016-01-01", "2016-12-31", "2017-02-15", 50.0)
                ]}
            }
        });

        let bundle = select_latest_income_bundle(&us_gaap).expect("coherent revenue/net income");

        assert_eq!(bundle.period_end, "2026-03-31");
        assert!(bundle.gross_profit.is_none());
        assert!(bundle
            .quality_flags
            .iter()
            .any(|flag| flag.starts_with("gross_profit_ttm_excluded_because_no_fact_matched")));
    }

    fn sec_test_fact(concept: &str, start: &str, end: &str, value: f64) -> SecFact {
        SecFact {
            concept: concept.to_string(),
            unit: "USD".to_string(),
            form: Some("10-Q".to_string()),
            start: Some(start.to_string()),
            end: Some(end.to_string()),
            filed: Some(end.to_string()),
            fiscal_year: Some(2026),
            fiscal_period: Some("Q1".to_string()),
            accession: Some("test".to_string()),
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
