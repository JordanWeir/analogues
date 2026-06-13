/// Tables and metadata available when the narrative researcher runs (post catalog).
pub fn workspace_schema_hint() -> &'static str {
    r#"Workspace SQLite state at this stage:
- SEC raw facts, concept catalog, canonical mappings, and starter fundamentals are loaded.
- sources, claims, narrative_map, and narrative_map_items may already contain durable board state from prior runs — read the Existing narrative board section in your prompt before writing.
- Never delete or wipe tables; append new evidence and update existing narrative sides/sections as needed.

Table tiers (use in this order):
1. Context — stock_info(ticker, company_name, currency, sector, industry), run_metadata(ticker, created_at, financial_fetch_status)
2. Fundamentals — fundamentals(metric_key, metric_value, period, source_note): headline revenue, EPS, price, margins
3. Observations — fundamental_observations(metric_key, period_end, period_type, ...): no `period` column (that is on `fundamentals` only). Income-statement flow metrics use period suffixes in metric_key (e.g. revenue_quarter, revenue_ytd, revenue_annual); default to *_quarter for time-series work. Sort by period_end DESC, not period.
4. Narrative board — sources(id, title, url, ...), claims(id, claim, source_id, side, ...), narrative_map, narrative_map_items
5. Catalog signal — concept_catalog_entries(concept_name, label, narrative_tags, latest_period_end, series_usability)
6. Avoid overwriting — canonical_metric_mappings and fundamental_observations are inputs, not outputs for this agent

LIMIT rules:
- Always ORDER BY before LIMIT.
- Catalog narrative search: LIMIT 15–20, ORDER BY latest_period_end DESC.
- Results truncate at 200 rows per workspace_sql call."#
}

/// Golden-path workflow using incremental capture tools (not one giant JSON submit).
pub fn narrative_research_golden_path() -> &'static str {
    r#"Golden path — review existing board state first, then use incremental capture tools (web search between SQL rounds as needed):

Phase 0 — Review existing board (prompt + workspace_sql):
Read the Existing narrative board JSON in your prompt. Note existing source ids, claims, narrative sides, cruxes, and section drafts.
Only research and capture what is missing, stale, or needs correction.

Phase 1 — Orient (workspace_sql, one round if fundamentals need refresh):
SELECT ticker, company_name, currency, sector, industry FROM stock_info;
SELECT metric_key, metric_value, period, source_note FROM fundamentals
  WHERE metric_key IN ('revenue_ttm','net_income_ttm','eps_ttm','current_price','market_cap')
  ORDER BY metric_key;

Phase 2 — Source discovery (web search, as needed):
Search for recent filings, transcripts, and credible bull/bear commentary when gaps exist.
Call capture_sources only for NEW sources (duplicate url/title returns existing id).

Phase 3 — Claims (capture_claims, as needed):
Add only claims not already on the board. Reuse source_id from existing or newly captured sources.

Phase 4–5 — Narrative sides (capture_narrative_side):
Use capture_narrative_side to UPDATE bull, bear, dominant, consensus when you have better text.
Skip sides that are already strong unless new evidence warrants revision.

Phase 6–7 — Agreements and cruxes (capture_narrative_items):
Add only new agreement/crux items. Duplicate bodies are skipped automatically.

Phase 8 — Orientation + sections:
capture_orientation and capture_section update existing drafts when needed.

Phase 9 — Gaps (optional):
capture_research_gap for unresolved questions.

Phase 10 — Finalize:
Call finalize_narrative_research when gates should pass. Fix validation errors, then finalize again."#
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
        assert!(hint.contains("no `period` column"));
    }

    #[test]
    fn golden_path_reviews_existing_board_first() {
        let path = narrative_research_golden_path();
        assert!(path.contains("Review existing board"));
        assert!(path.contains("capture_narrative_side"));
        assert!(path.contains("finalize_narrative_research"));
    }
}
