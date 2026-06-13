use crate::{
    services::{financial_run::FinancialRun, http_json::fetch_json},
    workspace::{AvRawFact, MarketHeadlines, StarterFundamentals},
};
use chrono::Utc;
use loco_rs::prelude::*;
use serde_json::Value;
use std::env;

const ALPHA_VANTAGE_BASE_URL: &str = "https://www.alphavantage.co/query";
pub const ALPHA_VANTAGE_SOURCE: &str = "Alpha Vantage";
const OVERVIEW_FUNCTION: &str = "OVERVIEW";
const INCOME_STATEMENT_FUNCTION: &str = "INCOME_STATEMENT";
const BALANCE_SHEET_FUNCTION: &str = "BALANCE_SHEET";
const CASH_FLOW_FUNCTION: &str = "CASH_FLOW";

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

        let starter = starter_from_overview(&overview, &period_end);
        let market_headlines = build_market_headlines_from_overview(&overview, &starter);

        Ok(AlphaVantageIngestResult {
            ticker: ticker.to_uppercase(),
            fetched_at,
            company_name,
            currency,
            raw_facts,
            market_headlines,
            data_sources: vec![
                format!("{ALPHA_VANTAGE_SOURCE} {OVERVIEW_FUNCTION}"),
                format!("{ALPHA_VANTAGE_SOURCE} {INCOME_STATEMENT_FUNCTION}"),
                format!("{ALPHA_VANTAGE_SOURCE} {BALANCE_SHEET_FUNCTION}"),
                format!("{ALPHA_VANTAGE_SOURCE} {CASH_FLOW_FUNCTION}"),
            ],
            source_notes: vec![
                "Ingested Alpha Vantage fundamentals time series into av_raw_facts.".to_string(),
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
}
