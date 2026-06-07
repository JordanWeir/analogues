use crate::tasks::init_workspace::{
    CanonicalMapping, FinancialSnapshot, FundamentalObservation, SecRawFact,
};
use chrono::NaiveDate;
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct ConceptCatalog;

#[derive(Debug, Clone)]
pub(crate) struct SecFact {
    pub(crate) concept: String,
    pub(crate) form: Option<String>,
    pub(crate) start: Option<String>,
    pub(crate) end: Option<String>,
    pub(crate) filed: Option<String>,
    pub(crate) value: f64,
}

#[derive(Debug, Clone, Copy)]
struct CanonicalMetricSpec {
    canonical_key: &'static str,
    metric_key: &'static str,
    metric_label: &'static str,
    statement_type: &'static str,
    unit_hint: &'static str,
    seed_concepts: &'static [&'static str],
    display_order: i64,
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

impl ConceptCatalog {
    pub async fn seed_canonical_definitions(
        db: &sea_orm::DatabaseConnection,
        created_at: &str,
    ) -> Result<()> {
        for spec in CANONICAL_METRIC_SPECS {
            execute_sql(
                db,
                &format!(
                    "INSERT INTO canonical_metric_definitions (
                        canonical_key, metric_key, metric_label, statement_type, unit_hint,
                        display_order, created_at
                    ) VALUES (
                        '{}', '{}', '{}', '{}', '{}', {}, '{}'
                    )
                    ON CONFLICT(canonical_key) DO UPDATE SET
                        metric_key = excluded.metric_key,
                        metric_label = excluded.metric_label,
                        statement_type = excluded.statement_type,
                        unit_hint = excluded.unit_hint,
                        display_order = excluded.display_order",
                    sql_quote(spec.canonical_key),
                    sql_quote(spec.metric_key),
                    sql_quote(spec.metric_label),
                    sql_quote(spec.statement_type),
                    sql_quote(spec.unit_hint),
                    spec.display_order,
                    sql_quote(created_at),
                ),
            )
            .await?;
        }

        Ok(())
    }

    pub fn seed_canonical_mappings(raw_facts: &[SecRawFact]) -> Vec<CanonicalMapping> {
        seed_canonical_mappings(raw_facts)
    }

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

    pub(crate) fn apply_income_bundle(snapshot: &mut FinancialSnapshot, bundle: &IncomeBundle) {
        append_bundle_observations(snapshot, bundle);
        snapshot.revenue_ttm = bundle.revenue.as_ref().map(|metric| metric.value);
        snapshot.net_income_ttm = bundle.net_income.as_ref().map(|metric| metric.value);
        snapshot.gross_profit_ttm = bundle.gross_profit.as_ref().map(|metric| metric.value);
        snapshot.operating_income_ttm = bundle.operating_income.as_ref().map(|metric| metric.value);
        snapshot.gross_margin = ratio(snapshot.gross_profit_ttm, snapshot.revenue_ttm);
        snapshot.operating_margin = ratio(snapshot.operating_income_ttm, snapshot.revenue_ttm);
        snapshot.net_margin = ratio(snapshot.net_income_ttm, snapshot.revenue_ttm);
        snapshot.fundamental_period_end = Some(bundle.period_end.clone());
        snapshot.source_notes.extend(bundle.source_notes.clone());
        snapshot.quality_flags.extend(bundle.quality_flags.clone());
    }

    pub fn classify_period(fact: &SecRawFact) -> &'static str {
        fact_period_type(fact)
    }
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
const CANONICAL_METRIC_SPECS: &[CanonicalMetricSpec] = &[
    CanonicalMetricSpec {
        canonical_key: "revenue",
        metric_key: "revenue",
        metric_label: "Revenue",
        statement_type: "income_statement",
        unit_hint: "USD",
        seed_concepts: REVENUE_CONCEPTS,
        display_order: 10,
    },
    CanonicalMetricSpec {
        canonical_key: "net_income",
        metric_key: "net_income",
        metric_label: "Net income",
        statement_type: "income_statement",
        unit_hint: "USD",
        seed_concepts: NET_INCOME_CONCEPTS,
        display_order: 20,
    },
    CanonicalMetricSpec {
        canonical_key: "gross_profit",
        metric_key: "gross_profit",
        metric_label: "Gross profit",
        statement_type: "income_statement",
        unit_hint: "USD",
        seed_concepts: GROSS_PROFIT_CONCEPTS,
        display_order: 30,
    },
    CanonicalMetricSpec {
        canonical_key: "operating_income",
        metric_key: "operating_income",
        metric_label: "Operating income",
        statement_type: "income_statement",
        unit_hint: "USD",
        seed_concepts: OPERATING_INCOME_CONCEPTS,
        display_order: 40,
    },
    CanonicalMetricSpec {
        canonical_key: "shares_outstanding",
        metric_key: "diluted_shares",
        metric_label: "Diluted shares",
        statement_type: "income_statement",
        unit_hint: "shares",
        seed_concepts: DILUTED_SHARES_CONCEPTS,
        display_order: 50,
    },
    CanonicalMetricSpec {
        canonical_key: "eps",
        metric_key: "eps",
        metric_label: "Diluted EPS",
        statement_type: "income_statement",
        unit_hint: "USD/shares",
        seed_concepts: EPS_CONCEPTS,
        display_order: 60,
    },
    CanonicalMetricSpec {
        canonical_key: "cash",
        metric_key: "cash",
        metric_label: "Cash and equivalents",
        statement_type: "balance_sheet",
        unit_hint: "USD",
        seed_concepts: CASH_CONCEPTS,
        display_order: 70,
    },
    CanonicalMetricSpec {
        canonical_key: "debt_current",
        metric_key: "debt_current",
        metric_label: "Current debt",
        statement_type: "balance_sheet",
        unit_hint: "USD",
        seed_concepts: DEBT_CURRENT_CONCEPTS,
        display_order: 80,
    },
    CanonicalMetricSpec {
        canonical_key: "debt_noncurrent",
        metric_key: "debt_noncurrent",
        metric_label: "Noncurrent debt",
        statement_type: "balance_sheet",
        unit_hint: "USD",
        seed_concepts: DEBT_NONCURRENT_CONCEPTS,
        display_order: 90,
    },
];

fn seed_canonical_mappings(raw_facts: &[SecRawFact]) -> Vec<CanonicalMapping> {
    let mut mappings = Vec::new();
    for spec in CANONICAL_METRIC_SPECS {
        for concept_name in spec.seed_concepts {
            for unit in raw_facts
                .iter()
                .filter(|fact| {
                    fact.taxonomy == "us-gaap"
                        && fact.concept_name == *concept_name
                        && unit_matches(&fact.unit, spec.unit_hint)
                })
                .map(|fact| fact.unit.clone())
            {
                if mappings.iter().any(|mapping: &CanonicalMapping| {
                    mapping.canonical_key == spec.canonical_key
                        && mapping.taxonomy == "us-gaap"
                        && mapping.concept_name == *concept_name
                        && mapping.unit == unit
                }) {
                    continue;
                }
                mappings.push(CanonicalMapping {
                    canonical_key: spec.canonical_key,
                    metric_key: spec.metric_key,
                    metric_label: spec.metric_label,
                    statement_type: spec.statement_type,
                    taxonomy: "us-gaap".to_string(),
                    concept_name: (*concept_name).to_string(),
                    unit,
                    confidence: "medium",
                    rationale: format!(
                        "Seeded from known SEC concept candidate for canonical metric '{}'. Agent review should confirm or replace this mapping.",
                        spec.canonical_key
                    ),
                    selected_by: "heuristic_seed",
                    is_active: true,
                });
            }
        }
    }
    mappings
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

fn unit_matches(unit: &str, unit_hint: &str) -> bool {
    unit.to_lowercase().contains(&unit_hint.to_lowercase())
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
