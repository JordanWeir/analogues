pub fn explorer_schema_hint() -> &'static str {
    r#"Workspace SQLite state at financial analysis stage:
- Ingest, concept catalog, canonical mappings, and fundamentals are available.
- Narrative context may live in narrative_map, narrative_map_items, claims, and sources.
- Lane 4 writes crux_candidates and supporting_metric_selections.
- Lane 5 writes analysis_runs (draft) and analysis_experiments (finalized).

Table tiers (use in this order):
1. Narrative context — narrative_map, narrative_map_items, claims, sources, data_gaps.
2. Judgment — crux_candidates(crux_key, title, statement, watch_condition, confirming_signal, breaking_signal, disposition, payload_json).
3. Catalog search — concept_catalog_entries(taxonomy, concept_name, label, unit, fact_count, latest_period_end, dominant_period_shape, series_usability, narrative_tags).
4. Core flows — canonical_fundamental_observations, fundamental_observations, fundamentals.
5. Experiments — analysis_experiments(experiment_key, crux_id, question, purpose, sql_body, period_basis, disposition, outputs_json), analysis_runs(run_key, status, result_json).
6. Confirmation — sec_raw_facts for spot checks after catalog shortlist; claims + sources when SEC lags narrative.

Schema quirks:
- stock_info uses company_name (not name): ticker, company_name, exchange, sector, industry.
- sec_raw_facts is single-company; there is no ticker column. Filter by taxonomy, concept_name, unit, fiscal_period, period_end.
- sec_raw_facts columns: taxonomy, concept_name, label, unit, form, period_start, period_end, filed_at, fiscal_year, fiscal_period, frame, metric_value.
- sec_raw_facts may store multiple fiscal_year labels for the same period_end (restatement artifact). Do NOT GROUP BY fiscal_year for annual series — use GROUP BY period_end and pick MAX(filed_at) or MAX(metric_value). If a draft returns row_count >> distinct fiscal years, fix SQL before finalizing.
- When SEC latest_period_end lags claims or open data_gaps, note staleness in quality_flags, assumptions, or source_note. You may reference claims as input_type claim with source provenance — never treat stale SEC as current without disclosure.

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
SELECT item_type, item_order, body FROM narrative_map_items ORDER BY item_order LIMIT 20;
SELECT claim, claim_type, side FROM claims ORDER BY id LIMIT 20;
SELECT gap_key, description, status FROM data_gaps WHERE status = 'open';
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
- Read every narrative crux in the prompt context. Produce one promoted or background crux_candidate per narrative crux, or cluster 2 related narrative cruxes into one mechanic with linked_claim_ids.
- When narrative has 3+ crux items, submit at least 2 promoted crux_candidates covering distinct bridge archetypes (e.g. backlog_to_cash_conversion AND capex_to_funding_pressure).
- Draft 2–5 crux candidates total, not one per metric and not a single crux that collapses the whole board.
- Each crux must be falsifiable: include watch_condition, confirming_signal, breaking_signal.
- Prefer bridge archetypes: backlog_to_cash_conversion, capex_to_funding_pressure, debt_to_eps, obligation_build, working_capital_pressure.
- Persist at least 2 supporting_metrics from catalog search with rationale tying each metric to a crux.
- If SEC latest_period_end lags claims, add a quality_flags entry (e.g. sec_facts_stale) naming the concept and periods.

Phase 4 — Submit:
Call submit_crux_triage with cruxes, supporting_metrics, quality_flags, open_questions.
quality_flags objects require flag_key, severity, description (not gap_key).
open_questions objects require gap_key, description (not flag_key).
Fix validation errors and resubmit."#
}

// @TODO: This would be better if we instantiated an example of the object, and then used toSTring.  That gives better guarantees the shape continues to be correct as schema change.
pub fn crux_triage_submit_example() -> &'static str {
    r#"{"cruxes":[{"crux_key":"rpo_conversion","title":"RPO conversion","statement":"Backlog must convert fast enough to fund capex.","bridge_archetype":"backlog_to_cash_conversion","narrative_side":"bear","watch_condition":"RPO/revenue trend","confirming_signal":"OCF keeps pace with capex","breaking_signal":"OCF lags guided capex","disposition":"promoted","rationale":"Core funding mechanic.","cluster_members":[{"taxonomy":"us-gaap","concept_name":"RevenueRemainingPerformanceObligation","unit":"USD","role":"driver"}],"linked_claim_ids":[4]}],"supporting_metrics":[{"selection_scope":"crux_support","crux_key":"rpo_conversion","taxonomy":"us-gaap","concept_name":"RevenueRemainingPerformanceObligation","unit":"USD","rationale":"Backlog driver","period_basis":"instant","quality_status":"ok"}],"quality_flags":[{"flag_key":"sec_rpo_stale","severity":"warning","description":"SEC RPO ends 2026-02-28; narrative cites $638B at FY2026 Q4 — use claims for forward figures."}],"open_questions":[{"gap_key":"rpo_customer_concentration","description":"Is a single customer >50% of RPO? Not disclosed in SEC filings."}]}"#
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

Compare SEC freshness summary in prompt context to claims. Flag gaps before finalizing.

Phase 1 — Pick one question per experiment:
Good: "Is capex rising faster than operating cash flow on an annual basis?"
Good: "What RPO conversion rate would be needed to fund FY27 guided capex at current OCF?"
Bad: "Analyze the whole business."

Phase 2 — run_analysis_draft:
- Provide run_key, question, sql_body, period_basis, optional crux_key, assumptions, inputs.
- Use one consistent period basis per query.
- Prefer ratios and simple bridges: capex/revenue, capex/OCF, RPO/deferred_revenue, interest/operating_income.
- Dedupe annual SEC series with GROUP BY period_end only (not fiscal_year).
- Review returned rows before judging. If row_count looks inflated, fix SQL and re-draft.

Phase 3 — finalize_analysis:
- Pass run_key from the draft you are judging.
- sql_body and period_basis may be omitted; they default from the draft run.
- purpose must be one of: historical_investigation | sensitivity | forward_projection | scenario_validation.
- When claims include forward guidance, at least one promoted experiment must use sensitivity or forward_projection.
- If results are useful: disposition promoted or candidate, include arithmetic outputs AND a separate interpretation output row.
- If SEC data is stale vs claims, record staleness in assumptions or source_note.
- If not useful: disposition rejected with rejection_reason, or discard by not finalizing.
- Promoted experiments must include at least one arithmetic/ratio output and one interpretation output.

Phase 4 — Repeat for 2–4 focused experiments across different promoted crux mechanics (not all on one crux).

Phase 5 — submit_mechanics_experiments:
- Per-crux fan-out worker: promote 1–2 experiments for your assigned crux only; finish with per_worker true.
- Lane-complete worker: after ≥2 promoted experiments exist workspace-wide, finish with summary only (per_worker false).
- On penultimate turn: finalize any pending drafts first.

Arithmetic vs interpretation:
- Arithmetic rows: kind ratio | arithmetic | series_point | bridge_step with value/formula.
- Interpretation rows: kind interpretation with text only.
- Never hide arithmetic inside interpretation prose."#
}

pub fn mechanics_finalize_example() -> &'static str {
    r#"finalize_analysis example (after run_analysis_draft with run_key "capex_ocf_draft"):
{"run_key":"capex_ocf_draft","experiment":{"experiment_key":"capex_ocf_pressure","question":"Is capex rising faster than operating cash flow on an annual basis?","purpose":"historical_investigation","period_basis":"annual","crux_key":"capex_to_funding_pressure","disposition":"promoted","rationale":"FY2025 capex/OCF exceeded 1.0 with negative free cash flow.","inputs":[{"input_type":"concept","taxonomy":"us-gaap","concept_name":"PaymentsToAcquirePropertyPlantAndEquipment","unit":"USD"},{"input_type":"concept","taxonomy":"us-gaap","concept_name":"NetCashProvidedByUsedInOperatingActivities","unit":"USD"}],"outputs":[{"kind":"ratio","label":"Capex / OCF FY2025","value":1.019,"unit":"ratio","period_end":"2025-05-31"},{"kind":"interpretation","label":"Funding gap read","text":"Capex now exceeds operating cash flow, implying external funding for the build-out."}]}}

Forward/sensitivity example:
{"run_key":"rpo_conversion_sensitivity","experiment":{"experiment_key":"rpo_conversion_to_fund_capex","question":"What annual RPO conversion rate closes the FY27 funding gap at guided capex?","purpose":"sensitivity","period_basis":"annual","crux_key":"rpo_conversion_quality","disposition":"promoted","assumptions":[{"key":"Guided FY27 net capex","value":"~$70B from claims","note":"SEC annual capex may lag; not treated as current"}],"inputs":[{"input_type":"claim","note":"FY27 capex guidance from official release"}],"outputs":[{"kind":"ratio","label":"Implied conversion rate","value":0.12,"unit":"ratio"},{"kind":"interpretation","label":"Sensitivity read","text":"At ~12% annual RPO conversion, backlog could fund guided build-out; below that, external financing dominates."}]}}

sql_body may be omitted; it defaults to the draft executed_sql.
Finish lane with submit_mechanics_experiments: {"summary":"..."}"#
}

pub fn mechanics_per_worker_submit_example() -> &'static str {
    r#"Per-crux fan-out worker finish:
{"summary":"Promoted sensitivity experiment for rpo_conversion_to_capex_justification.","per_worker":true,"crux_key":"rpo_conversion_to_capex_justification"}"#
}

pub fn mechanics_scout_submit_example() -> &'static str {
    r#"Mechanics scout worker finish:
{"summary":"Covered debt rollover and dilution cruxes lacking experiments.","per_worker":true,"scout":true}"#
}
