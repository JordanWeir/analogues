use crate::{
    services::http_json::fetch_json,
    tasks::init_workspace::{SecRawFact},
};
use chrono::Utc;
use loco_rs::prelude::*;
use serde_json::Value;

const SEC_USER_AGENT: &str = "stock-agent-2/0.1 research@example.local";
const SEC_TICKERS_URL: &str = "https://www.sec.gov/files/company_tickers.json";
pub struct SecFactsProvider {
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct SecCompanyIdentity {
    pub ticker: String,
    pub cik: i64,
    pub company_title: Option<String>,
    pub lookup_source: String,
}

#[derive(Debug, Clone)]
pub struct SecCompanyFactsPayload {
    pub identity: SecCompanyIdentity,
    pub fetched_at: String,
    pub source_url: String,
    pub raw_json: Value,
}

impl SecFactsProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    pub fn provider_name(&self) -> &'static str {
        "SEC Company Facts"
    }

    pub async fn lookup_company(&self, ticker: &str) -> Result<SecCompanyIdentity> {
        let payload = fetch_json(&self.client, SEC_TICKERS_URL, Some(SEC_USER_AGENT)).await?;
        let ticker_upper = ticker.to_uppercase();
        let company = payload
            .as_object()
            .and_then(|companies| {
                companies.values().find(|company| {
                    company
                        .get("ticker")
                        .and_then(Value::as_str)
                        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(&ticker_upper))
                })
            })
            .ok_or_else(|| {
                Error::string(&format!("Ticker {ticker} was not found in SEC tickers"))
            })?;

        let cik = company
            .get("cik_str")
            .and_then(Value::as_i64)
            .ok_or_else(|| Error::string("SEC ticker record did not include cik_str"))?;
        let ticker = company
            .get("ticker")
            .and_then(Value::as_str)
            .map_or(ticker_upper, |ticker| ticker.to_uppercase());
        let company_title = company
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_string);

        Ok(SecCompanyIdentity {
            ticker,
            cik,
            company_title,
            lookup_source: SEC_TICKERS_URL.to_string(),
        })
    }

    pub async fn fetch_company_facts(
        &self,
        identity: &SecCompanyIdentity,
    ) -> Result<SecCompanyFactsPayload> {
        let source_url = self.source_url(identity);
        let raw_json = fetch_json(&self.client, &source_url, Some(SEC_USER_AGENT)).await?;

        Ok(SecCompanyFactsPayload {
            identity: identity.clone(),
            fetched_at: Utc::now().to_rfc3339(),
            source_url,
            raw_json,
        })
    }

    pub fn extract_raw_facts(&self, payload: &SecCompanyFactsPayload) -> Result<Vec<SecRawFact>> {
        let facts_root = payload
            .raw_json
            .get("facts")
            .ok_or_else(|| Error::string("SEC Company Facts response did not include facts"))?;

        Ok(extract_raw_facts_from_root(facts_root, &payload.fetched_at))
    }

    pub fn source_url(&self, identity: &SecCompanyIdentity) -> String {
        format!(
            "https://data.sec.gov/api/xbrl/companyfacts/CIK{:010}.json",
            identity.cik
        )
    }
}

pub(crate) fn extract_raw_facts_from_root(facts_root: &Value, fetched_at: &str) -> Vec<SecRawFact> {
    facts_root
        .as_object()
        .into_iter()
        .flat_map(|taxonomies| {
            taxonomies.iter().flat_map(move |(taxonomy, concepts)| {
                concepts.as_object().into_iter().flat_map(move |concepts| {
                    concepts
                        .iter()
                        .flat_map(move |(concept_name, concept_payload)| {
                            let label = string_at(concept_payload, "label");
                            let description = string_at(concept_payload, "description");
                            concept_payload
                                .get("units")
                                .and_then(Value::as_object)
                                .into_iter()
                                .flat_map(move |units| {
                                    let label = label.clone();
                                    let description = description.clone();
                                    units.iter().flat_map(move |(unit, values)| {
                                        let label = label.clone();
                                        let description = description.clone();
                                        values.as_array().into_iter().flatten().filter_map(
                                            move |value| {
                                                sec_raw_fact(
                                                    taxonomy,
                                                    concept_name,
                                                    label.clone(),
                                                    description.clone(),
                                                    unit,
                                                    value,
                                                    fetched_at,
                                                )
                                            },
                                        )
                                    })
                                })
                        })
                })
            })
        })
        .collect()
}

fn sec_raw_fact(
    taxonomy: &str,
    concept_name: &str,
    label: Option<String>,
    description: Option<String>,
    unit: &str,
    value: &Value,
    fetched_at: &str,
) -> Option<SecRawFact> {
    Some(SecRawFact {
        taxonomy: taxonomy.to_string(),
        concept_name: concept_name.to_string(),
        label,
        description,
        unit: unit.to_string(),
        form: string_at(value, "form"),
        start: string_at(value, "start"),
        end: string_at(value, "end"),
        filed: string_at(value, "filed"),
        fiscal_year: value.get("fy").and_then(Value::as_i64),
        fiscal_period: string_at(value, "fp"),
        accession: string_at(value, "accn"),
        frame: string_at(value, "frame"),
        value: number_at(value, "val")?,
        raw_json: serde_json::to_string(value).ok()?,
        fetched_at: fetched_at.to_string(),
    })
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
