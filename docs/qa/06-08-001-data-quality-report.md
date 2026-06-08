# Data Quality Report — ORCL initWorkspace (2026-06-08)

## Scope

QA inspection of three Oracle initialization runs produced on 2026-06-08:

| Run | SQLite path | Created (UTC) | Invocation |
|-----|-------------|---------------|------------|
| 1 | `reports/stock-narrative-research/ORCL-2026-06-08-1/run.sqlite` | 2026-06-08T04:13:28 | `cargo loco task initWorkspace ticker:ORCL` |
| 2 | `reports/stock-narrative-research/ORCL-2026-06-08-2/run.sqlite` | 2026-06-08T04:15:26 | `cargo loco task initWorkspace ticker:ORCL mapping_strategy:heuristic` |
| 3 | `reports/stock-narrative-research/ORCL-2026-06-08-3/run.sqlite` | 2026-06-08T04:29:36 | `cargo loco task initWorkspace ticker:ORCL mapping_strategy:none` |

Runs 1 and 2 were generated with different `initWorkspace` invocations but produced substantively identical financial output. Row counts, `fundamentals` values, `data_quality_flags`, canonical mappings, SEC raw facts (core fields), and observation-layer metrics all matched. The only differences were run identity metadata (`run_slug`, `created_at`, `updated_at`, `fetched_at`, and similar timestamp columns). Re-running init — with or without an explicit `mapping_strategy:heuristic` — did not change Oracle's financial results in any meaningful way.

Run 3 (`mapping_strategy:none`) is a different initialization mode; see [Appendix D](#d-mapping_strategynone-ingest-only) below.

## Verdict

**Partial pass.** The workspace is a strong SEC-facts substrate for narrative research: broad raw ingestion, auditable canonical mappings, rich quarterly observations through **Q3 FY26** (filed 2026-03-11), and company-specific backlog via RPO. Headline `fundamentals` remain the weak link — several fields are **mislabeled as TTM**, anchored to **FY2025 (2025-05-31)** while newer quarters exist, and market cap mixes a **June 2026 price** with **FY2025 weighted-average diluted shares**.

Compared to [06-06-001-data-quality-report.md](./06-06-001-data-quality-report.md), this run shows clear progress: headline debt is now correct ($134.6B vs the prior $7.3B stale current-debt bug), starter fundamentals expanded from 5 to 13 rows, and debt canonical mappings (`NotesPayableCurrent`, `LongTermNotesAndLoans`) are active. Period typing in the observation layer also correctly distinguishes `quarter`, `ytd`, `annual`, and `instant` for core metrics.

## What The Workspace Captures Well

- **Run identity:** `run_metadata` records ticker `ORCL`, schema v2, status `initialized`, `financial_fetch_status=succeeded`.
- **SEC breadth:** **25,369** raw facts across **513** concepts; **516** catalog entries with usability/plot metadata (**290** `plot_ready`, **304** `long_history`).
- **Provenance:** Raw facts retain `accession`, `filed_at`, `form`, and `raw_json` on all 25,369 rows; labels on 25,367 rows.
- **Canonical layer:** 9 definitions + 9 active mappings with confidence, rationale, and `catalog_candidate_scoring` provenance. Revenue maps to `RevenueFromContractWithCustomerExcludingAssessedTax`; debt split into current/noncurrent.
- **Time series depth:** **1,311** `fundamental_observations` with annual, quarterly, YTD, and TTM period types. Latest quarter values align with Oracle's Q3 FY26 release.
- **Scenario hooks:** `concept_catalog_entries` includes **RPO** (`RevenueRemainingPerformanceObligation`, latest **$552.6B** at 2026-02-28). Downstream tables (`sources`, `claims`, `scenario_*`, `supporting_metric_selections`) exist but are empty as expected post-init.
- **Transparency flags:** 7 `data_quality_flags` document TTM fallbacks, mixed-frequency market cap, and P/E / P/S methodology.

## Data Quality Findings

### Critical

_None._ Headline debt is no longer materially wrong (see prior report).

### High

- **`revenue_ttm`, `net_income_ttm`, `operating_income_ttm`, `eps_ttm` are FY2025 annual, not TTM.** DB: revenue **$57.399B** (FY2025, period `2025-05-31`). Q3 FY26 alone is **$17.19B**; YTD through Feb 2026 is **$48.17B**. Flags say `*_annual_fallback_used`, but metric keys still say `_ttm` and `fundamentals.period` is `2025-05-31`. Downstream agents can treat stale FY data as current TTM.

- **Market cap mixes frequencies.** Price **$213.68** (Yahoo, ~Jun 5 2026) × shares **2.866B** (FY2025 weighted-average diluted) → **$612.4B**. Oracle IR shows **~$614.6B** with **~2.88B** common shares outstanding. ~$2B gap from share-count methodology/date, not a filing error. Flag: `market_cap_derived_from_mixed_frequency_price_and_shares`.

### Medium

- **`stock_info` profile incomplete:** `exchange`, `sector`, `industry` blank. Company name is `ORACLE CORP`; currency `USD` is present.

- **`total_debt` value is current, period label is stale.** Value **$134.6B** = current (**$9.89B**) + noncurrent (**$124.7B**) at **2026-02-28**, but `fundamentals.period` shows **2025-05-31**. No `total_debt` row in `fundamental_observations`.

- **No `data_gaps` rows** despite known limitations (no TTM bridge, missing gross profit in starter set, no FCF/capex headlines). Gaps table is empty; only info-level flags exist.

- **Cloud/segment revenue not in catalog** as active series (deprecated segment tags only). RPO is present; cloud revenue ($8.9B Q3) lives in earnings releases, not persisted XBRL concepts. Expected for SEC Facts, but limits cloud-narrative scenarios without manual sourcing.

- **Mapping strategy has no observable effect.** Default init and `mapping_strategy:heuristic` produced identical mappings (`catalog_candidate_scoring` on all 9 canonical keys), identical observations, and identical headline fundamentals. Either heuristic is already the default, or the parameter did not change behavior for ORCL.

### Low

- **`generated/` and `artifacts` empty**; 11 `sections` are `pending` placeholders — fine for init-only, but no chart/HTML artifacts yet.

- **`concept_review_decisions` empty** — mappings are heuristic-only; no LLM review pass recorded.

## Product Readiness

A later research agent **can** build a credible Oracle memo from this workspace **without re-fetching SEC facts**:

- Deterministic quarterly/annual income statement, balance sheet cash/debt, EPS, and RPO series are queryable.
- Concept discovery is strong (516-entry catalog, narrative tags like `backlog`).
- Canonical choices are auditable via `canonical_metric_mappings`.

Gaps for scenario work:

- Headline snapshot overstates recency (`*_ttm` labels).
- No starter **OCF, capex, FCF, net debt** despite `NetCashProvidedByUsedInOperatingActivities` (108 facts) in raw layer.
- No cloud revenue time series in XBRL; narrative would need IR/earnings sources (`sources` table is ready but empty).
- Profile metadata (exchange, sector) missing for industry-context sections.

## Web Validation

| Field | DB Value | External Value | Source | Status |
|-------|----------|----------------|--------|--------|
| Company / listing | `ORACLE CORP`, exchange blank | Oracle Corp, **NYSE: ORCL** | [Oracle IR stock page](https://investor.oracle.com/stock-information/default.aspx) | **Partial** — identity OK, exchange missing |
| Price | **$213.68** | **$213.68** (Jun 5, 2026 close) | Oracle IR | **Match** |
| Market cap | **$612.4B** (derived) | **$614.55B** | Oracle IR | **Close** (~0.3% low; share-count/date mix) |
| Shares outstanding | **2.866B** (FY2025 WA diluted) | **~2.88B** common shares | Yahoo key stats / Oracle IR | **Mismatch matters** — use common shares for market cap |
| FY2025 revenue | **$57.399B** | **$57.399B** | 10-K / financial data providers | **Match** |
| Q3 FY26 revenue | **$17.19B** (quarter obs.) | **$17.2B** | [Oracle Q3 FY26 release](https://investor.oracle.com/investor-news/news-details/2026/Oracle-Announces-Fiscal-Year-2026-Third-Quarter-Financial-Results/default.aspx) | **Match** |
| Q3 FY26 GAAP EPS | **$1.27** | **$1.27** | Oracle Q3 FY26 release | **Match** |
| Q3 FY26 net income | **$3.721B** | **~$3.72B** | Oracle Q3 FY26 release | **Match** |
| Q3 FY26 operating income | **$5.464B** | **$5.5B** GAAP | Oracle Q3 FY26 release | **Close** |
| RPO | **$552.6B** | **$553B** | Oracle Q3 FY26 release | **Match** |
| Cash (2026-02-28) | **$38.455B** | Consistent with Q3 FY26 10-Q | SEC filing via DB | **Plausible** |
| Total debt | **$134.6B** | Sum of current + long-term notes per 10-Q concepts | SEC via DB | **Internally consistent** |

## Recommendations

1. **Rename or recompute TTM fields** when only an annual fallback exists (`revenue_fy2025` vs `revenue_ttm`), or build a true TTM bridge from four quarters when available.
2. **Use `EntityCommonStockSharesOutstanding` latest instant** for market-cap math; keep weighted-average shares for EPS only. Log period mismatch explicitly.
3. **Align `fundamentals.period` with the as-of date** of each metric (debt/cash at 2026-02-28 vs income at 2025-05-31).
4. **Populate `data_gaps`** for missing TTM, missing gross profit in starter set, missing cloud segment XBRL, and blank exchange/sector.
5. **Extend canonical definitions** to OCF and capex (`NetCashProvidedByUsedInOperatingActivities`, `PaymentsToAcquirePropertyPlantAndEquipment`) and derive FCF/net debt in starter fundamentals.
6. **Enrich `stock_info`** from exchange/profile source (NYSE, Technology / Software).
7. **Clarify `mapping_strategy` behavior** — document whether `heuristic` is the default and persist the chosen strategy in `run_metadata` so QA can distinguish runs.
8. **Add QA regression for mega-cap cloud names** like ORCL: verify latest quarter revenue/EPS/RPO surface in observations *and* optionally in headline fundamentals.

## Summary

Initialization for ORCL produces a durable, auditable SEC research substrate with correct observation-layer data through Q3 FY26 when mapping runs. The headline `fundamentals` table should not be trusted without reading quality flags and the observation layer. Default init and `mapping_strategy:heuristic` produced the same mapped workspace; `mapping_strategy:none` skips headlines entirely and leaves agents to work from raw facts until mapping is applied.

---

## Deep Dive: TTM Mislabeling and Mixed-Frequency Market Cap

This section explains the two highest-severity headline issues in more detail: what the database actually contains, why the mismatch arises in current init logic, how it can corrupt downstream work, and what we should do instead.

### A. The TTM Problem

#### Key observations

The observation layer and the headline layer tell different stories about recency.

| Layer | What it shows for revenue | Period / as-of |
|-------|---------------------------|----------------|
| `fundamentals.revenue_ttm` | **$57.399B** | `2025-05-31` (labeled "Revenue TTM") |
| `canonical_fundamental_observations` (`quarter`) | **$17.19B** | Q3 FY26, `2026-02-28` |
| `canonical_fundamental_observations` (`ytd`) | **$48.173B** | 9 months through `2026-02-28` |
| `canonical_fundamental_observations` (`annual`) | **$57.399B** | FY2025, `2025-05-31` |

The same pattern applies to `net_income_ttm`, `operating_income_ttm`, and `eps_ttm`:

- Headline **net income** = **$12.443B** (FY2025 annual), vs **$3.721B** in the latest quarter and **$12.783B** YTD through Q3 FY26.
- Headline **EPS** = **$4.34** (FY2025 diluted annual), vs **$1.27** in Q3 FY26.

The system *does* know this is not a true TTM. Evidence is persisted in three places:

1. **`data_quality_flags`:** `revenue_ttm_annual_fallback_used`, `net_income_ttm_annual_fallback_used`, `operating_income_ttm_annual_fallback_used`.
2. **`stock_info.source_note`:** explicit text that annual values were used "because a contiguous TTM bridge was unavailable."
3. **Observation `source_note` on bundle metrics:** same annual-fallback explanation.

Yet the **metric keys and labels** in `fundamentals` still say `_ttm` ("Revenue TTM", "EPS TTM"), and almost all income-statement headlines share a single `period` of `2025-05-31` via `fundamental_period_end`.

#### Why the fallback happened (current logic)

Init resolves income headlines through `ConceptCatalog::select_latest_baseline_bundle` → `ttm_series_for_metric` in `concept_catalog.rs`:

1. **First choice:** build TTM by summing **four contiguous quarterly facts** (`ttm_windows`). Each quarter in the window must pass `is_contiguous_ttm_window`.
2. **Fallback:** if no TTM window is found, take the **latest 10-K annual** duration fact (250–380 days) and use that value.
3. **Bundle selection:** pick the latest `period_end` where revenue and net income both exist for the same period.
4. **Apply:** `apply_income_bundle` writes those values into `snapshot.revenue_ttm`, `net_income_ttm`, etc., and sets `fundamental_period_end`.

For Oracle, step 1 produced **no TTM windows** (likely due to fiscal-calendar quarter shapes, duplicate/restated quarter rows, or contiguity rules not matching Oracle's May-year-end cadence). Step 2 then selected **FY2025 10-K** values through `2025-05-31` — the most recent *aligned annual* bundle — and stored them under `*_ttm` field names.

Margins in headlines are internally consistent with that annual bundle (operating margin ~30.8%, net margin ~21.7%) but are **not** margins on a rolling twelve months ending in 2026.

Gross profit is further excluded from the bundle: flag `gross_profit_ttm_excluded_because_no_fact_matched_baseline_period_2025-05-31`, so `gross_margin` never appears in `fundamentals` despite gross profit existing in the raw layer.

#### Why this is disruptive downstream

Agents and tasks that read `fundamentals` first — without also querying flags or `fundamental_observations` — will draw wrong conclusions:

1. **False recency.** A field named `revenue_ttm` with a populated value reads as "current trailing revenue." For Oracle it is **FY2025 annual revenue**, roughly **nine months stale** relative to the latest 10-Q. An agent building a "current snapshot" or "financial_math" section can quote $57.4B as today's revenue run-rate.

2. **Broken valuation ratios.** `trailing_pe` (49.2×) is computed as `current_price / eps_ttm`, where `eps_ttm` is FY2025 diluted EPS ($4.34). That is not trailing P/E in the market-data sense; it is **spot price divided by last fiscal year EPS**. Flag `trailing_pe_uses_market_price_and_latest_filing_period_eps` exists but is info-level and easy to miss.

3. **Broken P/S.** `price_to_sales_ttm` (10.7×) = `market_cap / revenue_ttm`, mixing a **June 2026 market cap** with **FY2025 revenue**. Revenue growth in FY26 (Q3 up 22% YoY) is invisible to this ratio.

4. **Scenario anchoring.** `scenario_periods` and projection tasks often seed from headline fundamentals. If they inherit $57.4B revenue and 21.7% net margin as "baseline," scenario math starts from an outdated income profile even though YTD and quarterly observations would support a fresher anchor.

5. **Silent override of the good layer.** The observation layer has correct Q3 FY26 GAAP figures validated against Oracle's release. Headlines look complete (13 populated rows), so an agent may **skip** the observation layer entirely. The flags are informational, not blocking; `data_gaps` is empty.

6. **Inconsistent period column.** `cash` ($38.455B) and `total_debt` ($134.6B) are latest balance-sheet values (2026-02-28), but `fundamentals.period` on those rows still says `2025-05-31` because it is copied from `fundamental_period_end` of the income bundle. Downstream code cannot tell which as-of date each headline metric uses.

#### What logic should do instead

**Principle:** metric name, value, and period must agree. If we cannot produce a true TTM, we must not call it TTM in the key or label.

Expected behavior:

1. **Honest naming on fallback.** When `ttm_windows` is empty and annual fallback is used, persist as `revenue_fy2025` / `net_income_fy2025` (or generic `revenue_annual` with `period_type=annual` metadata), not `revenue_ttm`. Labels should read "Revenue (FY2025 annual)" not "Revenue TTM."

2. **Prefer fresher defensible aggregates when TTM fails.** For Oracle specifically, reasonable alternatives in priority order:
   - Sum of last four reported quarters (fix contiguity detection for May fiscal year-ends if quarters exist but fail the window test).
   - Latest **YTD × annualization** only when clearly labeled and flagged.
   - Latest **single quarter** as `revenue_latest_quarter` for snapshot use.
   - FY annual as last resort, explicitly stale.

3. **Per-metric `period` in `fundamentals`.** Stop using one `fundamental_period_end` for every row. Cash/debt should show `2026-02-28`; income metrics should show their actual period end and `period_type`.

4. **Elevate severity and open gaps.** `*_annual_fallback_used` should be **high** severity (not info) and create a `data_gaps` row (e.g. `ttm_bridge_unavailable`) so downstream agents see it without scanning flags.

5. **Expose both snapshot and headline roles.** Observation layer remains source of truth for time series; headlines should either reflect the **latest quarter** or the **best available TTM**, never an annual fallback disguised as TTM.

6. **Derived ratios should inherit period semantics.** `trailing_pe` should use TTM EPS if available; if only annual EPS exists, rename to `pe_vs_fy_eps` or refuse to compute until inputs align. Same for `price_to_sales_ttm`.

---

### B. Mixed-Frequency Market Cap

#### Key observations

| Input | Value in DB | Actual as-of / meaning |
|-------|-------------|------------------------|
| `current_price` | **$213.68** | Yahoo chart, ~**Jun 5, 2026** close |
| `shares_outstanding` | **2.866B** | **FY2025 weighted-average diluted** shares (`WeightedAverageNumberOfDilutedSharesOutstanding`, period `2025-05-31`) |
| `market_cap` | **$612.4B** | `price × shares` (derived) |

Alternative share counts already present in `sec_raw_facts`:

| Concept | Latest value | As-of |
|---------|--------------|-------|
| `EntityCommonStockSharesOutstanding` | **2.876B** | `2026-03-05` |
| `WeightedAverageNumberOfDilutedSharesOutstanding` (Q3 FY26 quarter) | **~2.914B** | `2026-02-28` |

External reference (Oracle IR, Jun 5, 2026): price **$213.68**, market cap **~$614.6B**, shares **~2.88B**.

The ~$2B gap vs Oracle IR is not a filing error; it comes from using **FY2025 weighted-average diluted shares** (2.866B) instead of **latest common shares outstanding** (~2.88B) with a **current** price.

#### Why the mismatch arises (current logic)

1. **Price** comes from Yahoo during init (`fetch_price_metadata`), with no `period` stored on `fundamentals.current_price`.

2. **Shares** for the headline row come from the canonical mapping `shares_outstanding` → `WeightedAverageNumberOfDilutedSharesOutstanding`. `latest_value_fact` is called with `prefer_period_end` set to the income bundle's `fundamental_period_end` (`2025-05-31`), so shares align with the **annual income baseline**, not the latest instant.

3. **Market cap** is derived in `FinancialSnapshot::compute_derived_metrics` when not provided directly:
   ```text
   market_cap = current_price × shares_outstanding
   ```
   A flag is pushed: `market_cap_derived_from_mixed_frequency_price_and_shares`.

4. **Persistence** writes `shares_outstanding.period` from `fundamental_period_end` (`2025-05-31`) even though the economic meaning of the row is "shares used for market cap math" — and `market_cap.period` is **null**.

So three different time axes are combined: **today's price**, **last fiscal year's average share count**, and (for ratios) **last fiscal year's earnings/revenue**.

#### Why this is disruptive downstream

1. **Valuation level errors.** Market cap feeds `price_to_sales_ttm`, peer comparisons, and scenario price bands. A ~0.3% error on Oracle is small, but the **pattern** scales badly for companies with buybacks, splits, or rapid share issuance (Oracle has been active on both debt and equity financing). Market cap can be wrong by several percent without any error flag above info level.

2. **Concept confusion.** `shares_outstanding` in `fundamentals` is weighted-average diluted shares for EPS purposes, not common shares outstanding for market cap. Downstream agents routinely treat one "shares" number as both. Here the label says "Shares outstanding" but the concept is **diluted WA for FY2025**.

3. **P/E and P/S compound the frequency mismatch.** `trailing_pe` mixes **spot price** with **FY2025 EPS**. `price_to_sales_ttm` mixes **derived market cap (spot × stale shares)** with **FY2025 revenue**. Multiple incompatible as-of dates collapse into ratios that look precise (49.2×, 10.7×) but are not standard valuation metrics.

4. **Narrative risk.** Research text often says "at a $612B market cap" and "trading at 49× earnings" as if those are contemporaneous facts. Competitor comparison and analogue matching use these headines as features; stale or mixed inputs skew similarity search and Monte Carlo inputs.

5. **No quote timestamp on price.** Without `as_of` on `current_price`, agents cannot tell whether price is from init time, prior close, or delayed feed — compounding trust in stale ratios.

#### What logic should do instead

**Principle:** market cap and EPS should use share counts appropriate to each purpose; every input should carry an explicit as-of.

Expected behavior:

1. **Split share concepts in headlines.**
   - `shares_outstanding_common` ← `EntityCommonStockSharesOutstanding` (latest instant, e.g. 2026-03-05).
   - `shares_diluted_weighted_avg` ← `WeightedAverageNumberOfDilutedSharesOutstanding` for the relevant income period (quarter, TTM, or FY).

2. **Market cap derivation.**  
   `market_cap = current_price × shares_outstanding_common`  
   Persist `period` / `as_of` on all three inputs and on `market_cap` itself. Prefer provider market cap when available; only derive when missing.

3. **Do not align market-cap shares to income bundle period.** The `prefer_period_end` constraint makes sense for EPS alignment but is wrong for market cap. Cash/debt already use `latest_value_fact(..., None)` for latest instant; market-cap shares should follow the same pattern.

4. **EPS and P/E pairing.** `eps_ttm` (or `eps_fy2025` if fallback) should pair with the share count from the **same income period**. `trailing_pe` should require matching periods or emit a high-severity flag and refuse the ratio.

5. **Quote metadata.** Store price timestamp and source on `fundamentals` (or `stock_info`): `price_as_of`, `price_source`. Oracle IR shows delayed 20-minute pricing; that belongs in provenance.

6. **Gaps and flags.** When derived market cap uses inputs from different dates, open a `data_gaps` entry and use **high** severity — not info — so scenario and report tasks surface it in readiness checks.

---

### C. How the two issues interact

The TTM and market-cap problems are separate but **multiply** in derived metrics:

```text
price_to_sales_ttm = (price_Jun2026 × shares_FY2025) / revenue_FY2025
trailing_pe        =  price_Jun2026 / eps_FY2025
```

Both ratios look like standard valuation multiples but are actually **hybrid constructs** across three time axes. The observation layer has the pieces to do better (Q3 FY26 quarters, latest common shares, YTD income), but headlines present a deceptively complete snapshot.

**Minimum bar for downstream safety:** any consumer of `fundamentals` should treat rows with `*_ttm` suffixes as untrusted until `period_type` and quality flags are checked — or init should stop emitting misleading keys. The better fix is to make headlines honest so agents can use `fundamentals` as a fast, deterministic entry point without re-deriving everything from observations.

---

### D. `mapping_strategy:none` (ingest-only)

A third run was inspected with explicit mapping disabled:

```bash
cargo loco task initWorkspace ticker:ORCL mapping_strategy:none
```

This mode deliberately stops after SEC ingestion. It calls `persist_sec_ingestion` instead of `persist_financial_snapshot`, so canonical mapping, observations, headline `fundamentals`, and `data_quality_flags` are never written.

| | Runs 1–2 | Run 3 (`none`) |
|---|----------|----------------|
| `financial_fetch_status` | `succeeded` | `ingested` |
| `financial_fetch_error` | null | `canonical mapping and starter fundamentals deferred` |
| `sec_raw_facts` | 25,369 | 25,369 |
| `concept_catalog_entries` | 516 | 516 |
| `canonical_metric_mappings` | 9 | **0** |
| `fundamental_observations` | 1,311 | **0** |
| `fundamentals` | 13 | **0** |
| `data_quality_flags` | 7 | **0** |

**Trade-off:** Run 3 does not exhibit the TTM mislabeling or mixed-frequency market cap bugs from Appendices A–C — because it never writes those headline rows. In that sense it *avoids* persisting incorrect deterministic summaries. The cost is that downstream agentic consumers cannot take a shortcut through `fundamentals`; they must infer metrics from `sec_raw_facts` and `concept_catalog_entries` (or wait for a later mapping pass). That is arguably safer than trusting a deceptively complete headline table: agents are pushed toward the auditable raw layer rather than into misleading traps like `revenue_ttm` that is actually FY2025 annual.

Caveats for run 3:

- **Price is fetched but not persisted.** Yahoo metadata is mentioned in `stock_info.source_note`, but `fundamentals` has no `current_price` row.
- **No `data_gaps` row** despite deferred mapping — only `run_metadata.financial_fetch_error` records the state. Downstream readiness checks should treat `ingested` + deferred error as explicitly not ready for scenario/headline work.
- **Not narrative-ready on its own** until canonical mapping runs; it is raw-ingest-only by design.
