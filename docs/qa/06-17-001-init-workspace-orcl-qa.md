# Init Workspace QA — ORCL (`ORCL-2026-06-17-1`)

## Scope

QA of `initWorkspace` substrate for Oracle Corporation. This run was inspected after downstream narrative and scenario workers had also executed; init-layer findings are separated from post-init artifacts where relevant.

| Field | Value |
|-------|-------|
| SQLite | `reports/stock-narrative-research/ORCL-2026-06-17-1/run.sqlite` |
| Run slug | `ORCL-2026-06-17-1` |
| Schema version | 5 |
| `run_metadata.status` | `initialized` |
| `financial_fetch_status` | `partial` — missing current share price, market cap |
| `created_at` | 2026-06-17T01:05:39Z |
| Workers | 26 recorded (`narrative_researcher`, `financial_model_explorer` ×19, `scenario_builder` ×6) |

Prior related QA: [06-16-001](./06-16-001-init-workspace-price-ttm-orcl-qa.md), [06-15-001](./06-15-001-data-quality-report-orcl.md).

---

## Verdict

**Partial pass.** The workspace is a strong research substrate for Oracle: 25,369 SEC raw facts (513 concepts), 5,643 Alpha Vantage facts, 1,779 curated observations with auditable TTM math, 1,254 daily price bars, and a 516-entry concept catalog including RPO and capex concepts. Headline fundamentals reconcile well to Oracle's Q4 FY2026 earnings release for revenue, net income, cash, and EPS.

The main init gaps are product wiring and semantics: `current_price` and `market_cap` are still missing from `fundamentals` despite usable `daily_price_bars`, company profile fields (exchange, sector, industry) are blank, canonical headline metrics are Alpha Vantage–only (SEC facts are not materialized into observations), and `total_debt` ($156.2B) appears to include operating lease liabilities (~$26.6B) without labeling that distinction.

---

## Workspace Shape

| Table / artifact | Count | Notes |
|------------------|------:|-------|
| `sec_raw_facts` | 25,369 | 513 unique concepts; latest filing 2026-03-11 |
| `av_raw_facts` | 5,643 | Quarterly and balance-sheet time series |
| `fundamental_observations` | 1,779 | All `Alpha Vantage`; includes derived TTM/HLOC/bands |
| `fundamentals` (starter) | 11 | No `current_price` or `market_cap` |
| `daily_price_bars` | 1,254 | 2021-06-17 → 2026-06-16 |
| `concept_catalog_entries` | 516 | RPO, contract liabilities, capex concepts present |
| `canonical_metric_mappings` | 10 | All AV-sourced (`av_deterministic`) |
| `data_gaps` | 29 | Includes open `starter_financials` |
| `data_quality_flags` | 85 | 61 info-level HLOC-unavailable flags |
| `claims` | 28 | Post-init narrative research |
| `scenario_periods` | 100 | Post-init scenario work |
| `generated/report.html` | present | Report artifact exists |

`run_metadata` records workspace and SQLite paths deterministically. Empty downstream placeholders (`content_blocks`, `historical_analogues`, `watch_items`) coexist with populated narrative/scenario tables because this run progressed past init.

---

## What The Workspace Captures Well

- **Run identity:** `run_metadata` records ticker, slug, paths, schema v5, `initialized` status, and explicitly flags `financial_fetch_status = partial` with missing price/market cap.
- **SEC breadth:** 25,369 `sec_raw_facts` across 513 unique `concept_name` values; latest filing date 2026-03-11 (Q3 FY2026 10-Q). Includes `RevenueRemainingPerformanceObligation` ($552.6B as of 2026-02-28), contract liability concepts, and capex-related disclosures.
- **Concept catalog:** 516 entries with fact counts, period shapes, and narrative tags — usable for company-specific scenario mining (RPO, contract liabilities, capex incurred-not-paid).
- **Starter fundamentals (AV-sourced, period 2026-05-31):** Revenue TTM $67.36B, net income TTM $17.09B, EPS TTM $5.94, cash $31.29B, shares 2.876B — all align with Oracle's [Q4 FY2026 release](https://investor.oracle.com/investor-news/news-details/2026/Oracle-Announces-Record-Q4-and-FY-2026-Results-Driven-by-Cloud-Infrastructure--Cloud-Applications/default.aspx).
- **TTM audit:** Latest `revenue_ttm` ($67.358B) equals the sum of four contiguous quarters ($14.93B + $16.06B + $17.19B + $19.18B). Internally consistent.
- **Price custody:** `daily_price_bars` through 2026-06-16 close $188.33, with `source_type`, `fetched_at`, OHLCV, and adjusted close.
- **Derived time series:** Quarterly HLOC, P/E bands, P/S bands, rolling TTM windows, and explicit `av_quarter_price_hloc_unavailable_for_*` info flags for pre-coverage quarters.
- **Gap/flag persistence:** `starter_financials` gap is open; quality flags document coverage limits and RPO/OpenAI disclosure boundaries.

---

## TTM Audit

### Latest window (period end 2026-05-31)

| Metric | DB value | Sum of 4 quarters | External (Oracle FY2026) | Status |
|--------|----------|-------------------|--------------------------|--------|
| Revenue TTM | $67.358B | $67.358B | $67.4B (earnings release) | Match |
| Net income TTM | $17.087B | $17.087B | $17.0B GAAP (earnings release) | Match |
| Gross profit TTM | $44.329B | $44.329B | not separately cited | Internally consistent |
| Operating income TTM | $20.778B | — | $20.6B GAAP FY2026 | Close (+0.9%) |
| EPS TTM (derived) | $5.941 | $17.087B ÷ 2.876B shares | $5.83 GAAP FY2026 EPS | Methodology difference |
| EPS (AV overview row) | $5.84 | — | Yahoo Finance TTM EPS $5.84 | Match |

### Quarterly spot check (Q4 FY2026, ended 2026-05-31)

| Metric | DB | External | Status |
|--------|-----|----------|--------|
| Revenue | $19.184B | $19.18B (CNBC / 8-K) | Match |
| Net income | $4.304B | $4.2B GAAP (earnings release) | Close (+2.5%; AV vs filed) |
| Derived quarter EPS | $1.50 | $1.45 GAAP | Close (+3%; shares methodology) |

### TTM methodology notes

- TTM windows require four contiguous quarters with 80–100 day gaps between period ends. Latest revenue TTM sums cleanly.
- **EPS TTM ($5.941) vs AV overview ($5.84):** Derived EPS uses trailing net income divided by quarter-end shares outstanding (2.876B). AV/Yahoo use diluted weighted-average shares over the fiscal year. Both are defensible; the ~1.7% gap matters for precise P/E comparisons.
- **Pre-existing duplicate `net_income_quarter` rows** from Alpha Vantage ingestion inflate row counts without changing sums.

---

## Debt and Balance Sheet Audit

### `total_debt` semantics

| Component | DB (`fundamentals.total_debt`) | Filed balance sheet (2026-05-31) | Source |
|-----------|-------------------------------|----------------------------------|--------|
| Notes payable, current | included | $7.199B | Oracle Q4 FY2026 tables |
| Notes payable, non-current | included | $122.342B | Oracle Q4 FY2026 tables |
| Operating lease liabilities | **appears included** | $26.648B | Oracle Q4 FY2026 tables |
| **Total** | **$156.189B** (AV `shortLongTermDebtTotal`) | **$129.541B** (notes only) | — |

The $26.65B gap between headline `total_debt` and filed notes payable equals operating lease liabilities exactly ($7.199B + $122.342B + $26.648B = $156.189B). Downstream net-debt math will be wrong if agents treat `total_debt` as financial debt alone.

### Component debt observations

| Field | Latest period in observations | Value | Issue |
|-------|------------------------------|-------|-------|
| `debt_current` | 2026-05-31 | $7.199B | Matches filed current notes |
| `debt_noncurrent` | 2026-02-28 | $124.718B | Stale vs May balance sheet ($122.342B non-current notes) |
| `cash` | 2026-05-31 | $31.289B | Exact match to filed cash |

---

## Data Quality Findings

### Critical

_None identified for init substrate integrity. Headline revenue, income, and cash are materially correct._

### High

- **`current_price` / `market_cap` absent from `fundamentals`** despite 1,254 daily bars and derivable values (~$188 × 2.876B ≈ $541B). `run_metadata.financial_fetch_status` correctly says partial, but downstream agents querying `fundamentals` alone will miss valuation context.
- **`total_debt` semantics ambiguous.** DB headline is notes + operating leases (~$156.2B) but is not labeled as such. Filed financial debt (notes only) is $129.5B. Net-debt and leverage scenarios can be materially mis-stated.

### Medium

- **SEC facts not linked to canonical observations.** All 1,779 `fundamental_observations` are `Alpha Vantage`; 25k SEC facts exist only in `sec_raw_facts` + catalog. Canonical mappings are AV-only (`av_deterministic`). SEC-sourced headline validation/audit paths are missing.
- **SEC filing lag on Q4 FY2026.** Latest SEC fact `period_end` is 2026-02-28 (Q3). Q4 earnings (Jun 10, 2026) RPO of $638B is not yet in `sec_raw_facts`; catalog still shows RPO max $552.6B. Expected until 10-K/8-K facts ingest.
- **`canonical_key='revenue'` mixes dollar and per-share series.** `metric_key` distinguishes `revenue_quarter` vs `revenue_per_share_quarter`, but naive `canonical_key` queries return values from $6.67 to $67.4B under the same key.
- **`debt_noncurrent` observations stale at 2026-02-28** while `debt_current` and `total_debt` are at 2026-05-31. Component debt series are period-mismatched.
- **Company profile incomplete.** `stock_info` has ticker, name, USD currency, but exchange, sector, and industry are blank with no logged gap.
- **Valuation history truncated to ~20 quarters.** Alpha Vantage daily output window (~1,825 days) limits HLOC and P/E/P/S bands to fiscal quarters after mid-2021.

### Low

- **Duplicate observation rows** (e.g., cash and debt_current appear twice at 2026-05-31). Does not break sums but adds query noise.
- **61 info-level HLOC flags** clutter `data_quality_flags` (85 total). Consider collapsing into a single summary flag.
- **EPS methodology split.** Derived EPS TTM $5.94 vs Oracle filed GAAP $5.83 — expected (quarter-end shares vs diluted weighted average). Should be documented for P/E band interpretation.

---

## Product Readiness

| Need | Status |
|------|--------|
| Deterministic daily prices | **Yes** — `daily_price_bars` with provenance |
| Quarterly price HLOC | **Yes** — 20 recent quarters |
| Quarterly P/E and P/S bands | **Yes** — 20 recent quarters, auditable |
| Rolling TTM time series | **Yes** — sums verified against quarterly inputs |
| Starter spot price / market cap | **No** — gap persists despite bars |
| SEC facts for narrative mining | **Yes** — 513 concepts, strong catalog |
| SEC-linked canonical observations | **No** — AV-only observation layer |
| Auditable debt breakdown | **Partial** — total includes leases; components stale/mismatched |
| Company profile baseline | **Partial** — name and currency only |
| Scenario / narrative hooks | **Yes** — gaps, flags, claims, concept catalog populated |

A later research agent can build a credible Oracle-specific scenario report from this workspace without re-fetching SEC facts or quarterly fundamentals. It should not silently trust `fundamentals.total_debt` for net-debt analysis, and it must derive price/market cap from `daily_price_bars` until the starter wiring gap is closed.

---

## Web Validation

| Field | DB value | External value | Source | Matters? |
|-------|----------|----------------|--------|----------|
| Company | Oracle Corporation | Oracle Corporation | Oracle IR | No |
| Currency | USD | USD | Oracle filings | No |
| Price (2026-06-16) | $188.33 close | $191.62 intraday; $192.64 prior close | [Oracle IR](https://investor.oracle.com/stock-information/default.aspx) | **Yes** — not in `fundamentals` |
| Shares outstanding | 2.876B | ~2.88B | Google Finance / Oracle IR | No (~1.4%) |
| Market cap | *missing* | ~$546–559B | Oracle IR / Google Finance | **Yes** — derivable as ~$541B |
| Q4 FY2026 revenue | $19.184B | $19.18B | [CNBC](https://www.cnbc.com/2026/06/10/oracle-orcl-q4-earnings-report-2026.html) / Oracle 8-K | No |
| FY2026 revenue TTM | $67.358B | $67.4B | Oracle earnings release | No |
| FY2026 net income | $17.087B | $17.0B GAAP | Oracle earnings release | No |
| FY2026 GAAP EPS | $5.94 derived / $5.84 AV | $5.83 | Oracle earnings release | Low — methodology |
| Cash (2026-05-31) | $31.289B | $31.289B | [Oracle balance sheet](https://s23.q4cdn.com/440135859/files/doc_earnings/2026/q4/supplemental-info/Q426_Form8K_Exhibit99-1_Earnings_Release_Tables-Final.pdf) | No — exact match |
| Current debt | $7.199B | $7.199B notes payable current | Oracle balance sheet | No |
| Non-current notes | not in `fundamentals` | $122.342B | Oracle balance sheet | Medium — missing headline |
| Total debt (headline) | $156.189B | $129.541B (notes only) | Oracle balance sheet | **Yes** — includes ~$26.6B leases |
| RPO | $552.6B (SEC, Q3) | $638B (Q4 earnings) | SEC 10-Q vs earnings release | Medium — expected filing lag |

---

## Recommendations

1. **Wire `current_price` and `market_cap` into `fundamentals`** from latest `daily_price_bars` close × `shares_outstanding`; close the `starter_financials` gap automatically when bars exist.
2. **Split or relabel `total_debt`:** persist `notes_payable_total` and `operating_lease_liabilities` separately; derive `total_debt_incl_leases` explicitly if that is the intended AV mapping.
3. **Add SEC canonical mappings** for at least revenue, net income, cash, and debt concepts so `fundamental_observations` can be SEC-audited alongside AV.
4. **Materialize post-earnings SEC facts** after Q4 8-K filing so RPO and balance-sheet concepts reach 2026-05-31 in `sec_raw_facts`.
5. **Use distinct `canonical_key` for per-share metrics** (e.g., `revenue_per_share`) instead of sharing `revenue`.
6. **Populate `stock_info` exchange/sector/industry** from overview provider or SEC `dei` taxonomy facts; log gaps when unavailable.
7. **Align `debt_noncurrent` period** with latest balance-sheet date when AV `longTermDebt` lags but `shortLongTermDebtTotal` is current.
8. **Collapse HLOC-unavailable flags** into one summary quality flag per run instead of 61 per-quarter info rows.
9. **Add automated QA tests** that assert: (a) latest TTM window sums match quarterly inputs, (b) `current_price` is populated when bars exist, (c) `total_debt` components reconcile to headline with explicit lease treatment.
