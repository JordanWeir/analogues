use crate::{
    services::{financial_run::FinancialRun, http_json::fetch_json},
    workspace::{DerivedFundamentals, FundamentalObservation, MarketHeadlines, StarterFundamentals},
};
use chrono::Utc;
use loco_rs::prelude::*;
use serde_json::Value;
use std::env;

const ALPHA_VANTAGE_BASE_URL: &str = "https://www.alphavantage.co/query";
pub const ALPHA_VANTAGE_SOURCE: &str = "Alpha Vantage";
const OVERVIEW_FUNCTION: &str = "OVERVIEW";
const BALANCE_SHEET_FUNCTION: &str = "BALANCE_SHEET";

pub struct AlphaVantageFundamentalsProvider {
    client: reqwest::Client,
    api_key: String,
}

#[derive(Debug, Clone)]
pub struct AlphaVantageFundamentalsSnapshot {
    pub ticker: String,
    pub fetched_at: String,
    pub company_name: Option<String>,
    pub currency: Option<String>,
    pub derived: DerivedFundamentals,
    pub market_headlines: MarketHeadlines,
    pub data_sources: Vec<String>,
    pub source_notes: Vec<String>,
}

impl AlphaVantageFundamentalsProvider {
    pub fn new(client: reqwest::Client, api_key: impl Into<String>) -> Self {
        Self {
            client,
            api_key: api_key.into(),
        }
    }

    pub fn from_env(client: reqwest::Client) -> Option<Self> {
        env::var("ALPHA_VANTAGE_API_KEY")
            .ok()
            .filter(|key| !key.trim().is_empty())
            .map(|api_key| Self::new(client, api_key))
    }

    pub fn provider_name(&self) -> &'static str {
        ALPHA_VANTAGE_SOURCE
    }

    pub async fn fetch_snapshot(&self, ticker: &str) -> Result<AlphaVantageFundamentalsSnapshot> {
        let overview = self.fetch_function(ticker, OVERVIEW_FUNCTION).await?;
        ensure_av_payload(&overview, OVERVIEW_FUNCTION)?;

        let balance_sheet = self
            .fetch_function(ticker, BALANCE_SHEET_FUNCTION)
            .await
            .ok();

        let fetched_at = Utc::now().to_rfc3339();
        let company_name = string_field(&overview, "Name");
        let currency = string_field(&overview, "Currency");
        let period_end = string_field(&overview, "LatestQuarter");

        let mut derived = build_derived_from_overview(&overview, currency.as_deref(), &period_end);
        if let Some(balance_sheet) = balance_sheet.as_ref() {
            enrich_from_balance_sheet(&mut derived, balance_sheet, currency.as_deref(), &period_end);
        }

        let market_headlines = build_market_headlines_from_overview(&overview, &derived.starter);

        Ok(AlphaVantageFundamentalsSnapshot {
            ticker: ticker.to_uppercase(),
            fetched_at,
            company_name,
            currency,
            derived,
            market_headlines,
            data_sources: vec![
                format!("{ALPHA_VANTAGE_SOURCE} {OVERVIEW_FUNCTION}"),
                format!("{ALPHA_VANTAGE_SOURCE} {BALANCE_SHEET_FUNCTION}"),
            ],
            source_notes: vec![
                "Fetched current TTM fundamentals from Alpha Vantage company overview.".to_string(),
            ],
        })
    }

    async fn fetch_function(&self, ticker: &str, function: &str) -> Result<Value> {
        let url = format!(
            "{ALPHA_VANTAGE_BASE_URL}?function={function}&symbol={ticker}&apikey={}",
            self.api_key
        );
        fetch_json(&self.client, &url, None).await
    }
}

pub fn merge_alpha_vantage_starter(
    target: &mut StarterFundamentals,
    source: &StarterFundamentals,
) {
    merge_option(&mut target.shares_outstanding, source.shares_outstanding);
    merge_option(&mut target.revenue_ttm, source.revenue_ttm);
    merge_option(&mut target.net_income_ttm, source.net_income_ttm);
    merge_option(&mut target.gross_profit_ttm, source.gross_profit_ttm);
    merge_option(&mut target.operating_income_ttm, source.operating_income_ttm);
    merge_option(&mut target.gross_margin, source.gross_margin);
    merge_option(&mut target.operating_margin, source.operating_margin);
    merge_option(&mut target.net_margin, source.net_margin);
    merge_option(&mut target.eps_ttm, source.eps_ttm);
    merge_option(&mut target.cash, source.cash);
    merge_option(&mut target.total_debt, source.total_debt);
    if target.fundamental_period_end.is_none() {
        target.fundamental_period_end = source.fundamental_period_end.clone();
    }
}

pub fn merge_alpha_vantage_into_run(run: &mut FinancialRun, snapshot: &AlphaVantageFundamentalsSnapshot) {
    if run.company_name.is_none() {
        run.company_name = snapshot.company_name.clone();
    }
    if run.currency.is_none() {
        run.currency = snapshot.currency.clone();
    }
    run.fundamental_source = Some(ALPHA_VANTAGE_SOURCE.to_string());
    extend_unique(&mut run.data_sources, snapshot.data_sources.clone());
    extend_unique(&mut run.source_notes, snapshot.source_notes.clone());
    extend_unique(
        &mut run.quality_flags,
        snapshot.derived.quality_flags.clone(),
    );

    if run.derived.is_none() {
        run.derived = Some(DerivedFundamentals::default());
    }
    let derived = run.derived.as_mut().expect("derived layer");
    merge_alpha_vantage_starter(&mut derived.starter, &snapshot.derived.starter);
    derived
        .observations
        .extend(snapshot.derived.observations.clone());
    extend_unique(&mut derived.quality_flags, snapshot.derived.quality_flags.clone());
    extend_unique(&mut derived.source_notes, snapshot.derived.source_notes.clone());

    if run.market.is_none() {
        run.merge_market(crate::workspace::MarketQuoteSnapshot {
            ticker: snapshot.ticker.clone(),
            fetched_at: snapshot.fetched_at.clone(),
            currency: snapshot.currency.clone(),
            company_name: snapshot.company_name.clone(),
            headlines: MarketHeadlines::default(),
            observations: Vec::new(),
            data_sources: vec![ALPHA_VANTAGE_SOURCE.to_string()],
            source_notes: Vec::new(),
        });
    }
    let market = run.market.as_mut().expect("market layer");
    merge_option(
        &mut market.headlines.market_cap,
        snapshot.market_headlines.market_cap,
    );
    merge_option(
        &mut market.headlines.trailing_pe,
        snapshot.market_headlines.trailing_pe,
    );
    merge_option(
        &mut market.headlines.price_to_sales_ttm,
        snapshot.market_headlines.price_to_sales_ttm,
    );
    if market.headlines.current_price.is_none() {
        market.headlines.current_price = snapshot.market_headlines.current_price;
    }
}

pub fn alpha_vantage_financial_run(snapshot: &AlphaVantageFundamentalsSnapshot) -> FinancialRun {
    let mut run = FinancialRun::new(&snapshot.ticker);
    run.fetched_at = snapshot.fetched_at.clone();
    run.currency = snapshot.currency.clone();
    run.company_name = snapshot.company_name.clone();
    merge_alpha_vantage_into_run(&mut run, snapshot);
    run
}

fn build_derived_from_overview(
    overview: &Value,
    currency: Option<&str>,
    period_end: &Option<String>,
) -> DerivedFundamentals {
    let revenue_ttm = numeric_field(overview, "RevenueTTM");
    let gross_profit_ttm = numeric_field(overview, "GrossProfitTTM");
    let shares_outstanding = numeric_field(overview, "SharesOutstanding");
    let eps_ttm = numeric_field(overview, "DilutedEPSTTM")
        .or_else(|| numeric_field(overview, "EPS"));
    let operating_margin = numeric_field(overview, "OperatingMarginTTM");
    let net_margin = numeric_field(overview, "ProfitMargin");
    let gross_margin = ratio(gross_profit_ttm, revenue_ttm);
    let operating_income_ttm = multiply(operating_margin, revenue_ttm);
    let net_income_ttm = multiply(net_margin, revenue_ttm);

    let mut derived = DerivedFundamentals {
        starter: StarterFundamentals {
            shares_outstanding,
            revenue_ttm,
            net_income_ttm,
            gross_profit_ttm,
            operating_income_ttm,
            gross_margin,
            operating_margin,
            net_margin,
            eps_ttm,
            fundamental_period_end: period_end.clone(),
            ..StarterFundamentals::default()
        },
        quality_flags: vec!["alpha_vantage_current_ttm_fundamentals".to_string()],
        source_notes: vec![
            "Starter fundamentals sourced from Alpha Vantage OVERVIEW TTM fields.".to_string(),
        ],
        ..DerivedFundamentals::default()
    };

    derived.observations = build_overview_observations(
        overview,
        currency,
        period_end,
        &derived.starter,
    );
    derived
}

fn enrich_from_balance_sheet(
    derived: &mut DerivedFundamentals,
    balance_sheet: &Value,
    currency: Option<&str>,
    period_end: &Option<String>,
) {
    let Some(report) = latest_quarterly_report(balance_sheet) else {
        derived
            .quality_flags
            .push("alpha_vantage_balance_sheet_missing_quarterly_report".to_string());
        return;
    };

    let report_period = string_field(report, "fiscalDateEnding").or_else(|| period_end.clone());
    let cash = numeric_field(report, "cashAndCashEquivalentsAtCarryingValue")
        .or_else(|| numeric_field(report, "cashAndShortTermInvestments"));
    let total_debt = numeric_field(report, "shortLongTermDebtTotal").or_else(|| {
        match (
            numeric_field(report, "shortTermDebt"),
            numeric_field(report, "longTermDebt"),
        ) {
            (Some(short), Some(long)) => Some(short + long),
            (Some(short), None) => Some(short),
            (None, Some(long)) => Some(long),
            (None, None) => None,
        }
    });

    if let Some(cash) = cash {
        derived.starter.cash = Some(cash);
        derived.observations.push(observation(
            Some("cash"),
            "cash",
            "Cash and equivalents",
            "balance_sheet",
            "instant",
            report_period.as_deref(),
            cash,
            currency,
            "cashAndCashEquivalentsAtCarryingValue",
            false,
        ));
    }
    if let Some(total_debt) = total_debt {
        derived.starter.total_debt = Some(total_debt);
        derived.observations.push(observation(
            Some("debt_noncurrent"),
            "total_debt",
            "Total debt",
            "balance_sheet",
            "instant",
            report_period.as_deref(),
            total_debt,
            currency,
            "shortLongTermDebtTotal",
            false,
        ));
    }
    derived.source_notes.push(
        "Balance sheet cash and debt enriched from Alpha Vantage BALANCE_SHEET.".to_string(),
    );
}

fn build_market_headlines_from_overview(
    overview: &Value,
    starter: &StarterFundamentals,
) -> MarketHeadlines {
    let market_cap = numeric_field(overview, "MarketCapitalization");
    let trailing_pe = numeric_field(overview, "TrailingPE").or_else(|| numeric_field(overview, "PERatio"));
    let price_to_sales_ttm = numeric_field(overview, "PriceToSalesRatioTTM");
    let current_price = ratio(market_cap, starter.shares_outstanding);

    MarketHeadlines {
        current_price,
        market_cap,
        trailing_pe,
        price_to_sales_ttm,
    }
}

fn build_overview_observations(
    overview: &Value,
    currency: Option<&str>,
    period_end: &Option<String>,
    starter: &StarterFundamentals,
) -> Vec<FundamentalObservation> {
    let period = period_end.as_deref();
    let mut observations = Vec::new();

    if let Some(value) = starter.revenue_ttm {
        observations.push(observation(
            Some("revenue"),
            "revenue_ttm",
            "Revenue TTM",
            "income_statement",
            "ttm",
            period,
            value,
            currency,
            "RevenueTTM",
            false,
        ));
    }
    if let Some(value) = starter.gross_profit_ttm {
        observations.push(observation(
            Some("gross_profit"),
            "gross_profit_ttm",
            "Gross profit TTM",
            "income_statement",
            "ttm",
            period,
            value,
            currency,
            "GrossProfitTTM",
            false,
        ));
    }
    if let Some(value) = starter.net_income_ttm {
        observations.push(observation(
            Some("net_income"),
            "net_income_ttm",
            "Net income TTM",
            "income_statement",
            "ttm",
            period,
            value,
            currency,
            "ProfitMargin x RevenueTTM",
            true,
        ));
    }
    if let Some(value) = starter.operating_income_ttm {
        observations.push(observation(
            Some("operating_income"),
            "operating_income_ttm",
            "Operating income TTM",
            "income_statement",
            "ttm",
            period,
            value,
            currency,
            "OperatingMarginTTM x RevenueTTM",
            true,
        ));
    }
    if let Some(value) = starter.shares_outstanding {
        observations.push(observation(
            Some("shares_outstanding"),
            "shares_outstanding",
            "Shares outstanding",
            "market",
            "instant",
            period,
            value,
            Some("shares"),
            "SharesOutstanding",
            false,
        ));
    }
    if let Some(value) = starter.eps_ttm {
        observations.push(observation(
            Some("eps"),
            "eps_ttm",
            "EPS TTM",
            "income_statement",
            "ttm",
            period,
            value,
            currency,
            "DilutedEPSTTM",
            false,
        ));
    }

    for (metric_key, metric_label, field, canonical_key, is_ratio) in [
        (
            "gross_margin",
            "Gross margin",
            "GrossProfitTTM / RevenueTTM",
            None,
            true,
        ),
        (
            "operating_margin",
            "Operating margin",
            "OperatingMarginTTM",
            None,
            true,
        ),
        ("net_margin", "Net margin", "ProfitMargin", None, true),
    ] {
        let value = match metric_key {
            "gross_margin" => starter.gross_margin,
            "operating_margin" => starter.operating_margin,
            "net_margin" => starter.net_margin,
            _ => None,
        };
        if let Some(value) = value {
            observations.push(observation(
                canonical_key,
                metric_key,
                metric_label,
                "income_statement",
                "ttm",
                period,
                value,
                if is_ratio { Some("ratio") } else { currency },
                field,
                metric_key == "gross_margin" || metric_key == "net_margin",
            ));
        }
    }

    let _ = overview;
    observations
}

fn observation(
    canonical_key: Option<&str>,
    metric_key: &str,
    metric_label: &str,
    statement_type: &str,
    period_type: &str,
    period_end: Option<&str>,
    value: f64,
    unit: Option<&str>,
    concept_name: &str,
    is_derived: bool,
) -> FundamentalObservation {
    FundamentalObservation {
        canonical_key: canonical_key.map(str::to_string),
        metric_key: metric_key.to_string(),
        metric_label: metric_label.to_string(),
        statement_type: statement_type.to_string(),
        period_type: period_type.to_string(),
        period_start: None,
        period_end: period_end.map(str::to_string),
        as_of_date: None,
        filed_at: None,
        fiscal_year: None,
        fiscal_period: None,
        value,
        unit: unit.map(str::to_string),
        source_type: ALPHA_VANTAGE_SOURCE.to_string(),
        source_note: Some(format!("Alpha Vantage field {concept_name}")),
        concept_name: Some(concept_name.to_string()),
        form: None,
        accession: None,
        quality: Some("alpha_vantage_current".to_string()),
        is_derived,
    }
}

fn latest_quarterly_report(balance_sheet: &Value) -> Option<&Value> {
    balance_sheet
        .get("quarterlyReports")
        .and_then(Value::as_array)
        .and_then(|reports| reports.first())
}

fn ensure_av_payload(payload: &Value, function: &str) -> Result<()> {
    if let Some(message) = payload
        .get("Note")
        .or_else(|| payload.get("Information"))
        .or_else(|| payload.get("Error Message"))
        .and_then(Value::as_str)
    {
        return Err(Error::string(&format!(
            "Alpha Vantage {function} request failed: {message}"
        )));
    }
    if function == OVERVIEW_FUNCTION && payload.get("Symbol").is_none() {
        return Err(Error::string(&format!(
            "Alpha Vantage {function} response did not include company overview for symbol"
        )));
    }
    Ok(())
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|field| match field {
        Value::String(text) if !text.is_empty() && text != "None" && text != "-" => {
            Some(text.to_string())
        }
        _ => None,
    })
}

fn numeric_field(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(parse_numeric_value)
}

fn parse_numeric_value(value: &Value) -> Option<f64> {
    match value {
        Value::String(text) if text.is_empty() || text == "None" || text == "-" => None,
        Value::String(text) => text.parse().ok(),
        Value::Number(number) => number.as_f64(),
        _ => None,
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

fn merge_option(target: &mut Option<f64>, value: Option<f64>) {
    if value.is_some() {
        *target = value;
    }
}

fn extend_unique(target: &mut Vec<String>, updates: Vec<String>) {
    for update in updates {
        if !target.contains(&update) {
            target.push(update);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_overview() -> Value {
        json!({
            "Symbol": "MSFT",
            "Name": "Microsoft Corporation",
            "Currency": "USD",
            "LatestQuarter": "2025-03-31",
            "RevenueTTM": "245122000000",
            "GrossProfitTTM": "171000000000",
            "ProfitMargin": "0.357",
            "OperatingMarginTTM": "0.456",
            "DilutedEPSTTM": "11.86",
            "SharesOutstanding": "7430000000",
            "MarketCapitalization": "3100000000000",
            "TrailingPE": "35.1",
            "PriceToSalesRatioTTM": "12.6"
        })
    }

    fn sample_balance_sheet() -> Value {
        json!({
            "quarterlyReports": [{
                "fiscalDateEnding": "2025-03-31",
                "cashAndCashEquivalentsAtCarryingValue": "80000000000",
                "shortLongTermDebtTotal": "42000000000"
            }]
        })
    }

    #[test]
    fn overview_maps_to_starter_fundamentals() {
        let derived = build_derived_from_overview(
            &sample_overview(),
            Some("USD"),
            &Some("2025-03-31".to_string()),
        );
        assert_eq!(derived.starter.revenue_ttm, Some(245_122_000_000.0));
        assert_eq!(derived.starter.gross_profit_ttm, Some(171_000_000_000.0));
        assert_eq!(derived.starter.shares_outstanding, Some(7_430_000_000.0));
        assert_eq!(derived.starter.eps_ttm, Some(11.86));
        assert!((derived.starter.net_margin.unwrap() - 0.357).abs() < f64::EPSILON);
        assert!(derived.starter.net_income_ttm.unwrap() > 0.0);
    }

    #[test]
    fn balance_sheet_enriches_cash_and_debt() {
        let mut derived = build_derived_from_overview(
            &sample_overview(),
            Some("USD"),
            &Some("2025-03-31".to_string()),
        );
        enrich_from_balance_sheet(
            &mut derived,
            &sample_balance_sheet(),
            Some("USD"),
            &Some("2025-03-31".to_string()),
        );
        assert_eq!(derived.starter.cash, Some(80_000_000_000.0));
        assert_eq!(derived.starter.total_debt, Some(42_000_000_000.0));
    }

    #[test]
    fn merge_prefers_alpha_vantage_values() {
        let mut starter = StarterFundamentals {
            revenue_ttm: Some(1.0),
            eps_ttm: Some(2.0),
            ..StarterFundamentals::default()
        };
        let av = StarterFundamentals {
            revenue_ttm: Some(100.0),
            net_margin: Some(0.2),
            ..StarterFundamentals::default()
        };
        merge_alpha_vantage_starter(&mut starter, &av);
        assert_eq!(starter.revenue_ttm, Some(100.0));
        assert_eq!(starter.eps_ttm, Some(2.0));
        assert_eq!(starter.net_margin, Some(0.2));
    }

    #[test]
    fn rejects_rate_limit_payload() {
        let payload = json!({"Note": "Thank you for using Alpha Vantage!"});
        assert!(ensure_av_payload(&payload, OVERVIEW_FUNCTION).is_err());
    }
}
