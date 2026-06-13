# Init Workspace & Narrative Research QA — ORCL (2026-06-13)

## Scope

(RUNNING "google/gemini-3.1-flash-lite")

QA inspection of **init workspace substrate** and **narrative researcher output** for one Oracle run, compared against prior ORCL QA reports. This run was executed **after the narrative prompts overhaul**.

| Run | SQLite path | Model | Prior QA |
|-----|-------------|-------|----------|
| 13 | `reports/stock-narrative-research/ORCL-2026-06-13-13/run.sqlite` | `google/gemini-3.1-flash-lite` | This report |
| 12 | `reports/stock-narrative-research/ORCL-2026-06-13-12/run.sqlite` | `google/gemini-3.1-flash-lite` | `06-13-008` |
| 11 | `reports/stock-narrative-research/ORCL-2026-06-13-11/run.sqlite` | `qwen/qwen3.7-max` | `06-13-007` |
| 3 | `reports/stock-narrative-research/ORCL-2026-06-13-3/run.sqlite` | `deepseek/deepseek-v4-flash` | `06-12-001` |

Web validation performed against official filings/press releases, CNBC, and secondary financial media (June 2026).

Worker telemetry (run 13): 17 agent rounds, 17 client tool calls, 0 web search requests, ~$0.023 cost, ~42s latency.

## Verdict

**Partial fail — structurally improved after the prompt overhaul, still not catalyst-ready.** Run 13 is a clear step up from run 12 on board depth and debate balance, but it remains **one quarter behind** (Q3 FY2026, no Q4 capture) and introduces a **new source-quality regression** (4/5 placeholder `example.com` URLs). It ranks **~5th in the ORCL shootout** — above the thin Gemini/mimo tier, below qwen-max and the DeepSeek leaders.

## Prompt Overhaul vs Run 12 (Same Model)

The overhaul clearly moved the agent toward filling the board rather than stopping at a stub:

| Dimension | Run 12 (pre) | Run 13 (post-overhaul) | Delta |
|---|---|---|---|
| Claims | 6 (5 bull / 1 bear) | **10 (7 bull / 3 bear)** | +67% claims, much better balance |
| Agreements | 2 | 2 | Same count; slightly more generic |
| Cruxes (`narrative_map_items`) | 2 | **5** | Meaningful depth gain |
| Metric-tagged claims | 0/6 | **4/10** | Workspace SQL grounding works |
| Section prose | Thin | **Richer** (`business_model` ~2,935 chars) | Better orientation scaffolding |
| RPO cited | **$553B** (Q3) | **No dollar figure** | Regression |
| Source URLs | 3 real | **1 real + 4 `example.com` placeholders** | Critical regression |
| Era | Q3 FY2026 | Q3 FY2026 | No timeliness gain |

**Net:** The overhaul fixed the "empty board" failure mode but did not fix freshness, source custody, or catalyst capture. In some ways run 13 is **less auditable** than run 12 despite being more verbose.

## Shootout Ranking (Updated)

| Rank | Run | Model | Era | Claims | Verdict |
|---|---|---|---|---|---|
| 1≈ | 3 | deepseek-v4-flash | Q4 FY26 | 36 | Partial pass |
| 1≈ | 6 | deepseek-v4-pro | Q4 FY26 | 16 | Partial pass |
| 3 | 9 | glm-5.1 | Q3+Q4 mix | 17 | Partial pass |
| 4 | 11 | qwen3.7-max | Q3 FY26 | 10 | Partial fail/pass |
| **5** | **13** | **gemini-3.1-flash-lite** | **Q3 FY26** | **10** | **Partial fail** |
| 6 | 7 | minimax-m3 | Q3 FY26 | 16 | Partial fail |
| 7≈ | 12 | gemini-3.1-flash-lite | Q3 FY26 | 6 | Fail |
| 7≈ | 5 | mimo-v2.5-pro | FY2025 | 14 | Fail |
| 7≈ | 8 | gemini-3-flash | FY2025 | 5 | Fail |
| 10 | 10 | qwen3.7-plus | FY2024 | 6 | Fail (worst) |

Run 13 sits **between qwen-max and minimax**: more balanced than run 12, but far from DeepSeek/qwen-max on company-specific cruxes (OpenAI concentration, off-BS CapEx, GPU depreciation, cRPO cancellability, etc.).

## What The Narratives Section Captures Well

| Artifact | Count | Status |
|---|---|---|
| Sources | 5 | 1 official Q3 FY26; **4 placeholder `example.com` URLs** |
| Claims | 10 | Bull 7 / Bear 3 |
| Agreements | 2 | Cloud pivot; debt/M&A capital structure |
| Cruxes | 5 | OCI growth, AI capex/margins, deleveraging, RPO conversion, pricing power |
| `crux_candidates` rows | **0** | Cruxes only in `narrative_map_items` |
| Custom gaps | 1 | `starter_financials` only |
| Validation gates | All pass | Including `narrative_debate_present` |

**Genuine strengths vs run 12:**

- **Workspace-grounded Q3 numbers** on revenue ($17.19B), net income ($3.72B), EPS ($1.27), and debt ($134.6B) with `metric` hooks.
- **Three-layer business model** prose (legacy / cloud apps / OCI) is coherent and readable.
- **Cerner integration** surfaced as a multi-year cash-flow theme — rare in shallow runs.
- **Bear side expanded** to 3 claims (debt, margins, competition) vs run 12's single bear claim.
- **Five cruxes** are generic but correctly framed around the OCI-vs-hyperscaler tension.

**Compared to qwen-max (run 11):** Run 13 matches claim count (10) but lacks OpenAI/Stargate concentration, off-BS financing stress, OCI instantaneous margin (~14%), GPU useful-life accounting, cRPO cancellability, and custom filing-lag gaps.

## Data Quality Findings

### Critical

None at FY2024/$98B severity.

### High

- **`[High]` Q4 FY2026 entirely absent** (all sections, claims, cruxes): Run executed June 13, three days after Q4 (June 10). No $638B RPO, $19.2B Q4 revenue, -$23.7B FY26 FCF, $55.7B FY26 capex, ~$70B FY27 net capex, $40B financing, $75B BYOH/prepaid GPU, or post-earnings selloff. `why_now` frames "upcoming quarters" as proof-points — **the catalyst already happened**.

- **`[High]` Placeholder source custody** (`sources` #2–#5): Four sources use `https://example.com/...` URLs (`Financial news`, `Market commentary`). All 10 claims cite `source_id = 1` (official Q3 release), but the pack **looks** like five-source research. `claims_source_custody` gate passed anyway.

- **`[High]` RPO de-quantified** (bull claim #6): Says RPO "grew substantially… reaching record levels" with **no $553B or $638B**. Run 12 at least cited $553B. For ORCL narrative work, an unnumbered RPO claim is nearly useless.

### Medium

- **`[Medium]` Revenue growth rate wrong** (bull claim #1): Claims **15% YoY**; official Q3 FY26 release reports **22% YoY** ($17.19B). The dollar value is correct; the growth framing is not.

- **`[Medium]` Debt figure is Q3/FY2025-era** (bear claim #4): $134.6B matches workspace `fundamentals` (period `2025-05-31`), not post-Q4 leverage. Acceptable as workspace truth, but narratives don't flag staleness.

- **`[Medium]` All claims `high` confidence** — including qualitative claims on OCI momentum, RPO, and competition with no quantification.

- **`[Medium]` Generic cruxes** vs shootout leaders: No OpenAI concentration, financing architecture, RPO conversion %, BYOH economics, or post-Q4 valuation reset.

- **`[Medium]` `crux_candidates` table empty** — downstream scenario tooling may expect normalized crux rows.

### Low

- **Init workspace unchanged:** FY2025 TTM fundamentals ($57.4B revenue), no price/market cap; 25,369 SEC raw facts / 513 concepts — substrate is fine.

- **Source #1 URL typo:** `Third-Ready-Financial-Results` in the investor URL.

- **`web_search_requests: 0`** despite `openrouter:web_search` being enabled in worker metadata.

## Product Readiness

**Init workspace:** Partial pass (unchanged). Broad SEC facts universe and starter fundamentals present; price/market cap gap logged.

**Init + narrative (run 13):** Partial fail. Usable as a **structural smoke test** after the prompt overhaul (depth, balance, metric hooks), but not for June 2026 catalyst scenario work without Q4 refresh and source rewrite.

## Web Validation

| Field | Run 13 Narrative | External (Jun 2026) | Status |
|---|---|---|---|
| Q3 revenue | $17.19B | $17.19B (Q3 FY26) | **Confirmed** |
| Q3 revenue YoY | 15% | **22%** | **Wrong rate** |
| Q3 net income | $3.72B | $3.72B | **Confirmed** |
| Q3 GAAP EPS | $1.27 | $1.27 | **Confirmed** |
| Total debt | $134.6B | ~$134.6B at Q3 balance sheet | **Confirmed** (stale vs Q4) |
| RPO | "record levels" (no $) | $553B Q3; **$638B Q4** | **Missing + stale era** |
| FY26 revenue | Not cited | $67.4B (+17%) | **Missing** |
| FY26 FCF | Not cited | -$23.7B | **Missing** |
| FY26/FY27 CapEx | Not cited | $55.7B actual; ~$70B FY27 net | **Missing** |
| OpenAI / Stargate | Not cited | Central to Q4 debate | **Missing** |

## Big Ideas Missing

All Q4 FY2026 catalyst threads: $638B RPO, Q4 revenue/93% IaaS growth, negative FCF, capex scale, $40B financing, $75B BYOH, Abilene cancellation, post-selloff valuation, credit/CDS stress, one-time EPS adjustments, Morningstar/Burry, RPO 12-month conversion (~12%), securities litigation.

## Judgment On Prompt Overhaul + `gemini-3.1-flash-lite`

### What the overhaul fixed

- Minimum board depth (claims, cruxes, sections).
- Bull/bear balance (no longer 5:1 advocacy).
- Workspace metric hooks on headline financials.
- Richer structural prose (`business_model`, narrative sides).

### What it did not fix

- **Timeliness** — still Q3-blind to June 10 Q4.
- **Source discipline** — placeholder URLs got worse, not better.
- **Company-specific crux quality** — still generic cloud/debt themes.
- **RPO quantification** — actually regressed from run 12.

## Recommendations

1. **Fail boards with placeholder source URLs** — `claims_source_custody` should catch `example.com`.
2. **Require catalyst-quarter source** when run date is within N days of earnings.
3. **Minimum RPO dollar anchor** for ORCL-class names when official Q3/Q4 sources exist.
4. **Revenue growth claims must match filed %** or downgrade confidence.
5. **Populate `crux_candidates`** from `narrative_map_items` at finalize.
6. Keep **DeepSeek flash/pro as production path**; gemini-3.1-flash-lite post-overhaul is a better smoke test but not catalyst-ready.

## Summary

After the narrative prompts overhaul, `google/gemini-3.1-flash-lite` on run 13 produces a **more complete but still inadequate** ORCL board: 10 claims, 5 cruxes, better debate balance, and real Q3 dollar hooks — but **no Q4 capture**, **no RPO number**, **wrong revenue growth %**, and **80% placeholder sources**. It improves on run 12's thin stub (~5th in the shootout vs ~7th) without approaching DeepSeek or qwen-max analytical depth. **Not usable for June 2026 catalyst scenario work** without a full Q4 refresh and source rewrite.
