use crate::{
    services::fundamental_deriver::{classify_period as classify_fact_period, unit_matches},
    workspace::{CanonicalMapping, ConceptCatalogEntry, SecRawFact},
};
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default)]
pub struct ConceptCatalog;

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
pub struct CanonicalMappingCandidate {
    pub mapping: CanonicalMapping,
    pub score: i64,
    pub fact_count: i64,
    pub latest_period_end: Option<String>,
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

    pub fn is_known_canonical_key(canonical_key: &str) -> bool {
        CANONICAL_METRIC_SPECS
            .iter()
            .any(|spec| spec.canonical_key == canonical_key)
    }

    pub fn mapping_from_review_decision(
        canonical_key: &str,
        taxonomy: &str,
        concept_name: &str,
        unit: &str,
        confidence: &str,
        rationale: &str,
        selected_by: &str,
        raw_facts: &[SecRawFact],
    ) -> Option<CanonicalMapping> {
        let spec = CANONICAL_METRIC_SPECS
            .iter()
            .find(|spec| spec.canonical_key == canonical_key)?;
        let exists = raw_facts.iter().any(|fact| {
            fact.taxonomy == taxonomy && fact.concept_name == concept_name && fact.unit == unit
        });
        if !exists {
            return None;
        }
        Some(CanonicalMapping {
            canonical_key: canonical_key.to_string(),
            metric_key: spec.metric_key.to_string(),
            metric_label: spec.metric_label.to_string(),
            statement_type: spec.statement_type.to_string(),
            taxonomy: taxonomy.to_string(),
            concept_name: concept_name.to_string(),
            unit: unit.to_string(),
            confidence: confidence.to_string(),
            rationale: rationale.to_string(),
            selected_by: selected_by.to_string(),
            is_active: true,
        })
    }

    pub fn materialize_catalog_entries(raw_facts: &[SecRawFact]) -> Vec<ConceptCatalogEntry> {
        materialize_catalog_entries(raw_facts)
    }

    pub fn canonical_mapping_candidates(
        entries: &[ConceptCatalogEntry],
    ) -> Vec<CanonicalMappingCandidate> {
        canonical_mapping_candidates(entries)
    }
    pub fn classify_period(fact: &SecRawFact) -> &'static str {
        classify_fact_period(fact)
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
    "NotesPayableCurrent",
    "LongTermDebtAndFinanceLeaseObligationsCurrent",
];
const DEBT_NONCURRENT_CONCEPTS: &[&str] = &[
    "LongTermNotesAndLoans",
    "LongTermNotesPayable",
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

fn materialize_catalog_entries(raw_facts: &[SecRawFact]) -> Vec<ConceptCatalogEntry> {
    let mut by_concept: BTreeMap<(String, String, String), Vec<&SecRawFact>> = BTreeMap::new();
    for fact in raw_facts {
        by_concept
            .entry((
                fact.taxonomy.clone(),
                fact.concept_name.clone(),
                fact.unit.clone(),
            ))
            .or_default()
            .push(fact);
    }

    by_concept
        .into_iter()
        .map(|((taxonomy, concept_name, unit), facts)| {
            let mut period_shape_counts: BTreeMap<String, i64> = BTreeMap::new();
            let mut labels = BTreeSet::new();
            let mut descriptions = BTreeSet::new();
            let mut earliest_period_end: Option<String> = None;
            let mut latest_period_end: Option<String> = None;
            let mut latest_filed_at: Option<String> = None;
            let mut min_value: Option<f64> = None;
            let mut max_value: Option<f64> = None;

            for fact in &facts {
                *period_shape_counts
                    .entry(classify_fact_period(fact).to_string())
                    .or_insert(0) += 1;
                if let Some(label) = &fact.label {
                    labels.insert(label.clone());
                }
                if let Some(description) = &fact.description {
                    descriptions.insert(description.clone());
                }
                update_min_string(&mut earliest_period_end, fact.end.as_deref());
                update_max_string(&mut latest_period_end, fact.end.as_deref());
                update_max_string(&mut latest_filed_at, fact.filed.as_deref());
                update_min_number(&mut min_value, fact.value);
                update_max_number(&mut max_value, fact.value);
            }

            let fact_count = i64::try_from(facts.len()).unwrap_or(i64::MAX);
            let dominant_period_shape = dominant_period_shape(&period_shape_counts);
            let series_usability =
                classify_series_usability(fact_count, latest_period_end.as_deref());
            let plot_readiness =
                classify_plot_readiness(fact_count, &period_shape_counts, &dominant_period_shape);
            let label = labels.into_iter().next();
            let description = descriptions.into_iter().next();
            let narrative_tags =
                narrative_tags_for_concept(&concept_name, label.as_deref(), description.as_deref());

            ConceptCatalogEntry {
                taxonomy,
                concept_name,
                label,
                description,
                unit,
                fact_count,
                earliest_period_end,
                latest_period_end,
                latest_filed_at,
                min_value,
                max_value,
                period_shape_counts,
                dominant_period_shape,
                series_usability,
                plot_readiness,
                narrative_tags,
            }
        })
        .collect()
}

fn seed_canonical_mappings(raw_facts: &[SecRawFact]) -> Vec<CanonicalMapping> {
    let entries = materialize_catalog_entries(raw_facts);
    let mut selected = Vec::new();
    let candidates = canonical_mapping_candidates(&entries);
    for spec in CANONICAL_METRIC_SPECS {
        let Some(candidate) = candidates
            .iter()
            .filter(|candidate| candidate.mapping.canonical_key == spec.canonical_key)
            .max_by(|left, right| compare_mapping_candidates(left, right, spec))
        else {
            continue;
        };
        if candidate.score >= minimum_activation_score(spec) {
            selected.push(candidate.mapping.clone());
        }
    }
    selected
}

fn canonical_mapping_candidates(entries: &[ConceptCatalogEntry]) -> Vec<CanonicalMappingCandidate> {
    let mut candidates = Vec::new();
    for spec in CANONICAL_METRIC_SPECS {
        for entry in entries {
            let Some(score) = score_entry_for_metric(entry, spec) else {
                continue;
            };
            candidates.push(CanonicalMappingCandidate {
                mapping: CanonicalMapping {
                    canonical_key: spec.canonical_key.to_string(),
                    metric_key: spec.metric_key.to_string(),
                    metric_label: spec.metric_label.to_string(),
                    statement_type: spec.statement_type.to_string(),
                    taxonomy: entry.taxonomy.clone(),
                    concept_name: entry.concept_name.clone(),
                    unit: entry.unit.clone(),
                    confidence: confidence_for_score(score).to_string(),
                    rationale: candidate_rationale(entry, spec, score),
                    selected_by: "catalog_candidate_scoring".to_string(),
                    is_active: true,
                },
                score,
                fact_count: entry.fact_count,
                latest_period_end: entry.latest_period_end.clone(),
            });
        }
    }
    candidates
}

fn compare_mapping_candidates(
    left: &CanonicalMappingCandidate,
    right: &CanonicalMappingCandidate,
    spec: &CanonicalMetricSpec,
) -> std::cmp::Ordering {
    left.score
        .cmp(&right.score)
        .then_with(|| match spec.statement_type {
            "balance_sheet" => left
                .latest_period_end
                .as_deref()
                .unwrap_or("")
                .cmp(right.latest_period_end.as_deref().unwrap_or("")),
            _ => left.fact_count.cmp(&right.fact_count),
        })
        .then(left.fact_count.cmp(&right.fact_count))
}

fn score_entry_for_metric(entry: &ConceptCatalogEntry, spec: &CanonicalMetricSpec) -> Option<i64> {
    if !unit_matches(&entry.unit, spec.unit_hint) {
        return None;
    }

    let haystack = concept_text(entry);
    let mut score = 0;
    if entry.taxonomy == "us-gaap" {
        score += 8;
    }
    if spec
        .seed_concepts
        .iter()
        .any(|concept| *concept == entry.concept_name)
    {
        score += 120;
    }
    score += period_shape_score(entry, spec);
    score += history_score(entry.fact_count);
    score += metric_term_score(&haystack, spec.canonical_key);
    score -= metric_negative_score(&haystack, spec.canonical_key);
    if matches!(spec.canonical_key, "debt_current" | "debt_noncurrent") {
        score -= debt_balance_negative_score(entry);
    }

    (score > 0).then_some(score)
}

fn concept_text(entry: &ConceptCatalogEntry) -> String {
    format!(
        "{} {} {}",
        split_camel_case(&entry.concept_name),
        entry.label.as_deref().unwrap_or_default(),
        entry.description.as_deref().unwrap_or_default()
    )
    .to_lowercase()
}

fn metric_term_score(haystack: &str, canonical_key: &str) -> i64 {
    let terms = match canonical_key {
        "revenue" => &[
            "revenue",
            "sales",
            "contract with customer",
            "contracts with customers",
            "turnover",
        ][..],
        "net_income" => &["net income", "net loss", "profit loss", "earnings"][..],
        "gross_profit" => &["gross profit", "gross income"][..],
        "operating_income" => &[
            "operating income",
            "operating loss",
            "income from operations",
        ][..],
        "shares_outstanding" => &[
            "weighted average",
            "diluted shares",
            "shares outstanding",
            "common stock shares outstanding",
        ][..],
        "eps" => &["earnings per share", "diluted eps", "per share diluted"][..],
        "cash" => &[
            "cash and cash equivalents",
            "cash equivalents",
            "restricted cash",
        ][..],
        "debt_current" => &[
            "current debt",
            "notes payable, current",
            "notes payable current",
            "debt, current",
            "short term borrowings",
            "borrowings, current",
            "current finance lease",
        ][..],
        "debt_noncurrent" => &[
            "long term debt",
            "noncurrent debt",
            "notes payable, noncurrent",
            "notes payable noncurrent",
            "notes and loans, noncurrent",
            "notes and loans noncurrent",
            "borrowings, non-current",
            "noncurrent finance lease",
        ][..],
        _ => &[][..],
    };
    terms
        .iter()
        .filter(|term| haystack.contains(**term))
        .map(|term| {
            if term.split_whitespace().count() > 1 {
                28
            } else {
                16
            }
        })
        .sum()
}

fn metric_negative_score(haystack: &str, canonical_key: &str) -> i64 {
    let terms = match canonical_key {
        "revenue" => &[
            "remaining performance obligation",
            "backlog",
            "deferred revenue",
            "cost of revenue",
            "unearned revenue",
        ][..],
        "net_income" => &["comprehensive income", "income tax", "operating income"][..],
        "gross_profit" => &["gross profit margin", "revenue"][..],
        "operating_income" => &["operating margin", "cash flow"][..],
        "shares_outstanding" => &["authorized", "issued", "repurchased"][..],
        "eps" => &["basic", "anti dilutive"][..],
        "cash" => &["cash flow", "cash paid", "cash received"][..],
        "debt_current" => &[
            "noncurrent",
            "long term",
            "maturities",
            "repayments of principal",
            "payments due",
            "proceeds from",
            "repayments of",
        ][..],
        "debt_noncurrent" => &[
            "current debt",
            "short term",
            "maturities",
            "repayments of principal",
            "payments due",
            "proceeds from",
            "repayments of",
        ][..],
        _ => &[][..],
    };
    terms
        .iter()
        .filter(|term| haystack.contains(**term))
        .map(|_| 34)
        .sum()
}

fn debt_balance_negative_score(entry: &ConceptCatalogEntry) -> i64 {
    let name = entry.concept_name.as_str();
    let haystack = concept_text(entry);
    let mut penalty = 0i64;

    if name.contains("Maturities")
        || name.contains("RepaymentsOfPrincipal")
        || name.contains("PaymentsDue")
        || name.starts_with("ProceedsFrom")
        || name.starts_with("RepaymentsOf")
    {
        penalty += 100;
    }

    if name.contains("AvailableForSale") && name.contains("Debt") {
        penalty += 80;
    }

    if matches!(entry.dominant_period_shape.as_str(), "ytd" | "annual") {
        penalty += 40;
    }

    for term in [
        "maturities",
        "repayments of principal",
        "payments due",
        "proceeds from issuance",
        "repayments of debt",
    ] {
        if haystack.contains(term) {
            penalty += 50;
        }
    }

    penalty
}

fn period_shape_score(entry: &ConceptCatalogEntry, spec: &CanonicalMetricSpec) -> i64 {
    match spec.statement_type {
        "income_statement" if entry.period_shape_counts.contains_key("quarter") => 16,
        "income_statement" if entry.period_shape_counts.contains_key("annual") => 10,
        "balance_sheet" if entry.period_shape_counts.contains_key("instant") => 16,
        _ => 0,
    }
}

fn history_score(fact_count: i64) -> i64 {
    match fact_count {
        20.. => 12,
        8..=19 => 8,
        3..=7 => 4,
        _ => 0,
    }
}

fn minimum_activation_score(spec: &CanonicalMetricSpec) -> i64 {
    if spec.seed_concepts.is_empty() {
        55
    } else {
        60
    }
}

fn confidence_for_score(score: i64) -> &'static str {
    match score {
        135.. => "high",
        80..=134 => "medium",
        _ => "low",
    }
}

fn candidate_rationale(
    entry: &ConceptCatalogEntry,
    spec: &CanonicalMetricSpec,
    score: i64,
) -> String {
    let alias_note = if spec
        .seed_concepts
        .iter()
        .any(|concept| *concept == entry.concept_name)
    {
        "matched a known alias prior"
    } else {
        "selected from full concept inventory"
    };
    format!(
        "{alias_note} for canonical metric '{}' with score {score}. Unit '{}', dominant period shape '{}', and {} facts were considered; LLM review can confirm, replace, or mark unavailable.",
        spec.canonical_key, entry.unit, entry.dominant_period_shape, entry.fact_count
    )
}

fn dominant_period_shape(period_shape_counts: &BTreeMap<String, i64>) -> String {
    period_shape_counts
        .iter()
        .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
        .map(|(shape, _)| shape.clone())
        .unwrap_or_else(|| "unknown".to_string())
}

fn classify_series_usability(fact_count: i64, latest_period_end: Option<&str>) -> String {
    if latest_period_end.is_none() {
        return "event_point".to_string();
    }
    match fact_count {
        20.. => "long_history",
        8..=19 => "medium_history",
        3..=7 => "sparse",
        _ => "event_point",
    }
    .to_string()
}

fn classify_plot_readiness(
    fact_count: i64,
    period_shape_counts: &BTreeMap<String, i64>,
    dominant_period_shape: &str,
) -> String {
    let dominant_count = period_shape_counts
        .get(dominant_period_shape)
        .copied()
        .unwrap_or_default();
    if fact_count >= 8 && dominant_count * 2 >= fact_count {
        "plot_ready".to_string()
    } else if fact_count >= 3 {
        "limited".to_string()
    } else {
        "not_plot_ready".to_string()
    }
}

fn narrative_tags_for_concept(
    concept_name: &str,
    label: Option<&str>,
    description: Option<&str>,
) -> Vec<String> {
    let text = format!(
        "{} {} {}",
        split_camel_case(concept_name),
        label.unwrap_or_default(),
        description.unwrap_or_default()
    )
    .to_lowercase();
    let mut tags = Vec::new();
    for (tag, terms) in [
        (
            "backlog",
            &[
                "backlog",
                "remaining performance obligation",
                "unsatisfied performance",
            ][..],
        ),
        (
            "conversion",
            &["deferred revenue", "contract liability", "unearned revenue"][..],
        ),
        (
            "capex",
            &[
                "capital expenditure",
                "property plant equipment",
                "construction in progress",
            ][..],
        ),
        ("lease", &["lease", "right of use"][..]),
        (
            "purchase_obligation",
            &["purchase obligation", "commitment"][..],
        ),
        (
            "debt",
            &["debt", "borrowings", "notes payable", "finance lease"][..],
        ),
        ("interest", &["interest expense", "interest income"][..]),
        (
            "working_capital",
            &["inventory", "receivable", "payable", "working capital"][..],
        ),
        (
            "capital_return",
            &["dividend", "share repurchase", "treasury stock"][..],
        ),
        (
            "dilution",
            &["stock compensation", "diluted shares", "option", "rsu"][..],
        ),
        (
            "margin",
            &["gross profit", "operating income", "margin"][..],
        ),
        ("tax", &["income tax", "deferred tax", "tax expense"][..]),
    ] {
        if terms.iter().any(|term| text.contains(*term)) {
            tags.push(tag.to_string());
        }
    }
    tags
}

fn split_camel_case(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 8);
    for (index, character) in value.chars().enumerate() {
        if index > 0 && character.is_ascii_uppercase() {
            output.push(' ');
        }
        output.push(character);
    }
    output
}

fn update_min_string(target: &mut Option<String>, candidate: Option<&str>) {
    let Some(candidate) = candidate else {
        return;
    };
    if target
        .as_deref()
        .is_none_or(|existing| candidate < existing)
    {
        *target = Some(candidate.to_string());
    }
}

fn update_max_string(target: &mut Option<String>, candidate: Option<&str>) {
    let Some(candidate) = candidate else {
        return;
    };
    if target
        .as_deref()
        .is_none_or(|existing| candidate > existing)
    {
        *target = Some(candidate.to_string());
    }
}

fn update_min_number(target: &mut Option<f64>, candidate: f64) {
    if target.is_none_or(|existing| candidate < existing) {
        *target = Some(candidate);
    }
}

fn update_max_number(target: &mut Option<f64>, candidate: f64) {
    if target.is_none_or(|existing| candidate > existing) {
        *target = Some(candidate);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::SecRawFact;

    fn debt_fact(concept_name: &str, label: &str, period_end: &str, value: f64) -> SecRawFact {
        SecRawFact {
            taxonomy: "us-gaap".to_string(),
            concept_name: concept_name.to_string(),
            label: Some(label.to_string()),
            description: None,
            unit: "USD".to_string(),
            form: Some("10-K".to_string()),
            start: None,
            end: Some(period_end.to_string()),
            filed: Some("2026-03-11".to_string()),
            fiscal_year: None,
            fiscal_period: None,
            accession: None,
            frame: None,
            value,
            raw_json: "{}".to_string(),
            fetched_at: "2026-06-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn debt_noncurrent_prefers_notes_payable_over_maturity_schedule() {
        let facts = vec![
            debt_fact(
                "LongTermDebtMaturitiesRepaymentsOfPrincipalInYearThree",
                "Long-Term Debt, Maturity, Year Three",
                "2025-05-31",
                10.145e9,
            ),
            debt_fact(
                "LongTermNotesAndLoans",
                "Notes and Loans, Noncurrent",
                "2026-02-28",
                124.718e9,
            ),
        ];

        let mappings = ConceptCatalog::seed_canonical_mappings(&facts);
        let noncurrent = mappings
            .iter()
            .find(|mapping| mapping.canonical_key == "debt_noncurrent")
            .expect("debt_noncurrent mapping");

        assert_eq!(noncurrent.concept_name, "LongTermNotesAndLoans");
        assert_eq!(noncurrent.confidence, "high");
    }

    #[test]
    fn debt_current_prefers_fresher_notes_payable_current() {
        let facts = vec![
            debt_fact("DebtCurrent", "Debt, Current", "2025-05-31", 7.271e9),
            debt_fact(
                "NotesPayableCurrent",
                "Notes Payable, Current",
                "2026-02-28",
                9.887e9,
            ),
        ];

        let mappings = ConceptCatalog::seed_canonical_mappings(&facts);
        let current = mappings
            .iter()
            .find(|mapping| mapping.canonical_key == "debt_current")
            .expect("debt_current mapping");

        assert_eq!(current.concept_name, "NotesPayableCurrent");
        assert_eq!(current.confidence, "high");
    }

    #[test]
    fn debt_noncurrent_prefers_fresher_notes_and_loans_over_notes_payable() {
        let facts = vec![
            debt_fact(
                "LongTermNotesPayable",
                "Notes Payable, Noncurrent",
                "2025-05-31",
                85.297e9,
            ),
            debt_fact(
                "LongTermNotesAndLoans",
                "Notes and Loans, Noncurrent",
                "2026-02-28",
                124.718e9,
            ),
        ];

        let mappings = ConceptCatalog::seed_canonical_mappings(&facts);
        let noncurrent = mappings
            .iter()
            .find(|mapping| mapping.canonical_key == "debt_noncurrent")
            .expect("debt_noncurrent mapping");

        assert_eq!(noncurrent.concept_name, "LongTermNotesAndLoans");
    }

    #[test]
    fn maturity_schedule_concepts_are_not_debt_balance_candidates() {
        let entries = vec![ConceptCatalogEntry {
            taxonomy: "us-gaap".to_string(),
            concept_name: "LongTermDebtMaturitiesRepaymentsOfPrincipalInYearThree".to_string(),
            label: Some("Long-Term Debt, Maturity, Year Three".to_string()),
            description: None,
            unit: "USD".to_string(),
            fact_count: 18,
            earliest_period_end: Some("2010-08-31".to_string()),
            latest_period_end: Some("2025-05-31".to_string()),
            latest_filed_at: Some("2025-06-18".to_string()),
            min_value: Some(0.0),
            max_value: Some(10.145e9),
            period_shape_counts: BTreeMap::from([("instant".to_string(), 18)]),
            dominant_period_shape: "instant".to_string(),
            series_usability: "usable".to_string(),
            plot_readiness: "ready".to_string(),
            narrative_tags: Vec::new(),
        }];

        let candidates = ConceptCatalog::canonical_mapping_candidates(&entries);
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.mapping.canonical_key != "debt_noncurrent"),
            "maturity schedule concepts should not score as debt_noncurrent candidates"
        );
    }
}
