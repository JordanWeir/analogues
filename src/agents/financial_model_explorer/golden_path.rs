pub fn explorer_schema_hint() -> &'static str {
    r#"Workspace SQLite state at financial analysis stage:
- Ingest, concept catalog, canonical mappings, and fundamentals are available.
- Narrative context may live in narrative_map, narrative_map_items, claims, and sources.
- Lane 4 writes crux_candidates and supporting_metric_selections.
- Lane 5 writes analysis_runs (draft) and analysis_experiments (finalized).

Table tiers (use in this order):
1. Narrative context — narrative_map, narrative_map_items, claims, sources.
2. Judgment — crux_candidates(crux_key, title, statement, watch_condition, confirming_signal, breaking_signal, disposition, payload_json).
3. Catalog search — concept_catalog_entries(taxonomy, concept_name, label, unit, fact_count, latest_period_end, dominant_period_shape, series_usability, narrative_tags).
4. Core flows — canonical_fundamental_observations, fundamental_observations, fundamentals.
5. Experiments — analysis_experiments(experiment_key, crux_id, question, purpose, sql_body, period_basis, disposition, outputs_json), analysis_runs(run_key, status, result_json).
6. Confirmation — sec_raw_facts for spot checks only after catalog shortlist.

LIMIT rules:
- Always ORDER BY before LIMIT.
- Catalog search: LIMIT 15–20.
- Latest facts for one concept: LIMIT 10.
- Results truncate at 200 rows per workspace_sql call.

Period discipline:
- Pick one period_basis per query: quarter | ytd | annual | instant.
- Never mix quarter, ytd, and annual in one arithmetic line.
- Instant metrics are balance-sheet snapshots; duration metrics are flows."#
}

pub fn crux_triage_golden_path() -> &'static str {
    r#"Golden path — identify falsifiable crux candidates (target ≤8 workspace_sql rounds):

Phase 0 — Orient (one round, parallel SQL):
SELECT dominant, bull, bear, consensus, counter_narrative FROM narrative_map WHERE id = 1;
SELECT item_type, body FROM narrative_map_items ORDER BY item_order LIMIT 20;
SELECT claim, claim_type, side FROM claims ORDER BY id LIMIT 20;
SELECT COUNT(*) AS catalog_concepts FROM concept_catalog_entries;
SELECT canonical_key, metric_label FROM canonical_metric_definitions ORDER BY display_order;

Phase 1 — Search mechanics by narrative theme (1–2 rounds):
SELECT taxonomy, concept_name, label, unit, fact_count, latest_period_end,
       dominant_period_shape, series_usability, narrative_tags
FROM concept_catalog_entries
WHERE series_usability NOT IN ('stale', 'event_point')
  AND latest_period_end IS NOT NULL
  AND (
    narrative_tags LIKE '%backlog%' OR narrative_tags LIKE '%capex%'
    OR narrative_tags LIKE '%conversion%' OR narrative_tags LIKE '%debt%'
    OR narrative_tags LIKE '%margin%' OR narrative_tags LIKE '%dilution%'
  )
ORDER BY latest_period_end DESC, fact_count DESC
LIMIT 20;

Phase 2 — Confirm top concepts (one round):
SELECT concept_name, metric_value, period_end, period_start, fiscal_period, form, filed_at
FROM sec_raw_facts
WHERE taxonomy = :taxonomy AND concept_name = :concept AND unit = :unit
ORDER BY period_end DESC, filed_at DESC
LIMIT 10;

Phase 3 — Cluster and crux (agent reasoning):
- Group 2–5 concepts into one mechanic cluster when they answer the same narrative tension.
- Draft 2–5 crux candidates total, not one per metric.
- Each crux must be falsifiable: include watch_condition, confirming_signal, breaking_signal.
- Prefer bridge archetypes: backlog_to_cash_conversion, capex_to_funding_pressure, debt_to_eps, obligation_build, working_capital_pressure.
- Flag sparse/stale/mixed-period concepts in quality_flags; do not promote them as smooth series.

Phase 4 — Submit:
Call submit_crux_triage with cruxes, supporting_metrics, quality_flags, open_questions.
Fix validation errors and resubmit."#
}

pub fn mechanics_experiment_golden_path() -> &'static str {
    r#"Golden path — financial mechanics experiments (draft then judge):

Phase 0 — Load crux context (one round):
SELECT crux_key, title, statement, bridge_archetype, watch_condition, confirming_signal, breaking_signal
FROM crux_candidates
WHERE disposition = 'promoted' AND status = 'active'
ORDER BY id;

SELECT experiment_key, question, disposition, purpose
FROM analysis_experiments
WHERE disposition IN ('promoted', 'candidate')
ORDER BY updated_at DESC
LIMIT 10;

Phase 1 — Pick one question per experiment:
Good: "Is capex rising faster than operating cash flow on an annual basis?"
Bad: "Analyze the whole business."

Phase 2 — run_analysis_draft:
- Provide run_key, question, sql_body, period_basis, optional crux_key, assumptions, inputs.
- Use one consistent period basis per query.
- Prefer ratios and simple bridges: capex/revenue, capex/OCF, RPO/deferred_revenue, interest/operating_income.
- Review returned rows before judging.

Phase 3 — finalize_analysis:
- If results are useful: disposition promoted or candidate, include arithmetic outputs AND a separate interpretation output row.
- If not useful: disposition rejected with rejection_reason, or discard by not finalizing.
- Promoted experiments must include at least one arithmetic/ratio output and one interpretation output.

Phase 4 — Repeat for 2–4 focused experiments across different crux mechanics.

Phase 5 — submit_mechanics_experiments when at least one promoted experiment exists.

Arithmetic vs interpretation:
- Arithmetic rows: kind ratio | arithmetic | series_point | bridge_step with value/formula.
- Interpretation rows: kind interpretation with text only.
- Never hide arithmetic inside interpretation prose."#
}
