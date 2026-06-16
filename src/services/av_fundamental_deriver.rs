use crate::{
    services::{
        av_canonical_mapping::AV_TAXONOMY,
        fundamental_deriver::period_suffixed_metric_key,
    },
    workspace::{AvRawFact, CanonicalMapping, DerivedFundamentals, FundamentalObservation},
};

pub const AV_SOURCE_TYPE: &str = "Alpha Vantage";

#[derive(Debug, Clone)]
pub struct AvFundamentalDeriver;

impl AvFundamentalDeriver {
    pub fn build_observations(
        raw_facts: &[AvRawFact],
        mappings: &[CanonicalMapping],
    ) -> Vec<FundamentalObservation> {
        mappings
            .iter()
            .filter(|mapping| mapping.is_active && mapping.taxonomy == AV_TAXONOMY)
            .flat_map(|mapping| {
                raw_facts
                    .iter()
                    .filter(|fact| mapping.field_name_matches(fact))
                    .map(|fact| av_observation(mapping, fact))
            })
            .collect()
    }

    pub fn derive_starter_fundamentals(
        raw_facts: &[AvRawFact],
        mappings: &[CanonicalMapping],
        currency: Option<&str>,
    ) -> DerivedFundamentals {
        let mut derived = DerivedFundamentals::default();
        derived.observations = Self::build_observations(raw_facts, mappings);
        derived.source_notes.push(
            "Starter fundamentals derived from Alpha Vantage av_raw_facts time series.".to_string(),
        );

        let period_end = latest_annual_or_quarter_period(raw_facts, "totalRevenue")
            .or_else(|| latest_fact_period(raw_facts, "DilutedEPSTTM"))
            .or_else(|| latest_fact_period(raw_facts, "netIncome"));

        if let Some(period_end) = period_end.as_deref() {
            derived.starter.fundamental_period_end = Some(period_end.to_string());
            derived.starter.revenue_ttm =
                latest_flow_value(raw_facts, "totalRevenue", period_end, "annual")
                    .or_else(|| latest_flow_value(raw_facts, "totalRevenue", period_end, "quarter"));
            derived.starter.net_income_ttm =
                latest_flow_value(raw_facts, "netIncome", period_end, "annual")
                    .or_else(|| latest_flow_value(raw_facts, "netIncome", period_end, "quarter"));
            derived.starter.gross_profit_ttm =
                latest_flow_value(raw_facts, "grossProfit", period_end, "annual")
                    .or_else(|| latest_flow_value(raw_facts, "grossProfit", period_end, "quarter"));
            derived.starter.operating_income_ttm =
                latest_flow_value(raw_facts, "operatingIncome", period_end, "annual").or_else(|| {
                    latest_flow_value(raw_facts, "operatingIncome", period_end, "quarter")
                });
            derived.starter.gross_margin =
                ratio(derived.starter.gross_profit_ttm, derived.starter.revenue_ttm);
            derived.starter.operating_margin =
                ratio(derived.starter.operating_income_ttm, derived.starter.revenue_ttm);
            derived.starter.net_margin =
                ratio(derived.starter.net_income_ttm, derived.starter.revenue_ttm);
        } else {
            derived
                .quality_flags
                .push("av_income_statement_no_coherent_headline_period".to_string());
        }

        derived.starter.eps_ttm = latest_overview_or_fact(raw_facts, "DilutedEPSTTM")
            .or_else(|| latest_overview_or_fact(raw_facts, "EPS"))
            .or_else(|| {
                ratio(
                    derived.starter.net_income_ttm,
                    latest_overview_or_fact(raw_facts, "SharesOutstanding"),
                )
            });

        derived.starter.shares_outstanding =
            latest_overview_or_fact(raw_facts, "SharesOutstanding").or_else(|| {
                latest_balance_sheet_value(raw_facts, "commonStockSharesOutstanding")
            });

        derived.starter.cash =
            latest_balance_sheet_value(raw_facts, "cashAndCashEquivalentsAtCarryingValue");
        derived.starter.total_debt = latest_balance_sheet_value(raw_facts, "shortLongTermDebtTotal")
            .or_else(|| {
                match (
                    latest_balance_sheet_value(raw_facts, "shortTermDebt"),
                    latest_balance_sheet_value(raw_facts, "longTermDebt"),
                ) {
                    (Some(short), Some(long)) => Some(short + long),
                    (Some(short), None) => Some(short),
                    (None, Some(long)) => Some(long),
                    (None, None) => None,
                }
            });

        if derived.starter.revenue_ttm.is_none() {
            derived
                .quality_flags
                .push("revenue_ttm_unavailable_from_av_raw_facts".to_string());
        }
        if derived.starter.eps_ttm.is_none() {
            derived
                .quality_flags
                .push("eps_ttm_unavailable_from_av_raw_facts".to_string());
        }

        let _ = currency;
        derived
    }
}

trait AvMappingMatch {
    fn field_name_matches(&self, fact: &AvRawFact) -> bool;
}

impl AvMappingMatch for CanonicalMapping {
    fn field_name_matches(&self, fact: &AvRawFact) -> bool {
        self.taxonomy == AV_TAXONOMY && self.concept_name == fact.field_name
    }
}

fn av_observation(mapping: &CanonicalMapping, fact: &AvRawFact) -> FundamentalObservation {
    let period_type = fact.period_type.clone();
    FundamentalObservation {
        canonical_key: Some(mapping.canonical_key.clone()),
        metric_key: period_suffixed_metric_key(
            &mapping.metric_key,
            &mapping.statement_type,
            &period_type,
        ),
        metric_label: mapping.metric_label.clone(),
        statement_type: mapping.statement_type.clone(),
        period_type,
        period_start: None,
        period_end: Some(fact.period_end.clone()),
        as_of_date: Some(fact.period_end.clone()),
        filed_at: None,
        fiscal_year: None,
        fiscal_period: None,
        value: fact.value,
        unit: Some(fact.unit.clone()),
        source_type: AV_SOURCE_TYPE.to_string(),
        source_note: Some(format!(
            "Alpha Vantage {} {} field {}",
            fact.endpoint, fact.report_type, fact.field_name
        )),
        concept_name: Some(fact.field_name.clone()),
        form: None,
        accession: None,
        quality: Some("av_raw_fact".to_string()),
        is_derived: false,
    }
}

fn latest_annual_or_quarter_period(raw_facts: &[AvRawFact], field_name: &str) -> Option<String> {
    raw_facts
        .iter()
        .filter(|fact| fact.field_name == field_name)
        .filter(|fact| fact.period_type == "annual" || fact.period_type == "quarter")
        .map(|fact| fact.period_end.clone())
        .max()
}

fn latest_fact_period(raw_facts: &[AvRawFact], field_name: &str) -> Option<String> {
    raw_facts
        .iter()
        .filter(|fact| fact.field_name == field_name)
        .map(|fact| fact.period_end.clone())
        .max()
}

fn latest_flow_value(
    raw_facts: &[AvRawFact],
    field_name: &str,
    period_end: &str,
    report_type: &str,
) -> Option<f64> {
    raw_facts
        .iter()
        .filter(|fact| fact.field_name == field_name)
        .filter(|fact| fact.period_end == period_end)
        .filter(|fact| fact.report_type == report_type)
        .map(|fact| fact.value)
        .next()
}

fn latest_balance_sheet_value(raw_facts: &[AvRawFact], field_name: &str) -> Option<f64> {
    raw_facts
        .iter()
        .filter(|fact| fact.endpoint == "BALANCE_SHEET")
        .filter(|fact| fact.field_name == field_name)
        .max_by(|left, right| left.period_end.cmp(&right.period_end))
        .map(|fact| fact.value)
}

fn latest_overview_or_fact(raw_facts: &[AvRawFact], field_name: &str) -> Option<f64> {
    raw_facts
        .iter()
        .filter(|fact| fact.field_name == field_name)
        .max_by(|left, right| left.period_end.cmp(&right.period_end))
        .map(|fact| fact.value)
}

fn ratio(numerator: Option<f64>, denominator: Option<f64>) -> Option<f64> {
    match (numerator, denominator) {
        (Some(numerator), Some(denominator)) if denominator != 0.0 => Some(numerator / denominator),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_fact(field_name: &str, value: f64, period_end: &str, report_type: &str) -> AvRawFact {
        AvRawFact {
            endpoint: "INCOME_STATEMENT".to_string(),
            report_type: report_type.to_string(),
            field_name: field_name.to_string(),
            label: None,
            period_end: period_end.to_string(),
            period_type: if report_type == "annual" {
                "annual".to_string()
            } else {
                "quarter".to_string()
            },
            unit: "USD".to_string(),
            currency: Some("USD".to_string()),
            value,
            raw_json: "{}".to_string(),
            fetched_at: "2026-06-13T00:00:00Z".to_string(),
        }
    }

    fn sample_mappings() -> Vec<CanonicalMapping> {
        vec![
            CanonicalMapping {
                canonical_key: "revenue".to_string(),
                metric_key: "revenue".to_string(),
                metric_label: "Revenue".to_string(),
                statement_type: "income_statement".to_string(),
                taxonomy: AV_TAXONOMY.to_string(),
                concept_name: "totalRevenue".to_string(),
                unit: "USD".to_string(),
                confidence: "high".to_string(),
                rationale: "test".to_string(),
                selected_by: "test".to_string(),
                is_active: true,
            },
            CanonicalMapping {
                canonical_key: "net_income".to_string(),
                metric_key: "net_income".to_string(),
                metric_label: "Net income".to_string(),
                statement_type: "income_statement".to_string(),
                taxonomy: AV_TAXONOMY.to_string(),
                concept_name: "netIncome".to_string(),
                unit: "USD".to_string(),
                confidence: "high".to_string(),
                rationale: "test".to_string(),
                selected_by: "test".to_string(),
                is_active: true,
            },
            CanonicalMapping {
                canonical_key: "eps".to_string(),
                metric_key: "eps".to_string(),
                metric_label: "Diluted EPS".to_string(),
                statement_type: "income_statement".to_string(),
                taxonomy: AV_TAXONOMY.to_string(),
                concept_name: "DilutedEPSTTM".to_string(),
                unit: "USD".to_string(),
                confidence: "high".to_string(),
                rationale: "test".to_string(),
                selected_by: "test".to_string(),
                is_active: true,
            },
        ]
    }

    #[test]
    fn builds_observations_for_mapped_fields() {
        let facts = vec![
            sample_fact("totalRevenue", 100.0, "2025-12-31", "annual"),
            sample_fact("netIncome", 10.0, "2025-12-31", "annual"),
            AvRawFact {
                endpoint: "OVERVIEW".to_string(),
                report_type: "overview".to_string(),
                field_name: "DilutedEPSTTM".to_string(),
                label: None,
                period_end: "2025-12-31".to_string(),
                period_type: "ttm".to_string(),
                unit: "USD".to_string(),
                currency: Some("USD".to_string()),
                value: 1.25,
                raw_json: "{}".to_string(),
                fetched_at: "2026-06-13T00:00:00Z".to_string(),
            },
        ];
        let observations = AvFundamentalDeriver::build_observations(&facts, &sample_mappings());
        assert_eq!(observations.len(), 3);
        assert!(observations
            .iter()
            .any(|observation| observation.metric_key == "revenue_annual"));
    }

    #[test]
    fn derives_starter_headlines_from_latest_annual() {
        let facts = vec![
            sample_fact("totalRevenue", 100.0, "2024-12-31", "annual"),
            sample_fact("totalRevenue", 120.0, "2025-12-31", "annual"),
            sample_fact("netIncome", 12.0, "2025-12-31", "annual"),
            AvRawFact {
                endpoint: "OVERVIEW".to_string(),
                report_type: "overview".to_string(),
                field_name: "DilutedEPSTTM".to_string(),
                label: None,
                period_end: "2025-12-31".to_string(),
                period_type: "ttm".to_string(),
                unit: "USD".to_string(),
                currency: Some("USD".to_string()),
                value: 1.25,
                raw_json: "{}".to_string(),
                fetched_at: "2026-06-13T00:00:00Z".to_string(),
            },
        ];
        let derived =
            AvFundamentalDeriver::derive_starter_fundamentals(&facts, &sample_mappings(), Some("USD"));
        assert_eq!(derived.starter.revenue_ttm, Some(120.0));
        assert_eq!(derived.starter.net_income_ttm, Some(12.0));
        assert_eq!(derived.starter.eps_ttm, Some(1.25));
    }
}
