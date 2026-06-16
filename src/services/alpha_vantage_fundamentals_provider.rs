use crate::{
    services::{financial_run::FinancialRun, http_json::fetch_json},
    workspace::{AvRawFact, DailyPriceBar, MarketHeadlines, StarterFundamentals},
};
use chrono::{Duration, NaiveDate, Utc};
use loco_rs::prelude::*;
use serde_json::Value;
use std::env;

const ALPHA_VANTAGE_BASE_URL: &str = "https://www.alphavantage.co/query";
pub const ALPHA_VANTAGE_SOURCE: &str = "Alpha Vantage";
const OVERVIEW_FUNCTION: &str = "OVERVIEW";
const INCOME_STATEMENT_FUNCTION: &str = "INCOME_STATEMENT";
const BALANCE_SHEET_FUNCTION: &str = "BALANCE_SHEET";
const CASH_FLOW_FUNCTION: &str = "CASH_FLOW";
const DAILY_TIME_SERIES_FUNCTION: &str = "TIME_SERIES_DAILY_ADJUSTED";
const DEFAULT_DAILY_PRICE_LOOKBACK_DAYS: i64 = 365 * 5;

const REPORT_META_KEYS: &[&str] = &["fiscalDateEnding", "reportedCurrency"];

const OVERVIEW_FIELDS: &[(&str, &str, &str)] = &[
    ("RevenueTTM", "USD", "ttm"),
    ("GrossProfitTTM", "USD", "ttm"),
    ("DilutedEPSTTM", "USD", "ttm"),
    ("EPS", "USD", "ttm"),
    ("SharesOutstanding", "shares", "instant"),
    ("MarketCapitalization", "USD", "instant"),
    ("TrailingPE", "ratio", "instant"),
    ("PriceToSalesRatioTTM", "ratio", "instant"),
    ("OperatingMarginTTM", "ratio", "ttm"),
    ("ProfitMargin", "ratio", "ttm"),
];

pub struct AlphaVantageFundamentalsProvider {
    client: reqwest::Client,
    api_key: String,
}

#[derive(Debug, Clone)]
pub struct AlphaVantageIngestResult {
    pub ticker: String,
    pub fetched_at: String,
    pub company_name: Option<String>,
    pub currency: Option<String>,
    pub raw_facts: Vec<AvRawFact>,
    pub market_headlines: MarketHeadlines,
    pub daily_prices: Vec<DailyPriceBar>,
    pub data_sources: Vec<String>,
    pub source_notes: Vec<String>,
}

impl AlphaVantageIngestResult {
    pub fn latest_daily_close(&self) -> Option<f64> {
        self.daily_prices.last().map(|bar| bar.close)
    }
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

    pub async fn fetch_raw_time_series(&self, ticker: &str) -> Result<AlphaVantageIngestResult> {
        let overview = self.fetch_function(ticker, OVERVIEW_FUNCTION).await?;
        ensure_av_payload(&overview, OVERVIEW_FUNCTION)?;

        let income_statement = self
            .fetch_function(ticker, INCOME_STATEMENT_FUNCTION)
            .await?;
        ensure_av_payload(&income_statement, INCOME_STATEMENT_FUNCTION)?;

        let balance_sheet = self.fetch_function(ticker, BALANCE_SHEET_FUNCTION).await?;
        ensure_av_payload(&balance_sheet, BALANCE_SHEET_FUNCTION)?;

        let cash_flow = self.fetch_function(ticker, CASH_FLOW_FUNCTION).await.ok();

        let fetched_at = Utc::now().to_rfc3339();
        let company_name = string_field(&overview, "Name");
        let currency = string_field(&overview, "Currency");
        let period_end = string_field(&overview, "LatestQuarter")
            .unwrap_or_else(|| fetched_at[..10].to_string());

        let mut raw_facts = Vec::new();
        raw_facts.extend(extract_statement_facts(
            &income_statement,
            INCOME_STATEMENT_FUNCTION,
            "quarterlyReports",
            "quarterly",
            "quarter",
            &fetched_at,
        ));
        raw_facts.extend(extract_statement_facts(
            &income_statement,
            INCOME_STATEMENT_FUNCTION,
            "annualReports",
            "annual",
            "annual",
            &fetched_at,
        ));
        raw_facts.extend(extract_statement_facts(
            &balance_sheet,
            BALANCE_SHEET_FUNCTION,
            "quarterlyReports",
            "quarterly",
            "instant",
            &fetched_at,
        ));
        raw_facts.extend(extract_statement_facts(
            &balance_sheet,
            BALANCE_SHEET_FUNCTION,
            "annualReports",
            "annual",
            "instant",
            &fetched_at,
        ));
        if let Some(cash_flow) = cash_flow.as_ref() {
            if ensure_av_payload(cash_flow, CASH_FLOW_FUNCTION).is_ok() {
                raw_facts.extend(extract_statement_facts(
                    cash_flow,
                    CASH_FLOW_FUNCTION,
                    "quarterlyReports",
                    "quarterly",
                    "quarter",
                    &fetched_at,
                ));
                raw_facts.extend(extract_statement_facts(
                    cash_flow,
                    CASH_FLOW_FUNCTION,
                    "annualReports",
                    "annual",
                    "annual",
                    &fetched_at,
                ));
            }
        }
        raw_facts.extend(extract_overview_facts(
            &overview,
            &period_end,
            currency.as_deref(),
            &fetched_at,
        ));

        if raw_facts.is_empty() {
            return Err(Error::string(
                "Alpha Vantage ingest returned no av_raw_facts rows",
            ));
        }

        let mut data_sources = vec![
            format!("{ALPHA_VANTAGE_SOURCE} {OVERVIEW_FUNCTION}"),
            format!("{ALPHA_VANTAGE_SOURCE} {INCOME_STATEMENT_FUNCTION}"),
            format!("{ALPHA_VANTAGE_SOURCE} {BALANCE_SHEET_FUNCTION}"),
            format!("{ALPHA_VANTAGE_SOURCE} {CASH_FLOW_FUNCTION}"),
        ];
        let mut source_notes = vec![
            "Ingested Alpha Vantage fundamentals time series into av_raw_facts.".to_string(),
        ];

        let mut daily_prices = Vec::new();
        match self.fetch_daily_prices(ticker).await {
            Ok(prices) => {
                if prices.is_empty() {
                    source_notes.push(
                        "Alpha Vantage daily price series returned no bars in lookback window."
                            .to_string(),
                    );
                } else {
                    data_sources.push(format!(
                        "{ALPHA_VANTAGE_SOURCE} {DAILY_TIME_SERIES_FUNCTION}"
                    ));
                    source_notes.push(format!(
                        "Fetched {} daily OHLC bars from Alpha Vantage (last {} days).",
                        prices.len(),
                        daily_price_lookback_days()
                    ));
                    daily_prices = prices;
                }
            }
            Err(err) => {
                source_notes.push(format!("Alpha Vantage daily price fetch failed: {err}"));
            }
        }

        let starter = starter_from_overview(&overview, &period_end);
        let mut market_headlines = build_market_headlines_from_overview(&overview, &starter);
        if let Some(close) = daily_prices.last().map(|bar| bar.close) {
            market_headlines.current_price = Some(close);
        }

        Ok(AlphaVantageIngestResult {
            ticker: ticker.to_uppercase(),
            fetched_at,
            company_name,
            currency,
            raw_facts,
            market_headlines,
            daily_prices,
            data_sources,
            source_notes,
        })
    }

    pub async fn fetch_daily_prices(&self, ticker: &str) -> Result<Vec<DailyPriceBar>> {
        let payload = self.fetch_daily_time_series(ticker, "full").await?;
        ensure_av_payload(&payload, DAILY_TIME_SERIES_FUNCTION)?;
        parse_daily_prices(&payload, daily_price_lookback_days())
    }

    async fn fetch_daily_time_series(&self, ticker: &str, outputsize: &str) -> Result<Value> {
        let url = format!(
            "{ALPHA_VANTAGE_BASE_URL}?function={DAILY_TIME_SERIES_FUNCTION}&symbol={ticker}&outputsize={outputsize}&apikey={}",
            self.api_key
        );
        fetch_json(&self.client, &url, None).await
    }

    async fn fetch_function(&self, ticker: &str, function: &str) -> Result<Value> {
        let url = format!(
            "{ALPHA_VANTAGE_BASE_URL}?function={function}&symbol={ticker}&apikey={}",
            self.api_key
        );
        fetch_json(&self.client, &url, None).await
    }
}

fn extract_statement_facts(
    payload: &Value,
    endpoint: &str,
    reports_key: &str,
    report_type: &str,
    period_type: &str,
    fetched_at: &str,
) -> Vec<AvRawFact> {
    let Some(reports) = payload.get(reports_key).and_then(Value::as_array) else {
        return Vec::new();
    };

    reports
        .iter()
        .flat_map(|report| {
            let period_end = string_field(report, "fiscalDateEnding").unwrap_or_default();
            let currency = string_field(report, "reportedCurrency");
            let raw_json = report.to_string();
            report
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(field_name, value)| {
                    if REPORT_META_KEYS.contains(&field_name.as_str()) {
                        return None;
                    }
                    let metric_value = parse_numeric_value(value)?;
                    let unit = infer_unit(field_name, currency.as_deref());
                    Some(AvRawFact {
                        endpoint: endpoint.to_string(),
                        report_type: report_type.to_string(),
                        field_name: field_name.clone(),
                        label: None,
                        period_end: period_end.clone(),
                        period_type: period_type.to_string(),
                        unit,
                        currency: currency.clone(),
                        value: metric_value,
                        raw_json: raw_json.clone(),
                        fetched_at: fetched_at.to_string(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn extract_overview_facts(
    overview: &Value,
    period_end: &str,
    currency: Option<&str>,
    fetched_at: &str,
) -> Vec<AvRawFact> {
    let raw_json = overview.to_string();
    OVERVIEW_FIELDS
        .iter()
        .filter_map(|(field_name, unit, period_type)| {
            let value = numeric_field(overview, field_name)?;
            Some(AvRawFact {
                endpoint: OVERVIEW_FUNCTION.to_string(),
                report_type: "overview".to_string(),
                field_name: (*field_name).to_string(),
                label: None,
                period_end: period_end.to_string(),
                period_type: (*period_type).to_string(),
                unit: (*unit).to_string(),
                currency: currency.map(str::to_string),
                value,
                raw_json: raw_json.clone(),
                fetched_at: fetched_at.to_string(),
            })
        })
        .collect()
}

fn starter_from_overview(overview: &Value, period_end: &str) -> StarterFundamentals {
    let revenue_ttm = numeric_field(overview, "RevenueTTM");
    let gross_profit_ttm = numeric_field(overview, "GrossProfitTTM");
    let shares_outstanding = numeric_field(overview, "SharesOutstanding");
    let eps_ttm = numeric_field(overview, "DilutedEPSTTM").or_else(|| numeric_field(overview, "EPS"));
    let operating_margin = numeric_field(overview, "OperatingMarginTTM");
    let net_margin = numeric_field(overview, "ProfitMargin");
    StarterFundamentals {
        shares_outstanding,
        revenue_ttm,
        net_income_ttm: multiply(net_margin, revenue_ttm),
        gross_profit_ttm,
        operating_income_ttm: multiply(operating_margin, revenue_ttm),
        gross_margin: ratio(gross_profit_ttm, revenue_ttm),
        operating_margin,
        net_margin,
        eps_ttm,
        fundamental_period_end: Some(period_end.to_string()),
        ..StarterFundamentals::default()
    }
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

fn infer_unit(field_name: &str, currency: Option<&str>) -> String {
    let lower = field_name.to_lowercase();
    if lower.contains("share") {
        return "shares".to_string();
    }
    if lower.contains("margin") || lower.contains("ratio") || lower.contains("percent") {
        return "ratio".to_string();
    }
    currency.unwrap_or("USD").to_string()
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
    if matches!(
        function,
        INCOME_STATEMENT_FUNCTION | BALANCE_SHEET_FUNCTION | CASH_FLOW_FUNCTION
    ) && payload.get("symbol").is_none()
    {
        return Err(Error::string(&format!(
            "Alpha Vantage {function} response did not include financial statement payload"
        )));
    }
    if function == DAILY_TIME_SERIES_FUNCTION && payload.get("Time Series (Daily)").is_none() {
        return Err(Error::string(&format!(
            "Alpha Vantage {function} response did not include daily time series"
        )));
    }
    Ok(())
}

fn daily_price_lookback_days() -> i64 {
    std::env::var("ALPHA_VANTAGE_DAILY_PRICE_LOOKBACK_DAYS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|days: &i64| *days > 0)
        .unwrap_or(DEFAULT_DAILY_PRICE_LOOKBACK_DAYS)
}

fn parse_daily_prices(payload: &Value, lookback_days: i64) -> Result<Vec<DailyPriceBar>> {
    let series = payload
        .get("Time Series (Daily)")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            Error::string("Alpha Vantage daily time series response did not include price bars")
        })?;

    let cutoff = Utc::now().date_naive() - Duration::days(lookback_days);
    let mut bars = series
        .iter()
        .filter_map(|(trade_date, values)| parse_daily_price_bar(trade_date, values))
        .filter(|bar| {
            NaiveDate::parse_from_str(&bar.trade_date, "%Y-%m-%d")
                .map(|date| date >= cutoff)
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    bars.sort_by(|left, right| left.trade_date.cmp(&right.trade_date));
    Ok(bars)
}

fn parse_daily_price_bar(trade_date: &str, values: &Value) -> Option<DailyPriceBar> {
    let open = numeric_field(values, "1. open")?;
    let high = numeric_field(values, "2. high")?;
    let low = numeric_field(values, "3. low")?;
    let close = numeric_field(values, "4. close")?;
    let adjusted_close = numeric_field(values, "5. adjusted close");
    let volume = numeric_field(values, "6. volume")
        .or_else(|| numeric_field(values, "5. volume"))?;

    Some(DailyPriceBar {
        trade_date: trade_date.to_string(),
        open,
        high,
        low,
        close,
        volume,
        adjusted_close,
    })
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

pub fn alpha_vantage_market_run(snapshot: &AlphaVantageIngestResult) -> FinancialRun {
    let mut run = FinancialRun::new(&snapshot.ticker);
    run.fetched_at = snapshot.fetched_at.clone();
    run.currency = snapshot.currency.clone();
    run.company_name = snapshot.company_name.clone();
    run.fundamental_source = Some(ALPHA_VANTAGE_SOURCE.to_string());
    run.merge_market(crate::workspace::MarketQuoteSnapshot {
        ticker: snapshot.ticker.clone(),
        fetched_at: snapshot.fetched_at.clone(),
        currency: snapshot.currency.clone(),
        company_name: snapshot.company_name.clone(),
        headlines: snapshot.market_headlines.clone(),
        observations: Vec::new(),
        data_sources: snapshot.data_sources.clone(),
        source_notes: snapshot.source_notes.clone(),
    });
    run
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_quarterly_income_statement_facts() {
        let payload = json!({
            "symbol": "IBM",
            "quarterlyReports": [{
                "fiscalDateEnding": "2025-03-31",
                "reportedCurrency": "USD",
                "totalRevenue": "1000000000",
                "netIncome": "100000000"
            }]
        });
        let facts = extract_statement_facts(
            &payload,
            INCOME_STATEMENT_FUNCTION,
            "quarterlyReports",
            "quarterly",
            "quarter",
            "2026-06-13T00:00:00Z",
        );
        assert_eq!(facts.len(), 2);
        assert!(facts.iter().any(|fact| fact.field_name == "totalRevenue"));
    }

    #[test]
    fn extracts_overview_ttm_fields() {
        let payload = json!({
            "Symbol": "IBM",
            "LatestQuarter": "2025-03-31",
            "Currency": "USD",
            "RevenueTTM": "1000000000",
            "DilutedEPSTTM": "5.84"
        });
        let facts = extract_overview_facts(&payload, "2025-03-31", Some("USD"), "2026-06-13T00:00:00Z");
        assert!(facts.iter().any(|fact| fact.field_name == "DilutedEPSTTM"));
        assert!(facts.iter().any(|fact| fact.field_name == "RevenueTTM"));
    }

    #[test]
    fn rejects_rate_limit_payload() {
        let payload = json!({"Note": "Thank you for using Alpha Vantage!"});
        assert!(ensure_av_payload(&payload, OVERVIEW_FUNCTION).is_err());
    }

    #[test]
    fn parses_daily_price_series_within_lookback() {
        let payload = json!({
            "Time Series (Daily)": {
                "2025-01-02": {
                    "1. open": "100.0",
                    "2. high": "101.0",
                    "3. low": "99.0",
                    "4. close": "100.5",
                    "5. adjusted close": "100.5",
                    "6. volume": "1000000"
                },
                "2099-01-02": {
                    "1. open": "200.0",
                    "2. high": "201.0",
                    "3. low": "199.0",
                    "4. close": "200.5",
                    "5. adjusted close": "200.5",
                    "6. volume": "2000000"
                }
            }
        });

        let bars = parse_daily_prices(&payload, 365).expect("bars");
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].trade_date, "2099-01-02");
        assert_eq!(bars[0].close, 200.5);
    }

    #[test]
    fn ingest_prefers_latest_daily_close_for_current_price() {
        let overview = json!({
            "MarketCapitalization": "3100000000000",
            "SharesOutstanding": "7430000000"
        });
        let starter = starter_from_overview(&overview, "2025-03-31");
        let daily_prices = vec![DailyPriceBar {
            trade_date: "2026-06-11".to_string(),
            open: 412.0,
            high: 418.0,
            low: 411.0,
            close: 417.25,
            volume: 1_100_000.0,
            adjusted_close: Some(417.25),
        }];
        let mut market_headlines = build_market_headlines_from_overview(&overview, &starter);
        if let Some(close) = daily_prices.last().map(|bar| bar.close) {
            market_headlines.current_price = Some(close);
        }

        let ingest = AlphaVantageIngestResult {
            ticker: "MSFT".to_string(),
            fetched_at: "2026-06-11T00:00:00Z".to_string(),
            company_name: Some("Microsoft Corporation".to_string()),
            currency: Some("USD".to_string()),
            raw_facts: Vec::new(),
            market_headlines,
            daily_prices,
            data_sources: Vec::new(),
            source_notes: Vec::new(),
        };

        assert_eq!(ingest.latest_daily_close(), Some(417.25));
        assert_eq!(ingest.market_headlines.current_price, Some(417.25));
    }
}
