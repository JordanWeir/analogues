use crate::workspace::{AvRawFact, CanonicalMapping};
use loco_rs::prelude::*;

pub const AV_TAXONOMY: &str = "alpha-vantage";
const SELECTED_BY: &str = "av_deterministic";

#[derive(Debug, Clone, Copy)]
struct AvFieldSpec {
    canonical_key: &'static str,
    metric_key: &'static str,
    metric_label: &'static str,
    statement_type: &'static str,
    field_name: &'static str,
    unit: &'static str,
    required: bool,
}

const AV_FIELD_SPECS: &[AvFieldSpec] = &[
    AvFieldSpec {
        canonical_key: "revenue",
        metric_key: "revenue",
        metric_label: "Revenue",
        statement_type: "income_statement",
        field_name: "totalRevenue",
        unit: "USD",
        required: true,
    },
    AvFieldSpec {
        canonical_key: "net_income",
        metric_key: "net_income",
        metric_label: "Net income",
        statement_type: "income_statement",
        field_name: "netIncome",
        unit: "USD",
        required: true,
    },
    AvFieldSpec {
        canonical_key: "gross_profit",
        metric_key: "gross_profit",
        metric_label: "Gross profit",
        statement_type: "income_statement",
        field_name: "grossProfit",
        unit: "USD",
        required: false,
    },
    AvFieldSpec {
        canonical_key: "operating_income",
        metric_key: "operating_income",
        metric_label: "Operating income",
        statement_type: "income_statement",
        field_name: "operatingIncome",
        unit: "USD",
        required: false,
    },
    AvFieldSpec {
        canonical_key: "eps",
        metric_key: "eps",
        metric_label: "Diluted EPS",
        statement_type: "income_statement",
        field_name: "DilutedEPSTTM",
        unit: "USD",
        required: true,
    },
    AvFieldSpec {
        canonical_key: "eps",
        metric_key: "eps",
        metric_label: "Reported EPS",
        statement_type: "income_statement",
        field_name: "reportedEPS",
        unit: "USD",
        required: false,
    },
    AvFieldSpec {
        canonical_key: "shares_outstanding",
        metric_key: "diluted_shares",
        metric_label: "Diluted shares",
        statement_type: "income_statement",
        field_name: "SharesOutstanding",
        unit: "shares",
        required: false,
    },
    AvFieldSpec {
        canonical_key: "shares_outstanding",
        metric_key: "diluted_shares_quarter",
        metric_label: "Common shares outstanding",
        statement_type: "balance_sheet",
        field_name: "commonStockSharesOutstanding",
        unit: "shares",
        required: false,
    },
    AvFieldSpec {
        canonical_key: "cash",
        metric_key: "cash",
        metric_label: "Cash and equivalents",
        statement_type: "balance_sheet",
        field_name: "cashAndCashEquivalentsAtCarryingValue",
        unit: "USD",
        required: false,
    },
    AvFieldSpec {
        canonical_key: "debt_current",
        metric_key: "debt_current",
        metric_label: "Current debt",
        statement_type: "balance_sheet",
        field_name: "shortTermDebt",
        unit: "USD",
        required: false,
    },
    AvFieldSpec {
        canonical_key: "debt_noncurrent",
        metric_key: "debt_noncurrent",
        metric_label: "Noncurrent debt",
        statement_type: "balance_sheet",
        field_name: "longTermDebt",
        unit: "USD",
        required: false,
    },
];

pub struct AvCanonicalResolution {
    pub mappings: Vec<CanonicalMapping>,
    pub quality_flags: Vec<String>,
}

pub fn resolve_av_canonical_mappings(raw_facts: &[AvRawFact]) -> Result<AvCanonicalResolution> {
    let mut mappings = Vec::new();
    let mut quality_flags = Vec::new();
    let mut missing_required = Vec::new();

    for spec in AV_FIELD_SPECS {
        let has_fact = raw_facts
            .iter()
            .any(|fact| fact.field_name == spec.field_name);
        if !has_fact {
            if spec.required {
                missing_required.push(spec.canonical_key);
            } else {
                quality_flags.push(format!(
                    "av_canonical_{}_unavailable",
                    spec.canonical_key.replace('_', "-")
                ));
            }
            continue;
        }

        mappings.push(CanonicalMapping {
            canonical_key: spec.canonical_key.to_string(),
            metric_key: spec.metric_key.to_string(),
            metric_label: spec.metric_label.to_string(),
            statement_type: spec.statement_type.to_string(),
            taxonomy: AV_TAXONOMY.to_string(),
            concept_name: spec.field_name.to_string(),
            unit: spec.unit.to_string(),
            confidence: "high".to_string(),
            rationale: format!(
                "deterministic Alpha Vantage field mapping for '{}'",
                spec.field_name
            ),
            selected_by: SELECTED_BY.to_string(),
            is_active: true,
        });
    }

    if !missing_required.is_empty() {
        return Err(Error::string(&format!(
            "required Alpha Vantage canonical fields missing from av_raw_facts: {}",
            missing_required.join(", ")
        )));
    }

    if mappings.is_empty() {
        return Err(Error::string(
            "no Alpha Vantage canonical mappings could be resolved",
        ));
    }

    Ok(AvCanonicalResolution {
        mappings,
        quality_flags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(field_name: &str) -> AvRawFact {
        AvRawFact {
            endpoint: "INCOME_STATEMENT".to_string(),
            report_type: "quarterly".to_string(),
            field_name: field_name.to_string(),
            label: None,
            period_end: "2025-03-31".to_string(),
            period_type: "quarter".to_string(),
            unit: "USD".to_string(),
            currency: Some("USD".to_string()),
            value: 100.0,
            raw_json: "{}".to_string(),
            fetched_at: "2026-06-13T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn resolves_required_mappings_when_facts_present() {
        let facts = vec![
            fact("totalRevenue"),
            fact("netIncome"),
            fact("DilutedEPSTTM"),
        ];
        let resolution = resolve_av_canonical_mappings(&facts).expect("resolve");
        assert!(resolution
            .mappings
            .iter()
            .any(|mapping| mapping.canonical_key == "revenue"));
        assert!(resolution
            .mappings
            .iter()
            .any(|mapping| mapping.canonical_key == "net_income"));
        assert!(resolution
            .mappings
            .iter()
            .any(|mapping| mapping.canonical_key == "eps"));
    }

    #[test]
    fn fails_when_required_revenue_missing() {
        let facts = vec![fact("netIncome"), fact("DilutedEPSTTM")];
        assert!(resolve_av_canonical_mappings(&facts).is_err());
    }
}
