/// Tables and metadata available when the narrative researcher runs (post catalog).
pub fn workspace_schema_hint() -> &'static str {
    r#"Workspace SQLite state at this stage:
- SEC raw facts, concept catalog, canonical mappings, and starter fundamentals are loaded.
- sources, claims, narrative_map, and narrative_map_items are EMPTY until you capture them.

Table tiers (use in this order):
1. Context — stock_info(ticker, company_name, currency, sector, industry), run_metadata(ticker, created_at, financial_fetch_status)
2. Fundamentals — fundamentals(metric_key, metric_value, period, source_note): headline revenue, EPS, price, margins
3. Catalog signal — concept_catalog_entries(concept_name, label, narrative_tags, latest_period_end, series_usability): company-specific mechanics tagged for narrative work
4. Avoid overwriting — canonical_metric_mappings and fundamental_observations are inputs, not outputs for this agent

LIMIT rules:
- Always ORDER BY before LIMIT.
- Catalog narrative search: LIMIT 15–20, ORDER BY latest_period_end DESC.
- Results truncate at 200 rows per workspace_sql call."#
}

/// Golden-path workflow using incremental capture tools (not one giant JSON submit).
pub fn narrative_research_golden_path() -> &'static str {
    r#"Golden path — use incremental capture tools in this order (web search between SQL rounds as needed):

Phase 0 — Orient (workspace_sql, one round):
SELECT ticker, company_name, currency, sector, industry FROM stock_info;
SELECT metric_key, metric_value, period, source_note FROM fundamentals
  WHERE metric_key IN ('revenue_ttm','net_income_ttm','eps_ttm','current_price','market_cap')
  ORDER BY metric_key;

Phase 1 — Source discovery (web search, 3–5 rounds):
Search for recent filings, earnings transcripts, investor presentations, and credible bull/bear commentary.
After each useful discovery round, call capture_sources with 1–3 sources at a time.

Phase 2 — Claims (capture_claims, multiple calls):
Extract claims from sources as you go. Tag side (bull/bear/neutral/consensus), claim_type, and confidence.
Use source_id from prior capture_sources responses. Group by narrative angle when helpful.

Phase 3 — Bull narrative (capture_narrative_side):
Call capture_narrative_side with side="bull" after researching the bull case. Steelman it.

Phase 4 — Bear narrative (capture_narrative_side):
Call capture_narrative_side with side="bear" after researching risks and skeptics.

Phase 5 — Dominant + consensus (capture_narrative_side):
Capture side="dominant" (what the market is pricing) and side="consensus" (shared assumptions).
Optional: side="counter_narrative" for the under-discussed alternative.

Phase 6 — Agreements (capture_narrative_items):
Call capture_narrative_items with item_type="agreement" for 1–3 points bulls and bears both accept.

Phase 7 — Cruxes (capture_narrative_items):
Call capture_narrative_items with item_type="crux" for 2–5 falsifiable debate points that would change the story.

Phase 8 — Orientation + sections:
capture_orientation — dominant_question, current_setup, time_horizon
capture_section section_key="business_model"
capture_section section_key="why_now"

Phase 9 — Gaps (optional):
capture_research_gap for unresolved source or data questions.

Phase 10 — Finalize:
Call finalize_narrative_research when all required pieces are captured. Fix validation errors and capture missing pieces, then finalize again."#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_hint_documents_post_catalog_state() {
        let hint = workspace_schema_hint();
        assert!(hint.contains("concept_catalog_entries"));
        assert!(hint.contains("EMPTY until you capture"));
    }

    #[test]
    fn golden_path_uses_incremental_capture_tools() {
        let path = narrative_research_golden_path();
        assert!(path.contains("capture_narrative_side"));
        assert!(path.contains("capture_narrative_items"));
        assert!(path.contains("finalize_narrative_research"));
    }
}
