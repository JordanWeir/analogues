# Data Quality Report — MSFT initWorkspace (2026-06-08)

## Scope

QA inspection of one Microsoft initialization run:

| Run | SQLite path | Created (UTC) | Invocation |
|-----|-------------|---------------|------------|
| 1 | `reports/stock-narrative-research/MSFT-2026-06-08-1/run.sqlite` | 2026-06-08T22:57:29 | `initWorkspace` (ticker MSFT) |

## Verdict

**Partial pass.** The workspace has a strong SEC Facts substrate and a usable observation layer for core income-statement metrics through **Q3 FY2026** (filed 2026-04-29). The headline `fundamentals` table is misleading: it is anchored to **2021-03-31 TTM** values, debt canonicalization is broken, and market/profile fields are missing. A downstream agent querying `fundamentals` alone would get stale data; one querying `fundamental_observations` or `sec_raw_facts` would do much better.

## What The Workspace Captures Well

- **Run identity:** `run_metadata` records ticker `MSFT`, slug `MSFT-2026-06-08-1`, schema v2, status `initialized`, and paths to the SQLite file.
- **SEC breadth:** **32,070** raw facts across **547** unique concepts; periods from 2007 through **2026-03-31**; latest filing **2026-04-29**.
- **Concept catalog:** **552** entries with labels, fact counts, period shapes, and usability tags — suitable for later "interesting concept" discovery (e.g. `RevenueRemainingPerformanceObligation` at **$633B** as of 2026-03-31).
- **Core income-statement observations match filings.** Latest quarter (2026-03-31): revenue **$82.9B**, net income **$31.8B**, operating income **$38.4B**, EPS **$4.27** — all match Microsoft's Q3 FY2026 press release.
- **Canonical mappings for core P&L are sensible.** Revenue → `RevenueFromContractWithCustomerExcludingAssessedTax`, EPS → `EarningsPerShareDiluted`, etc., with confidence, rationale, and `catalog_candidate_scoring` provenance.
- **Provenance:** Raw facts retain accession, form, filed date, fiscal period, and `raw_json` payloads.
- **Gaps and flags persisted:** `financial_fetch_status=partial` with open `starter_financials` gap for price/market cap; quality flags document EPS/shares period mismatches.

## Data Quality Findings

### Critical

- **`fundamentals` starter table is 5+ years stale.** All 11 rows use period `2021-03-31` (e.g. revenue TTM **$160.0B** vs actual ~$350B+ TTM today). Root cause: TTM bundle selection in `select_latest_income_bundle` appears to fail for MSFT's June fiscal calendar, falling back to the last period where revenue + net income TTM align.

- **`total_debt` is materially wrong ($10.75B).** Canonical debt maps to deprecated concepts: `ShortTermBorrowings` (latest 2018) and `LongTermNotesPayable` (latest 2012). Raw facts have current debt via `LongTermDebt` (**$40.3B**), `LongTermDebtCurrent` (**$8.8B**), and `LongTermDebtNoncurrent` (**$31.4B**) as of 2026-03-31.

### High

- **Missing market data.** No current price or market cap. `run_metadata.financial_fetch_status=partial`; gap logged correctly, but headline valuation metrics are absent.

- **`stock_info` profile incomplete:** `exchange`, `sector`, `industry` blank; no CIK. Company name is `MICROSOFT CORP`; currency `USD` is present.

- **Starter shares are period-mismatched.** `shares_outstanding` = **7.61B** tied to 2021 bundle period; latest diluted weighted average is **7.45B** (2026-03-31). Quality flags exist but the stored value is still misleading.

### Medium

- **Debt canonical concepts need review.** `debt_current` confidence is `low`; mappings were never reviewed (`concept_review_decisions` empty). Better candidates exist in the catalog (`LongTermDebtCurrent`, `LongTermDebtNoncurrent`, `LongTermDebt`).

- **No segment/Azure revenue in XBRL layer.** Only generic segment metadata (`NumberOfReportableSegments`). Segment breakdown lives outside standard us-gaap concept names — expected for MSFT, but limits automated scenario metrics unless supplemented.

- **Duplicate/multi-context facts.** Same period/concept can have multiple values (e.g. OCF Q3 2026 shows **$127.5B** and **$46.7B**; capex shows **$80.1B** and **$30.9B**) — likely YTD vs quarter or different statement contexts. Downstream queries need `period_type` filtering.

### Low

- **`generated/` directory empty**; 11 `sections` are `pending` placeholders — fine for init-only, but no chart/HTML artifacts yet.

- **Generic `source_note` on all fundamentals rows.** Same boilerplate text regardless of metric source.

## Product Readiness

| Capability | Status |
|------------|--------|
| Deterministic fundamentals (headline) | **Weak** — `fundamentals` table unreliable |
| Time-series observations | **Strong** — 1,827 observations through Q3 FY2026 |
| Raw SEC concept discovery | **Strong** — 547 concepts; RPO/capex/OCF available |
| Canonical concept audit trail | **Partial** — P&L good; debt broken; no LLM review pass |
| Provenance | **Strong** — accession, form, filed_at, raw_json |
| Market/valuation context | **Missing** — price, market cap, exchange |
| Narrative/scenario scaffolding | **Ready** — 11 pending sections; empty narrative/scenario tables as expected |
| Scenario-specific metrics | **Partial** — RPO and capex in raw layer but not canonicalized |

A research agent could build a credible financial snapshot from `fundamental_observations` + `concept_catalog_entries`, but would be misled if it reads `fundamentals` or `total_debt` without checking the observation layer.

## Web Validation

| Field | DB Value | External Value | Source | Status |
|-------|----------|----------------|--------|--------|
| Q3 FY2026 revenue (2026-03-31) | $82.886B | $82.9B | [MSFT Q3 FY26 press release](https://www.microsoft.com/en-us/investor/earnings/fy-2026-q3/press-release-webcast) | Match |
| Q3 FY2026 net income | $31.778B | $31.8B | Same | Match |
| Q3 FY2026 operating income | $38.398B | $38.4B | Same | Match |
| Q3 FY2026 diluted EPS | $4.27 | $4.27 | Same | Match |
| FY2025 annual revenue (2025-06-30) | $281.724B | ~$281.7B | Same | Match |
| Cash (2026-03-31) | $32.105B | ~$32.1B (incl. restricted) | SEC 10-Q / press release | Match (broader cash concept) |
| Long-term debt (2026-03-31, raw) | $40.262B | ~$40.3B | SEC balance sheet | Match |
| Total debt (starter) | $10.75B | ~$40.3B | SEC | **Mismatch — wrong canonical concepts** |
| Revenue TTM (starter) | $160.0B | ~$350B+ implied | Filings | **Mismatch — stale TTM bundle** |
| RPO (2026-03-31) | $633B | ~$627B commercial RPO cited | Earnings call | Close; DB may include total vs commercial-only |
| Current price | missing | ~$410.56 | StockAnalysis (2026-06-08) | Missing in DB |
| Market cap | missing | ~$3.05T | Same | Missing in DB |
| Shares outstanding | 7.61B (starter) / 7.45B (obs.) | ~7.43B | Same | Starter stale; observations closer |
| Exchange / sector | blank | NASDAQ / Technology | Same | Missing in DB |

## Recommendations

1. **Fix TTM bundle selection for non-calendar fiscal years.** MSFT's June FY breaks contiguous calendar-quarter TTM windows. Prefer fiscal-quarter TTM, latest annual fallback, or latest quarter + YTD rather than landing on 2021-03-31.
2. **Remap debt canonical metrics** to `LongTermDebtCurrent` + `LongTermDebtNoncurrent` (or `LongTermDebt`) — validated through 2026-03-31. Flag the old mappings inactive.
3. **Repair or regenerate `fundamentals` after TTM/debt fixes**, or add a `derived_at` / `as_of_period` column so stale rows are obvious.
4. **Resolve market data fetch failure** (Yahoo chart endpoint noted in `source_note`) — price, quote timestamp, and market cap are high-impact for narrative work.
5. **Enrich `stock_info`** with exchange (NASDAQ), sector/industry, CIK, and fiscal year end — log gaps when unavailable.
6. **Run concept review pass** (`concept_review_decisions`) especially for low-confidence debt mappings.
7. **Add canonical mappings for scenario-relevant metrics** already in raw data: OCF, capex, FCF, RPO — at least as catalog highlights, not necessarily starter fundamentals.
8. **Deduplicate or rank observations** when multiple facts share period/concept (YTD vs quarter, multiple accession revisions).

## Key Table Counts

| Table | Rows |
|-------|------|
| `sec_raw_facts` | 32,070 |
| `concept_catalog_entries` | 552 |
| `fundamental_observations` | 1,827 |
| `fundamentals` | 11 |
| `canonical_metric_mappings` | 9 |
| `data_gaps` | 1 |
| `data_quality_flags` | 2 |
| `sections` | 11 (all pending) |
