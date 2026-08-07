# Init Workspace QA — ORCL Price, HLOC, Valuation Bands, and TTM (`ORCL-2026-06-16-1`)

## Scope

QA of `initWorkspace` after upgrades to capture daily price bars, quarterly price HLOC, quarterly P/E and P/S min/max bands, and improved TTM derivation. Compared against the prior ORCL init run without those features.

| Field | New run | Prior run |
|-------|---------|-----------|
| SQLite | `reports/stock-narrative-research/ORCL-2026-06-16-1/run.sqlite` | `reports/stock-narrative-research/ORCL-2026-06-15-1/run.sqlite` |
| Run slug | `ORCL-2026-06-16-1` | `ORCL-2026-06-15-1` |
| Schema version | 5 | 5 |
| `run_metadata.status` | `initialized` | `initialized` |
| `financial_fetch_status` | `partial` — missing current share price, market cap | `partial` — same |
| `created_at` | 2026-06-16T06:08:34Z | 2026-06-15T02:08:22Z |
| Workers | 22 / 22 success (init-only) | 25 / 25 success (full pipeline in prior QA) |

Prior baseline QA: [06-15-001](./06-15-001-data-quality-report-orcl.md).

---

## Verdict

**Partial pass.** The new run successfully delivers the intended price and derived-time-series upgrades: 1,254 daily OHLC bars, 78 rolling TTM windows, and 20 quarters of HLOC plus P/E and P/S bands that reconcile to the underlying bars. TTM revenue, net income, and gross profit audit cleanly against summed quarterly Alpha Vantage facts. The main remaining gap is product wiring — daily prices are persisted but `current_price` and `market_cap` are still absent from `fundamentals`, and valuation history only reaches back ~5 years (20 fiscal quarters) because Alpha Vantage daily history is capped at ~1,825 days.

---

## Upgrade Delta vs Prior Run

| Capability | `ORCL-2026-06-15-1` | `ORCL-2026-06-16-1` |
|------------|---------------------|---------------------|
| `daily_price_bars` table | absent | **1,254 rows** (2021-06-17 → 2026-06-15) |
| `fundamental_observations` | 807 (raw AV only) | **1,779** (+972 derived rows) |
| Quarterly price HLOC | none | **20 quarters** (`price_quarter_open/high/low/close`) |
| Quarterly P/E min/max | none | **20 quarters** (`pe_quarter_min/max`) |
| Quarterly P/S min/max | none | **20 quarters** (`price_to_revenue_quarter_min/max`) |
| Rolling TTM series | headline only | **78 windows** (`revenue_ttm`, `net_income_ttm`, `gross_profit_ttm`, `operating_income_ttm`, `eps_ttm`, `revenue_per_share_ttm`) |
| Per-quarter derived EPS / rev per share | none | **81 quarters each** |
| `diluted_shares_quarter` | 1 overview row | **81 quarterly rows** |
| `eps_ttm` in `fundamentals` | 5.84 (AV overview) | **5.941** (derived NI TTM ÷ quarter-end shares) |
| `gross_profit_ttm` in `fundamentals` | $43.92B | **$44.33B** (4-quarter sum; prior understated by ~$410M) |
| HLOC unavailable flags | none | **61 info flags** for quarters before daily-bar coverage |

---

## What The Workspace Captures Well

- **Daily price custody:** `daily_price_bars` stores open, high, low, close, volume, adjusted close, source (`Alpha Vantage`), and `fetched_at`. Latest bar: $192.64 on 2026-06-15.
- **Quarterly HLOC aggregation:** For FY2026 Q4 (period ended 2026-05-31), stored values (open $141.62, high $226.29, low $134.57, close $225.78) match recomputation from `daily_price_bars` within the quarter window (63 trading days).
- **Valuation bands:** P/E min/max for the same quarter (23.2× – 38.0×) match manual recomputation using daily close ÷ TTM EPS ($5.941). P/S bands use TTM revenue per share ($23.42).
- **TTM math is auditable:** For the latest eight TTM windows, `revenue_ttm`, `net_income_ttm`, and `gross_profit_ttm` each equal the sum of four contiguous quarterly observations (diff = $0). Example latest window ending 2026-05-31: revenue $67.358B, net income $17.087B, gross profit $44.329B.
- **SEC substrate unchanged:** 25,369 `sec_raw_facts`, 5,643 `av_raw_facts` — same ingestion breadth as prior run.
- **Explicit coverage gaps:** 61 `av_quarter_price_hloc_unavailable_for_*` quality flags document quarters that fall before the daily-bar history window instead of failing silently.

---

## TTM Audit

### Latest window (period end 2026-05-31)

| Metric | DB value | Sum of 4 quarters | External (Oracle FY2026) | Status |
|--------|----------|-------------------|--------------------------|--------|
| Revenue TTM | $67.358B | $67.358B | $67.4B (earnings release) | Match |
| Net income TTM | $17.087B | $17.087B | $17.0B GAAP (earnings release) | Match |
| Gross profit TTM | $44.329B | $44.329B | not separately cited | Internally consistent |
| Operating income TTM | $20.778B | — | $20.6B GAAP FY2026 | Close (+0.9%) |
| EPS TTM (derived) | $5.941 | $17.087B ÷ 2.876B shares | $5.83 GAAP FY2026 EPS | Methodology difference (see below) |
| EPS (AV overview row) | $5.84 | — | Yahoo Finance TTM EPS $5.84 | Match |

### Quarterly spot check (Q4 FY2026, ended 2026-05-31)

| Metric | DB | External | Status |
|--------|-----|----------|--------|
| Revenue | $19.184B | $19.18B (CNBC / 8-K) | Match |
| Net income | $4.304B | $4.2B GAAP (earnings release) | Close (+2.5%; AV vs filed) |
| Derived quarter EPS | $1.50 | $1.45 GAAP | Close (+3%; shares methodology) |

### TTM methodology notes

- TTM windows require **four contiguous quarters** with 80–100 day gaps between period ends (`AvDerivedTimeSeries::is_contiguous_av_quarter_window`). This correctly skips broken fiscal windows and produces 78 valid windows from 81 revenue quarters.
- **EPS TTM ($5.941) vs AV overview ($5.84):** Derived EPS uses trailing net income divided by **quarter-end shares outstanding** (2.876B). AV/Yahoo use **diluted weighted-average shares** over the fiscal year. Both are defensible; the ~1.7% gap matters for precise P/E comparisons. Downstream agents should prefer the derived series for time-consistency with the P/E bands, and note the AV overview figure where providers disagree.
- **Gross profit TTM improvement:** Prior run stored $43.916B; new run $44.329B. The new value equals the sum of four quarterly `gross_profit_quarter` observations and is the correct rolling TTM. Prior headline was understated by ~$410M.
- **Pre-existing duplicate `net_income_quarter` rows** (e.g. two identical rows per period back to 2006) are unchanged from Alpha Vantage ingestion; TTM sums still work because values are identical, but deduplication would reduce noise.

---

## Price and Valuation Band Audit

### Latest quarter with full bands (ended 2026-05-31)

| Field | DB | Cross-check |
|-------|-----|-------------|
| Quarter open | $141.62 | First bar after 2026-02-28 |
| Quarter high | $226.29 | MAX(`daily_price_bars.high`) in window |
| Quarter low | $134.57 | MIN(`daily_price_bars.low`) in window |
| Quarter close | $225.78 | Last bar on or before 2026-05-31 |
| P/E min / max | 23.2× / 38.0× | Recomputed from bars ÷ $5.941 EPS TTM |
| P/S min / max | 5.89× / 9.64× | Recomputed from bars ÷ $23.42 rev/share TTM |

### Spot valuation (2026-06-15 close from `daily_price_bars`)

| Metric | DB | External | Status |
|--------|-----|----------|--------|
| Close | $192.64 | $192.64 (Oracle IR / MacroTrends) | Match |
| Implied P/E (derived EPS) | 32.4× | Yahoo P/E TTM 31.53× (EPS $5.84) | Consistent band |
| Implied P/S | 8.23× | — | Reasonable vs quarter band 5.9–9.6× |

### Coverage limitation

- Daily bars span **2021-06-17 → 2026-06-15** (~5 years). Earliest quarter with HLOC is **2021-08-31**; **61 older quarters** have no price bands and carry info flags.
- P/E bands are **suppressed when TTM EPS ≤ 0** (e.g. 2021-11-30 loss quarter) — correct behavior per `av_derived_time_series.rs`.

---

## Data Quality Findings

### High

- **`current_price` and `market_cap` still missing from `fundamentals` despite 1,254 daily bars.** `financial_fetch_status = partial` and `starter_financials` gap remain open. Latest close ($192.64) and implied market cap (~$554B at 2.876B shares) are computable from persisted data but not promoted to starter fields. This blocks Monte Carlo return math and forward-multiple claims that depend on spot price.

### Medium

- **Valuation history truncated to ~20 quarters.** Alpha Vantage daily output window (~1,825 days) limits HLOC and P/E/P/S bands to fiscal quarters after mid-2021. Older quarters are flagged but not backfilled. Scenario and analogue work spanning full AV quarterly history (back to ~2006) cannot chart valuation bands without extending price ingestion.
- **Dual EPS TTM sources without precedence rule.** `fundamentals.eps_ttm` uses derived $5.941; a separate `eps` observation row still holds AV overview $5.84. Agents querying the wrong key will get different P/E denominators.
- **Quarter-end shares for TTM per-share metrics.** `diluted_shares_quarter` uses `commonStockSharesOutstanding` from the balance sheet at quarter end, not weighted-average diluted shares from the income statement. This affects `eps_ttm`, `revenue_per_share_ttm`, and all P/E/P/S bands.

### Low

- **61 info-level HLOC flags clutter `data_quality_flags` (88 total).** Consider rolling these into a single summary flag plus a `data_gaps` entry for pre-2021 price coverage.
- **Duplicate `net_income_quarter` observations** from AV source data inflate row counts without changing sums.

---

## Product Readiness

| Need | Status |
|------|--------|
| Deterministic daily prices | **Yes** — `daily_price_bars` with provenance |
| Quarterly price HLOC | **Yes** — 20 recent quarters |
| Quarterly P/E and P/S bands | **Yes** — 20 recent quarters, auditable |
| Rolling TTM time series | **Yes** — 78 windows, sums verified |
| Starter spot price / market cap | **No** — gap persists despite bars |
| Full-history valuation charts | **Partial** — ~5 years only |
| SEC facts for narrative mining | **Yes** — unchanged, strong |
| Scenario / report artifacts | **Not evaluated** — init-only run (22 workers) |

The initialized workspace is materially better for valuation-band and TTM charting than `ORCL-2026-06-15-1`. A later research agent can build quarter-level P/E and P/S ranges and rolling TTM series without re-fetching Alpha Vantage, provided it respects the 5-year price window and EPS methodology.

---

## Web Validation

| Field | DB value | External value | Source | Matters? |
|-------|----------|----------------|--------|----------|
| FY2026 revenue TTM | $67.358B | $67.4B | [Oracle Q4 FY2026 release](https://investor.oracle.com/investor-news/news-details/2026/Oracle-Announces-Record-Q4-and-FY-2026-Results-Driven-by-Cloud-Infrastructure--Cloud-Applications/default.aspx) | No — rounding |
| FY2026 GAAP net income | $17.087B | $17.0B | Oracle earnings release | No |
| FY2026 GAAP EPS | $5.941 derived / $5.84 AV | $5.83 | Oracle / Yahoo Finance | Low — shares methodology |
| Q4 FY2026 revenue | $19.184B | $19.18B | CNBC / 8-K | No |
| Close 2026-06-15 | $192.64 | $192.64 | Oracle IR stock quote | No |
| Market cap (implied) | not in DB (~$554B) | $554.04B | Oracle IR | **Yes** — should be persisted |
| Yahoo P/E TTM | 32.4× implied (derived EPS) | 31.53× | Yahoo Finance | Low — EPS denominator |

---

## Recommendations

1. **Promote latest `daily_price_bars.close` to `fundamentals.current_price` and compute `market_cap`** from close × `shares_outstanding` during `derive_starter_fundamentals_on_workspace`. Close the `starter_financials` and `narrative_current_share_price_market_cap` gaps when bars exist.
2. **Document EPS TTM precedence** — prefer derived `eps_ttm` for band consistency; retain AV overview as `eps_av_overview` or drop the duplicate `eps` row when derived is present.
3. **Extend daily price history** beyond 1,825 days (or add a second provider) so HLOC and valuation bands cover the full AV quarterly history (~81 quarters).
4. **Collapse HLOC-unavailable flags** into one summary quality flag per run instead of 61 per-quarter info rows.
5. **Consider weighted-average diluted shares** for TTM EPS and P/E bands where AV income-statement diluted share count is available, to align closer to provider-reported TTM EPS ($5.84).
6. **Add automated QA tests** that assert: (a) latest TTM window sums match quarterly inputs, (b) quarter HLOC matches `daily_price_bars` aggregation, (c) P/E min/max match bar recomputation, and (d) `current_price` is populated when bars exist.
