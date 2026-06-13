# Init Workspace & Narrative Research QA — ORCL (2026-06-13)

## Scope

(RUNNING "deepseek/deepseek-v4-flash")

QA inspection of **init workspace substrate** and **narrative researcher output** for one Oracle run, compared against prior ORCL QA reports. This run was executed **after the narrative prompts overhaul**.

| Run | SQLite path | Model | Prior QA |
|-----|-------------|-------|----------|
| 15 | `reports/stock-narrative-research/ORCL-2026-06-13-15/run.sqlite` | `deepseek/deepseek-v4-flash` | This report |
| 14 | `reports/stock-narrative-research/ORCL-2026-06-13-14/run.sqlite` | `z-ai/glm-5.1` | `06-13-010` |
| 3 | `reports/stock-narrative-research/ORCL-2026-06-13-3/run.sqlite` | `deepseek/deepseek-v4-flash` | `06-12-001` |
| 13 | `reports/stock-narrative-research/ORCL-2026-06-13-13/run.sqlite` | `google/gemini-3.1-flash-lite` | `06-13-009` |

Web validation performed against official filings/press releases, CNBC, and secondary financial media (June 2026).

Worker telemetry (run 15): 9 agent rounds, 31 client tool calls, 0 web search requests, ~$0.059 cost, ~221s latency.

## Verdict

**Partial pass — lean, Q4-accurate, production-usable.** `deepseek/deepseek-v4-flash` post-overhaul delivers a tight catalyst board with **correct FY2026 revenue ($67.4B) and EPS separation ($7.63 actual / $8.05 FY27 guide / $6.83 ex-gains)** — avoiding the recurring glm arithmetic errors documented in `06-13-010`. Q4 sources are official; three thoughtful custom gaps; OpenAI concentration properly marked `inference`. Thinner than run 3 (15 vs 36 claims) but **higher number hygiene** and faster/cheaper. Ranks **~1st–2nd in the shootout** with run 3 on timeliness and accuracy; below run 3 on breadth.

## Post-Overhaul vs Run 3 (Same Model, Pre-Overhaul)

| Dimension | Run 3 (pre) | Run 15 (post-overhaul) | Delta |
|---|---|---|---|
| Claims | 36 (16b/19b/1n) | **15 (8b/5b/2n)** | Leaner; still balanced |
| Sources | 15 | 6 | Fewer but all Q4-relevant |
| Agreements | 10 | 3 | Thinner debate scaffolding |
| Cruxes | 8 | **7** | Comparable quality |
| Custom gaps | 2 | **4** | Better self-awareness |
| FY26 revenue | Correct in board | **$67.4B (+17%) — correct** | No $65.1B error |
| FY26 vs FY27 EPS | Stale orphan claim | **$7.63 / $6.83 ex-gains vs $8.05 guide — correct** | Major hygiene win |
| Agent rounds | 27 | **9** | ~3× faster |
| Cost | ~$0.10 | **~$0.06** | Cheaper |
| Latency | ~428s | **~221s** | ~2× faster |

**Net:** Prompt overhaul + DeepSeek trades breadth for accuracy and efficiency. Run 15 is the **reference implementation** for post-overhaul catalyst work; run 3 remains richer for secondary-theme mining if claim hygiene is applied manually.

## Post-Overhaul vs Run 14 (GLM)

| Dimension | Run 14 (glm) | Run 15 (deepseek) |
|---|---|---|
| FY26 revenue | **$65.1B — wrong** | **$67.4B — correct** |
| FY26 EPS | **$8.05 conflated with actual** | **$7.63 / $6.83 / $8.05 guide — correct** |
| FY27 growth math | Wrong comparator | **~34% from $67.4B — correct** |
| Claims | 15 | 15 |
| Custom gaps | 2 | **4** |
| OpenAI RPO | High confidence | **`inference` + gap logged** |
| Verdict | Partial pass | **Partial pass (stronger)** |

## Shootout Ranking (Updated)

| Rank | Run | Model | Era | Claims | Verdict |
|---|---|---|---|---|---|
| **1≈** | **15** | **deepseek-v4-flash** | **Q4 FY26** | **15** | **Partial pass (lean)** |
| 1≈ | 3 | deepseek-v4-flash | Q4 FY26 | 36 | Partial pass |
| 1≈ | 6 | deepseek-v4-pro | Q4 FY26 | 16 | Partial pass |
| 4 | 14 | glm-5.1 | Q4 FY26 | 15 | Partial pass |
| 5 | 9 | glm-5.1 | Q3+Q4 mix | 17 | Partial pass |
| 6 | 11 | qwen3.7-max | Q3 FY26 | 10 | Partial fail/pass |
| 7 | 13 | gemini-3.1-flash-lite | Q3 FY26 | 10 | Partial fail |

Run 15 ties run 3 on accuracy with **better EPS/revenue hygiene** and **worse breadth**.

## What The Narratives Section Captures Well

| Artifact | Count | Status |
|---|---|---|
| Sources | 6 | Official Q4 release + 8-K + CNBC + TrendSpider + 2× Seeking Alpha |
| Claims | 15 | Bull 8 / Bear 5 / Neutral 2 |
| Agreements | 3 | IaaS hypergrowth; $638B RPO visibility; FY27 ~34% growth consensus |
| Cruxes | 7 | RPO conversion, funding, OpenAI, margins, BYOH economics, multicloud, FCF/P-E |
| `crux_candidates` rows | **0** | Cruxes only in `narrative_map_items` |
| Custom gaps | 4 | Starter financials; Q4 SEC ingest lag; price/mcap; OpenAI RPO detail |
| Metric hooks | 15/15 | All claims carry `metric` keys |
| Validation gates | All pass | Including `narrative_debate_present` |

**Standout strengths:**

- **Correct headline financials** with ex-gains footnotes on both Q4 and FY26 EPS (claims #2, #5).
- **$638B RPO** with Q3 sequential (+$85B) and **$75B BYOH/prepaid** (claims #4, #10).
- **-$23.7B FCF**, **$55.7B FY26 capex**, **~$70B FY27 net capex** (claims #6, #11).
- **$40B FY27 financing** including **$20B ATM** (claim #8) — catalyst driver captured.
- **OpenAI >50% of RPO** marked `inference`, sourced to BofA via CNBC, with custom gap (claim #9).
- **404% multicloud AI Database** growth — company-disclosed fastest-ever business (claim #13).
- **Post-earnings selloff** ~10% despite beat (claim #12).
- **RPO 12-month conversion ~12%** in crux #1 — filing-grade detail.
- **`counter_narrative`** frames capital-raise selloff as infrastructure-development opportunity.
- **Four custom gaps** — best gap quality in post-overhaul runs.

## Data Quality Findings

### Critical

None. No materially wrong headline numbers that would corrupt scenario work.

### High

None at run 14 severity (wrong FY26 revenue/EPS).

### Medium

- **`[Medium]` Thinner board than run 3** (15 vs 36 claims, 3 vs 10 agreements): Missing Abilene cancellation, securities litigation, Michael Burry short, Morningstar moat downgrade, explicit RPO dollar schedule in claims (only in crux #1), and credit/CDS stress threads run 3 captured.

- **`[Medium]` Agreement #3 overstates consensus** (`narrative_map_items`): "All sides agree" FY27 ~34% growth is achievable — debatable; bears question funding and conversion, not headline demand.

- **`[Medium]` Debt figure Q3-stale** (`business_model`): $134.6B cited as Q3 FY2026; gap logs Q4 SEC ingest lag but narrative doesn't flag leverage bridge to post-Q4.

- **`[Medium]` Claim #14 bundles unrelated items** (CFO hire + Michigan datacenter JV): Tangential to core catalyst; weak thematic cohesion.

- **`[Medium]` `crux_candidates` table empty** — downstream scenario tooling may expect normalized crux rows.

### Low

- **`[Low]` `why_now` truncates EPS guide** to "~$8" instead of $8.05.

- **`[Low]` Init workspace unchanged:** FY2025 TTM in `fundamentals`; price/mcap gap logged and partially filled via web source in claim #12.

- **`[Low]` `web_search_requests: 0`** despite tool availability.

- **`[Low]` Orientation drawdown ~43%** vs ~47% in other runs — minor date/measurement variance.

## Product Readiness

**Init workspace:** Partial pass (unchanged). Agent logged Q4 SEC ingest lag and used sources to compensate.

**Init + narrative (run 15):** **Partial pass — catalyst-ready.** Usable for June 2026 ORCL scenario work without manual number fixes. Recommended **primary post-overhaul production model** for timeliness + accuracy; pair with run 3-style depth pass only if secondary themes are needed.

## Web Validation

| Field | Run 15 Narrative | External (Jun 2026) | Status |
|---|---|---|---|
| Q4 revenue | $19.18B (+21%) | $19.184B (+21%) | **Confirmed** |
| Q4 non-GAAP EPS | $2.11 ($2.03 ex-gains) | $2.11 ($2.03 ex-gains) | **Confirmed** |
| Q4 cloud revenue | $9.9B (+47%) | $9.913B (+47%) | **Confirmed** |
| Q4 IaaS | $5.8B (+93%) | ~$5.8B (+93%) | **Confirmed** |
| FY26 revenue | **$67.4B (+17%)** | **$67.357B (+17%)** | **Confirmed** |
| FY26 non-GAAP EPS | **$7.63 ($6.83 ex-gains)** | **$7.63 ($6.83 ex-gains)** | **Confirmed** |
| FY27 EPS guide | **$8.05 (+18% ex-gains)** | **$8.05** | **Confirmed** (correctly labeled guidance) |
| FY27 revenue guide | $90B (~34% growth) | $90B (~33.5% from FY26) | **Confirmed** |
| RPO | $638B (+363%) | $638B | **Confirmed** |
| BYOH/prepaid | $75B | $75B | **Confirmed** |
| FY26 FCF | -$23.7B | -$23.7B | **Confirmed** |
| FY26 capex | $55.7B (+162%) | $55.7B | **Confirmed** |
| FY27 net capex | ~$70B | ~$70B | **Confirmed** |
| $40B financing | Cited | Confirmed | **Confirmed** |
| OpenAI >50% RPO | Inference, BofA/CNBC | Plausible | **Plausible** (gap logged) |
| Multicloud DB +404% | Cited | Official release | **Confirmed** |
| Stock ~$184, -10% AH | Cited | CNBC | **Confirmed** |
| RPO 12-mo conversion ~12% | Crux #1 | Earnings call | **Confirmed** |

## Big Ideas Missing

1. **Abilene datacenter cancellation** (run 3/6 captured).
2. **Securities litigation** and **Michael Burry short**.
3. **Morningstar moat downgrade** (Wide → Narrow).
4. **S&P BBB negative outlook** (run 9 captured).
5. **One-time Ampere/Bloom EPS** — mentioned in claims but not as standalone bear thread.
6. **GPU utilization 97.5%** (run 14 captured; absent here).
7. **Cerner/Health AI** — brief in official source context only.

## Judgment On Prompt Overhaul + `deepseek/deepseek-v4-flash`

### Strengths

- **Correct FY26/FY27 number separation** — avoids glm-style $65.1B / $8.05-as-actual errors.
- **Best post-overhaul board** for catalyst-day work.
- **Inference discipline** on OpenAI RPO with gap.
- **15/15 metric-tagged claims** — full workspace grounding.
- **~2× faster and cheaper** than run 3 with comparable crux quality.

### Weaknesses

- **~60% fewer claims** than run 3 — less secondary-theme coverage.
- **Only 3 agreements** — thinner shared-ground framing.
- **No web search usage.**

## Recommendations

1. **Default post-overhaul production path:** `deepseek-v4-flash` primary (run 15 profile); glm-5.1 second pass for breadth if needed.
2. **Add minimum agreement floor** (≥5) if debate scaffolding matters for downstream agents.
3. **Finalize-time number cross-check** against captured official source — run 15 would pass; glm run 14 would fail.
4. **Populate `crux_candidates`** from `narrative_map_items` at finalize.
5. Shared init fixes (Q4 SEC ingest, live price/mcap) remain necessary; run 15 compensates well via sources and gaps.

## Summary

`deepseek/deepseek-v4-flash` on run 15 is the **best post-overhaul ORCL board**: 15 accurate claims, 7 cruxes, official Q4 sources, correct FY26/FY27 math, and four actionable gaps — in half the time and cost of run 3. Thinner than the original DeepSeek run but **strictly better on the recurring number-error failure mode**. **Catalyst-ready** for downstream scenario work without manual revenue/EPS fixes.
