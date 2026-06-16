# Init Workspace & Financial Mechanics QA — ORCL (2026-06-13, Run 20)

## Scope

QA inspection of **init workspace substrate**, **narrative researcher output**, and **financial analysis lanes** (`identify_crux_candidates` + `financial_mechanics_experiments`) for one Oracle run. This is the first ORCL run in the QA series that completed the full financial-analysis path through promoted mechanics experiments.

| Run | SQLite path | Model | Lanes completed |
|-----|-------------|-------|-----------------|
| **20** | `reports/stock-narrative-research/ORCL-2026-06-13-20/run.sqlite` | `deepseek/deepseek-v4-flash` | init → narrative → crux triage → mechanics experiments |
| 15 | `reports/stock-narrative-research/ORCL-2026-06-13-15/run.sqlite` | `deepseek/deepseek-v4-flash` | narrative only (prior QA `06-13-011`) |

Worker telemetry (run 20):

| Worker | Rounds | Tool calls | Cost | Latency |
|--------|--------|------------|------|---------|
| `narrative_researcher` | 9 | 25 | ~$0.051 | ~152s |
| `financial_model_explorer` (crux triage) | 12 | 36 | ~$0.023 | ~173s |
| `financial_model_explorer` (mechanics) | 15 | 31 | ~$0.017 | ~160s |

Web validation performed against Oracle Q4 FY2026 press release, 8-K, and CNBC (June 2026).

## Verdict

**Partial pass — mechanics lane executes cleanly and produces auditable SQL experiments, but modelling coverage is narrow and materially stale relative to the Q4 catalyst narrative.**

The financial experiment workflow **passed all 22 quality gates** and persisted **3 promoted + 1 background** experiment with proper arithmetic/interpretation splits, `inputs_json`, `bridge_json`, and finalized `analysis_runs`. SQL arithmetic for historical RPO and pre-FY2026 cash-flow bridges is **directionally correct and reproducible**.

However, the modelling layer **under-triages narrative tension** (1 promoted crux vs 7 narrative cruxes), runs **only historical investigations** (no sensitivity or forward projection), and leans on **SEC facts that lag Q4 FY2026** — so capex/OCF and RPO conclusions understate the live funding-pressure story the narrative already captured ($55.7B capex, $32B OCF, $638B RPO). **Not yet scenario-ready** without either fresher filings or explicit gap flags on experiment outputs.

---

## Financial Experiments & Modelling — Deep Dive

### Pipeline shape

| Artifact | Count | Notes |
|----------|-------|-------|
| `crux_candidates` (promoted) | **1** | `rpo_conversion_quality` only |
| `supporting_metric_selections` | **0** | Triage did not persist supporting metrics |
| `analysis_experiments` | 4 | 3 promoted, 1 background |
| `analysis_runs` (finalized) | 4 | All linked to experiments |
| `scenario_*` tables | 0 | Not run in this pipeline stage |
| `monte_carlo_config` | 1 (seed only) | No `monte_carlo_summary` yet |
| `sections.financial_math` | pending | Awaiting downstream drafting |

All mechanics experiments attach to **crux_id 1** (`rpo_conversion_quality`). None exercise the other six narrative cruxes (OpenAI viability, FCF-at-capex-pace, competitive differentiation, dilution from $40B raise, AI margin steady state, legacy software decay).

### Experiment inventory

| Key | Disposition | Question (abridged) | Purpose | Assessment |
|-----|-------------|---------------------|---------|------------|
| `rpo_to_revenue_ratio` | **promoted** | Is RPO growing faster than annual revenue? | `historical_investigation` | **Good.** FY ratios 1.36× → 1.85× → 2.40× match re-executed SQL. Q3 FY2026 point ($552.6B RPO) correctly has null revenue ratio. Interpretation links backlog accumulation to conversion gap. |
| `rpo_growth_momentum` | **promoted** | Is RPO accelerating sequentially? | `historical_investigation` | **Good for DB coverage.** Sequential series through Q3 FY2026 ($552.6B) with +230% Q1 FY2026 jump is accurate. **Missing Q4 +$85B step to $638B** because SEC facts stop at 2026-02-28. |
| `capex_ocf_pressure` | **promoted** | Is capex consuming a growing share of OCF? | `historical_investigation` | **Directionally right, materially understated.** FY2025 capex/OCF = 1.019 and FCF ≈ -$0.39B are correct for persisted SEC annual facts. Official FY2026 is capex **$55.7B** on OCF **$32.0B** (FCF **-$23.7B**) — ~2.6× worse than the promoted headline implies. |
| `interest_coverage_pressure` | **background** | Is interest expense rising vs operating income? | `historical_investigation` | **Reasonable demotion.** Ratio peaked FY2023 (26.8%) then improved to 20.2% FY2025 as op income grew. Valid secondary signal, not the primary June 2026 tension. |

**Modelling posture summary:** Four focused, falsifiable questions; zero `sensitivity`, `forward_projection`, or `scenario_validation` experiments. No bridge from FY2027 guidance ($90B revenue, ~$70B net capex) into quantitative experiments. No use of `RevenueRemainingPerformanceObligationPercentage` (conversion-rate concept; latest SEC fact in workspace is FY2021).

### What went well

- **Golden-path compliance:** Draft → judge flow used `run_analysis_draft` + `finalize_analysis`; promoted rows include both arithmetic (`ratio`, `series_point`) and `interpretation` outputs.
- **Auditable SQL:** Experiments query `sec_raw_facts` with explicit `concept_name`, `fiscal_period`, and `period_end` filters — not black-box prose.
- **Bridge metadata:** All four experiments record `bridge_json` with archetype `backlog_to_cash_conversion`, tying arithmetic to the RPO-funding narrative chain.
- **Inputs custody:** Promoted experiments record `inputs_json` concept provenance (RPO, revenue, capex, OCF, interest, op income).
- **Gate hygiene:** All financial mechanics gates pass (`arithmetic_vs_interpretation_split`, `inputs_and_units_recorded`, `promoted_linked_to_sources`, etc.).
- **Judgment on interest coverage:** Backgrounding a secondary debt-service series shows appropriate prioritization within the step budget.

### Modelling gaps and risks

| Severity | Finding | Where | Why it matters |
|----------|---------|-------|----------------|
| **High** | **Single crux triage** vs 7 narrative cruxes | `crux_candidates` (1 row) vs `narrative_map_items` (7 crux rows) | Downstream scenario work inherits one bridge archetype. OpenAI concentration, dilution, FCF recovery, and margin steady-state mechanics have no structured experiment hooks. |
| **High** | **Capex/OCF experiment ends at FY2025 SEC facts** | `capex_ocf_pressure` outputs; latest annual OCF/capex in `sec_raw_facts` = 2025-05-31 | Promoted conclusion ("capex now exceeds OCF") is true but **understates** post-Q4 funding gap by ~$33B on the capex side alone. A scenario agent could treat FY2025 as current. |
| **High** | **RPO experiments stop at Q3 FY2026 ($552.6B)** | `rpo_growth_momentum`, `sec_raw_facts` latest RPO period_end = 2026-02-28 | Narrative and claims cite **$638B** Q4 RPO (+$85B sequential). Experiments miss the catalyst-day figure entirely. |
| **Medium** | **No forward or sensitivity experiments** | All `purpose = historical_investigation` | Cannot answer "what RPO conversion rate closes the funding gap?" or "what FY27 capex/OCF breaks FCF positive?" from persisted experiments alone. |
| **Medium** | **`supporting_metric_selections` empty** | Triage worker run 2 | Golden path expects supporting metrics with rationale; catalog search value is not persisted for audit. |
| **Medium** | **Period mixing in RPO/revenue SQL** | `rpo_to_revenue_ratio` includes `fiscal_period IN ('FY','Q3')` in one series | FY and Q3 RPO points share a year-based revenue join. FY ratios are fine; the Q3 FY2026 row is ratio-null (safe) but the pattern is fragile for other fiscal calendars. |
| **Medium** | **`analysis_runs` row inflation from `fiscal_year` duplicates** | `capex_ocf_pressure` result_json has 12 rows for 5 fiscal years | `sec_raw_facts` stores multiple `fiscal_year` labels per `period_end` (SEC restatement artifact). SQL `GROUP BY period_end, fiscal_year` duplicates rows. Finalized outputs are correct at the tail but noisy for consumers. |
| **Low** | **`financial_math` section still pending** | `sections` | Experiments exist but are not yet surfaced in report prose. |
| **Low** | **Monte Carlo config seeded, not executed** | `monte_carlo_config` only | Expected at this stage; not a failure. |

### Crux triage quality

The lone promoted crux is well-formed:

- **Key:** `rpo_conversion_quality`
- **Archetype:** `backlog_to_cash_conversion`
- **Falsifiability:** watch / confirming / breaking signals present
- **Cluster:** `RevenueRemainingPerformanceObligation` as driver concept

It aligns with narrative crux #1 (RPO conversion vs GPU re-sale economics) but **does not subsume** cruxes #2–#7 (OpenAI viability, FCF-at-capex-pace, differentiation, dilution, margin steady state, legacy decay). Validation only requires `≥1 crux`; golden path targets **2–5**. The agent converged on the strongest ORCL tension but left most of the narrative debate unmodelled.

---

## Init Workspace (Substrate)

**Partial pass** — same core profile as prior ORCL runs.

| Family | Evidence |
|--------|----------|
| Identity | `stock_info`: ORCL, Oracle Corporation, USD |
| SEC breadth | 25,369 `sec_raw_facts`; 513 unique concepts; 516 catalog entries |
| Starter fundamentals | 11 `fundamentals` rows via Alpha Vantage (revenue TTM $67.36B, debt $156.2B, etc.) |
| Canonical layer | 9 definitions / 9 AV mappings (high confidence, deterministic) |
| Observations | 807 `fundamental_observations` |
| Gaps | 4 open (`starter_financials`, OpenAI RPO, infra margins, price/mcap) |

**SEC ingest lag (recurring):** No Q4 FY2026 (period_end 2026-05-31) rows for RPO, annual capex, or annual OCF in `sec_raw_facts`. Alpha Vantage TTM fundamentals reflect FY2026 revenue ($67.36B) but not the $55.7B capex / $32B OCF cash-flow statement. This lag is the root cause of experiment staleness, not SQL errors.

---

## Narrative Layer (Brief)

Run 20 narrative is **Q4-accurate** and comparable to run 15 (`06-13-011`): 18 claims, 5 sources, 7 narrative cruxes, 3 agreements. Headline numbers ($638B RPO, -$23.7B FCF, $55.7B capex, FY26 $67.4B revenue) match official sources. **`crux_candidates` now has 1 row** (improvement over run 15's empty table) but still far below the 7 narrative cruxes.

---

## Web Validation (Experiment-Critical Fields)

| Field | Workspace / Experiment | External (Jun 2026) | Status |
|-------|------------------------|---------------------|--------|
| FY2025 RPO | $137.8B (`rpo_growth_momentum`) | ~$138B (FY2025 10-K) | **Confirmed** |
| Q3 FY2026 RPO | $552.6B (`sec_raw_facts`, experiments) | ~$553B (Q3 FY2026 filing) | **Confirmed** |
| Q4 FY2026 RPO | **Absent** (narrative: $638B) | **$638B** (+$85B seq.) | **Gap — ingest lag** |
| FY2023–25 RPO/Revenue ratios | 1.36 / 1.85 / 2.40 | Consistent with filed revenue & RPO | **Confirmed** |
| FY2025 capex (SEC) | $21.2B | ~$21.2B (FY2025 10-K) | **Confirmed for FY2025** |
| FY2026 capex | **Not in SEC facts** (experiment stops FY2025) | **$55.7B** | **Gap — experiment understates** |
| FY2026 OCF | **Not in SEC facts** (narrative claim: $32.0B) | **$32.0B** | **Gap — not modelled** |
| FY2026 FCF | Not experimentally computed | **-$23.7B** | **Gap** |
| FY2026 revenue TTM | $67.36B (`fundamentals`) | $67.4B | **Confirmed** |

When narrative claims and experiments disagree, the cause is **data freshness**, not hallucination — but downstream agents must not treat FY2025 experiment tails as Q4-current without a gap flag.

---

## Product Readiness

| Stage | Status | Notes |
|-------|--------|-------|
| Init substrate | Partial pass | Rich SEC catalog; price/mcap gap; SEC lag on latest quarter |
| Narrative | Partial pass | Q4-accurate claims; 7 cruxes in map |
| Crux triage | **Partial fail** | 1 promoted crux; 0 supporting metrics |
| Mechanics experiments | **Partial pass** | Auditable, gated, but historical-only and stale vs catalyst |
| Scenario / Monte Carlo | Not started | Config seeded only |
| End-to-end scenario report | **Not ready** | Needs fresher facts, more crux coverage, and forward bridges |

**Could a later research agent build a smart scenario from this workspace?** Partially. It can cite reproducible RPO accumulation and pre-FY2026 funding pressure SQL. It **cannot** rely on persisted experiments alone for Q4 FCF/capex dynamics, RPO conversion sensitivity, dilution math, or multi-crux scenario conditioning without re-querying sources or re-fetching SEC facts.

---

## Recommendations

### Financial modelling / experiments

1. **Require minimum crux count (≥2)** in `submit_crux_triage` when narrative map has ≥3 cruxes — force coverage beyond the dominant RPO thread.
2. **Require at least one non-`historical_investigation` experiment** per run (e.g. `sensitivity` on RPO conversion rate or `forward_projection` on FY27 capex vs guided revenue).
3. **Stamp experiment staleness:** When latest `sec_raw_facts.period_end` predates narrative claim dates, persist a `data_gaps` or `data_quality_flags` row linked to `experiment_key`.
4. **Deduplicate annual SEC pulls:** Prefer `MAX(filed_at)` or `DISTINCT period_end` without `fiscal_year` in `GROUP BY` to avoid `analysis_runs` row inflation.
5. **Surface `RevenueRemainingPerformanceObligationPercentage`** or earnings-call conversion metrics when SEC concept is stale — with explicit non-GAAP / call-source provenance.
6. **Populate `supporting_metric_selections`** during triage; reject finalize if catalog search yielded metrics but none were persisted.
7. **Draft `financial_math` section** from promoted `outputs_json` + `bridge_json` in a downstream lane so experiments reach the report reader.

### Init / ingest (unchanged priorities)

8. **Accelerate post-earnings SEC Company Facts refresh** so Q4 FY2026 annual flows and RPO land before mechanics experiments run.
9. **Close `starter_financials` gap** (price, market cap) before scenario valuation work.

---

## Summary

Run 20 proves the **financial mechanics experiment lane works end-to-end**: gated, SQL-auditable, arithmetic/interpretation split, three promoted experiments with coherent RPO→funding bridge logic. The **modelling quality is intentionally narrow and historically anchored** — strong for demonstrating the product mechanic, weak for June 2026 catalyst-conditioned scenario work. The biggest improvement lever is not better SQL syntax but **fresher SEC facts, broader crux triage, and at least one forward/sensitivity experiment** that connects narrative guidance ($638B RPO, $55.7B capex, $90B FY27 revenue) to quantitative falsifiers.
