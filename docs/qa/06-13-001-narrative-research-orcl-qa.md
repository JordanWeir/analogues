# Init Workspace & Narrative Research QA — ORCL (2026-06-13)

## Scope

(RUNNING "xiaomi/mimo-v2.5-pro")

QA inspection of **init workspace substrate** and **narrative researcher output** for one Oracle run, compared against the prior narrative QA in `06-12-001-narrative-research-orcl-qa.md` (run 3, `deepseek/deepseek-v4-flash`).

| Run | SQLite path | Model | Focus |
|-----|-------------|-------|-------|
| 5 | `reports/stock-narrative-research/ORCL-2026-06-13-5/run.sqlite` | `xiaomi/mimo-v2.5-pro` | Init workspace, narrative map, claims, sources, sections, cruxes |
| 3 (baseline) | `reports/stock-narrative-research/ORCL-2026-06-13-3/run.sqlite` | `deepseek/deepseek-v4-flash` | Prior QA reference |

Web validation performed against official filings/press releases, CNBC, and secondary financial media (June 2026).

Worker telemetry (run 5): 27 agent rounds, 27 client tool calls, 0 web search requests, ~$0.08 cost, ~299s latency.

## Verdict

**Fail (regression vs run 3).** The init workspace substrate is unchanged from run 3 — still FY2025-era starter fundamentals, no price/market cap, and no Q4 FY2026 ingestion. The narrative layer regressed sharply: `xiaomi/mimo-v2.5-pro` produced a thinner, year-stale board anchored on Q4 FY2025 (June 2025) instead of the June 10, 2026 Q4 FY2026 catalyst that run 3 handled well. Init gaps are slightly worse because the explicit `narrative_q4_fy2026_workspace_ingestion` gap was dropped.

## Comparison To Prior QA (Run 3)

| Layer | Run 3 (06-12-001) | Run 5 (this inspection) | Delta |
|---|---|---|---|
| Init fundamentals | FY2025 TTM (`2025-05-31`) | Identical | Same |
| `fundamental_observations` | 1,310 rows, max `2026-02-28` | Identical | Same |
| `sec_raw_facts` | 25,369 facts, max filed `2026-03-11` | Identical | Same |
| Price / market cap | Missing | Still missing | Same |
| Q4 FY2026 ingestion gap | Logged explicitly | Removed; replaced by valuation gap | Worse |
| Narrative claims | 36 | 14 | Worse |
| Narrative sources | 15 | 7 | Worse |
| Agreements / cruxes | 10 / 8 | 7 / 7 | Worse |
| Metric-linked claims | 0/36 | 0/14 | Same |
| Narrative timeliness | Q4 FY2026-aware | Q4 FY2025-era | Much worse |
| `fundamentals_summary` fix | Recommended | Not done | Same |

## What The Workspace Captures Well

Init strengths are unchanged and remain solid for discovery work:

| Artifact | Count | Status |
|---|---|---|
| `sec_raw_facts` | 25,369 | 513 unique concepts; max filed `2026-03-11` |
| `concept_catalog_entries` | 516 | Broad catalog with fact counts, period shapes, narrative tags |
| `fundamental_observations` | 1,310 | Quarterly/YTD/annual series through Q3 FY2026 |
| `canonical_metric_mappings` | 9 | High/medium confidence with rationale |
| `canonical_metric_definitions` | 9 | Core metrics defined |
| Quality gates (init + catalog) | pass/warn | `flow_metrics_period_labeled` warns on multi-shape flow concepts |

Representative Q3 FY2026 observations are internally consistent:

- Revenue (quarter): $17.19B (`2026-02-28`, filed `2026-03-11`)
- Net income (quarter): $3.72B
- EPS (quarter): $1.27
- Cash: $38.5B
- Debt current + non-current: $9.9B + $124.7B = ~$134.6B
- RPO (raw fact): $552.6B

Canonical mappings are auditable (e.g. revenue → `RevenueFromContractWithCustomerExcludingAssessedTax`, shares → `WeightedAverageNumberOfDilutedSharesOutstanding`).

## Data Quality Findings

### Critical

- **`[Critical]` Narrative anchored on wrong fiscal year** (run 5, all claims/sources): Primary sources are Q4 FY2025 materials (June–November 2025). Claims cite RPO of $138B (+41% YoY), FY2026 capex of $16B+, and OpenAI RPO of ~$30B. The live market debate (per prior QA and external sources) centers on Q4 FY2026: RPO $638B (+363% YoY), FY2027 net capex ~$70B, FY2027 revenue guide ~$90B. A downstream agent treating this board as current would build scenarios on obsolete assumptions.

### High

- **`[High]` Q4 FY2026 not ingested; gap no longer logged** (`fundamental_observations`, `data_gaps`): Latest observation period is Q3 FY2026 (`2026-02-28`). No Q4 FY2026 revenue ($19.2B), EPS, FCF (-$23.7B), or RPO ($638B) in observations or starter fundamentals. Run 3 explicitly logged `narrative_q4_fy2026_workspace_ingestion`; run 5 replaced it with `narrative_current_valuation_context` only.

- **`[High]` Starter fundamentals stale relative to catalyst** (`fundamentals`): Headline TTM metrics remain at `2025-05-31` — revenue $57.4B, EPS $4.34, total debt $134.6B. `fundamentals_summary` in `agent.rs` still queries fixed columns including `current_price` and `market_cap` that are not populated. The TODO to pull from catalog-selected time series remains unaddressed.

- **`[High]` Price and market cap missing** (`run_metadata`, `fundamentals`, `data_gaps`): `financial_fetch_status: partial`; `starter_financials` gap open. No valuation context for narrative or scenario work.

### Medium

- **`[Medium]` `total_debt` headline misaligned with latest quarter** (`fundamentals` vs `fundamental_observations`): Starter `total_debt` is FY2025 (`2025-05-31`, $134.6B) while quarterly debt observations exist through Q3 FY2026. Bear claim #10 in run 5 cites May 2025 debt without period context.

- **`[Medium]` No claims linked to workspace metrics** (`claims`): 0/14 claims use the `metric` column. Narratives are not grounded in `concept_catalog_entries` or `fundamental_observations` despite `workspace_sql` availability.

- **`[Medium]` Company profile incomplete** (`stock_info`): Exchange, sector, and industry are blank. Only ticker, company name, and currency (USD) are set.

- **`[Medium]` RPO composition not surfaced** (raw facts + narrative): `RevenueRemainingPerformanceObligation` exists in raw facts (31 observations, latest $552.6B Q3) but Q4 prepaid-GPU disclosure ($75B of $638B) is absent from narrative and un-ingested from filings.

### Low

- **`[Low]` TTM metrics use annual fallback** (`data_quality_flags`): `revenue_ttm`, `net_income_ttm`, `operating_income_ttm` flagged as annual-fallback; `gross_profit_ttm` excluded for baseline period.

- **`[Low]` `web_search_requests: 0`** in worker telemetry despite 7 sourced URLs — discovery path not observable.

## Product Readiness

**Init workspace alone:** Partial pass. Broad SEC fact custody, concept catalog, canonical traceability, and period-labeled observations support deterministic fundamentals and interesting-concept discovery. Not sufficient for catalyst-day work without manual re-fetch: missing market quote, stale starter TTM, no Q4 FY2026 rows, and weakened gap logging.

**Init + narrative (run 5):** Fail. The agent run succeeded validation gates but produced a board that is less useful than run 3. A later research agent could not build a smart, company-specific scenario report from this workspace without silently re-fetching everything and discarding the narrative layer.

**Gaps for downstream work:**

- No metric hooks from claims → catalog/fundamentals
- Pending sections (`financial_snapshot`, `watch_items`, `scenario_assumptions`) correctly empty
- No `crux_candidates` promoted (expected for this lane)
- Narratives behind market reality; init substrate unchanged since run 3

## Web Validation

| Field | Run 5 DB | External (Jun 2026) | Source | Status |
|---|---|---|---|---|
| Latest filed quarter | Q3 FY2026 (`2026-02-28`) | Q4 FY2026 ended `2026-05-31` | SEC / press release | **Stale by one quarter** |
| Q3 revenue | $17.19B | Consistent with Q3 filing | SEC Company Facts | **Confirmed** |
| RPO (latest in DB) | $552.6B (Q3) | $638B (Q4, +363% YoY) | Oracle investor release, CNBC | **Stale** |
| RPO (narrative claim) | $138B (+41% YoY) | $638B (Q4 FY2026) | Run 5 claim #1 vs official | **Wrong era** |
| FY2026 total revenue | Not in starter fundamentals | $67.4B (+17%) | Official release | **Missing** |
| Q4 revenue | Not ingested | $19.2B (+21%) | Official release, CNBC | **Missing** |
| Total debt (starter) | $134.6B @ May 2025 | Higher post-Q4 financing | Observations + press | **Misleading period** |
| Price / market cap | Missing | ~$184 post-earnings (Jun 11) | CNBC, financial media | **Missing** |
| FY2027 revenue guide | Not in workspace | ~$90B (+34%) | Official release | **Missing** |
| FY2027 non-GAAP EPS | Not in workspace | $8.05 | Official release | **Missing** |

## Narrative Board Summary (Run 5)

| Artifact | Count | Notes |
|---|---|---|
| Sources | 7 | All Q4 FY2025-era (Jun–Nov 2025) |
| Claims | 14 | Bull 9 / Bear 5; RPO, capex, OpenAI figures stale |
| Narrative map items | 14 | 7 agreements, 7 cruxes; no dominant/consensus/counter sides captured as separate items |
| Sections | 11 | orientation, business_model, why_now, narrative_map drafted; rest pending |
| Data gaps | 2 | `starter_financials`, `narrative_current_valuation_context` |

Run 5 cruxes reference $16B+ capex and ~$30B OpenAI RPO — pre-Q4 FY2026 framing. Run 3 cruxes covered backlog conversion, ROI, margins, moat, concentration, FCF, financing, and share gains against the $638B / $70B capex debate.

## Recommendations

1. **Re-ingest or manually seed Q4 FY2026** into `fundamental_observations` and refresh starter `fundamentals` (revenue, EPS, FCF, RPO, debt).
2. **Restore explicit ingestion gap** (`narrative_q4_fy2026_workspace_ingestion` or generalized filing-lag gap) whenever SEC Company Facts trail the latest earnings catalyst.
3. **Fix price/market-cap fetch** and close `starter_financials` gap.
4. **Wire `fundamentals_summary`** to catalog manager outputs — recent time series per selected metric, not six fixed FY2025 columns.
5. **Add source-freshness guidance** to narrative researcher prompt: prefer latest fiscal-year sources; reject prior-year earnings materials when a newer quarter has been reported.
6. **Re-run narrative research** on refreshed workspace; compare `xiaomi/mimo-v2.5-pro` vs `deepseek/deepseek-v4-flash` on timeliness, not just gate pass rate.
7. **Metric linking** — require at least a few SQL-grounded claims tied to `fundamental_observations` or catalog entries.

## Summary

Run 5's init workspace is a carbon copy of run 3's substrate — adequate for historical discovery, inadequate for the June 2026 Q4 catalyst. Narrative execution with `xiaomi/mimo-v2.5-pro` regressed: fewer claims, older sources, and a debate frame anchored a full fiscal year behind the market. The prior QA's tooling recommendations remain open; the new finding is that a successful agent run on an unchanged workspace can produce a materially worse board than the prior model if source selection drifts backward.
