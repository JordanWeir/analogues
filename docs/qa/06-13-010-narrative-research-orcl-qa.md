# Init Workspace & Narrative Research QA — ORCL (2026-06-13)

## Scope

(RUNNING "z-ai/glm-5.1")

QA inspection of **init workspace substrate** and **narrative researcher output** for one Oracle run, compared against prior ORCL QA reports. This run was executed **after the narrative prompts overhaul**.

| Run | SQLite path | Model | Prior QA |
|-----|-------------|-------|----------|
| 14 | `reports/stock-narrative-research/ORCL-2026-06-13-14/run.sqlite` | `z-ai/glm-5.1` | This report |
| 13 | `reports/stock-narrative-research/ORCL-2026-06-13-13/run.sqlite` | `google/gemini-3.1-flash-lite` | `06-13-009` |
| 9 | `reports/stock-narrative-research/ORCL-2026-06-13-9/run.sqlite` | `z-ai/glm-5.1` | `06-13-005` |
| 3 | `reports/stock-narrative-research/ORCL-2026-06-13-3/run.sqlite` | `deepseek/deepseek-v4-flash` | `06-12-001` |

Web validation performed against official filings/press releases, CNBC, Morningstar, and secondary financial media (June 2026).

Worker telemetry (run 14): 10 agent rounds, 27 client tool calls, 0 web search requests, ~$0.15 cost, ~142s latency.

## Verdict

**Partial pass — Q4-catalyst-aware and the strongest board since the DeepSeek runs, with a few high-severity number errors.** Run 14 captures the June 10 inflection ($638B RPO, -$23.7B FCF, $70B capex, OpenAI concentration, RPO conversion schedule, post-selloff valuation) with real sources and a populated `counter_narrative`. It ranks **~2nd–3rd in the shootout** — clearly ahead of run 13 and the Gemini tier, competitive with run 9, still a notch below DeepSeek flash/pro on claim hygiene.

## vs Run 13 (Gemini Post-Overhaul) and Run 9 (Prior GLM)

| Dimension | Run 13 (gemini) | Run 9 (glm) | **Run 14 (glm)** |
|---|---|---|---|
| Era | Q3 FY26 | Q3+Q4 mixed | **Q4 FY26** |
| Claims | 10 (7b/3b) | 17 (9b/8b) | **15 (8b/7b)** |
| Sources | 1 real + 4 placeholders | 8 (no official Q4) | **6 (official Q4 + 8-K + transcript)** |
| RPO | Unquantified | $638B (with Q3 orphans) | **$638B, clean** |
| Cruxes | 5 generic | 6 strong | **6 strong** |
| `counter_narrative` | Empty | Present | **BYOH accounting risk — excellent** |
| Custom gaps | 1 | 2 | **2** (incl. Q4 SEC ingestion lag) |
| Verdict | Partial fail | Partial pass | **Partial pass** |

Run 14 is a **material upgrade** over run 13 and a **refinement** of run 9: official Q4 sources, Q4 ingestion gap logged, no obvious Q3 orphan RPO claim, and better debate balance.

## Shootout Ranking (Updated)

| Rank | Run | Model | Era | Claims | Verdict |
|---|---|---|---|---|---|
| 1≈ | 3 | deepseek-v4-flash | Q4 FY26 | 36 | Partial pass |
| 1≈ | 6 | deepseek-v4-pro | Q4 FY26 | 16 | Partial pass |
| **3≈** | **14** | **glm-5.1** | **Q4 FY26** | **15** | **Partial pass** |
| 3≈ | 9 | glm-5.1 | Q3+Q4 mix | 17 | Partial pass |
| 5 | 11 | qwen3.7-max | Q3 FY26 | 10 | Partial fail/pass |
| 6 | 13 | gemini-3.1-flash-lite | Q3 FY26 | 10 | Partial fail |
| 7 | 7 | minimax-m3 | Q3 FY26 | 16 | Partial fail |
| 8≈ | 12 | gemini-3.1-flash-lite | Q3 FY26 | 6 | Fail |
| 8≈ | 5 | mimo-v2.5-pro | FY2025 | 14 | Fail |
| 8≈ | 8 | gemini-3-flash | FY2025 | 5 | Fail |
| 11 | 10 | qwen3.7-plus | FY2024 | 6 | Fail (worst) |

Run 14 is the **best glm run** and likely **third overall** — ahead of run 9 on source custody and Q4 cleanliness, behind DeepSeek on breadth (36 vs 15 claims) and error rate.

## What The Narratives Section Captures Well

| Artifact | Count | Status |
|---|---|---|
| Sources | 6 | Q4 press release, 8-K, transcript, Morningstar, Invezz, IR stock page — **all real URLs** |
| Claims | 15 | Bull 8 / Bear 7 — well balanced |
| Agreements | 4 | OCI growth real, leverage extreme, RPO is the anchor, Q4 noisy/BYOH |
| Cruxes | 6 | RPO conversion, OCI sustain, FCF vs debt, OpenAI concentration, margins, legacy decline |
| `crux_candidates` rows | **0** | Cruxes only in `narrative_map_items` |
| Custom gaps | 2 | `starter_financials`; `narrative_q4_fy2026_sec_ingestion_lag` |
| Metric hooks | 7/15 | Workspace + SEC concept keys |
| Validation gates | All pass | Including `narrative_debate_present` |

**Standout analytical threads:**

- **$638B RPO** with Q3 ($552.6B) and FY25 ($137.8B) trajectory (claims #5, #14).
- **RPO conversion schedule**: 12% / 34% / 54% beyond 36 months (claim #11, crux #1).
- **OpenAI ~$300B (~47% of RPO)** concentration (claim #9, crux #4).
- **-$23.7B FY26 FCF** and **$70B FY27 net capex** (claim #6, crux #3).
- **97.5% GPU utilization** (claim #10).
- **Stock drawdown** ~47% from $345.72 to $184.13 (claim #12).
- **`counter_narrative`** on BYOH/prepaid RPO accounting — filing-grade nuance rare in the shootout.
- **Orientation** correctly frames June 10 catalyst and 2–3 year conversion window.

**Compared to run 9:** Run 14 adds official Q4 press release and 8-K (run 9 relied on Futurum secondary), logs Q4 SEC ingestion lag, avoids unreconciled $553B orphan claim, and populates a stronger `counter_narrative`. Fewer claims (15 vs 17) but cleaner Q4 framing.

## Data Quality Findings

### Critical

None at FY2024/$98B severity. Q4 headline numbers are present in the right places.

### High

- **`[High]` FY2026 revenue wrong** (bull claim #2): Board says **$65.1B (+24% YoY)**; official release is **$67.4B (+17%)**. Material error for scenario math.

- **`[High]` FY2026 vs FY2027 EPS conflated** (bull claims #3, #7): Claim #3 says FY2026 non-GAAP EPS was **$8.05**; actual FY2026 non-GAAP EPS was **$7.63** ($6.83 ex one-times). **$8.05 is FY2027 guidance.** Claim #7 then calls FY27 EPS "~flat YoY" — wrong comparator because FY26 was misstated.

- **`[High]` Side tag mismatch** (claim #14): Tagged **bull** but argues BYOH/prepaid RPO **"may not convert to cash flow the same way"** — bear/counter-narrative content on the bull side. Could skew downstream scenario weighting.

### Medium

- **`[Medium]` Debt-to-equity "400%+"** (claim #8) vs **3.5x** (claim #13): Internally inconsistent; ~3.5x is closer to workspace Q3 equity ($38.5B) vs debt ($134.6B).

- **`[Medium]` Q4 debt/equity may be stale** (claims #8, #13): Uses Q3 FY26 balance sheet through workspace; Q4 release may show different leverage.

- **`[Medium]` No $75B BYOH/prepaid dollar figure** in claims — theme in `counter_narrative` and agreement #4, but not quantified.

- **`[Medium]` Missing secondary threads** vs run 3: Abilene cancellation, $40B financing structure, Ampere/Bloom EPS adjustments, securities litigation, Morningstar moat downgrade.

- **`[Medium]` `crux_candidates` table empty** — downstream scenario tooling may expect normalized crux rows.

### Low

- **Init workspace unchanged:** FY2025 TTM fundamentals, no price in `fundamentals` (though claim #12 cites $184.13 from IR source).

- **`web_search_requests: 0`** despite `openrouter:web_search` being enabled.

## Product Readiness

**Init workspace:** Partial pass (unchanged). Agent logged Q4 SEC ingestion lag; partially grounded debt and capex accrual claims to workspace observations.

**Init + narrative (run 14):** Partial pass. **Production-usable for catalyst-day ORCL narrative work** after fixing claims #2, #3, #7, and re-tagging #14. Best glm output in the shootout; credible second-pass analyst alongside DeepSeek flash/pro.

## Web Validation

| Field | Run 14 Narrative | External (Jun 2026) | Status |
|---|---|---|---|
| Q4 revenue | $19.2B (+21%) | $19.2B | **Confirmed** |
| FY26 revenue | **$65.1B (+24%)** | **$67.4B (+17%)** | **Wrong** |
| Q4 non-GAAP EPS | $2.11 | $2.11 | **Confirmed** |
| FY26 non-GAAP EPS | **$8.05** | **$7.63** ($6.83 adj.) | **Wrong — used FY27 guide** |
| RPO | $638B | $638B | **Confirmed** |
| FY26 FCF | -$23.7B | -$23.7B | **Confirmed** |
| FY27 capex | $70B net | ~$70B net | **Confirmed** |
| FY27 revenue guide | $90B | $90B | **Confirmed** |
| OCI +93% Q4 | Cited | Confirmed | **Confirmed** |
| GPU utilization 97.5% | Cited | Earnings call | **Confirmed** |
| OpenAI ~$300B / 47% | Cited | WSJ/analyst secondary | **Plausible** |
| Stock ~$184 / -47% | Cited | Market data | **Plausible** |

## Big Ideas Missing

1. **$75B BYOH/prepaid dollar figure** in claims (theme present, number absent).
2. **$40B FY2027 financing plan** (mandatory convert, $20B ATM).
3. **Abilene datacenter cancellation**.
4. **One-time Ampere/Bloom EPS adjustments** for FY2027 comparables.
5. **Morningstar moat downgrade** (Wide → Narrow).
6. **Michael Burry short**, securities litigation.
7. **S&P BBB negative outlook** (run 9 captured; run 14 omits).

## Judgment On `z-ai/glm-5.1` Post Prompt Overhaul

### Strengths

- **Q4 capture with official sources** — press release, 8-K, transcript in pack (run 9 lacked official Q4).
- **Best glm run** — balanced debate, six falsifiable cruxes, populated counter-narrative.
- **Q4 ingestion gap logged** — self-aware about workspace staleness.
- **Company-specific depth:** OpenAI concentration, RPO conversion schedule, GPU utilization, drawdown levels, BYOH accounting risk.
- **Efficient:** 10 rounds, ~$0.15, ~142s — strong depth-per-dollar.

### Weaknesses

- **FY2026 revenue and EPS errors** — could corrupt scenario projections if not caught.
- **Claim #14 side tag** — bull/bear hygiene failure.
- **Fewer claims than run 3** — missing financing, litigation, credit-rating threads.
- **No web search usage** despite tool availability.

## Recommendations

1. **Validate FY revenue/EPS claims against official release** before finalize — run 14 would fail a simple cross-check.
2. **Side-tag validation** — flag claims whose text contradicts assigned `side`.
3. **Populate `crux_candidates`** from `narrative_map_items` at finalize.
4. **Use glm-5.1 as post-overhaul production tier** alongside DeepSeek — not gemini flash-lite for catalyst work.
5. Shared init fixes (Q4 ingest, price/market cap) remain necessary; run 14 partially compensated via sources and gap logging.

## Summary

`z-ai/glm-5.1` on run 14 delivers a **partial pass**: the strongest post-overhaul board in the shootout, Q4-catalyst-ready with official sources, balanced debate, and sophisticated BYOH/RPO accounting tension — marred by FY2026 revenue/EPS errors and one mis-tagged claim. Ranks **third overall** behind DeepSeek flash/pro, ahead of run 9, qwen-max, and all Gemini runs. Fix claims #2, #3, #7, and #14 before downstream scenario work.
