# Init Workspace & Narrative Research QA — ORCL (2026-06-13)

## Scope

(RUNNING "google/gemini-3.1-flash-lite")

QA inspection of **init workspace substrate** and **narrative researcher output** for one Oracle run, compared against prior ORCL QA reports:

| Run | SQLite path | Model | Prior QA |
|-----|-------------|-------|----------|
| 12 | `reports/stock-narrative-research/ORCL-2026-06-13-12/run.sqlite` | `google/gemini-3.1-flash-lite` | This report |
| 8 | `reports/stock-narrative-research/ORCL-2026-06-13-8/run.sqlite` | `google/gemini-3-flash-preview` | `06-13-004` |
| 11 | `reports/stock-narrative-research/ORCL-2026-06-13-11/run.sqlite` | `qwen/qwen3.7-max` | `06-13-007` |
| 3 | `reports/stock-narrative-research/ORCL-2026-06-13-3/run.sqlite` | `deepseek/deepseek-v4-flash` | `06-12-001` |

Web validation performed against official filings/press releases, TIKR, and secondary financial media (June 2026).

Worker telemetry (run 12): 18 agent rounds, 21 client tool calls, 0 web search requests, ~$0.025 cost, ~34s latency.

## Verdict

**Fail — thin, Q3-stale, bull-skewed stub.** `google/gemini-3.1-flash-lite` produced a minimal board: **6 claims (5 bull / 1 bear), 3 sources, 2 agreements, 2 cruxes**. RPO **$553B** (Q3 FY2026) and CapEx **~$50B** are one quarter behind the June 13 catalyst; FY2025 revenue ($57.4B, +7%) anchors the bull case. No Q4 FY2026 ($638B RPO, -$23.7B FCF, $70B capex). Fastest and cheapest Gemini run (~34s, ~$0.025) but barely more substantive than `gemini-3-flash-preview` (run 8). Not usable for downstream work.

## Shootout Ranking (Full Series)

| Rank | Run | Model | RPO / Era | Claims | Verdict |
|---|---|---|---|---|---|
| 1≈ | 3 | deepseek-v4-flash | $638B / Q4 FY26 | 36 | Partial pass |
| 1≈ | 6 | deepseek-v4-pro | $638B / Q4 FY26 | 16 | Partial pass |
| 3 | 9 | glm-5.1 | Mixed Q3+Q4 | 17 | Partial pass |
| 4 | 11 | qwen3.7-max | $553B / Q3 FY26 | 10 | Partial fail/pass |
| 5 | 7 | minimax-m3 | $553B / Q3 FY26 | 16 | Partial fail |
| 6≈ | 5 | mimo-v2.5-pro | $138B / FY2025 | 14 | Fail |
| 6≈ | **12** | **gemini-3.1-flash-lite** | **$553B / Q3 FY26** | **6** | **Fail** |
| 6≈ | 8 | gemini-3-flash-preview | $138B / FY2025 | 5 | Fail |
| 9 | 10 | qwen3.7-plus | $98B / FY2024 | 6 | Fail (worst) |

**Run 12 ranks with mimo/gemini-3-flash tier — ahead of qwen-plus only.**

## What The Narratives Section Captures Well

Very little beyond generic OCI-transition boilerplate:

| Artifact | Count | Status |
|---|---|---|
| Sources | 3 | FY2025 official + TIKR $553B article + Oracle multi-cloud PR |
| Claims | 6 | Bull 5 / Bear 1 — **heavily skewed** |
| Agreements | 2 | OCI-linked growth; unprecedented CapEx |
| Cruxes | 2 | RPO conversion vs $50B spend; AI demand vs hyperscaler pricing |

**Directionally correct but thin themes:**

- OCI as growth engine — valid, undated.
- Large RPO and CapEx — correct direction; wrong quarter and magnitude for June 2026.
- Multi-cloud interoperability — real strategy, repeated twice in bull claims without evidence tier.
- Bear margin/impairment risk — single bear claim; no FCF, debt, OpenAI concentration, or credit stress.

**`narrative_map` JSON** includes bull/bear/consensus/dominant sides, but all reference **$553B RPO** and **$50B CapEx** — pre-Q4 framing. `counter_narrative` empty.

## Data Quality Findings

### Critical

None at FY2024/$98B severity. Q3 $553B is internally consistent with source #2 but **one quarter stale** for a June 13 post-Q4 run.

### High

- **`[High]` Q4 FY2026 absent** (entire board): No $638B RPO, Q4 revenue $19.2B, -$23.7B FY26 FCF, $70B+ FY27 capex, $40B financing, $75B BYOH, or post-earnings selloff. `why_now` frames valuation around **$553B backlog and $50B CapEx** — the pre-Q4 debate.

- **`[High]` Bull/bear imbalance** (`claims`): **5 bull / 1 bear** at all `high` confidence. Reads as advocacy, not research. No OpenAI concentration, financing stress, or execution setbacks.

- **`[High]` Thin board passed validation** (`quality_gate_results`): 6 claims, 2 cruxes, 2 agreements — same gate-failure class as runs 8 and 10. `narrative_debate_present` passed with one bear claim.

- **`[High]` FY2025 revenue anchor** (bull claim #1): **$57.4B (+7% YoY)** at high confidence — workspace TTM, not FY2026 $67.4B (+17%).

### Medium

- **`[Medium]` Mixed-era sources** (`sources`): Official source is **Q4 FY2025** (June 2025); RPO from TIKR **$553B** article (Q3 FY2026 era). No Q4 FY2026 primary source. Source #2 title includes **"Web Search Result"** — sloppy metadata.

- **`[Medium]` CapEx understated** (claims, cruxes): **~$50B** vs Q4-reported **$55.7B FY26** and **~$70B FY27 net**.

- **`[Medium]` Zero metric hooks** (`claims`): 0/6 use `metric` column; no workspace SQL grounding.

- **`[Medium]` Duplicate bull claims** (#4 and #5): Both restate multi-cloud interoperability with near-identical wording.

### Low

- **`[Low]` Init workspace unchanged:** FY2025 TTM fundamentals, no price/market cap.

- **`[Low]` Fastest run in shootout (~34s)** — speed correlates with shallowness.

- **`[Low]` `web_search_requests: 0`** despite source titled "Web Search Result."

## Gemini Family Comparison

| | Run 8 (`gemini-3-flash-preview`) | Run 12 (`gemini-3.1-flash-lite`) |
|---|---|---|
| Claims | 5 (3b/2b) | 6 (5b/1b) |
| RPO | $138B (FY2025) | $553B (Q3 FY2026) |
| Sources | 3 (2 placeholder URLs) | 3 (real URLs) |
| Agreements / cruxes | 3 / 3 | 2 / 2 |
| Latency | ~56s | **~34s** |
| Cost | ~$0.06 | **~$0.025** |
| Verdict | Fail | Fail (slightly newer RPO, still fail) |

**3.1-flash-lite** updates RPO from FY2025 to Q3 but remains a stub — not a meaningful upgrade over 3-flash-preview for production use.

## Product Readiness

**Init + narrative (run 12):** **Fail.** Discard for catalyst work. Slightly less misleading than run 8 on RPO scale, but thinner debate structure and worse bull/bear balance.

## Web Validation

| Field | Run 12 Narrative | External (Jun 2026) | Status |
|---|---|---|---|
| RPO | $553B | $638B (Q4 FY2026) | **Stale — one quarter** |
| FY25 revenue | $57.4B (+7%) | Correct for FY2025 | **Confirmed** (wrong era) |
| FY26 revenue | Not cited | $67.4B | **Missing** |
| CapEx | ~$50B | $55.7B FY26; ~$70B FY27 net | **Understated** |
| FY26 FCF | Not cited | -$23.7B | **Missing** |
| Q4 revenue / IaaS | Not cited | $19.2B; +93% IaaS | **Missing** |
| Multi-cloud strategy | Cited | Real Oracle initiative | **Directionally true** |
| OpenAI / Stargate | Not cited | Central to Q4 debate | **Missing** |

## Big Ideas Missing

All Q4 FY2026 catalyst threads plus most secondary themes: OpenAI concentration, BYOH $75B, $40B financing, Abilene, credit/CDS, Cerner, litigation, GPU utilization, margin accounting, post-selloff valuation, Morningstar/Burry, RPO conversion %, one-time EPS adjustments.

## Recommendations

1. **Do not use Gemini flash/lite variants** for catalyst-sensitive narrative research without freshness and depth gates.
2. **Fail boards with bull:bear ratio >3:1** at finalize for mega-cap names.
3. **Minimum depth floors** (≥10 claims, ≥5 cruxes) — run 12 would fail.
4. **Gemini 3.1-flash-lite is not an upgrade** over 3-flash-preview for this task — only RPO vintage improved.
5. Production path unchanged: **DeepSeek flash/pro primary**; glm-max or qwen-max optional second pass.

## Summary

`google/gemini-3.1-flash-lite` on run 12 is a **fail**: six high-confidence claims, one bear, Q3-era $553B RPO, no Q4 capture, ~34 seconds of work. Cheapest and fastest in the shootout, tied for shallowest with gemini-3-flash and qwen-plus. The Q3 RPO figure makes it marginally less wrong than run 8's $138B, but it remains unusable for June 2026 ORCL scenario work.
