# Data Quality Report — ORCL `ORCL-2026-06-15-1`

## Scope

End-to-end QA of a **complete** `initWorkspace` run on Oracle (`ORCL`), including initialization substrate, narrative/financial fan-out, and scenario generation. This run completed successfully (unlike `ORCL-2026-06-14-2`, which failed scenario detail on 4/5 scenarios).

| Field | Value |
|-------|-------|
| SQLite | `reports/stock-narrative-research/ORCL-2026-06-15-1/run.sqlite` |
| Run slug | `ORCL-2026-06-15-1` |
| Schema version | 5 |
| `run_metadata.status` | `initialized` |
| `run_metadata.financial_fetch_status` | `partial` — missing current share price, market cap |
| `run_metadata.created_at` | 2026-06-15T02:08:22Z |
| Quality gates | **30 pass / 1 warn** (all lanes through `scenario_generation`) |
| Workers | **25 / 25 success**; 311 agent rounds; ~$0.55 cost; 622 tool calls |

Compared to prior ORCL QA:

- [06-08-001](./06-08-001-data-quality-report.md) — SEC substrate strong; headline TTM mislabeling and mixed-frequency market cap.
- [06-14-001](./06-14-001-worker-telemetry-orcl-qa.md) — same pipeline, scenario lane failed (empty completion on 2 workers).

Deep dive on scenario data pathing: [06-15-002](./06-15-002-scenario-traceability-orcl.md).

This run is the first inspected **full-pipeline** ORCL workspace with persisted scenarios and Monte Carlo.

---

## Verdict

**Partial pass.** The workspace is a strong, company-specific research substrate for Oracle's post–Q4 FY2026 AI/capex narrative: broad SEC Facts custody, rich concept catalog, explicit gaps/flags, promoted cruxes, and five crux-driven scenarios. It is **not yet trustworthy as a self-contained fundamentals or valuation source** — price and market cap are missing, the canonical layer is Alpha Vantage–only despite 25k SEC facts, and scenario projections omit the core funding-stack metrics (OCF, CapEx, FCF, debt, RPO).

---

## Workspace Inventory

| Table / artifact | Rows | Notes |
|------------------|-----:|-------|
| `sec_raw_facts` | 25,369 | 513 unique concepts |
| `av_raw_facts` | 5,643 | 73 fields across IS/BS/CF/OVERVIEW |
| `concept_catalog_entries` | 516 | Usability tags, 31 RPO-related concepts |
| `fundamental_observations` | 807 | All `Alpha Vantage` source |
| `fundamentals` | 11 | No price, no market cap |
| `canonical_metric_mappings` | 9 | All `alpha-vantage` taxonomy |
| `data_gaps` | 17 | Open |
| `data_quality_flags` | 22 | SEC lag vs claims well documented |
| `sources` | 7 | Q4 FY2026 earnings, 8-K, news |
| `claims` | 19 | |
| `crux_candidates` | 9 | All promoted |
| `analysis_experiments` / `analysis_runs` | 25 each | |
| `scenario_assumptions` | 5 | P sums to 1.0 |
| `scenario_periods` | 92 | 16–20 quarters per scenario |
| `scenario_signals` | 53 | Confirming + breaking per scenario |
| `scenario_crux_assumptions` | 36 | |
| `scenario_sensitivities` | 24 | |
| `monte_carlo_summary` | 1 | Median terminal implied price ~$47 |
| `sections` | 11 | Placeholder keys only |
| `content_blocks` | 0 | |
| `artifacts` | 0 | `generated/` empty |

---

## What The Workspace Captures Well

- **Run identity and partial-fetch honesty:** `run_metadata` records paths, schema v5, and explicitly flags missing price/market cap. Matching `data_gaps.starter_financials` is open.
- **SEC Facts breadth:** 25,369 raw facts, 513 concepts — appropriate for a mega-cap filer with RPO, capex, debt issuance, leases, and segment-adjacent disclosures.
- **Alpha Vantage time series:** 5,643 observations across income statement, balance sheet, cash flow, and overview — enables scenario anchoring on FY2026 actuals.
- **Concept catalog for narrative mining:** 516 entries with `series_usability`, `plot_readiness`, and `narrative_tags` (debt, lease, dilution, etc.). `RevenueRemainingPerformanceObligation` is `plot_ready` (latest SEC $552.6B at 2026-02-28).
- **Transparency layer:** 17 gaps and 22 quality flags correctly distinguish SEC-filed Q3 FY2026 data from management Q4 FY2026 claims ($638B RPO, $75B prepayments, FY2027 $90B guidance).
- **Narrative and crux scaffolding:** Dominant narrative (cautious skepticism post-earnings), bull/bear debate, 9 falsifiable cruxes, 25 mechanics experiments with SQL bodies.
- **Scenario design:** Five mutually distinct, ORCL-specific storylines tied to promoted cruxes — OpenAI concentration shock, build-out execution, leverage spiral, RPO acceleration, gradual optimization. Headline FY2027 revenue paths align with narrative ($63B / $87B / $90B).
- **Pipeline completeness:** All lanes pass quality gates; scenario generation completes (contrast with 06-14-001).

---

## Data Quality Findings

### Critical

- **No `current_price` or `market_cap` in `fundamentals`.** `financial_fetch_status = partial`. Yahoo price fetch noted in `stock_info.source_note` but not persisted. Dilution, capital-burden, and Monte Carlo return math cannot anchor to spot. Gap `narrative_current_market_cap` cites ~$530B at $184.13 from web sources only.

### High

- **Canonical layer is 100% Alpha Vantage.** All 9 `canonical_metric_mappings` and all 807 `fundamental_observations` use `source_type = Alpha Vantage`. SEC facts are ingested but not linked to canonical metrics — canonicalization is not filing-auditable despite raw custody.
- **`debt_noncurrent` missing at latest period.** At `2026-05-31`, `debt_current` = $7.2B exists but AV `longTermDebt` has no row; latest `debt_noncurrent` stops at `2026-02-28` ($124.7B). Canonical debt split is incomplete at FY end.
- **`total_debt` ($156.2B) definition is ambiguous.** AV `shortLongTermDebtTotal` likely includes operating lease liabilities (~$26.6B non-current per FY2026 release) vs SEC notes payable of $7.2B current + $122.3B non-current (~$129.5B). Methodology not surfaced in `fundamentals.source_note`.
- **SEC facts lag narrative claims by one quarter.** Latest `sec_raw_facts` period_end is `2026-02-28` (Q3 FY2026 10-Q). Q4 FY2026 results (May 31) exist in AV/claims but not in SEC raw layer. Expected until 10-K ingestion; flags document it.

### Medium

- **`stock_info` profile incomplete:** ticker, company name, USD currency only — no exchange, sector, industry, or CIK.
- **Duplicate observations:** 140 duplicate groups in `fundamental_observations` (e.g., 4× `net_income` at `2026-05-31`). Risk of double-counting in naive queries.
- **EPS / shares metric mismatch:** Starter `eps_ttm` = $5.84 (AV `DilutedEPSTTM`) vs Q4 non-GAAP EPS $2.03 / FY non-GAAP $6.83 from earnings release. `shares_outstanding` = 2.876B vs Q4 diluted weighted average ~2.915B — different concepts.
- **FY2026 Q2 anchor anomaly in scenarios:** Scenario periods show Q2 FY2026 net margin 38.2% / EPS $2.10 — likely one-time investment gains not normalized before forward projection.
- **Scenario projection grid omits core crux metrics:** `scenario_periods` has revenue, margins, EPS, shares, terminal multiples — but no OCF, CapEx, FCF, debt, interest, or RPO. Funding-gap math lives in prose only.
- **Terminal valuation inconsistencies:** Scenario 3 `source_note` claims terminal blend ~$233/share; stored P/S and P/E bands imply ~$65. Monte Carlo median ~$47 vs ~$184 spot (no persisted anchor).
- **Label / horizon inconsistency across scenarios:** Mix of `FY2026 Q1` vs `FY26 Q1` vs `FY2030 Q4 (Terminal)`; scenarios 2 and 5 have 16 periods, others 20.

### Low

- **Empty report artifacts:** `artifacts` = 0; `content_blocks` = 0; `generated/` empty.
- **Scenario gate warning:** `quarterly_cadence_labeled` — target 16+ quarterly periods per scenario with cadence labels; minimum found 0.
- **P/E and P/S only on terminal quarter:** 19 of 20 periods per scenario have null valuation bands.

---

## Scenario Quality (Summary)

Five scenarios, probability-weighted 45% bullish / 30% bearish / 25% neutral:

| # | Key | Stance | P | FY2027 rev (sum of quarters) | Terminal EPS | Terminal implied price (blend) |
|---|-----|--------|---|------------------------------|--------------|--------------------------------|
| 1 | RPO acceleration | Bullish | 20% | ~$90B | $3.01 | ~$60–82 |
| 2 | OpenAI shock | Bearish | 15% | ~$63B | $0.82 | ~$15–22 |
| 3 | Build-out on track | Bullish | 25% | ~$90B | $2.96 | ~$65 |
| 4 | Leverage spiral | Bearish | 15% | ~$90B | $0.15 | ~$2–3 (0.4× P/S) |
| 5 | Gradual optimization | Neutral | 25% | ~$87B | $1.11 | ~$33–55 |

**Strengths:** Company-specific causal chains; crux-linked assumptions; falsifiable confirming/breaking signals; quantitative sensitivities; FY2027 revenue sums match narrative descriptions.

**Weaknesses:** No OCF/CapEx/FCF in period grid; terminal-only valuation bands; Monte Carlo heavily bear-skewed by scenario 4 distressed multiples; note vs stored band mismatches on bull terminals.

---

## Product Readiness

| Capability | Status |
|------------|--------|
| Deterministic company identity | Partial — name/ticker OK; exchange/CIK/sector missing |
| Market quote baseline | **Fail** — price and market cap not ingested |
| SEC Facts universe for discovery | **Pass** — 513 concepts, RPO/capex/debt/lease coverage |
| Canonical concept linking | **Partial** — mappings exist but AV-only; no SEC traceability |
| Starter fundamentals | **Partial** — TTM headline set at FY2026 end; debt composition unclear |
| Provenance / auditability | **Partial** — SEC raw JSON retained; canonical path not filing-linked |
| Narrative / crux / experiment layer | **Pass** — sources, claims, cruxes, experiments populated |
| Scenario-conditioned projections | **Partial** — strong narrative scaffolding; weak funding-stack math |
| Monte Carlo / price distribution | **Weak** — terminal-only sampling; no spot anchor; bear-skewed |

A research agent **can** build a credible Oracle memo from concept catalog, SEC raw facts, claims, and crux experiments **without re-fetching filings**. It **cannot** safely treat `fundamentals` or scenario terminal prices as filing-grounded or spot-anchored without cross-checking.

---

## Web Validation

| Field | DB Value | External Value | Source | Status |
|-------|----------|----------------|--------|--------|
| Company | Oracle Corporation (ORCL) | Oracle Corporation (NYSE: ORCL) | [Oracle IR](https://investor.oracle.com/stock-information/default.aspx) | Match |
| FY2026 revenue | $67.358B TTM (`revenue_ttm`) | $67.4B FY2026 total revenue | [Q4 FY2026 release](https://investor.oracle.com/investor-news/news-details/2026/Oracle-Announces-Record-Q4-and-FY-2026-Results-Driven-by-Cloud-Infrastructure--Cloud-Applications/default.aspx) | Match (~0.06%) |
| Q4 FY2026 revenue | $19.184B (AV) | $19.2B reported | Same release | Match |
| Cash (FY end) | $31.289B | $31,289M | Same release (balance sheet) | Match |
| Current borrowings | $7.199B | $7,199M notes payable, current | Same release | Match |
| Non-current borrowings | — (gap at FY end in obs.) | $122,342M | Same release | **Gap in canonical obs.** |
| Total debt (headline) | $156.189B | ~$129.5B notes + ~$26.6B operating leases | Release balance sheet | **Explainable diff** — label needed |
| RPO (SEC) | $552.6B (2026-02-28) | $553B Q3 FY2026 | SEC 10-Q via DB | Match |
| RPO (narrative) | Claims cite $638B | $638B at Q4 close | Earnings release | **SEC lag** — claim not yet filed |
| EPS (starter) | $5.84 TTM | Q4 non-GAAP $2.03; FY non-GAAP $6.83 | Earnings release | **Different metric** |
| Shares | 2,876M outstanding | 2,915M diluted WA (Q4) | Earnings release | **Different metric** |
| Price | **Missing** | $184.13 (Jun 12, 2026 close) | Oracle IR | **Gap** |
| Market cap | **Missing** | ~$529.6B | Oracle IR | **Gap** |

---

## Recommendations

### Initialization (`initWorkspace`)

1. **Fix quote ingestion** — persist `current_price`, `price_as_of`, `price_source`, and `market_cap` (common shares × price); close `starter_financials` and `narrative_current_market_cap` gaps.
2. **Add SEC-backed canonical mappings** alongside AV (revenue, cash, debt, shares, OCF, capex, RPO) with accession provenance.
3. **Clarify `total_debt` definition** in `fundamentals.source_note`; populate `debt_noncurrent` at latest period or derive from `shortLongTermDebtTotal − shortTermDebt`.
4. **Enrich `stock_info`** with CIK (0001341439), exchange (NYSE), sector/industry.
5. **Deduplicate `fundamental_observations`** on ingest (`canonical_key` + `period_end` + `concept_name`).
6. **Separate metric keys** for `eps_gaap_ttm` vs `eps_non_gaap`, `shares_outstanding` vs `shares_diluted_wa`.
7. **Re-fetch SEC facts** after FY2026 10-K filing to close Q4 gap in `sec_raw_facts`.

### Scenario generation

8. **Extend `scenario_periods`** with OCF, CapEx, FCF, net debt, and RPO (or link periods to experiment outputs that compute them).
9. **Normalize FY2026 actuals anchors** before projection (strip one-time investment gains from Q2 margin/EPS).
10. **Harmonize period labels** (`FY2026 Q1` everywhere) and horizons (16 vs 20 quarters).
11. **Reconcile terminal `source_note` claims with stored P/S and P/E bands** before Monte Carlo sampling.
12. **Anchor Monte Carlo to spot price** — express distribution as return from `current_price` when available.

### QA / regression

13. **Add mega-cap cloud regression** — verify price/market cap, latest-quarter SEC refresh after earnings, and FY revenue within 1% of IR release.
14. **Gate on canonical source mix** — warn when 100% of canonical observations are non-SEC despite SEC ingestion.
15. **Scenario completeness gate** — require OCF/CapEx columns or explicit gap record when cruxes reference funding gap.

---

## Summary

`ORCL-2026-06-15-1` is the strongest full-pipeline ORCL workspace inspected to date: complete scenario generation, rich narrative/crux layer, and excellent SEC Facts custody with honest gap/flag documentation. The main weaknesses are unchanged from earlier init QA themes — **missing market data**, **AV-only canonical layer**, and **ambiguous debt semantics** — plus new scenario-layer gaps around **funding-stack metrics** and **terminal valuation coherence**. Treat `fundamentals` and Monte Carlo output as directional narrative scaffolding, not deterministic valuation input, until quote ingestion and SEC canonical linking are fixed.
