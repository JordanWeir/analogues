/// Tables and metadata available at canonical-mapping time (ingest + derived catalog only).
pub fn workspace_schema_hint() -> &'static str {
    r#"Workspace SQLite state at this stage:
- Raw SEC facts and a derived concept catalog are loaded.
- canonical_metric_mappings, fundamental_observations, fundamentals, and concept_review_decisions are NOT populated yet — you produce the mapping decisions.

Table tiers (use in this order):
1. Targets — canonical_metric_definitions(canonical_key, metric_key, metric_label, statement_type, unit_hint, display_order): product metrics you must map.
2. Primary search — concept_catalog_entries(taxonomy, concept_name, label, description, unit, fact_count, earliest_period_end, latest_period_end, latest_filed_at, min_value, max_value, dominant_period_shape, series_usability, plot_readiness, narrative_tags): pre-classified inventory. Search here FIRST. dominant_period_shape and series_usability are already computed — use them.
3. Confirmation — sec_raw_facts(taxonomy, concept_name, label, description, unit, form, period_start, period_end, filed_at, fiscal_year, fiscal_period, accession, frame, metric_value, fetched_at): spot-check latest values and period semantics for concepts you will promote.
4. Context — run_metadata(ticker, financial_fetch_status, created_at), stock_info(ticker, company_name, currency).
5. Avoid as primary search — raw_fact_metric_catalog view duplicates concept_catalog_entries without usability metadata. Do not SELECT * from it without filters.

LIMIT rules:
- Always ORDER BY before LIMIT.
- Catalog candidate search: LIMIT 15–20, ORDER BY latest_period_end DESC, fact_count DESC.
- Latest fact for one concept: LIMIT 10, ORDER BY period_end DESC, filed_at DESC — inspect duplicates; do not assume row 1 without checking fiscal_period and period_start.
- Never dump the full catalog (500+ rows) — filter by unit, freshness, and metric-specific terms.
- Results truncate at 200 rows per call; stay well under that.

Selection rules (common failure modes to avoid):
- Balance sheet (cash, debt): require a recent latest_period_end; prefer dominant_period_shape = instant (period_start IS NULL in raw facts). Reject maturity schedules, repayments, proceeds, fair-value, and rollforward concepts for balance mappings.
- Income flows (revenue, net income, operating income): accept duration shapes; note quarter vs YTD vs annual from dominant_period_shape, fiscal_period, form, and period_start.
- Shares vs EPS: do not map shares_outstanding to point-in-time CommonStockSharesOutstanding when the product intent is diluted weighted-average shares for EPS math.
- Stale beats popular: a high fact_count concept with an old latest_period_end loses to a lower-count but current concept.
- Debt: if no single fresh noncurrent balance tag exists, use decision_type calculated_from_components and list inputs in rationale.
- Duplicate facts at the same period_end: prefer the row with the latest filed_at; warn if YTD and quarter values coexist."#
}

/// Golden-path workflow and prebuilt SQL recipes for the Fundamental Catalog Manager.
pub fn concept_review_golden_path() -> &'static str {
    r#"Golden path (target ≤6 workspace_sql rounds; batch independent queries in parallel when possible):

Phase 0 — Orient (one round, parallel SQL):
-- Metrics to map
SELECT canonical_key, metric_label, statement_type, unit_hint, display_order
FROM canonical_metric_definitions ORDER BY display_order;
-- Scope
SELECT COUNT(*) AS fact_count FROM sec_raw_facts;
SELECT COUNT(*) AS concept_count FROM concept_catalog_entries;
-- Company
SELECT ticker, financial_fetch_status FROM run_metadata;
SELECT ticker, company_name, currency FROM stock_info;

Phase 1 — Shortlist candidates per canonical_key (one or two rounds, parallel SQL):
Use concept_catalog_entries as the primary search surface. Template (substitute unit and term filters per metric):
SELECT taxonomy, concept_name, label, description, unit,
       fact_count, latest_period_end, latest_filed_at,
       dominant_period_shape, series_usability, narrative_tags
FROM concept_catalog_entries
WHERE unit = 'USD'
  AND latest_period_end IS NOT NULL
  AND series_usability NOT IN ('stale', 'event_point')
  AND concept_name NOT LIKE '%Maturit%'
  AND concept_name NOT LIKE '%Repayment%'
  AND concept_name NOT LIKE '%Proceeds%'
  AND (concept_name LIKE '%Revenue%' OR label LIKE '%revenue%')
ORDER BY latest_period_end DESC, fact_count DESC
LIMIT 15;

Metric-specific catalog filters (combine with unit_hint from canonical_metric_definitions):
- revenue: Revenue, Revenues, ContractWithCustomer, Sales
- net_income: NetIncomeLoss
- gross_profit: GrossProfit (reject if latest_period_end is stale)
- operating_income: OperatingIncomeLoss
- shares_outstanding: WeightedAverage, DilutedShares (unit shares)
- eps: EarningsPerShareDiluted (unit USD/shares)
- cash: CashAndCashEquivalents (instant balance)
- debt_current / debt_noncurrent: NotesPayable, LongTermDebt, DebtCurrent, DebtNoncurrent — exclude maturity/repayment tags

Phase 2 — Confirm top picks (one round, parallel SQL):
SELECT concept_name, metric_value, period_end, period_start,
       fiscal_year, fiscal_period, form, filed_at, unit
FROM sec_raw_facts
WHERE taxonomy = 'us-gaap' AND concept_name = :concept AND unit = :unit
ORDER BY period_end DESC, filed_at DESC
LIMIT 10;

Balance-sheet snapshot at latest period (batch debt/cash candidates):
SELECT concept_name, metric_value, period_end, filed_at, fiscal_period
FROM sec_raw_facts
WHERE period_end = :latest_balance_sheet_date
  AND concept_name IN (:candidate_list)
ORDER BY concept_name, filed_at DESC;

Phase 3 — Decide (no SQL):
For each canonical_key emit direct_mapping, calculated_from_components, unavailable, or review_required.
Include warnings for period mismatch, stale concepts, or ambiguous duplicates.

Phase 4 — Web validation:
Validate latest reported values for revenue, net income, EPS, and debt components against public filings or investor materials.

Phase 5 — Submit:
Call submit_concept_review with decisions and supporting_metrics. Fix validation errors and resubmit if needed."#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_hint_documents_ingest_stage_tables() {
        let hint = workspace_schema_hint();
        assert!(hint.contains("canonical_metric_definitions"));
        assert!(hint.contains("concept_catalog_entries"));
        assert!(hint.contains("NOT populated yet"));
        assert!(hint.contains("latest_period_end"));
    }

    #[test]
    fn golden_path_includes_orient_and_shortlist_recipes() {
        let path = concept_review_golden_path();
        assert!(path.contains("Phase 0"));
        assert!(path.contains("canonical_metric_definitions"));
        assert!(path.contains("latest_period_end DESC"));
        assert!(path.contains("calculated_from_components"));
    }
}
