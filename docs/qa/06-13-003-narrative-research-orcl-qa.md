# Init Workspace & Narrative Research QA — ORCL (2026-06-13)

## Scope

(RUNNING "minimax/minimax-m3")

QA inspection of **init workspace substrate** and **narrative researcher output** for one Oracle run, compared against prior ORCL QA reports:

| Run | SQLite path | Model | Prior QA |
|-----|-------------|-------|----------|
| 7 | `reports/stock-narrative-research/ORCL-2026-06-13-7/run.sqlite` | `minimax/minimax-m3` | This report |
| 3 | `reports/stock-narrative-research/ORCL-2026-06-13-3/run.sqlite` | `deepseek/deepseek-v4-flash` | `06-12-001` |
| 5 | `reports/stock-narrative-research/ORCL-2026-06-13-5/run.sqlite` | `xiaomi/mimo-v2.5-pro` | `06-13-001` |
| 6 | `reports/stock-narrative-research/ORCL-2026-06-13-6/run.sqlite` | `deepseek/deepseek-v4-pro` | `06-13-002` |

Web validation performed against official filings/press releases, CNBC, SEC Q3 8-K, and secondary financial media (June 2026).

Worker telemetry (run 7): 21 agent rounds, 39 client tool calls, 0 web search requests, ~$0.08 cost, ~364s latency.

## Verdict

**Partial fail — one quarter stale at the catalyst.** `minimax/minimax-m3` produced a thoughtful, credit-aware debate map with strong secondary-theme coverage (CDS spreads, Moody's project-finance framing, Cerner, BYOH structure, multicloud distribution). However, the board anchors on **Q3 FY2026** ($552.6B RPO, -$13.2B TTM FCF, $50B capex) despite the run executing **three days after** the June 10 Q4 FY2026 release. No Q4 official source appears in the pack; headline numbers are wrong or mis-framed for the current debate ($638B RPO, -$23.7B FY2026 FCF, $55.7B FY2026 capex, ~34% FY2027 growth). All 16 claims are marked `inference` confidence. Better than run 5 (`mimo`) on theme depth and workspace grounding; worse than runs 3 and 6 on timeliness and headline accuracy.

## Four-Way Model Comparison

| Dimension | Run 3 (`v4-flash`) | Run 5 (`mimo-v2.5-pro`) | Run 6 (`v4-pro`) | Run 7 (`minimax-m3`) |
|---|---|---|---|---|
| Narrative timeliness | Q4 FY2026 | Q4 FY2025 | Q4 FY2026 | **Q3 FY2026** |
| RPO cited | $638B | $138B | $638B | **$552.6B** |
| Claims | 36 | 14 | 16 | 16 |
| Sources | 15 | 7 | 9 | 8 |
| Agreements / cruxes | 10 / 8 | 7 / 7 | 5 / 5 | 5 / 6 |
| Claims with `metric` | 0/36 | 13/14 | 16/16 | **3/16** |
| High-confidence claims | Many | 13/14 | 13/16 | **0/16** |
| Q4 ingestion gap | Yes | No | No | No |
| Credit/CDS stress | Thin | No | No | **Yes** |
| Cerner / health | Thin | No | No | **Yes** |
| BYOH / prepaid GPU | Barely | No | Yes | **Yes** |
| Abilene execution | Thin | No | Yes | No |
| Init substrate | FY2025 TTM | Identical | Identical | Identical |
| Agent rounds | 27 | 27 | 27 | **21** |
| Cost | — | ~$0.08 | ~$0.63 | ~$0.08 |

**Ranking for this ORCL catalyst:** Run 3 ≈ Run 6 >> Run 7 >> Run 5.

## What The Narratives Section Captures Well

The board passed all narrative validation gates and has genuine analytical depth on several axes run 3 under-weighted:

| Artifact | Count | Status |
|---|---|---|
| Sources | 8 | Q3 FY2026 8-K, official Q3 release, CNBC, bear/bull commentary |
| Claims | 16 | Bull 9 / Bear 5 / Consensus 2 |
| Agreements | 5 | RPO conversion debate, OCI technical edge, legacy cash cow, capex scale, analyst lag |
| Cruxes | 6 | RPO conversion, OpenAI concentration, AI margin durability, BYOH financing limits, OCI moat, drawdown interpretation |
| Sections | orientation, business_model, why_now, narrative_map | Drafted |

**Dominant question is well-framed structurally:** "Will the $552.6B RPO convert to revenue and free cash flow… or does customer concentration, sub-35% AI margins, and ~$50B/year capex turn Oracle into the next value-destroying AI infrastructure equity?" — but the **$552.6B figure is one quarter stale**; Q4 ended at $638B.

**Strengths unique to this run:**

- **Credit-market angle:** Moody's "world's largest project financings" framing, CDS spreads at 2008-cycle wides — live debate thread runs 3/6 largely skipped.
- **Cerner explicitly in business model** as a SaaS/healthcare line funding the AI buildout.
- **BYOH / project-finance structure** described in bear narrative and crux #4 — bank single-counterparty limits, leased-back GPU exposure.
- **Multicloud distribution** (Database@AWS/Azure/Google) with 531% Q3 multicloud DB growth — accurate for Q3.
- **Base-rate warning** in orientation JSON — dotcom-era capex cycle analogue with distinguishing features noted.
- **Custom research gap** (`narrative_oracle_rpo_disaggregation`) — requests OpenAI/Stargate RPO share, AI margin trajectory, BYOH vs balance-sheet capex breakdown. More specific than generic valuation gaps.
- **Partial workspace grounding:** `revenue_quarter` and `rpo_total` claims align with persisted Q3 observations ($17.19B revenue, $552.6B RPO in `sec_raw_facts`). The agent read the workspace — but stopped at filing lag instead of supplementing with Q4 press release.

**Orientation and why_now** correctly identify the 50% drawdown, Stargate/OpenAI concentration, and the rating-consensus-as-lagging-indicator framing.

## Data Quality Findings

### Critical

- **`[Critical]` Board anchored on Q3 FY2026, missing Q4 catalyst** (sources, claims, agreements, cruxes, orientation): Run executed June 13, 2026; Q4 FY2026 reported June 10. Source pack tops out at Q3 (March 11 filing). No Q4 press release, 8-K, or CNBC Q4 coverage. Dominant question, agreements, and multiple claims cite **$552.6B RPO** and Q3 growth rates while the market is debating **$638B RPO** (+$85B sequential), -$23.7B FY2026 FCF, $70B+ FY2027 net capex, and $40B financing. Downstream scenario work built on this board would understate backlog scale and capex/FCF stress by a full quarter.

### High

- **`[High]` FY2027 growth mis-framed** (bull claim #3): States management raised FY27 guide to $90B "from prior $89B," implying **~24% growth**. Correct framing: FY2026 actual revenue $67.4B → FY2027 guide $90B = **~34% YoY**. This error propagates into agreement #1 ("$90B FY27 revenue guide… not in dispute") without the correct growth magnitude.

- **`[High]` Capex and FCF figures stale** (claims, agreements, business_model): Cites **$50B+ FY26 capex** and **~-$13.2B TTM FCF**. Q4 FY2026 reported **$55.7B FY26 capex** and **-$23.7B FY2026 FCF**. Bear case on funding stress is understated.

- **`[High]` RPO conversion timeline uses Q3 framing** (bull claim #2, crux #1): "$73B expected over the next 12 months" from Q3 guidance. Q4 disclosed **~12% of $638B (~$76.6B)** in next 12 months plus 34% in months 13–36. Directionally similar but tied to wrong RPO base.

- **`[High]` Zero high-confidence claims** (`claims`): All 16 claims marked `inference`. No claim tied to official Q4 source at high confidence. Weaker provenance discipline than runs 5 and 6.

### Medium

- **`[Medium]` Analyst consensus as stale bull evidence** (consensus claims, agreement #5): "41 of 51 Buy, ~43% upside per FactSet" used on both sides. Post-Q4 ~50% drawdown, this is a lagging indicator — run 6 QA flagged the same pattern. Here it is structurally embedded in the debate framework, which is intellectually honest but still risks anchoring scenarios to pre-selloff sentiment.

- **`[Medium]` `metric` column partially grounded:** 3/16 claims use workspace-like keys (`revenue_quarter`, `rpo_total`, `free_cash_flow_ttm`). Values match Q3 workspace/raw facts, not latest market reality. Progress over run 3's 0/36, regression from run 6's semantic 16/16.

- **`[Medium]` AI infrastructure gross margin ~32%** (bear claim): Plausible industry estimate but unsourced at filing tier; presented as structural fact in crux #3.

- **`[Medium]` OpenAI $25B ARR / $280B ARR by 2030** (bull claim): Forward analyst projections at inference confidence; ability-to-pay argument is reasonable but evidence tier is thin.

- **`[Medium]` No Q4 ingestion gap logged** (`data_gaps`): Custom `narrative_oracle_rpo_disaggregation` gap is useful but does not flag that Q4 results exist externally and are absent from workspace. Run 3 captured this explicitly.

### Low

- **`[Low]` Init workspace unchanged:** FY2025 TTM fundamentals, no price/market cap, 1,310 observations through Q3 FY2026. Same substrate as all prior runs.

- **`[Low]` `web_search_requests: 0`** despite 8 sourced URLs.

- **`[Low]` Fewer agent rounds (21 vs 27)** — may explain thinner Q4 discovery vs DeepSeek runs.

## Product Readiness

**Init workspace:** Partial pass (unchanged). Agent correctly read Q3 data from `sec_raw_facts` but workspace filing lag became a **ceiling** rather than a problem to work around with external Q4 sources.

**Init + narrative (run 7):** Partial fail for catalyst-day use. Credit/BYOH/Cerner threads are valuable for a refreshed board, but headline numbers are wrong for June 13. A scenario agent would need to discard or heavily reconcile the RPO, FCF, and capex claims before use.

**Gaps for downstream work:**

- Pending sections correctly empty
- No metric hooks to Q4 observations (don't exist in workspace)
- Missing Q4-specific threads: $75B prepaid GPU (Q4 disclosure), Abilene cancellation, $40B financing plan detail, one-time Ampere/Bloom EPS adjustments, Morningstar moat downgrade, securities litigation

## Web Validation

| Field | Run 7 DB / Narrative | External (Jun 2026) | Source | Status |
|---|---|---|---|---|
| RPO | $552.6B (Q3) | $638B (Q4, +363% YoY) | Q4 press release | **Stale — one quarter** |
| Q3 revenue | $17.19B (+22%) | $17.19B | Q3 8-K | **Confirmed** (but not the catalyst) |
| Q3 cloud / IaaS | $8.93B / $4.88B (+84%) | Matches Q3 filing | SEC 8-K | **Confirmed** |
| FY2027 revenue guide | $90B, ~24% growth | $90B, **~34%** vs $67.4B FY26 | Q4 release | **Guide OK; growth rate wrong** |
| FY2026 capex | $50B+ cited | $55.7B reported | Q4 release | **Stale / understated** |
| FY2026 FCF | ~-$13.2B TTM | -$23.7B FY2026 | Q4 release | **Stale / understated** |
| RPO 12-month conversion | $73B expected | ~12% of $638B (~$76.6B) | Q4 release | **Directionally close; wrong base** |
| OpenAI $300B deal | Cited | WSJ Sep 2025 reported | Secondary | **Confirmed** (reported) |
| CDS / credit stress | Cited | Live in Jun 2026 commentary | Substack, market data | **Plausible** (secondary) |
| Multicloud DB +531% | Q3 claim | Q3 earnings materials | Q3 8-K | **Confirmed** (Q3 only) |
| BYOH structure | Described | $75B prepaid/customer GPU per Q4 | Q4 release | **Theme confirmed**; $75B figure not in board |
| Workspace revenue TTM | $57.4B @ May 2025 | Superseded by $67.4B FY26 | Workspace vs official | **Stale in DB** |

## Big Ideas Missing From The Narrative Board

Present in Q4/post-Q4 commentary but absent or stale in run 7:

1. **Q4 FY2026 results entirely** — $19.2B Q4 revenue, 93% IaaS growth, $67.4B FY26 total.
2. **$75B prepaid/customer-supplied GPU** — Q4 explicit disclosure; BYOH theme present but not the dollar figure.
3. **$40B FY2027 financing plan** — mandatory convert, $20B ATM (runs 3/6 captured).
4. **Abilene datacenter expansion cancellation** — execution risk signal (run 6 captured).
5. **One-time Ampere/Bloom EPS adjustments** — FY2027 EPS growth framing.
6. **Morningstar moat downgrade** — Wide → Narrow.
7. **Michael Burry short** — bear positioning.
8. **Securities litigation** — backlog conversion allegations.
9. **TikTok US JV** — diversification from OpenAI (IBD source title mentions TikTok but claim body does not develop it).

## Judgment On `minimax/minimax-m3` vs Siblings

### Strengths

- **Analytical depth on credit/financing:** Best CDS/Moody's/BYOH project-finance treatment in the four-run series.
- **Legacy business context:** Cerner, Fusion, NetSuite as funding engine — more complete than run 6.
- **Workspace-aware:** Actually queried Q3 observations for revenue and RPO; `rpo_total` matches `sec_raw_facts`.
- **Research gap quality:** `narrative_oracle_rpo_disaggregation` is company-specific and actionable.
- **Base-rate orientation:** Thoughtful dotcom-cycle warning without dismissing structural differences.
- **Cost-efficient:** ~$0.08 for 21 rounds — similar cost to mimo, far cheaper than v4-pro, with better theme coverage than mimo.

### Weaknesses

- **Catalyst blindness:** Failed to incorporate Q4 FY2026 despite running 72 hours after release. Most serious timeliness failure after mimo's full-year staleness.
- **Workspace ceiling effect:** Grounded to stale Q3 SQL instead of supplementing with external Q4 — the opposite of runs 3/6 which web-compensated around stale init.
- **Confidence hygiene:** 100% inference — no high-confidence official anchors.
- **Growth math error:** ~24% vs ~34% FY2027 revenue growth misframes the bull case magnitude.
- **Thinner than run 3:** Same claim count as run 6 but fewer sources, no Q4, and no high-confidence provenance.

## Recommendations

1. **Prompt: Q4 freshness gate** — If run date is after latest earnings, require Q4 (or latest quarter) official source before finalize; reject boards where max source quarter lags market by >1 quarter.
2. **Prompt: workspace lag override** — When `sec_raw_facts` max `filed_at` predates known earnings date, mandate external press-release capture and log `narrative_q*_workspace_ingestion` gap.
3. **Validation: headline number checks** — Flag RPO/revenue/FCF claims that don't match within tolerance of latest known quarter.
4. **Validation: confidence floor** — Require at least a few `high` confidence claims tied to official sources.
5. **Re-ingest Q4 FY2026** — Same binding constraint as all runs; minimax shows the risk of SQL-grounding without fresh filings.
6. **Model selection:** `minimax-m3` may be useful for credit/structure-heavy second passes on an already Q4-current board; not reliable as primary catalyst researcher without freshness guards.

## Summary

`minimax/minimax-m3` on run 7 is a **mixed bag**: stronger on credit markets, Cerner, BYOH financing, and workspace SQL reads than mimo; weaker than both DeepSeek runs on the only question that mattered June 13 — **what did Q4 change?** The board reads like a high-quality Q3-era memo that missed the June 10 inflection. Re-run on refreshed workspace with mandatory Q4 source gates, or use minimax as a secondary analyst after a timely flash/pro pass.
