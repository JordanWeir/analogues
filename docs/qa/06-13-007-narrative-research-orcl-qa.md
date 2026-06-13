# Init Workspace & Narrative Research QA — ORCL (2026-06-13)

## Scope

(RUNNING "qwen/qwen3.7-max")

QA inspection of **init workspace substrate** and **narrative researcher output** for one Oracle run, compared against prior ORCL QA reports:

| Run | SQLite path | Model | Prior QA |
|-----|-------------|-------|----------|
| 11 | `reports/stock-narrative-research/ORCL-2026-06-13-11/run.sqlite` | `qwen/qwen3.7-max` | This report |
| 3 | `reports/stock-narrative-research/ORCL-2026-06-13-3/run.sqlite` | `deepseek/deepseek-v4-flash` | `06-12-001` |
| 6 | `reports/stock-narrative-research/ORCL-2026-06-13-6/run.sqlite` | `deepseek/deepseek-v4-pro` | `06-13-002` |
| 9 | `reports/stock-narrative-research/ORCL-2026-06-13-9/run.sqlite` | `z-ai/glm-5.1` | `06-13-005` |
| 7 | `reports/stock-narrative-research/ORCL-2026-06-13-7/run.sqlite` | `minimax/minimax-m3` | `06-13-003` |
| 10 | `reports/stock-narrative-research/ORCL-2026-06-13-10/run.sqlite` | `qwen/qwen3.7-plus` | `06-13-006` |

Web validation performed against official filings/press releases, CNBC, Investing.com, and secondary financial media (June 2026).

Worker telemetry (run 11): 8 agent rounds, 21 client tool calls, 0 web search requests, ~$0.12 cost, ~184s latency.

## Verdict

**Partial fail on timeliness; partial pass on analytical depth.** `qwen/qwen3.7-max` is dramatically better than `qwen3.7-plus` (run 10): 10 claims, 7 agreements, 8 cruxes, 4 thoughtful custom gaps, and sophisticated themes (cRPO cancellability, OCI 14% instantaneous margin, GPU depreciation, Blue Owl/Pimco financing stress). However the board stops at **Q3 FY2026** ($553B RPO) with **no Q4 FY2026** capture ($638B, -$23.7B FCF, $70B capex) despite running three days after the June 10 release. FY2025 and Q3 figures are mixed without reconciliation. Usable as a **structural** debate framework with manual Q4 refresh; not catalyst-ready as-is.

## Shootout Ranking (Updated)

| Rank | Run | Model | Timeliness | Verdict |
|---|---|---|---|---|
| 1 | 3 | deepseek-v4-flash | Q4 FY2026 | Partial pass |
| 1≈ | 6 | deepseek-v4-pro | Q4 FY2026 | Partial pass |
| 3 | 9 | glm-5.1 | Q3+Q4 mixed | Partial pass |
| 4 | **11** | **qwen3.7-max** | **Q3 FY2026** | **Partial fail/pass** |
| 5 | 7 | minimax-m3 | Q3 FY2026 | Partial fail |
| 6 | 5 | mimo-v2.5-pro | FY2025 | Fail |
| 7 | 8 | gemini-3-flash | FY2025 | Fail |
| 8 | 10 | qwen3.7-plus | FY2024 | Fail |

**Run 3 ≈ Run 6 ≥ Run 9 ≥ Run 11 >> Run 7 >> Run 5 >> Run 8 >> Run 10.**

## What The Narratives Section Captures Well

The board passed all validation gates and offers the richest Qwen output by a wide margin:

| Artifact | Count | Status |
|---|---|---|
| Sources | 5 | FY2025 official release + Q3 $553B Investing.com + bear/bull commentary |
| Claims | 10 | Bull 5 / Bear 5 |
| Agreements | 7 | Mix transition, OCI differentiation, $553B RPO real, legacy stickiness, margin insulation, CapEx/FCF tension, OpenAI anchor |
| Cruxes | 8 | RPO conversion, OpenAI share, OCI margins, GPU useful life, off-BS CapEx, AI demand durability, legacy conversion, sovereign demand |
| Custom gaps | 4 | OpenAI/Stargate concentration, RPO cancellability, off-BS CapEx structure, starter financials |

**Analytical strengths rare in the shootout:**

- **cRPO vs total RPO / cancellability** (crux #1, gap #2) — directly addresses backlog-quality skepticism.
- **OCI instantaneous gross margin ~14%** vs guided 30–35% at maturity (agreement #5, crux #3) — filing-grade granularity.
- **GPU depreciation useful-life sensitivity** (6yr vs 4–5yr) — accounting crux few models surfaced.
- **Off-balance-sheet CapEx stress:** Blue Owl walk-away, Pimco 7.5% coupon offload (bear claim, crux #5, gap #4).
- **OpenAI concentration framing** as debate range (bull ~30%, bear >54% of $553B) — intellectually honest.
- **Three-layer business model** (legacy annuity / cloud apps / OCI) — clearest structural write-up after run 3.
- **Base-rate warning** in orientation — IBM/SAP transformation caution.
- **Sovereign/Pentagon demand** diversification thread (bull claim, crux #8).

**Compared to `qwen3.7-plus` (run 10):** Night-and-day improvement — 10 vs 6 claims, 7 vs 0 agreements, 8 vs 2 cruxes, Q3 vs FY2024 era, real source URLs, medium-confidence calibration on speculative claims.

## Data Quality Findings

### Critical

None at run 10 severity (FY2024 / $98B RPO). Q3 FY2026 data is internally coherent for the March 2026 quarter.

### High

- **`[High]` Q4 FY2026 entirely absent** (sources, claims, agreements, cruxes, orientation): Run executed June 13; Q4 reported June 10. No $638B RPO, no Q4 revenue $19.2B, no -$23.7B FY26 FCF (board uses ~-$24.7B TTM close), no $70B FY27 net capex, no $40B financing, no $75B BYOH disclosure. Dominant question and `why_now` frame the **Q3 $553B** inflection, not the post-Q4 selloff catalyst.

- **`[High]` FY2025 and Q3 FY2026 claims coexist** (bull claim #1 vs agreements/cruxes): Claim #1 cites **$138B RPO (FY2025)** and FY2025 revenue at high confidence while agreements #3 and all cruxes use **$553B (Q3 FY2026)**. Same claim-hygiene failure class as glm-5.1 run 9.

- **`[High]` CapEx guide stale** (bear claim #2, agreement #6, business model): **~$50B FY2026 CapEx (+136% YoY)** — Q3-era guide. Q4 FY2026 reported **$55.7B actual** and **~$70B FY27 net** guidance. Understates funding stress at the catalyst.

### Medium

- **`[Medium]` No official Q4 or Q3 8-K in pack** (`sources`): Newest official source is **FY2025 Q4 release (June 2025)**. Q3 $553B context comes from Investing.com commentary (#5), not SEC 8-K or Oracle investor Q3 page.

- **`[Medium]` OpenAI ~54% of $553B** (bear claim #3): Plausible analyst inference at medium confidence; gap correctly flags lack of segment disclosure. Denominator should update to $638B post-Q4.

- **`[Medium]` Four custom gaps but no filing-lag gap** (`data_gaps`): Excellent company-specific gaps; missing `narrative_q4_fy2026_workspace_ingestion` that run 3 logged.

- **`[Medium]` 3/10 metric-tagged claims** (`revenue_ttm`, `capital_expenditures`, `operating_margin`) — partial workspace grounding; bull claim #1 `revenue_ttm` carries FY2025 numbers.

### Low

- **`[Low]` Init workspace unchanged:** FY2025 TTM fundamentals, no price/market cap.

- **`[Low]` Only 8 agent rounds** — efficient (21 tool calls) but less exhaustive than 27-round DeepSeek runs.

- **`[Low]` `web_search_requests: 0`**.

## Product Readiness

**Init workspace:** Partial pass (unchanged). Agent used workspace-aligned debt/margin concepts indirectly.

**Init + narrative (run 11):** Partial pass for **structural** scenario work (RPO quality, OCI margins, financing architecture, OpenAI concentration); partial fail for **catalyst-day** work without Q4 number refresh. Best Qwen model for depth; not substitute for DeepSeek flash/pro on timeliness.

## Web Validation

| Field | Run 11 DB / Narrative | External (Jun 2026) | Source | Status |
|---|---|---|---|---|
| RPO | $553B (Q3 FY2026) | $638B (Q4 FY2026) | Q4 release | **Stale — one quarter** |
| RPO (orphan claim #1) | $138B (FY2025) | Superseded | FY25 vs Q3/Q4 | **Stale — should retire** |
| Q3 OCI gross margin | ~14% instantaneous | Reported in Q3 materials | Q3 commentary | **Plausible** |
| FY2026 CapEx guide | ~$50B | $55.7B actual FY26; ~$70B FY27 net | Q4 release | **Understated** |
| TTM FCF | ~-$24.7B | -$23.7B FY2026 | Q4 release | **Close / directionally correct** |
| OpenAI RPO share | ~54% of $553B | >50% cited by analysts | CNBC, secondary | **Plausible** (estimate) |
| FY2025 revenue | $57.4B | Correct for FY2025 | FY2025 release | **Confirmed** (wrong era for catalyst) |
| Blue Owl / Pimco stress | Cited | Reported in market commentary | Secondary | **Plausible** |
| Q4 FY2026 revenue | Not cited | $19.2B | Official release | **Missing** |
| $75B BYOH/prepaid GPU | Not cited | $75B per Q4 release | Official release | **Missing** |
| $40B FY2027 financing | Not cited | Confirmed | Official release | **Missing** |

## Big Ideas Missing

1. **Q4 FY2026 headline results** entirely ($638B RPO, Q4 revenue, 93% IaaS).
2. **$75B prepaid/BYOH** dollar disclosure (theme in gaps, not quantified).
3. **$40B financing plan** (convert, ATM).
4. **Abilene datacenter cancellation**.
5. **Post-Q4 stock selloff** (~50% off peak) — not in orientation.
6. **One-time Ampere/Bloom EPS adjustments**.
7. **Morningstar moat downgrade**, Burry short, securities litigation.
8. **RPO 12-month conversion (~12% / ~$77B)**.

## Judgment On `qwen/qwen3.7-max` vs `qwen3.7-plus` and Siblings

### Strengths

- **Largest quality gap between plus and max** in the shootout — max is a serious analyst; plus is unusable.
- **Eight cruxes** — tied with run 3 for count; most sophisticated accounting/financing cruxes in the series.
- **Four actionable custom gaps** — best gap quality alongside glm run 9.
- **Intellectually honest concentration debate** (30% vs 54% range).
- **Real source URLs** (vs gemini placeholders).
- **~$0.12 / 8 rounds** — better depth-per-dollar than v4-pro.

### Weaknesses

- **Q4 blindness** — same class as minimax run 7; worse than glm run 9 which partially captured Q4.
- **Claim reconciliation failure** — FY2025 $138B RPO claim not retired.
- **CapEx scale understated** at catalyst date.
- **No high-confidence Q4 anchors** — official source is one year old.

## Recommendations

1. **Never deploy qwen3.7-plus and qwen3.7-max interchangeably** — plus failed catastrophically; max is a different tier.
2. **Q4 freshness gate** — mandatory for all models including max.
3. **Claim supersession pass** — retire FY2025 headline claims when Q3/Q4 captures exist.
4. **Consider max as structural second pass** (RPO quality, GPU accounting, financing) after DeepSeek establishes Q4-clean claims.
5. Shared init fixes (Q4 ingest, price/market cap) remain necessary.

## Summary

`qwen/qwen3.7-max` on run 11 is the **strongest Qwen run and fourth in the overall shootout**: excellent crux and gap quality, credible financing-stress and margin-accounting threads, but **one quarter behind** on the June 13 catalyst with no $638B RPO. Vastly superior to `qwen3.7-plus`. For production: use DeepSeek flash/pro first; qwen3.7-max optional second pass on structure — not as primary catalyst researcher without freshness guards.
