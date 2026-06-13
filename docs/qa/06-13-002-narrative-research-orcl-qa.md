# Init Workspace & Narrative Research QA — ORCL (2026-06-13)

## Scope

(RUNNING "deepseek/deepseek-v4-pro")

QA inspection of **init workspace substrate** and **narrative researcher output** for one Oracle run, compared against prior ORCL QA reports:

| Run | SQLite path | Model | Prior QA |
|-----|-------------|-------|----------|
| 6 | `reports/stock-narrative-research/ORCL-2026-06-13-6/run.sqlite` | `deepseek/deepseek-v4-pro` | This report |
| 3 | `reports/stock-narrative-research/ORCL-2026-06-13-3/run.sqlite` | `deepseek/deepseek-v4-flash` | `06-12-001` |
| 5 | `reports/stock-narrative-research/ORCL-2026-06-13-5/run.sqlite` | `xiaomi/mimo-v2.5-pro` | `06-13-001` |

Web validation performed against official filings/press releases, CNBC, Bloomberg/Dallas Morning News, and secondary financial media (June 2026).

Worker telemetry (run 6): 27 agent rounds, 30 client tool calls, 0 web search requests, ~$0.63 cost, ~326s latency.

## Verdict

**Partial pass — timely and usable, but thinner than run 3.** `deepseek/deepseek-v4-pro` produced a coherent Q4 FY2026-centered bull/bear map with accurate headline numbers ($638B RPO, $67.4B FY2026 revenue, -$23.7B FCF, $70B+ capex, $40B financing). It materially improves on run 5 (`xiaomi/mimo-v2.5-pro`) and surfaces several themes run 3 under-weighted (BYOH/prepaid GPU, RPO conversion timeline, Abilene execution). However, the board is less complete than run 3: fewer claims, sources, agreements, and cruxes; no explicit Q4 ingestion gap; analyst-consensus claim likely stale post-selloff; and several live debate threads remain absent (credit stress, litigation, Cerner, one-time EPS adjustments, moat downgrade).

## Three-Way Model Comparison

| Dimension | Run 3 (`v4-flash`) | Run 5 (`mimo-v2.5-pro`) | Run 6 (`v4-pro`) |
|---|---|---|---|
| Narrative timeliness | Q4 FY2026 | Q4 FY2025 (stale) | Q4 FY2026 |
| Claims | 36 | 14 | 16 |
| Sources | 15 | 7 | 9 |
| Agreements / cruxes | 10 / 8 | 7 / 7 | 5 / 5 |
| Claims with `metric` | 0/36 | 0/14 | **16/16** |
| Q4 ingestion gap logged | Yes | No | No |
| RPO $638B | Yes | No ($138B) | Yes |
| BYOH $75B prepaid GPU | Barely | No | **Yes** |
| Abilene datacenter risk | Thin | No | **Yes** |
| 80% DB market share error | Yes | N/A | **Avoided** |
| Init substrate | FY2025 TTM | Identical | Identical |
| Cost | — | ~$0.08 | ~$0.63 |

**Ranking for this ORCL catalyst:** Run 3 ≈ Run 6 >> Run 5. Run 6 is more focused and avoids some run 3 errors; run 3 is broader and more scenario-ready.

## What The Narratives Section Captures Well

The board passed all narrative validation gates and is anchored on the correct fiscal moment:

| Artifact | Count | Status |
|---|---|---|
| Sources | 9 | Q3 + Q4 FY2026 official releases, CNBC, bear/bull commentary |
| Claims | 16 | Bull 8 / Bear 8 |
| Agreements | 5 | Shared ground on demand, growth inflection, capex scale, RPO back-loading, execution risk |
| Cruxes | 5 | Build speed, RPO quality, funding, $90B guide, margin/EPS |
| Sections | orientation, business_model, why_now, narrative_map | Drafted; downstream sections pending |

**Dominant question is well-framed:** "Can Oracle convert its $638 billion AI infrastructure backlog into actual revenue and free cash flow before the financing burden destroys shareholder value?"

**Orientation JSON is strong.** Current setup cites mid-$160s stock, ~50% decline from peak, June 10 Q4 catalyst, $638B RPO, $70B+ capex, $40B financing, and Q1 FY2027 as next test. Time horizon 12–24 months through FY2027.

**Crux quality is good.** Five cruxes are falsifiable and map to the live debate: datacenter build speed vs Abilene setback, RPO convertibility, $150B+ cumulative capex funding, $90B revenue guide achievability, gross margin step-down vs $8.05 EPS.

**Improvements over run 3:**

- **BYOH / prepaid GPU ($75B)** surfaced prominently in bull claims and business model — run 3 QA flagged this as under-explored.
- **RPO conversion timeline (~12% in 12 months)** in bear claims and agreements — central to backlog skepticism.
- **Abilene expansion cancellation** (1.2GW → 2.0GW scrapped) with financing and power-grid context — confirmed by Bloomberg/Dallas Morning News (March 2026).
- **All claims carry `metric` labels** — semantic tags like `RPO`, `BYOH/prepaid contracts`, `Free cash flow`. Progress toward metric linking, though not yet wired to `fundamental_observations` keys.
- **No 80% database market share error** that corrupted run 3's moat framing.

**Business model section** credibly describes the three-segment mix (OCI IaaS, SaaS, legacy), names Meta/xAI/TikTok/AMD as customers, and explains the BYOH financing structure.

## Data Quality Findings

### Critical

None. Headline Q4 FY2026 numbers are materially correct and would not corrupt downstream scenario work.

### High

- **`[High]` Analyst consensus claim likely stale** (bull claim, `Analyst consensus`): States Strong Buy, 28 Buys / 5 Holds / 0 Sells, ~$265 average price target. Post-Q4 selloff (~50% off peak, stock ~$160s–$184), many analysts cut targets and sentiment shifted. Presenting pre- or mixed-era consensus as current bull evidence overstates Wall Street support. Run 3 at least captured Morningstar moat downgrade and Burry bear positioning.

- **`[High]` No Q4 FY2026 ingestion gap logged** (`data_gaps`): Only `starter_financials` (price/market cap) is open. Run 3 explicitly logged `narrative_q4_fy2026_workspace_ingestion`. Run 6 compensated via official sources but does not flag the workspace filing lag for downstream agents.

- **`[High]` Starter fundamentals still stale** (`fundamentals`): TTM metrics at `2025-05-31` — revenue $57.4B, EPS $4.34, debt $134.6B. `fundamentals_summary` in `agent.rs` unchanged. Init substrate identical to runs 3 and 5.

### Medium

- **`[Medium]` Thinner board than run 3:** 16 claims vs 36; 5 agreements vs 10; 5 cruxes vs 8. Missing normalized narrative sides (`dominant`, `bull`, `bear`, `consensus`, `counter_narrative` as `narrative_map_items`) — bull/bear text lives only in `narrative_map` section JSON. Less material for scenario prep.

- **`[Medium]` `metric` column is semantic, not SQL-grounded:** All 16 claims have labels like `RPO` or `Financing plan`, but none reference `canonical_key` or `fundamental_observations` IDs. Better than run 3's 0/36, not yet auditable against workspace SQL.

- **`[Medium]` Debt figures lack reconciled view:** Bear narrative cites negative FCF and $40B financing but does not bridge workspace `total_debt` ($134.6B FY2025) to post-Q4 balance sheet. Same class of issue as run 3.

- **`[Medium]` One-time EPS / adjusted growth framing absent:** Run 3 QA flagged FY2027 EPS growth mislabeling (5–6% vs 18% adjusted). Run 6 cites $8.05 EPS guide in bull claims but does not discuss Ampere/Bloom one-time gains distorting FY2026 comparables.

- **`[Medium]` Execution statistics need sourcing tier:** "Only one-third of 12GW US datacenters under construction" and "$64B in blocked projects" are plausible industry claims but presented at high confidence without filing-grade provenance.

### Low

- **`[Low]` Price / market cap missing** (`run_metadata`, `data_gaps`): `financial_fetch_status: partial`. Orientation cites "mid-$160s" from narrative research, not persisted quote.

- **`[Low]` `web_search_requests: 0`** despite 9 sourced URLs — discovery path not observable in telemetry.

- **`[Low]` Higher cost for comparable round count:** ~$0.63 vs ~$0.08 for `mimo-v2.5-pro` at same 27 rounds. Quality improved; cost efficiency did not.

## Product Readiness

**Init workspace:** Partial pass (unchanged). Broad SEC custody, 516 catalog entries, 1,310 observations through Q3 FY2026, canonical traceability. Inadequate for catalyst-day work without re-fetch: no market quote, stale starter TTM, no Q4 rows in observations.

**Init + narrative (run 6):** Partial pass. A later agent could build scenario work on the AI-infrastructure debate from agreements and cruxes. Board is usable but not as complete as run 3. Metric labels are a step toward SQL grounding; ingestion gap self-awareness regressed.

**Gaps for downstream work:**

- No `crux_candidates` promoted (expected)
- Pending sections correctly empty
- Narratives ahead of ingested fundamentals — agent compensated via official Q4 sources
- Missing secondary threads: credit/CDS stress, securities litigation, Cerner health, TikTok JV as diversification (mentioned in business model only), dividend sustainability, FY2030 long-term targets

## Web Validation

| Field | Run 6 DB / Narrative | External | Source | Status |
|---|---|---|---|---|
| RPO | $638B | $638B (+363% YoY) | Oracle Q4 press release | **Confirmed** |
| FY2026 revenue | $67.4B | $67.4B (+17%) | Official release | **Confirmed** |
| Q4 IaaS growth | 93% to $5.8B | 93% to $5.8B | Official release, CNBC | **Confirmed** |
| FY2026 FCF | -$23.7B | -$23.7B | Official release, press | **Confirmed** |
| FY2026 OpCF | $32B (+54%) | $32B | Official release | **Confirmed** |
| FY2026 capex | $55.7B | ~$55.7B reported | Official release | **Confirmed** |
| FY2027 net capex | $70B+ | ~$70B net | CNBC, official | **Confirmed** |
| FY2027 financing | $40B debt+equity | Same | Official release | **Confirmed** |
| FY2027 revenue guide | ~$90B (+34%) | $90B | Official release | **Confirmed** |
| FY2027 non-GAAP EPS | $8.05 | $8.05 | Official release | **Confirmed** |
| BYOH / prepaid GPU | $75B | $75B disclosed | Q4 press release | **Confirmed** |
| RPO 12-month conversion | ~12% | ~12% expected in next 12 months | Official release | **Confirmed** |
| Abilene expansion cancelled | 1.2GW → 2.0GW scrapped | Confirmed March 2026 | Bloomberg, Dallas Morning News | **Confirmed** |
| OpenAI $300B contract | Cited as concentration risk | WSJ Sep 2025 $300B/5yr deal reported | WSJ, DCD | **Confirmed** (reported; starts 2027) |
| Analyst Strong Buy / $265 PT | Stated as current | Post-Q4 downgrades, selloff | TipRanks (pre-earnings source in pack) | **Stale / misleading** |
| 30,000 layoffs (2 years) | Bear claim | Requires verification | — | **Unverified** |
| Init workspace revenue TTM | $57.4B @ May 2025 | Superseded by FY2026 $67.4B | Workspace vs official | **Stale in DB** |

## Big Ideas Missing From The Narrative Board

Compared to run 3 and live June 2026 commentary:

1. **Michael Burry short position** — material bear signal in run 3; absent here.
2. **Morningstar moat downgrade** (Wide → Narrow) — absent.
3. **One-time investment gains** (Ampere, Bloom) — EPS growth framing incomplete.
4. **Credit-market stress** — CDS highs, expensive debt raises, funding gap estimates.
5. **Cerner / Oracle Health** — $28B acquisition drag, VA delays, litigation.
6. **Securities litigation** — backlog conversion misstatement allegations.
7. **Dividend sustainability** — $0.50/qtr while FCF deeply negative.
8. **FY2030 long-term targets** — ~31% revenue CAGR management reconfirmed on Q4 call.
9. **Database moat / SaaS apocalypse** — run 3 overdid DB share; run 6 omits entirely. Net neutral but loses a real debate axis.
10. **Reported vs net capex framing** — $70B net vs $90–95B reported.

## Judgment On `deepseek/deepseek-v4-pro` vs Siblings

### Strengths

- **Source freshness:** Correctly prioritized Q4 FY2026 official release and June 2026 context. Major improvement over `mimo-v2.5-pro`.
- **Thematic precision:** BYOH, RPO back-loading, Abilene execution, and financing/dilution are the right cruxes for this moment.
- **Claim discipline:** Balanced 8/8 bull/bear split; 13/16 high confidence; no egregious factual errors on headline numbers.
- **Metric labeling:** First run in this series to populate `metric` on every claim.
- **Orientation quality:** Dominant question, catalyst calendar, and valuation context are investor-grade.

### Weaknesses

- **Board depth:** Stopped at minimum viable substance (16 claims, 5 cruxes) vs run 3's 36/8. Fewer tool calls (30 vs 43) suggests less exhaustive research.
- **Self-awareness:** Did not log workspace ingestion gap that run 3 captured.
- **Secondary themes:** Skipped credit, litigation, Cerner, Burry, moat downgrade — run 3 had more of these.
- **Stale bull evidence:** Pre-earnings analyst consensus used as high-confidence bull claim after a 50% drawdown.
- **Cost:** ~8× run 5 cost for a better but not clearly superior board vs run 3.

## Recommendations

1. **Re-ingest Q4 FY2026** into `fundamental_observations` and restore explicit filing-lag gaps.
2. **Fix price/market-cap fetch** before next ORCL run.
3. **Wire `metric` to canonical keys** — require at least a few claims linked to `fundamental_observations` or catalog entries, not just semantic labels.
4. **Source freshness validation** — downgrade or reject analyst-consensus claims when post-catalyst revisions exist.
5. **Depth floor in validation** — consider minimum claim/crux counts or require secondary-theme coverage for mega-cap catalyst runs.
6. **Model selection:** Use `deepseek/deepseek-v4-pro` or `v4-flash` over `mimo-v2.5-pro` for timeliness-sensitive narrative work on ORCL; flash may offer better cost/depth tradeoff pending A/B on same refreshed workspace.

## Summary

`deepseek/deepseek-v4-pro` on run 6 delivers a **credible, timely partial pass** — the right central story, accurate Q4 FY2026 numbers, and several improvements over both run 3 (BYOH, RPO conversion, Abilene) and run 5 (everything). It is **not clearly better than run 3 overall**: thinner coverage, no ingestion gap, stale analyst consensus, and missing secondary debate threads. Init workspace remains the binding constraint across all three runs. Re-ingest Q4, refresh quotes, and re-run flash vs pro on the same substrate for a fair model shootout.
