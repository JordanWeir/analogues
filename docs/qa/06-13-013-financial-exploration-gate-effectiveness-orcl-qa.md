# Financial Exploration Gate Effectiveness — ORCL Runs 20 vs 21

## Scope

Comparative QA of two consecutive Oracle runs after prompt updates and new validation gates for the financial analysis exploration stage (`identify_crux_candidates` + `financial_mechanics_experiments`). Init workspace substrate is identical between runs; differences are narrative output, crux triage, and mechanics experiments.

| Run | SQLite path | Created (UTC) | Financial lanes |
|-----|-------------|---------------|-----------------|
| **20** | `reports/stock-narrative-research/ORCL-2026-06-13-20/run.sqlite` | 2026-06-13T18:01 | Pre-gate expansion (gates not recorded for new checks) |
| **21** | `reports/stock-narrative-research/ORCL-2026-06-13-21/run.sqlite` | 2026-06-13T18:33 | Post prompt/gate changes (all gates pass) |

Model for all workers: `deepseek/deepseek-v4-flash`.

Web validation: Oracle Q4/FY2026 press release, 8-K, earnings call transcript (June 2026).

## Verdict

**Partial pass — the prompt and gate changes were effective at forcing structurally better financial exploration output in run 21, but modelling still under-covers the narrative board and leans on SEC facts that lag the Q4 catalyst.**

Run 20 would have **failed three of four new gates** if they had been enforced (`crux_coverage_vs_narrative`, `supporting_metrics_present`, `experiment_purpose_diversity`). Run 21 passes all 26 recorded gates and delivers a materially richer modelling layer: **4 promoted cruxes** (vs 1), **6 supporting metrics with rationale** (vs 0), and **forward/sensitivity experiments** (vs all-historical). The tradeoff is **~2× financial explorer cost and latency** (43 agent rounds / ~599s vs 27 / ~333s).

Init workspace quality is unchanged and still **partial** (missing price/market cap in fundamentals). SEC raw facts stop at **Q3 FY2026** (`period_end` 2026-02-28) for RPO, capex, and OCF — a substrate limitation both runs share.

---

## Gate & Prompt Effectiveness

### New gates introduced (from `explorer_context.rs` and lane gate modules)

| Gate | Lane | Threshold | Run 20 | Run 21 |
|------|------|-----------|--------|--------|
| `crux_coverage_vs_narrative` | identify_crux_candidates | ≥2 promoted cruxes when narrative has ≥3 crux items | **Would fail** (7 narrative, 1 promoted) | **Pass** (8 narrative, 4 promoted) |
| `supporting_metrics_present` | identify_crux_candidates | ≥2 `supporting_metric_selections` when cruxes promoted | **Would fail** (0) | **Pass** (6) |
| `min_promoted_experiments` | financial_mechanics_experiments | ≥2 promoted experiments | Pass (3) — gate not recorded | **Pass** (4) |
| `experiment_purpose_diversity` | financial_mechanics_experiments | ≥1 non-historical promoted experiment when guidance claims exist | **Would fail** (10 guidance claims, 0 non-historical) | **Pass** (7 guidance claims, 1 forward + 1 sensitivity) |

Run 20's `quality_gate_results` table has **22 rows** and omits the four gates above. Run 21 has **26 rows** with all four present and passing.

### Worker telemetry comparison

| Worker | Run 20 rounds / tools / cost / latency | Run 21 rounds / tools / cost / latency |
|--------|----------------------------------------|----------------------------------------|
| `narrative_researcher` | 9 / 25 / $0.051 / 152s | 16 / 31 / $0.067 / 205s |
| `financial_model_explorer` (crux triage) | 12 / 36 / $0.023 / 173s | 23 / 45 / $0.058 / 375s |
| `financial_model_explorer` (mechanics) | 15 / 31 / $0.017 / 160s | 20 / 34 / $0.020 / 224s |
| **Financial explorer total** | **27 / 67 / $0.040 / 333s** | **43 / 79 / $0.078 / 599s** |

The stricter golden path (map narrative cruxes, persist supporting metrics, require forward/sensitivity when guidance exists) clearly drove more exploration rounds. Output quality improved; cost roughly doubled.

---

## What Improved in Run 21

### Crux triage breadth

| | Run 20 | Run 21 |
|---|--------|--------|
| Narrative crux items | 7 | 8 |
| Promoted `crux_candidates` | 1 (`rpo_conversion_quality`) | 4 (`rpo_conversion`, `capex_roic`, `debt_sustainability`, `cloud_growth_margins`) |
| Distinct bridge archetypes | 1 (`backlog_to_cash_conversion`) | 4 (`backlog_to_cash_conversion`, `capex_to_funding_pressure`, `debt_to_eps`, `obligation_build`) |
| `supporting_metric_selections` | 0 | 6 (RPO, capex, OCF, debt, revenue, equity) |
| Falsifiability | watch/breaking present on sole crux | watch/breaking/confirming on all 4 promoted cruxes |

Run 21 maps the June 2026 tension surface more evenly: backlog conversion, capex/funding, debt sustainability, and cloud growth/margins — instead of collapsing everything into a single RPO mechanic.

### Experiment purpose mix

| Purpose | Run 20 promoted | Run 21 promoted |
|---------|-----------------|-----------------|
| `historical_investigation` | 3 | 2 |
| `forward_projection` | 0 | 1 (`capex_funding_gap_forward_projection`) |
| `sensitivity` | 0 | 1 (`rpo_conversion_funding_sensitivity`) |

Run 21's forward experiment projects FY27 guided ~$70B capex against OCF grown 15% from FY2025 baseline, yielding ~$46B funding gap and 0.34× OCF/capex coverage — directly addressing the capital-raise narrative. The sensitivity experiment estimates ~12.7% implied RPO conversion to fund guided capex, with historical conversion declining from 73.6% (FY2023) to 41.7% (FY2025).

### Golden-path compliance gains

- **Supporting metrics** now have `rationale`, `period_basis`, and `quality_status` tied to crux IDs.
- **Forward/sensitivity experiments** record `assumptions_json` and `claim`-typed inputs noting SEC staleness vs management guidance.
- **Crux-to-experiment linkage** spans multiple cruxes (crux 1 RPO, 2 capex, 3 debt) instead of all experiments on crux 1.
- **Arithmetic/interpretation split** maintained in both runs; promoted experiments include separate interpretation rows.

---

## What The Workspace Captures Well (Both Runs)

Init substrate is stable and suitable for exploration:

| Artifact | Run 20 | Run 21 |
|----------|--------|--------|
| `sec_raw_facts` | 25,369 | 25,369 |
| Distinct SEC concepts | 513 | 513 |
| `concept_catalog_entries` | 516 | 516 |
| `fundamental_observations` | 807 | 807 |
| `canonical_metric_mappings` | 9 | 9 |
| Shares outstanding (AV) | 2.876B (2026-05-31) | same |

Both runs ingest the same SEC/Alpha Vantage universe. The financial exploration improvements in run 21 come from agent behavior and gates, not fresher ingestion.

---

## Data Quality Findings

| Severity | Finding | Where | Run 20 | Run 21 | Suggested fix |
|----------|---------|-------|--------|--------|---------------|
| **Critical** | SEC facts lag Q4 FY2026 catalyst figures | `sec_raw_facts` latest RPO/capex/OCF `period_end` = 2026-02-28 | Yes | Yes | Ingest Q4 8-K facts or persist `data_quality_flags` / experiment staleness at triage (golden path asks for this; neither run wrote `data_quality_flags`) |
| **High** | Forward projection uses FY2025 OCF ($20.8B), not FY2026 actual ($32.0B) | `capex_funding_gap_forward_projection` assumptions | N/A | Yes | Prompt or gate: when FY2026 OCF exists in claims, require it as baseline or flag conservative bias |
| **High** | RPO sensitivity uses $552.6B (Q3 SEC), not $638B (Q4 claim) | `rpo_conversion_funding_sensitivity` | Partial (historical only) | Yes (noted in assumptions) | Same as SEC lag; sensitivity should bracket both bases |
| **High** | Narrative crux coverage still incomplete | 7–8 narrative cruxes vs 1–4 promoted | 1/7 | 4/8 | Consider gate requiring promoted count ≥ min(4, narrative_crux_count) or explicit background/reject rows per unmapped narrative crux |
| **Medium** | Orphan draft `analysis_runs` | `analysis_runs` status = `draft` | 0 | 2 (`rpo_conversion_sensitivity`, `rpo_conversion_to_fund`) | Cleanup gate or agent instruction to finalize/discard drafts before submit |
| **Medium** | Interest coverage interpretation may conflate FY2026 debt raised ($43B) with balance-sheet carrying amount | `interest_coverage_trend` interpretation | N/A | Yes | Judge pass could check claim-vs-concept semantics for debt metrics |
| **Medium** | Init workspace price/market cap gap persists | `run_metadata.financial_fetch_status` = partial | Yes | Yes | Close `starter_financials` gap in init (unchanged between runs) |
| **Low** | Financial explorer cost ~2× for better coverage | `worker_runs` | baseline | +95% cost | Acceptable for quality; monitor if crux triage can converge in fewer rounds with better Phase 0 caching |

### Unmapped narrative themes (run 21)

Eight narrative cruxes; four promoted mechanics. Run 21 also promoted `cloud_growth_margins` but attached **zero** experiments to it. The table below covers every narrative crux that lacked adequate experiment coverage.

---

## Missing Narrative Cruxes — Useful Investigations & Data Availability

This section evaluates what SQL-based investigations would have added value for narrative cruxes that run 21 under-covered, and whether the run 21 workspace (`ORCL-2026-06-13-21`) actually contains the data to support them via `workspace_sql` + `run_analysis_draft` / `finalize_analysis`.

### Summary matrix

| Narrative crux (#) | Useful investigation types | Data in workspace? | SQL-feasible? | Experiments run (run 21) |
|--------------------|---------------------------|-------------------|---------------|--------------------------|
| RPO conversion + margin compression (#1) | Conversion rate vs gross margin co-movement | RPO + revenue in SEC; margins in AV | **Partial** — margin yes; conversion rate SEC stale | 1 — `rpo_conversion_funding_sensitivity` (conversion vs capex only) |
| ROIC on $70B+ FY27 capex (#2) | Proxy ROIC vs management “high 20s” claim | Op income, equity, debt, capex in SEC; ROIC claim only | **Hybrid** — proxy from SEC; datacenter ROIC not disclosed | 2 — `capex_ocf_pressure`, `capex_funding_gap_forward_projection` (capex/OCF, not ROIC) |
| $162B debt / $250B obligations (#3) | Debt + leases vs equity; interest coverage | Debt, interest, op income, lease liability in SEC | **Yes** — partially done (`interest_coverage_trend`) | 1 — `interest_coverage_trend` (coverage only) |
| OpenAI ~$300B RPO concentration (#4) | Single-customer stress; % RPO at risk | Claim #16 only; no customer disclosure | **Hybrid** — claim assumptions + RPO SEC series | 0 dedicated — OpenAI mentioned in `rpo_conversion` crux prose + experiment interpretation only |
| Cloud growth deceleration (#5) | Q3 50% → Q4 47% cloud growth trend | Claims #2, #10; no cloud segment in SEC | **Hybrid** — claims only; crux promoted, **no experiments** | 0 — `cloud_growth_margins` promoted, no experiments |
| Agentic AI pricing / margins (#6) | Margin expansion vs GPU-as-a-service | No token/agentic concepts | **No** — qualitative only | 0 — no promoted crux |
| $40B capital raise vs FCF (#7) | Dilution %, external funding vs funding gap | Shares, issuance proceeds, debt flows, FCF claims | **Mostly yes** — strong missed opportunity | 0 dedicated — `$40B` cited in `capex_roic` crux statement; funding gap forward proj touches raise narrative in interpretation |
| Multicloud DB 404% growth (#8) | Growth durability; mix shift | Claim #6 only (single point) | **No** — one claim, no time series | 0 — no promoted crux |

### What the tooling supports well vs poorly

**Works well today**

- SEC time series: RPO, capex, OCF, debt, interest, equity, shares, issuance proceeds.
- Ratios, bridges, funding gaps, interest coverage.
- Forward/sensitivity with **claim inputs** + `assumptions_json` staleness notes (run 21 already does this for capex/RPO).

**Works only with claims (hybrid)**

- OpenAI concentration, FY27 guidance, Q4 RPO $638B, cloud growth rates, ROIC targets, $40B raise sizing.

**Does not work with current ingestion**

- Segment cloud / multicloud / database revenue series.
- Customer concentration disclosures.
- Agentic / token pricing metrics.
- Market-cap-based dilution math (no share price in DB — `starter_financials` gap).

---

### Per-crux detail (run 21 workspace)

#### 1. RPO conversion + margin compression (partially covered)

**What run 21 actually did**

| | |
|---|---|
| **Promoted crux** | `rpo_conversion` — “RPO backlog conversion and customer concentration risk” (also absorbs narrative #4 thematically) |
| **Experiments** | **`rpo_conversion_funding_sensitivity`** (`sensitivity`, promoted) — historical RPO/revenue conversion rates (FY2023–FY2025 SEC); implied ~12.7% conversion to fund $70B guided capex; uses Q3 RPO $552.6B |
| **Gap** | No margin time series (AV gross/operating margin vs RPO buildout); no `RevenueRemainingPerformanceObligationPercentage` series (SEC stale FY2021); margin compression only appears in interpretation prose |

**Useful investigations**

- Joint read: RPO/revenue ratio (done) **plus** gross/operating margin trend during backlog buildout.
- Sensitivity: implied conversion rate needed to fund capex at various margin assumptions.

**Data**

- `RevenueRemainingPerformanceObligation`, `RevenueFromContractWithCustomerExcludingAssessedTax` through Q3 FY2026.
- `RevenueRemainingPerformanceObligationPercentage` exists but **latest SEC point is FY2021** — too stale for conversion-rate arithmetic.
- Gross/operating margin available via Alpha Vantage in `fundamental_observations` / `fundamentals` (e.g. gross margin ~65% TTM at 2026-05-31).

**Verdict:** Margin side is SQL-able from AV; conversion-rate side needs claims or explicit staleness flag on the SEC concept.

---

#### 2. ROIC on FY27 capex (crux promoted; experiments are capex/OCF, not ROIC)

**What run 21 actually did**

| | |
|---|---|
| **Promoted crux** | `capex_roic` — “Capex intensity and ROIC sustainability” |
| **Experiments** | **`capex_ocf_pressure`** (`historical_investigation`, promoted) — capex/OCF ratio FY2021–FY2025 + Q3 YTD; funding gap $394M FY2025; ratio crossed 1.0× in FY2025 · **`capex_funding_gap_forward_projection`** (`forward_projection`, promoted) — FY27 $70B guided capex vs OCF grown 15% from FY2025 ($23.9B); ~$46B funding gap; 0.34× OCF/capex coverage |
| **Gap** | No proxy ROIC (`op income / (equity + debt)`); no sensitivity vs claim #11 “high 20s ROIC”; no capex-per-incremental-revenue; experiments measure funding pressure, not return on invested capital |

**Useful investigations**

- **Proxy ROIC:** `OperatingIncomeLoss / (StockholdersEquity + DebtInstrumentCarryingAmount)` over time.
- **Forward sensitivity:** At claim “high 20s ROIC” (claim #11) and $70B guided capex, what incremental operating income is implied vs FY2025 op income ($17.7B SEC)?
- **Capex efficiency:** capex per dollar of incremental revenue.

**Data**

- No `ReturnOnInvestedCapital` in `concept_catalog_entries`.
- Op income, equity, debt, capex all in `sec_raw_facts`.

**Verdict:** Proxy ROIC from SEC is a solid `historical_investigation`; validating management’s datacenter ROIC claim is not — requires claim-typed `forward_projection` or `sensitivity`.

---

#### 3. Debt / total obligations (partially covered)

**What run 21 actually did**

| | |
|---|---|
| **Promoted crux** | `debt_sustainability` — “Debt load sustainability and refinancing risk” |
| **Experiments** | **`interest_coverage_trend`** (`historical_investigation`, promoted) — interest coverage FY2021–FY2025 (trough 3.74× FY2023, recovery to 4.94× FY2025); `DebtInstrumentCarryingAmount` $43B Q3 FY2026 as series point |
| **Gap** | No debt+lease “total obligations” stack vs equity (~$250B claim #8); no debt-to-equity trend; no `ProceedsFromIssuanceOfSeniorLongTermDebt` vs repayment flows; crux statement cites $162B+ debt but experiment only models coverage ratio |

**Useful investigations**

- Interest coverage (done in `interest_coverage_trend`).
- **Total obligations proxy:** `DebtLongtermAndShorttermCombinedAmount` + `OperatingLeaseLiability` vs equity (claim #8 cites $162B debt, ~$250B with leases).
- Debt issuance vs repayment flows (`ProceedsFromIssuanceOfSeniorLongTermDebt`, `RepaymentsOfDebt`).

**Data**

- `DebtInstrumentCarryingAmount` $43B Q3 FY2026; `DebtLongtermAndShorttermCombinedAmount` $92.6B FY2025; `OperatingLeaseLiability` $21.3B Q3 FY2026.
- `fundamentals.total_debt` = $156.2B (Alpha Vantage).

**Verdict:** Fully SQL-able; run 21 covered coverage ratio but not obligations stack vs equity.

---

#### 4. OpenAI / customer concentration (~$300B of $638B RPO) — **no dedicated experiment**

**What run 21 actually did**

| | |
|---|---|
| **Promoted crux** | **Merged into `rpo_conversion`** (no standalone crux) — title and statement cite ~$300B OpenAI concentration; `breaking_signal` includes “OpenAI announces reduced commitment”; **`linked_claim_ids`: `[]`** (claim #16 not linked) |
| **Experiments** | **`rpo_conversion_funding_sensitivity`** only — SEC conversion math with **no** OpenAI share in `inputs_json` or `assumptions_json`; interpretation notes “OpenAI concentration at ~$300B” as margin-quality risk after concluding funding is ample at 12.7% conversion |
| **Gap** | No concentration stress (0%/25%/50% at-risk scenarios); no claim #16 as `input_type: claim`; no `data_gaps` row for undisclosed customer breakdown; agent effectively **downplays** funding risk in arithmetic, **qualifies** in interpretation |

**Useful investigations**

- **Sensitivity:** If X% of RPO fails to convert or is repriced, what revenue/cash shortfall vs guided capex?
- **Scenario validation:** Stress at 0%, 25%, 50% loss of inferred OpenAI block (~47% of $638B claim base).

**Data**

- Claim #16: OpenAI ≈ $300B of $638B RPO (analyst inference — not company disclosure).
- SEC RPO through Q3: $552.6B.
- **No** customer concentration concepts in catalog or `sec_raw_facts`.
- Run 21 has no `data_gaps` row for OpenAI RPO breakdown.

**Verdict:** Golden-path approach **works** as claim-driven `sensitivity` / `scenario_validation` with `input_type: claim` and provenance — not as auditable SEC arithmetic. Example experiment question: *“At 47% single-customer RPO share (claim), what conversion rate is required to fund $70B guided capex?”*

---

#### 5. Cloud growth deceleration — **crux promoted, zero experiments**

**What run 21 actually did**

| | |
|---|---|
| **Promoted crux** | `cloud_growth_margins` — “Cloud revenue growth trajectory and margin evolution” (statement cites 47% Q4 vs 50% Q3 deceleration and FY27 margin step-down warning) |
| **Experiments** | **None** — crux triage passed gates but mechanics lane attached zero `analysis_experiments` to this `crux_id` |
| **Gap** | Entire recommended investigation missing: no claims-based deceleration series (claims #2, #10), no spread vs total revenue growth from SEC/AV |

**Useful investigations**

- Cloud growth Q3 50% → Q4 47% deceleration (claims #2, #10).
- Spread between total revenue growth (SEC/AV) and cloud growth (claims).

**Data**

- Cloud growth figures exist only in **claims**, not SEC. `SegmentReportingInformationRevenue` in catalog ends 2013.
- Total revenue in SEC/AV through FY2026.

**Verdict:** **Hybrid** — query `claims` or build a small claim-derived series; record `source_note` that SEC has no cloud segment. High-value missed experiment given the promoted crux.

---

#### 6. Agentic AI pricing / margin expansion — **unmapped**

**What run 21 actually did**

| | |
|---|---|
| **Promoted crux** | **None** — narrative crux #6 not mapped to any `crux_candidate` |
| **Experiments** | **None** |
| **Gap** | Full gap — no triage, no supporting metrics, no experiments; only narrative prose |

**Useful investigations**

- Cloud gross margin trajectory; mix shift from capex-heavy IaaS to higher-margin pricing models.

**Data**

- **No** token, agentic, or cloud-segment gross margin concepts.
- SEC `GrossProfit` stops 2018; company-level margins from AV only.

**Verdict:** **Not SQL-investigable** with current workspace. Belongs in narrative + `data_gaps`, not a promoted experiment.

---

#### 7. $40B capital raise vs internal FCF — **unmapped**

**What run 21 actually did**

| | |
|---|---|
| **Promoted crux** | **None dedicated** — narrative folded partially into `capex_roic` statement (“$40B capital raise and $44.5B debt issuance confirm external funding dependency”) |
| **Experiments** | **`capex_funding_gap_forward_projection`** (indirect) — models ~$46B FY27 funding gap; interpretation compares gap to “$40B capital raise + typical debt capacity” but **no** share-count or issuance-proceeds SQL · **`capex_ocf_pressure`** (indirect) — historical external-funding need via capex > OCF |
| **Gap** | No `CommonStockSharesOutstanding` dilution series; no `ProceedsFromIssuanceOfCommonStock` / debt proceeds vs negative FCF; no claim #9 as experiment input; no dilution sensitivity for $20B ATM |

**Useful investigations**

- **Historical:** Equity + debt issuance vs OCF and capex (FY2021–FY2025).
- **Dilution:** `CommonStockSharesOutstanding` growth (2.665B FY2022 → 2.875B Q3 FY2026 ≈ +7.9%).
- **Forward bridge:** Compare modeled ~$46B FY27 funding gap to planned $40B raise — is external funding barely sufficient?
- **Dilution sensitivity:** Model $20B ATM as % of shares outstanding (price gap blocks dollar market-cap dilution).

**Data**

| Source | Available |
|--------|-----------|
| SEC | `CommonStockSharesOutstanding`, `WeightedAverageNumberOfDilutedSharesOutstanding`, `ProceedsFromIssuanceOfCommonStock`, `ProceedsFromIssuanceOfSeniorLongTermDebt`, `NetCashProvidedByUsedInFinancingActivities` |
| Claims | $40B raise (#9), FY2026 negative FCF context in narrative |
| Fundamentals | `shares_outstanding` 2.876B, `total_debt` $156B |

**Verdict:** **Strongest missed opportunity** for pure SQL experiments. Run 21’s `capex_funding_gap_forward_projection` touches the funding story but does not model dilution or issuance proceeds.

---

#### 8. Multicloud database 404% growth — **unmapped**

**What run 21 actually did**

| | |
|---|---|
| **Promoted crux** | **None** — claim #6 (bull, competitive position) exists in `claims` but no `crux_candidate` references multicloud database growth |
| **Experiments** | **None** |
| **Gap** | Full gap — 404% / 29% figures live only in claim #6 and narrative map prose; no `background` crux, no experiment |

**Useful investigations**

- Time series of multicloud DB vs enterprise DB vs total revenue.
- Share of revenue from database vs cloud infrastructure.

**Data**

- **Single claim** (#6): “Multicloud database revenue grew 404%… enterprise database 29%.”
- No multicloud or database segment concepts in catalog.

**Verdict:** **Not SQL-feasible** beyond logging the claim. Needs segment facts in SEC ingestion or multiple quarters captured as structured claims.

---

### Recommended “missing experiments” for run 21 workspace

If mechanics were re-run against the same SQLite file, these would add the most value without new ingestion:

| Priority | Experiment key | Crux anchor | Purpose | Feasibility |
|----------|----------------|-------------|---------|-------------|
| 1 | `dilution_and_external_funding` | capital raise (#7) | historical + forward | **SEC + claims** |
| 2 | `cloud_growth_deceleration` | `cloud_growth_margins` (#5) | historical (claims) | **Hybrid** |
| 3 | `openai_rpo_stress` | concentration (#4) | sensitivity | **Claim-driven** |
| 4 | `proxy_roic_vs_guidance` | `capex_roic` (#2) | sensitivity | **Hybrid** |
| 5 | `margin_vs_backlog_build` | `rpo_conversion` (#1) | historical | **AV + SEC** |

Skip SQL experiments for **agentic pricing** (#6) and **multicloud moat** (#8) until segment data or claim time series exist.

### Experiment coverage gap (run 21)

| Narrative # | Promoted crux | Experiments actually run | Count |
|-------------|---------------|--------------------------|-------|
| #1 RPO + margin | `rpo_conversion` | `rpo_conversion_funding_sensitivity` | 1 |
| #2 ROIC / capex | `capex_roic` | `capex_ocf_pressure`, `capex_funding_gap_forward_projection` | 2 |
| #3 Debt / obligations | `debt_sustainability` | `interest_coverage_trend` | 1 |
| #4 OpenAI concentration | *(merged into `rpo_conversion`)* | *(none dedicated — see #1 experiment interpretation only)* | 0 |
| #5 Cloud deceleration | `cloud_growth_margins` | — | **0** |
| #6 Agentic AI pricing | — | — | **0** |
| #7 $40B raise / dilution | *(partial in `capex_roic` prose)* | *(indirect via `capex_funding_gap_forward_projection` only)* | 0 |
| #8 Multicloud 404% | — | — | **0** |

**Total promoted experiments:** 4 across 3 of 4 promoted cruxes (`cloud_growth_margins` has a promoted crux but zero experiments).

---

## Product Readiness

| Capability | Run 20 | Run 21 |
|------------|--------|--------|
| Deterministic fundamentals from SEC/AV | Yes | Yes |
| Auditable canonical concept links | Yes | Yes |
| Multi-crux falsifiable mechanics | **No** | **Partial** |
| Supporting metric provenance | **No** | **Yes** |
| Forward/sensitivity when guidance present | **No** | **Yes** |
| Scenario-ready without re-fetch | **No** | **Closer** — still needs fresher SEC or explicit staleness flags |

Run 21 is meaningfully closer to supporting scenario-conditioned projections. A downstream scenario agent can now anchor on four bridge archetypes and quantitative forward/sensitivity outputs instead of a single RPO historical thread.

---

## Web Validation

Official sources: [Oracle Q4/FY2026 press release](https://investor.oracle.com/investor-news/news-details/2026/Oracle-Announces-Record-Q4-and-FY-2026-Results-Driven-by-Cloud-Infrastructure--Cloud-Applications/default.aspx), 8-K, earnings call (June 10–11, 2026).

| Field | DB / experiment value | External value | Status |
|-------|----------------------|----------------|--------|
| RPO (latest SEC in DB) | $552.6B (`period_end` 2026-02-28) | $638B Q4 FY2026 (+$85B QoQ) | **Stale in DB** — run 21 assumptions acknowledge; experiments understate backlog base |
| FY2026 capex (SEC annual in DB) | $21.2B (FY2025 annual fact used in historical experiments) | $55.7B FY2026 | **Stale** — historical experiments correctly use SEC through FY2025; miss post-Q4 intensity |
| FY2026 OCF (SEC annual in DB) | $20.8B FY2025; Q3 YTD $17.4B | $32.0B FY2026 | **Stale for forward base** — run 21 forward proj uses FY2025 × 1.15 = $23.9B, materially below $32B actual |
| FY2026 capex/OCF ratio (implied) | Run 21 historical Q3 YTD: 2.26× | ~1.74× ($55.7B / $32.0B) on FY2026 actuals | Directionally right (capex > OCF); YTD SEC ratio overstates vs full-year actual |
| FY27 guided net capex | $70B in claims + experiment assumptions | ~$70B net (excl. $20–25B prepayments) | **Match** |
| Q4 FY2026 revenue | Not in latest SEC revenue fact (`Revenues` ends 2025-05-31) | $19.2B | In claims only |
| Shares outstanding | 2.876B (AV, 2026-05-31) | ~2.88B (post-raise context) | **Approximate** |
| Interest coverage FY2025 | 4.94× (experiment) | Consistent with SEC op income / interest | **Match** |

The gates successfully forced experiments to **cite claims for forward guidance** and document SEC staleness in `assumptions_json`. They did not force use of FY2026 actual OCF where claims contain it — an opportunity for the next iteration.

---

## Side-by-Side Experiment Summary

### Run 20 (all on `rpo_conversion_quality`, all historical)

| Key | Disposition | Notes |
|-----|-------------|-------|
| `rpo_to_revenue_ratio` | promoted | FY ratio 1.36× → 2.40× — sound SQL |
| `rpo_growth_momentum` | promoted | Sequential RPO through Q3 $552.6B — misses Q4 $638B |
| `capex_ocf_pressure` | promoted | FY2025 capex/OCF 1.019× — correct for SEC, understates FY2026 |
| `interest_coverage_pressure` | background | Reasonable demotion |

### Run 21 (multi-crux, mixed purpose)

| Key | Crux | Purpose | Disposition | Notes |
|-----|------|---------|-------------|-------|
| `capex_ocf_pressure` | capex_roic | historical | promoted | Adds Q3 YTD 2.26× ratio; links to funding narrative |
| `interest_coverage_trend` | debt_sustainability | historical | promoted | Coverage 4.94× FY2025; debt carrying $43B Q3 |
| `capex_funding_gap_forward_projection` | capex_roic | forward | promoted | $46B gap at guided capex — conservative OCF base |
| `rpo_conversion_funding_sensitivity` | rpo_conversion | sensitivity | promoted | 12.7% implied conversion vs declining historical rates |

---

## Recommendations

1. **Gates are working — keep them.** Run 21 demonstrates that `crux_coverage_vs_narrative`, `supporting_metrics_present`, and `experiment_purpose_diversity` change agent behavior in the intended direction. Run 20 is a useful negative control.

2. **Add staleness enforcement.** Golden path asks for `data_quality_flags` when SEC lags claims; neither run persisted any. Consider a gate: if `claims` reference metrics newer than `sec_raw_facts.MAX(period_end)` for key concepts, require ≥1 `data_quality_flags` row or experiment `assumptions_json` staleness note on every promoted forward/sensitivity experiment.

3. **Tighten forward baseline selection.** When claims contain FY2026 OCF ($32B), reject forward projections that anchor only on FY2025 SEC without explicit dual-scenario framing.

4. **Narrative mapping completeness.** Optional gate: count of narrative crux items without a promoted or `background` `crux_candidate` referencing `linked_claim_ids` ≤ N. Would surface OpenAI/dilution/moat gaps.

5. **Draft run hygiene.** Reject mechanics lane if any `analysis_runs` remain in `draft` after submit, or auto-discard superseded drafts.

6. **Init workspace unchanged.** Price/market cap gap and SEC filing lag are upstream of exploration gates; fixing ingestion would amplify run 21's modelling quality without further prompt changes.

7. **Cost monitoring.** Crux triage rounds doubled (12 → 23). Consider caching Phase 0 narrative/SEC freshness context in the prompt (already partially done via `load_explorer_context`) to reduce redundant `workspace_sql` orient rounds.

8. **Napkin Math module (speculative).** For claim-only and hybrid cruxes (OpenAI stress, cloud deceleration, dual OCF baselines), consider a dedicated `napkin_math_series` table + write tool so agents persist small derived time series with provenance instead of burying numbers in per-experiment `assumptions_json`. See [Speculative: A “Napkin Math” Module](#speculative-a-napkin-math-module) below.

---

## Bottom Line

The prompt updates and validation gates **were effective**: run 21 would not have passed under the old bar, and it produces a substantially more useful financial exploration artifact — multi-crux triage, auditable supporting metrics, and forward/sensitivity experiments tied to FY27 guidance. The remaining weaknesses (SEC staleness, partial narrative mapping, conservative forward baselines) are now **visible and partially documented** rather than silently masked by a single historical RPO thread. Next iteration should focus on **staleness gates** and **narrative-to-crux completeness**, not rolling back the new requirements.

---

## Speculative: A “Napkin Math” Module

*This section is design speculation, not a committed roadmap. It responds to the gap surfaced above: many high-value narrative cruxes (OpenAI concentration, cloud deceleration, claim-only segment growth) need **small, explicit time series** that do not exist in SEC or AV — and stuffing them only into `assumptions_json` on a single experiment makes them hard to query, reuse, and audit across experiments.*

### Problem it solves

Today the financial explorer can:

1. Query `sec_raw_facts`, `fundamental_observations`, and `claims` via `workspace_sql`.
2. Run arithmetic in `run_analysis_draft` and persist results in `analysis_experiments.outputs_json`.

When the model needs a **derived or claim-sourced series** — e.g. quarterly cloud growth `[50%, 47%]`, OpenAI RPO share `[$300B of $638B]`, or a forward OCF bridge `[$32B actual FY2026, $36.8B +15% scenario]` — it has no durable home. Options today are:

- Embed numbers in experiment `assumptions_json` (opaque to SQL joins).
- Smuggle values into `interpretation` prose (fails arithmetic/interpretation split).
- Re-query `claims` text and re-parse each time (fragile).

A **Napkin Math** layer would let agents **write simple, labeled time series to the DB** with clear provenance, then **join them in later SQL** like any other metric — without pretending they are SEC facts.

### Design principles

1. **Never mix with SEC facts** — separate table or mandatory `source_tier = 'napkin_math'` so gates and UI can treat them differently.
2. **Provenance is required** — every series point links to `claim_id`, `experiment_key`, `assumption_key`, or `rationale` text; optional `confidence` and `period_basis`.
3. **Small and bounded** — cap series length (e.g. 24 points), numeric values only, no free-form blobs.
4. **Reversible** — upsert by `(series_key, period_label)`; support supersede/retract.
5. **Queryable in `workspace_sql`** — experiments should `JOIN napkin_math_series` alongside `sec_raw_facts`.

### Proposed schema (sketch)

```sql
-- Option A: dedicated table (preferred — hard separation from SEC)
CREATE TABLE napkin_math_series (
    id              INTEGER PRIMARY KEY,
    series_key      TEXT NOT NULL,          -- e.g. 'cloud_revenue_growth_yoy'
    series_label    TEXT NOT NULL,          -- human label
    period_label    TEXT NOT NULL,          -- 'Q3_FY2026', 'FY2027_guidance', '2026-05-31'
    period_end      TEXT,                   -- optional ISO date for joins
    metric_value    REAL NOT NULL,
    unit            TEXT NOT NULL,          -- 'ratio', 'USD', 'percent', 'shares'
    period_basis    TEXT,                   -- quarter | annual | instant | scenario
    source_tier     TEXT NOT NULL DEFAULT 'napkin_math',
    provenance_type TEXT NOT NULL,          -- claim | assumption | derived | analyst_note
    provenance_ref  TEXT,                   -- claim_id, experiment_key, or formula ref
    rationale       TEXT NOT NULL,
    created_by      TEXT NOT NULL,          -- worker_name:model
    superseded_by   INTEGER,                -- optional lineage
    created_at      TEXT NOT NULL,
    UNIQUE(series_key, period_label) ON CONFLICT REPLACE
);

CREATE INDEX idx_napkin_math_series_key ON napkin_math_series(series_key, period_end);
```

**Option B:** extend `fundamental_observations` with `source_tier = 'napkin_math'`. Simpler plumbing but risks contaminating canonical fundamentals and confusing gates that expect AV/SEC lineage. A separate table is clearer.

### Agent tool surface (sketch)

New tool: `write_napkin_series` (or `capture_napkin_math`)

```json
{
  "series_key": "cloud_revenue_growth_yoy",
  "series_label": "Cloud revenue YoY growth (from earnings claims)",
  "points": [
    {
      "period_label": "Q3_FY2026",
      "period_end": "2026-02-28",
      "metric_value": 0.50,
      "unit": "ratio",
      "period_basis": "quarter",
      "provenance_type": "claim",
      "provenance_ref": "claims:2",
      "rationale": "Claim #2: cloud revenue up 50% YoY in Q3 FY2026"
    },
    {
      "period_label": "Q4_FY2026",
      "period_end": "2026-05-31",
      "metric_value": 0.47,
      "unit": "ratio",
      "period_basis": "quarter",
      "provenance_type": "claim",
      "provenance_ref": "claims:10",
      "rationale": "Claim #10: decelerated to 47% in Q4"
    }
  ]
}
```

Read path: existing `workspace_sql` — no new read tool needed if schema is documented in `explorer_schema_hint`.

### Example workflows enabled for ORCL

| Gap from this QA | Napkin series | Follow-on experiment |
|------------------|---------------|----------------------|
| Cloud deceleration (#5) | `cloud_revenue_growth_yoy` from claims | SQL: compute Δ growth; join to total revenue from SEC |
| OpenAI concentration (#4) | `openai_rpo_share` = 0.47; `rpo_total` = 638B (claim tier) | Sensitivity: `at_risk_rpo = rpo_total * openai_rpo_share * stress_pct` |
| FY2026 OCF for forward proj | `ocf_scenario` points: actual $32B, conservative $23.9B | Refine `capex_funding_gap_forward_projection` with dual baseline |
| Multicloud 404% (#8) | Single-point `multicloud_db_growth_yoy` = 4.04 | Background crux only; still not a trend, but auditable |
| Dilution (#7) | `shares_issued_scenario` from $20B ATM ÷ assumed price | Join to `CommonStockSharesOutstanding` for fully-diluted path |

### Gate and quality implications

- **`promoted_linked_to_sources`:** Napkin series count as valid `inputs` if `provenance_ref` is set — same bar as `input_type: claim`.
- **New soft gate `napkin_math_labeled`:** Any experiment joining `napkin_math_series` must cite `series_key` in `inputs_json`; warn if `rationale` is empty.
- **Hard rule:** Promoted experiments cannot use napkin math **without** at least one SEC, AV, or `claims` anchor for the same crux — napkin math extends the board; it does not replace filings.
- **UI/report:** Render napkin series with a distinct badge (“Napkin Math”) so readers never confuse with GAAP.

### Relationship to existing artifacts

| Artifact | Role today | With Napkin Math |
|----------|------------|------------------|
| `claims` | Unstructured text + source | Upstream; napkin math **materializes** claim numbers into queryable points |
| `assumptions_json` on `analysis_runs` | Per-experiment scratch | Stays for one-off params; repeated series move to `napkin_math_series` |
| `analysis_experiments.outputs_json` | Experiment results | Still holds ratios/interpretations; inputs may reference napkin keys |
| `sec_raw_facts` | Filed truth | Unchanged; napkin math never writes here |

### Minimal MVP scope

1. One table + one write tool + schema hint update.
2. Golden path addendum: *“When claims provide numbers without SEC series, write `napkin_math_series` first, then draft SQL that joins them.”*
3. One fixture test: cloud growth deceleration from claims → experiment SQL → gate pass.
4. No Monte Carlo / scenario engine coupling until series provenance is stable.

### Risks

- **Garbage persistence** — model invents plausible numbers. Mitigate with `provenance_ref` required and optional judge step that rejects series without claim/filing link.
- **Series explosion** — cap per run (e.g. 20 series, 12 points each).
- **Double counting** — same number in claims, napkin math, and interpretation. Merge lane could dedupe by `series_key`.

### Why this fits the ORCL lesson

Run 21 proved gates can force **forward/sensitivity** experiments, but those experiments still **hand-wave claim-only inputs** inside `assumptions_json`. OpenAI stress, cloud deceleration, and dual OCF baselines are exactly the cases where napkin math would turn narrative tension into **reusable, joinable inputs** — without faking SEC filings or abandoning SQL-first mechanics.

---
