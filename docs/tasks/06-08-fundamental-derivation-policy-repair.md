# Fundamental Derivation Policy Repair

**Date:** 2026-06-08  
**Status:** Draft task  
**Scope:** Fix headline `fundamentals` derivation policy so metric names, values, periods, and share-count semantics agree. Behavior-changing work on phase 4 (derive metrics), separate from structural refactor in [06-07-decompose-init-entanglement.md](./06-07-decompose-init-entanglement.md).

## Why this is a separate task

The decomposition plan’s Step 5 (`FundamentalDeriver` extraction) is primarily **module hygiene**: move post-mapping logic out of `ConceptCatalog` / `init_workspace` so phase 4 can be re-run independently.

This task is **derivation policy**: change *what* gets computed and *how* it is labeled so downstream consumers (`generate_report`, scenario seeding, agents reading `fundamentals`) are not misled.

Extracting code without changing policy preserves known bugs. This doc tracks the policy fixes explicitly.

**Primary evidence:** [06-08-001-data-quality-report.md](../qa/06-08-001-data-quality-report.md) (ORCL init runs, 2026-06-08).

**Related but out of scope here:**
- Phase 1–3 pipeline decomposition (ingest, catalog, mapping resolvers)
- `ConceptCatalog` / `FundamentalDeriver` file moves (do in decomposition Step 5a unless policy changes land in the same module at once)
- Cloud/segment revenue not in XBRL, `stock_info` exchange/sector enrichment (separate ingest/profile tasks)

## Problem statement

After a full `initWorkspace` run with mapping enabled, the workspace has:

- A **strong observation layer** (`fundamental_observations`) with correct quarter/YTD/annual typing through latest filings.
- A **weak headline layer** (`fundamentals`) that looks complete but encodes stale or mixed-frequency values under misleading keys (`*_ttm`, `trailing_pe`, `price_to_sales_ttm`).

ORCL example (runs 1–2, 2026-06-08):

| Issue | Symptom | Root cause (today) |
|-------|---------|-------------------|
| TTM mislabeling | `revenue_ttm` = $57.4B @ `2025-05-31` (FY2025 annual) while Q3 FY26 quarter = $17.19B | `select_latest_income_bundle` falls back to annual; `apply_income_bundle` still writes `*_ttm` keys |
| Mixed-frequency market cap | Price Jun 2026 × FY2025 weighted-average diluted shares → ~$612B vs ~$615B IR | `compute_derived_metrics` + shares aligned to income bundle period, not latest common shares |
| Stale shared `period` | Cash/debt as-of 2026-02-28 but `fundamentals.period` = `2025-05-31` | Single `fundamental_period_end` copied to all headline rows |
| Silent severity | Annual fallback + mixed freq are `info` flags; `data_gaps` empty | Flags document issue but do not block readiness |

`mapping_strategy:none` avoids bad headlines by not writing them — that is intentional ingest-only behavior, not a fix for mapped runs.

## Definition of good

### Principle

**Metric name, value, and period must agree.** If we cannot produce a true TTM, we must not call it TTM in the key or label.

### Headline layer (`fundamentals`)

1. **Honest keys and labels** when TTM bridge fails — e.g. `revenue_fy2025` / `revenue_annual`, not `revenue_ttm`.
2. **Per-metric `period` and `period_type`** — income, balance sheet, and market metrics each carry their own as-of; no one `fundamental_period_end` for every row.
3. **Purpose-specific share counts:**
   - Market cap → latest `EntityCommonStockSharesOutstanding` (instant).
   - EPS → `WeightedAverageNumberOfDilutedSharesOutstanding` for the **same income period** as EPS.
4. **Ratios inherit input semantics** — `trailing_pe` only when EPS is true TTM or explicitly paired; otherwise rename (`pe_vs_fy_eps`) or omit and open a gap.
5. **Gaps and severity match risk** — TTM unavailable, mixed-frequency market cap, and ratio period mismatch → `data_gaps` + **high** severity flags, not info-only.
6. **Agents can trust `fundamentals` as a fast entry point** without re-deriving everything from observations — or readiness checks fail loudly.

### Observation layer (unchanged bar)

Keep current strength: full mapped fact history, correct `period_type` (`quarter`, `ytd`, `annual`, `instant`, `ttm`), auditable canonical mappings. Policy repair must not regress observation correctness.

### Explicit non-goals (this task)

- True TTM for every issuer (May fiscal year-ends, restated quarters, etc. may still block contiguity) — but fallback must be honest.
- Replacing `fundamentals` with views over observations (longer-term schema direction; see [06-06-003-database-structure-analysis.md](../qa/06-06-003-database-structure-analysis.md)).

## Code touchpoints

Logic to change lives today in:

| Area | File(s) | Functions / behavior |
|------|---------|----------------------|
| Income bundle / TTM | `concept_catalog.rs` | `ttm_series_for_metric`, `select_latest_income_bundle`, `apply_income_bundle` |
| Point-in-time picks | `concept_catalog.rs` | `latest_value_fact`, `total_latest_values` (shares vs cash/debt period rules) |
| Market ratios | `init_workspace.rs` | `FinancialSnapshot::compute_derived_metrics`, `fundamental_metrics()` |
| Persistence | `workspace_financial_store.rs` | `fundamentals` insert — may need `period_type` or richer columns later |
| Readiness | `generate_report.rs` | `validate_report_inputs`, scenario seeding from `revenue_ttm` / `eps_ttm` |

Target home after decomposition Step 5a: `fundamental_deriver.rs` (+ slim `init_workspace` orchestration).

## Work items

### 1. TTM naming and fallback policy

- [ ] When `ttm_windows` is empty and annual duration fact is used, persist **`revenue_annual`** (or `revenue_fy2025` when fiscal year known), not `revenue_ttm`. Same for net income, operating income, EPS.
- [ ] Labels in `fundamentals.metric_label` must match (`Revenue (FY2025 annual)` vs `Revenue TTM`).
- [ ] When a true four-quarter TTM is built, keep `*_ttm` keys and record `period_type=ttm` in observations (already partially done).
- [ ] Investigate ORCL contiguity failure: fix `is_contiguous_ttm_window` / fiscal May year-end handling if quarters exist but fail the window test.
- [ ] Fresher fallback order when TTM fails: four-quarter sum → latest quarter snapshot (`revenue_latest_quarter`) → YTD (labeled) → FY annual (stale, explicit).

### 2. Per-metric period and period_type

- [ ] Stop copying one `fundamental_period_end` to cash, debt, and income rows in `fundamental_metrics()`.
- [ ] Each `FundamentalInsert` / `fundamentals` row gets the as-of date for **that** metric.
- [ ] Document or add `period_type` on headlines if schema allows (otherwise encode in `metric_key` suffix).

### 3. Share count and market cap

- [ ] Split headline share concepts: common instant vs diluted weighted-average (different `metric_key`s).
- [ ] Market cap: `current_price × shares_outstanding_common` with `price_as_of` and share `period_end` in source notes.
- [ ] Do **not** pass income bundle `period_end` as `prefer_period_end` for market-cap share lookup.
- [ ] Prefer provider market cap when Yahoo supplies it; derive only when missing.

### 4. Derived ratios

- [ ] `trailing_pe`: require EPS period compatible with price semantics, or skip / rename.
- [ ] `price_to_sales_ttm`: require revenue period compatible with market cap, or skip / rename.
- [ ] Elevate flags to high severity; add `data_gaps` entries (`ttm_bridge_unavailable`, `mixed_frequency_market_cap`, etc.).

### 5. Downstream consumers

- [ ] Update `generate_report` validation to accept new keys or map annual/quarter headlines for scenario baseline.
- [ ] Readiness: treat high-severity derivation flags as report blockers unless observations provide an explicit override path.

### 6. Tests and fixtures

- [ ] ORCL regression fixture (from `ORCL-2026-06-08-1` expectations): Q3 FY26 quarter revenue/EPS in observations; headlines must not label FY2025 annual as TTM.
- [ ] Unit tests for: annual fallback naming, per-metric period, market cap share selection, ratio refusal on mismatch.
- [ ] Optional: extend [06-07-003-fixture-based-automated-canon-metric-checks.md](../qa/06-07-003-fixture-based-automated-canon-metric-checks.md) manifest with derivation policy assertions.

## Suggested implementation order

1. **Naming + flags + gaps** (low schema risk, high clarity) — stop lying in `*_ttm` keys when fallback is annual.
2. **Per-metric period** on `fundamentals` rows.
3. **Share split + market cap** frequency fix.
4. **TTM contiguity** improvements for May FY issuers (ORCL).
5. **generate_report** + scenario alignment.

Can run in parallel with decomposition **Step 5a** (`FundamentalDeriver` extract) if new policy is implemented in the extracted module from the start; otherwise land 5a first, then policy here.

## Success criteria

- [ ] ORCL full init: no `fundamentals` row uses `*_ttm` suffix when value is FY annual fallback.
- [ ] ORCL: `fundamentals.period` on cash/debt reflects balance-sheet as-of (e.g. 2026-02-28), not income bundle period.
- [ ] ORCL: market cap uses latest common shares; documented delta vs IR within expected tolerance (~0.5%).
- [ ] `data_gaps` populated when TTM bridge unavailable or mixed-frequency market cap is used.
- [ ] `generate_report` either passes with new headline shape or fails with explicit gap messages (no silent wrong scenario math).
- [ ] Existing unit tests updated; new ORCL-oriented derivation tests added.

## References

- QA: [06-08-001-data-quality-report.md](../qa/06-08-001-data-quality-report.md) — Appendices A–C (TTM, market cap, interaction)
- Prior QA: [06-06-001-data-quality-report.md](../qa/06-06-001-data-quality-report.md)
- Pipeline decomposition: [06-07-decompose-init-entanglement.md](./06-07-decompose-init-entanglement.md) (Step 5a extract only)
- Schema direction: [06-06-003-database-structure-analysis.md](../qa/06-06-003-database-structure-analysis.md)
