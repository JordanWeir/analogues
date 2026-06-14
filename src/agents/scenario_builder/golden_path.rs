pub fn scenario_schema_hint() -> &'static str {
    r#"Key tables for scenario work:
- av_raw_facts — PRIMARY projection time series (quarterly AlphaVantage): field_name, period_end, period_type, metric_value, report_type.
- av_fact_metric_catalog — browse AV fields with coverage dates.
- scenario_assumptions / scenario_periods / scenario_crux_assumptions — scenario outputs you persist.
- crux_candidates — promoted falsifiable mechanics (crux_key, statement, bridge_archetype).
- analysis_experiments — promoted SQL experiments (experiment_key, purpose, outputs_json) linked by crux_id.
- claims / sources — narrative guidance and official releases. Query sources with `SELECT id, title, source_type FROM sources ORDER BY id`.
- sec_raw_facts / fundamental_observations — secondary; often lag AV; use for bridges and staleness checks only.

AlphaVantage quarterly query pattern:
SELECT field_name, period_end, metric_value, unit
FROM av_raw_facts
WHERE report_type = 'quarterly' AND period_type = 'quarter'
  AND field_name IN ('totalRevenue', 'netIncome', 'operatingIncome', 'grossProfit',
                     'weightedAverageShsOutDil', 'operatingCashflow', 'capitalExpenditures')
ORDER BY period_end DESC
LIMIT 20;

Browse AV catalog:
SELECT field_name, label, fact_count, earliest_period_end, latest_period_end
FROM av_fact_metric_catalog
ORDER BY latest_period_end DESC
LIMIT 30;"#
}

pub fn scenario_blueprint_golden_path() -> &'static str {
    r#"Phase 1 — Survey inputs (workspace_sql + web_search if contradictions):
- Read narrative_map + narrative_map_items crux rows and claims with guidance.
- List promoted crux_candidates and forward/sensitivity analysis_experiments.
- Check av_fact_metric_catalog for quarterly coverage (latest_period_end).

Phase 2 — Design 4–6 company-specific scenarios (target 5):
- Names must reflect ORCL-specific crux resolution paths, not generic "Bull/Base/Bear" unless clearest.
- Include at least one bullish, one neutral, and one bearish stance.
- Assign probabilities that sum to ~1.0 before normalization.
- Link each scenario to promoted crux_keys and relevant experiment_keys.
- Describe how each scenario resolves the dominant funding/growth/concentration tensions differently.

Phase 3 — submit_scenario_blueprint with scenario_key slugs (snake_case), probabilities, and linked keys.
Do not write quarterly periods yet — detail workers handle projections."#
}

pub fn scenario_detail_golden_path() -> &'static str {
    r#"Phase 1 — Anchor historical quarters on AlphaVantage (PRIMARY):
- Pull ~4 trailing quarters from av_raw_facts (report_type='quarterly', period_type='quarter').
- Use totalRevenue, margins (grossProfit/totalRevenue, netIncome/totalRevenue), weightedAverageShsOutDil, EPS if available.
- Historical period_order 1..4: set absolute revenue from AV; period_end = AV period_end; period_type='quarter'.
- Use SEC facts and analysis_experiments only to validate or flag staleness — do not replace AV actuals.

Phase 2 — Project forward 12–20 quarters (3–5 years):
- period_order continues sequentially after historical rows.
- Use quarterly revenue_growth (not annualized) unless source_note explains annualization.
- Let crux resolution for THIS scenario drive growth/margin/share paths.
- Borrow napkin math from linked analysis_experiments (outputs_json) and claims for bridges.
- Interpolate or assume where AV lacks a field; document in source_note.
- web_search only to settle contradictions between claims, AV, and SEC.

Phase 3 — Terminal valuation on the LAST forward quarter only:
- Set ps_median (required) and optional ps_low/ps_high, pe bands.
- blend_ps_weight / blend_pe_weight default 0.5 unless scenario warrants skew.

Phase 4 — Crux assumptions, sensitivities, signals:
- crux_assumptions: link crux_key + optional experiment_key from blueprint.
- Optional source_id: reuse an existing sources.id from the workspace board (see Sources board in context). Do not invent ids — omit source_id when uncited.
- At least 2 sensitivities, 1+ confirming and 1+ breaking signals (specific, monitorable).

Phase 5 — submit_scenario_detail with per_worker true for fan-out workers."#
}

pub fn scenario_blueprint_submit_example() -> &'static str {
    r#"{"scenarios":[{"scenario_key":"rpo_funds_capex_bull","name":"RPO conversion funds capex without dilution","stance":"bullish","probability":0.25,"description":"Backlog converts fast enough; OCF scales with cloud revenue; capex self-funds by FY29.","crux_resolution_summary":"OpenAI concentration manageable; funding gap closes via OCF not equity.","linked_crux_keys":["capex_funding_pressure_orcl","openai_concentration_risk"],"linked_experiment_keys":["capex_funding_gap_forward_projection","rpo_conversion_to_fund_fy27_capex"]}],"projection_notes":["Use AV quarterly revenue through latest filed quarter; SEC lags one quarter on some series."]}"#
}

pub fn scenario_detail_submit_example() -> &'static str {
    r#"{"scenario_key":"rpo_funds_capex_bull","assumption_summary":"Revenue compounds 8% QoQ from latest AV quarter; margins expand 50bps/year; shares flat.","crux_assumptions":[{"crux_key":"capex_funding_pressure_orcl","crux":"Capex funding gap","assumption":"OCF reaches 85% of guided net capex by FY28","impact":"Self-funding path","experiment_key":"capex_funding_gap_forward_projection","source_id":1}],"sensitivities":["±200bps QoQ revenue growth","RPO conversion ±3pp"],"confirming_signals":["Quarterly OCF/capex ratio improves two quarters in a row"],"breaking_signals":["Equity raise announced for datacenter build"],"periods":[{"period_order":1,"label":"FY26 Q3","period_end":"2026-02-28","period_type":"quarter","revenue":17300000000.0,"net_margin":0.22,"diluted_shares":2876000000.0,"source_note":"AV actual"},{"period_order":5,"label":"FY27 Q3","period_end":"2027-02-28","period_type":"quarter","revenue_growth":0.08,"net_margin":0.23,"ps_low":6.0,"ps_median":7.5,"ps_high":9.0,"source_note":"Forward projection from scenario crux resolution"}],"per_worker":true}"#
}
