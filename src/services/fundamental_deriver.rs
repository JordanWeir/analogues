use crate::workspace::{CanonicalMapping, DerivedFundamentals, FundamentalObservation, SecRawFact};
use chrono::NaiveDate;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct FundamentalDeriver;

#[derive(Debug, Clone)]
pub(crate) struct SecFact {
    pub(crate) concept: String,
    pub(crate) form: Option<String>,
    pub(crate) start: Option<String>,
    pub(crate) end: Option<String>,
    pub(crate) filed: Option<String>,
    pub(crate) value: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct TtmMetric {
    pub(crate) metric_key: &'static str,
    pub(crate) value: f64,
    pub(crate) period_start: Option<String>,
    pub(crate) period_end: String,
    pub(crate) source_note: String,
    pub(crate) quality_flags: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct IncomeBundle {
    pub(crate) period_end: String,
    pub(crate) revenue: Option<TtmMetric>,
    pub(crate) net_income: Option<TtmMetric>,
    pub(crate) gross_profit: Option<TtmMetric>,
    pub(crate) operating_income: Option<TtmMetric>,
    pub(crate) source_notes: Vec<String>,
    pub(crate) quality_flags: Vec<String>,
}

impl FundamentalDeriver {
    pub fn build_observations(
        raw_facts: &[SecRawFact],
        mappings: &[CanonicalMapping],
    ) -> Vec<FundamentalObservation> {
        canonical_sec_observations(raw_facts, mappings)
    }

    pub(crate) fn select_latest_baseline_bundle(
        raw_facts: &[SecRawFact],
        mappings: &[CanonicalMapping],
    ) -> Option<IncomeBundle> {
        select_latest_income_bundle(raw_facts, mappings)
    }

    pub(crate) fn latest_value_fact(
        raw_facts: &[SecRawFact],
        mappings: &[CanonicalMapping],
        canonical_key: &str,
        unit_hint: &str,
        prefer_period_end: Option<&str>,
    ) -> Option<SecFact> {
        latest_value_fact(
            raw_facts,
            mappings,
            canonical_key,
            unit_hint,
            prefer_period_end,
        )
    }

    pub(crate) fn total_latest_values(
        raw_facts: &[SecRawFact],
        mappings: &[CanonicalMapping],
        canonical_keys: &[&str],
        unit_hint: &str,
    ) -> Option<f64> {
        total_latest_values(raw_facts, mappings, canonical_keys, unit_hint)
    }

    pub(crate) fn apply_income_bundle(
        derived: &mut DerivedFundamentals,
        bundle: &IncomeBundle,
        currency: Option<&str>,
    ) {
        append_bundle_observations(&mut derived.observations, bundle, currency);
        let starter = &mut derived.starter;
        starter.revenue_ttm = bundle.revenue.as_ref().map(|metric| metric.value);
        starter.net_income_ttm = bundle.net_income.as_ref().map(|metric| metric.value);
        starter.gross_profit_ttm = bundle.gross_profit.as_ref().map(|metric| metric.value);
        starter.operating_income_ttm = bundle.operating_income.as_ref().map(|metric| metric.value);
        starter.gross_margin = ratio(starter.gross_profit_ttm, starter.revenue_ttm);
        starter.operating_margin = ratio(starter.operating_income_ttm, starter.revenue_ttm);
        starter.net_margin = ratio(starter.net_income_ttm, starter.revenue_ttm);
        starter.fundamental_period_end = Some(bundle.period_end.clone());
        derived.source_notes.extend(bundle.source_notes.clone());
        derived.quality_flags.extend(bundle.quality_flags.clone());
    }

    pub fn derive_starter_fundamentals(
        raw_facts: &[SecRawFact],
        mappings: &[CanonicalMapping],
        currency: Option<&str>,
    ) -> DerivedFundamentals {
        let mut derived = DerivedFundamentals::default();
        derived.observations = Self::build_observations(raw_facts, mappings);
        let bundle = Self::select_latest_baseline_bundle(raw_facts, mappings);
        let shares_fact = Self::latest_value_fact(
            raw_facts,
            mappings,
            "shares_outstanding",
            "shares",
            bundle.as_ref().map(|bundle| bundle.period_end.as_str()),
        );
        let eps_fact = Self::latest_value_fact(
            raw_facts,
            mappings,
            "eps",
            "USD/shares",
            bundle.as_ref().map(|bundle| bundle.period_end.as_str()),
        );
        let cash_fact = Self::latest_value_fact(raw_facts, mappings, "cash", "USD", None);
        let debt = Self::total_latest_values(
            raw_facts,
            mappings,
            &["debt_current", "debt_noncurrent"],
            "USD",
        );

        if let Some(bundle) = bundle {
            Self::apply_income_bundle(&mut derived, &bundle, currency);
        } else {
            push_quality_flag(
                &mut derived.quality_flags,
                "sec_income_statement_no_coherent_ttm_or_annual_bundle",
            );
        }
        derived.starter.shares_outstanding = shares_fact.as_ref().map(|fact| fact.value);
        derived.starter.cash = cash_fact.as_ref().map(|fact| fact.value);
        derived.starter.total_debt = debt;
        derived.starter.eps_ttm = eps_fact
            .as_ref()
            .and_then(|fact| {
                (fact.end == derived.starter.fundamental_period_end).then_some(fact.value)
            })
            .or_else(|| {
                let shares_end = shares_fact.as_ref().and_then(|fact| fact.end.clone());
                (shares_end == derived.starter.fundamental_period_end)
                    .then(|| {
                        ratio(
                            derived.starter.net_income_ttm,
                            derived.starter.shares_outstanding,
                        )
                    })
                    .flatten()
            });
        if derived.starter.eps_ttm.is_none()
            && derived.starter.net_income_ttm.is_some()
            && derived.starter.shares_outstanding.is_some()
        {
            push_quality_flag(
                &mut derived.quality_flags,
                "eps_ttm_not_derived_because_share_count_period_did_not_match_income_period",
            );
        }
        if shares_fact.as_ref().and_then(|fact| fact.end.as_deref())
            != derived.starter.fundamental_period_end.as_deref()
        {
            push_quality_flag(
                &mut derived.quality_flags,
                "shares_outstanding_uses_latest_available_instant_not_income_period",
            );
        }
        derived
    }
}

pub fn classify_period(fact: &SecRawFact) -> &'static str {
    fact_period_type(fact)
}

pub(crate) fn unit_matches(unit: &str, unit_hint: &str) -> bool {
    unit.to_lowercase().contains(&unit_hint.to_lowercase())
}

fn ratio(numerator: Option<f64>, denominator: Option<f64>) -> Option<f64> {
    match (numerator, denominator) {
        (Some(numerator), Some(denominator)) if denominator != 0.0 => Some(numerator / denominator),
        _ => None,
    }
}

fn push_quality_flag(flags: &mut Vec<String>, flag: &str) {
    if !flags.iter().any(|existing| existing == flag) {
        flags.push(flag.to_string());
    }
}

fn extend_unique(target: &mut Vec<String>, updates: Vec<String>) {
    for update in updates {
        if !target.contains(&update) {
            target.push(update);
        }
    }
}

fn canonical_sec_observations(
    raw_facts: &[SecRawFact],
    mappings: &[CanonicalMapping],
) -> Vec<FundamentalObservation> {
    mappings
        .iter()
        .filter(|mapping| mapping.is_active)
        .flat_map(|mapping| {
            raw_facts
                .iter()
                .filter(move |fact| mapping_matches_fact(mapping, fact))
                .map(move |fact| sec_observation(mapping, fact))
        })
        .collect()
}

fn sec_observation(mapping: &CanonicalMapping, fact: &SecRawFact) -> FundamentalObservation {
    FundamentalObservation {
        canonical_key: Some(mapping.canonical_key.to_string()),
        metric_key: mapping.metric_key.to_string(),
        metric_label: mapping.metric_label.to_string(),
        statement_type: mapping.statement_type.to_string(),
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
            "{} from canonical SEC concept {}:{} filed {}.",
            mapping.metric_label,
            fact.taxonomy,
            fact.concept_name,
            fact.filed
                .clone()
                .unwrap_or_else(|| "unknown date".to_string())
        )),
        concept_name: Some(fact.concept_name.clone()),
        form: fact.form.clone(),
        accession: fact.accession.clone(),
        quality: None,
        is_derived: false,
    }
}

fn select_latest_income_bundle(
    raw_facts: &[SecRawFact],
    mappings: &[CanonicalMapping],
) -> Option<IncomeBundle> {
    let revenue = ttm_series_for_metric("revenue_ttm", raw_facts, mappings, "revenue", "USD");
    let net_income =
        ttm_series_for_metric("net_income_ttm", raw_facts, mappings, "net_income", "USD");
    let gross_profit = ttm_series_for_metric(
        "gross_profit_ttm",
        raw_facts,
        mappings,
        "gross_profit",
        "USD",
    );
    let operating_income = ttm_series_for_metric(
        "operating_income_ttm",
        raw_facts,
        mappings,
        "operating_income",
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

fn append_bundle_observations(
    observations: &mut Vec<FundamentalObservation>,
    bundle: &IncomeBundle,
    currency: Option<&str>,
) {
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
        observations.push(FundamentalObservation {
            canonical_key: Some(ttm_canonical_key(metric.metric_key).to_string()),
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
            unit: currency.map(str::to_string),
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
        observations.push(FundamentalObservation {
            canonical_key: Some(metric_key.to_string()),
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

fn ttm_canonical_key(metric_key: &str) -> &'static str {
    match metric_key {
        "revenue_ttm" => "revenue",
        "net_income_ttm" => "net_income",
        "gross_profit_ttm" => "gross_profit",
        "operating_income_ttm" => "operating_income",
        _ => "derived_metric",
    }
}

fn ttm_series_for_metric(
    metric_key: &'static str,
    raw_facts: &[SecRawFact],
    mappings: &[CanonicalMapping],
    canonical_key: &str,
    unit_hint: &str,
) -> Vec<TtmMetric> {
    let facts = facts_for_canonical(raw_facts, mappings, canonical_key, unit_hint);
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

pub(crate) fn ttm_windows(metric_key: &'static str, facts: &[SecFact]) -> Vec<TtmMetric> {
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

fn facts_for_canonical(
    raw_facts: &[SecRawFact],
    mappings: &[CanonicalMapping],
    canonical_key: &str,
    unit_hint: &str,
) -> Vec<SecFact> {
    mappings
        .iter()
        .filter(|mapping| mapping.is_active && mapping.canonical_key == canonical_key)
        .flat_map(|mapping| {
            raw_facts
                .iter()
                .filter(move |fact| {
                    mapping_matches_fact(mapping, fact) && unit_matches(&fact.unit, unit_hint)
                })
                .map(SecFact::from)
        })
        .collect()
}

fn mapping_matches_fact(mapping: &CanonicalMapping, fact: &SecRawFact) -> bool {
    mapping.taxonomy == fact.taxonomy
        && mapping.concept_name == fact.concept_name
        && mapping.unit == fact.unit
}

fn latest_value(
    raw_facts: &[SecRawFact],
    mappings: &[CanonicalMapping],
    canonical_key: &str,
    unit_hint: &str,
) -> Option<f64> {
    latest_value_fact(raw_facts, mappings, canonical_key, unit_hint, None).map(|fact| fact.value)
}

fn latest_value_fact(
    raw_facts: &[SecRawFact],
    mappings: &[CanonicalMapping],
    canonical_key: &str,
    unit_hint: &str,
    prefer_period_end: Option<&str>,
) -> Option<SecFact> {
    facts_for_canonical(raw_facts, mappings, canonical_key, unit_hint)
        .into_iter()
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
    raw_facts: &[SecRawFact],
    mappings: &[CanonicalMapping],
    canonical_keys: &[&str],
    unit_hint: &str,
) -> Option<f64> {
    let values: Vec<f64> = canonical_keys
        .iter()
        .filter_map(|canonical_key| latest_value(raw_facts, mappings, canonical_key, unit_hint))
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

fn fact_period_type(fact: &SecRawFact) -> &'static str {
    let sec_fact = SecFact::from(fact);
    let Some(days) = duration_days(&sec_fact) else {
        return "instant";
    };

    if (60..=120).contains(&days) {
        "quarter"
    } else if is_quarterly_filing(fact) || (121..=299).contains(&days) {
        "ytd"
    } else if (300..=390).contains(&days) {
        "annual"
    } else {
        "instant"
    }
}

fn is_quarterly_filing(fact: &SecRawFact) -> bool {
    fact.form
        .as_deref()
        .is_some_and(|form| matches!(form, "10-Q" | "10-Q/A"))
        && fact
            .fiscal_period
            .as_deref()
            .is_some_and(|period| matches!(period, "Q2" | "Q3" | "Q4"))
}

fn duration_days(fact: &SecFact) -> Option<i64> {
    let start = NaiveDate::parse_from_str(fact.start.as_deref()?, "%Y-%m-%d").ok()?;
    let end = NaiveDate::parse_from_str(fact.end.as_deref()?, "%Y-%m-%d").ok()?;
    Some((end - start).num_days())
}

impl From<&SecRawFact> for SecFact {
    fn from(fact: &SecRawFact) -> Self {
        Self {
            concept: fact.concept_name.clone(),
            form: fact.form.clone(),
            start: fact.start.clone(),
            end: fact.end.clone(),
            filed: fact.filed.clone(),
            value: fact.value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{
        concept_catalog::ConceptCatalog, sec_facts_provider::extract_raw_facts_from_root,
    };
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

        let bundle = FundamentalDeriver::select_latest_baseline_bundle(&raw_facts, &mappings)
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
        let observations = FundamentalDeriver::build_observations(&raw_facts, &mappings);

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

    fn sec_fact_json(
        form: &str,
        start: &str,
        end: &str,
        filed: &str,
        value: f64,
    ) -> serde_json::Value {
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
