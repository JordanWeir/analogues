use crate::{
    services::http_json::fetch_json, tasks::init_workspace::FinancialSnapshot,
    workspace::FundamentalObservation,
};
use loco_rs::prelude::*;
use serde_json::Value;

pub struct YahooChartMarketDataAdapter {
    client: reqwest::Client,
}

impl YahooChartMarketDataAdapter {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    pub async fn fetch_snapshot(&self, ticker: &str) -> Result<FinancialSnapshot> {
        let url = format!(
            "https://query1.finance.yahoo.com/v8/finance/chart/{ticker}?range=1d&interval=1d"
        );
        let payload = fetch_json(&self.client, &url, None).await?;
        let meta = payload
            .pointer("/chart/result/0/meta")
            .ok_or_else(|| Error::string("Yahoo chart response did not include quote metadata"))?;

        let mut snapshot = FinancialSnapshot::new(ticker);
        snapshot.currency = string_at(meta, "currency");
        snapshot.company_name =
            string_at(meta, "shortName").or_else(|| string_at(meta, "longName"));
        snapshot.current_price =
            number_at(meta, "regularMarketPrice").or_else(|| number_at(meta, "previousClose"));
        if let Some(price) = snapshot.current_price {
            snapshot.observations.push(FundamentalObservation {
                canonical_key: Some("current_price".to_string()),
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
