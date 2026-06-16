/// Tables and metadata available when the narrative researcher runs (post catalog).
pub fn workspace_schema_hint() -> &'static str {
    r#"Workspace SQLite state at this stage:
- Alpha Vantage raw facts drive canonical mappings, fundamental_observations, and fundamentals. SEC raw facts and concept catalog are optional niche inputs for company-specific metrics.
- sources, claims, narrative_map, and narrative_map_items may already contain durable board state from prior runs — read the Existing narrative board section in your prompt before writing.
- Never delete or wipe tables; append new evidence and update existing narrative sides/sections as needed.

Table tiers (use in this order):
1. Context — stock_info(ticker, company_name, currency, sector, industry), run_metadata(ticker, created_at, financial_fetch_status, financial_fetch_error), data_gaps(gap_key, description, status)
2. Fundamentals — fundamentals(metric_key, metric_value, period, source_note): headline TTM rows (may lag latest quarter)
3. Observations — fundamental_observations(metric_key, period_end, period_type, filed_at, metric_value, ...): no `period` column (that is on `fundamentals` only). Income-statement flow metrics use period suffixes in metric_key (e.g. revenue_quarter, revenue_ytd, revenue_annual); default to *_quarter for time-series work. Sort by period_end DESC, not period.
4. Filing freshness — av_raw_facts(fetched_at, period_end, field_name, metric_value): check MAX(fetched_at) and MAX(period_end) before trusting headline metrics. sec_raw_facts is optional for niche SEC-only concepts.
5. Narrative board — sources(id, title, url, source_type, published_at, ...), claims(id, claim, source_id, side, metric, notes, ...), narrative_map, narrative_map_items
6. Catalog signal — concept_catalog_entries(concept_name, label, narrative_tags, latest_period_end, latest_filed_at, series_usability) for SEC niche metrics only
7. Avoid overwriting — canonical_metric_mappings and fundamental_observations are inputs, not outputs for this agent

LIMIT rules:
- Always ORDER BY before LIMIT.
- Catalog narrative search: LIMIT 15–20, ORDER BY latest_period_end DESC.
- Results truncate at 200 rows per workspace_sql call."#
}

/// Golden-path workflow using incremental capture tools (not one giant JSON submit).
pub fn narrative_research_golden_path() -> &'static str {
    r#"Golden path — review existing board state first, then use incremental capture tools (web search between SQL rounds when filing lag or gaps exist):

Phase 0 — Review existing board (prompt + workspace_sql):
Read the Existing narrative board JSON in your prompt. Note existing source ids, claims, narrative sides, cruxes, section drafts, and any headline metrics tied to old fiscal periods.
Identify claims that a newer quarter supersedes; plan corrected replacements (use capture_claims.notes to reference superseded claim ids).

Phase 0.5 — Freshness check (workspace_sql, mandatory first round):
SELECT ticker, created_at, financial_fetch_status, financial_fetch_error FROM run_metadata;
SELECT gap_key, description, status FROM data_gaps WHERE status = 'open';
SELECT MAX(fetched_at) AS max_av_fetched_at, MAX(period_end) AS max_av_period_end FROM av_raw_facts;
SELECT MAX(period_end) AS max_obs_period_end
  FROM fundamental_observations WHERE metric_key = 'revenue_quarter';
SELECT MAX(filed_at) AS max_sec_filed_at, MAX(period_end) AS max_sec_period_end FROM sec_raw_facts;
SELECT MAX(latest_period_end) AS max_catalog_period_end, MAX(latest_filed_at) AS max_catalog_filed_at
  FROM concept_catalog_entries;
If max_av_fetched_at or max_obs_period_end lags the current catalyst (recent earnings, open ingestion gaps, or run_metadata created_at much later than max_av_fetched_at), you MUST web-search the latest official earnings release / 8-K and capture it before finalize. Log capture_research_gap when workspace ingestion still trails the market.

Phase 1 — Orient on latest persisted numbers (workspace_sql):
SELECT ticker, company_name, currency, sector, industry FROM stock_info;
SELECT metric_key, metric_value, period, source_note FROM fundamentals
  WHERE metric_key IN ('revenue_ttm','net_income_ttm','eps_ttm','current_price','market_cap','total_debt')
  ORDER BY metric_key;
SELECT metric_key, metric_value, period_end, filed_at, fiscal_year, fiscal_period
  FROM fundamental_observations
  WHERE metric_key IN (
    'revenue_quarter','net_income_quarter','eps_quarter',
    'cash','debt_current','debt_noncurrent'
  )
  ORDER BY period_end DESC, metric_key
  LIMIT 24;
SELECT concept_name, label, metric_value, period_end, filed_at
  FROM sec_raw_facts
  WHERE concept_name IN ('RevenueRemainingPerformanceObligation')
  ORDER BY period_end DESC, filed_at DESC
  LIMIT 4;
Prefer observation and av_raw_facts rows over stale fundamentals TTM when building claims for the latest quarter.

Phase 2 — Source discovery (web search, required when Phase 0.5 shows filing lag):
Search for the latest-quarter official press release, 8-K/exhibit, earnings transcript, and credible bull/bear commentary.
Call capture_sources for NEW sources (duplicate url/title returns existing id).
When workspace AV facts lag, capture at least one Official company source or Filing from the latest reported quarter before finalize.

Phase 3 — Claims (capture_claims):
Add claims for the current catalyst quarter. When replacing stale headline metrics, add corrected claims and note superseded claim ids in notes.
Reuse source_id from existing or newly captured sources. Link metric to workspace keys when possible.

Phase 4–5 — Narrative sides (capture_narrative_side):
Use capture_narrative_side to UPDATE bull, bear, dominant, consensus when you have better text — especially after a new earnings quarter.

Phase 6–7 — Agreements and cruxes (capture_narrative_items):
Add agreement and crux items for the live debate (minimum gates require several of each). Duplicate bodies are skipped automatically.

Phase 8 — Orientation + sections:
capture_orientation and capture_section should reflect the current catalyst quarter, not only persisted workspace TTM.

Phase 9 — Gaps (required when filing lag persists):
capture_research_gap for unresolved questions and for quarters not yet in fundamental_observations.

Phase 10 — Finalize:
Call finalize_narrative_research when gates should pass (≥10 claims, ≥5 cruxes, ≥2 bear claims, ≥1 agreement, ≥5 sources, plus narrative sides and sections). Fix validation errors, then finalize again."#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_hint_documents_existing_board_state() {
        let hint = workspace_schema_hint();
        assert!(hint.contains("concept_catalog_entries"));
        assert!(hint.contains("may already contain"));
        assert!(hint.contains("Never delete"));
        assert!(hint.contains("period_end"));
        assert!(hint.contains("data_gaps"));
        assert!(hint.contains("av_raw_facts"));
        assert!(hint.contains("sec_raw_facts"));
    }

    #[test]
    fn golden_path_reviews_existing_board_first() {
        let path = narrative_research_golden_path();
        assert!(path.contains("Review existing board"));
        assert!(path.contains("Phase 0.5"));
        assert!(path.contains("max_av_fetched_at"));
        assert!(path.contains("max_sec_filed_at"));
        assert!(path.contains("fundamental_observations"));
        assert!(path.contains("superseded"));
        assert!(path.contains("capture_narrative_side"));
        assert!(path.contains("finalize_narrative_research"));
    }
}
