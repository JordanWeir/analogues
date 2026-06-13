# Init Workspace & Narrative Research QA — ORCL (2026-06-13)

## Scope

(RUNNING "qwen/qwen3.7-plus")

QA inspection of **init workspace substrate** and **narrative researcher output** for one Oracle run, compared against prior ORCL QA reports:

| Run | SQLite path | Model | Prior QA |
|-----|-------------|-------|----------|
| 10 | `reports/stock-narrative-research/ORCL-2026-06-13-10/run.sqlite` | `qwen/qwen3.7-plus` | This report |
| 3 | `reports/stock-narrative-research/ORCL-2026-06-13-3/run.sqlite` | `deepseek/deepseek-v4-flash` | `06-12-001` |
| 6 | `reports/stock-narrative-research/ORCL-2026-06-13-6/run.sqlite` | `deepseek/deepseek-v4-pro` | `06-13-002` |
| 9 | `reports/stock-narrative-research/ORCL-2026-06-13-9/run.sqlite` | `z-ai/glm-5.1` | `06-13-005` |
| 8 | `reports/stock-narrative-research/ORCL-2026-06-13-8/run.sqlite` | `google/gemini-3-flash-preview` | `06-13-004` |

Web validation performed against official filings/press releases, CNBC, and secondary financial media (June 2026).

Worker telemetry (run 10): 13 agent rounds, 13 client tool calls, 0 web search requests, ~$0.05 cost, ~125s latency.

## Verdict

**Fail — most stale board in the shootout.** `qwen/qwen3.7-plus` produced a minimal narrative anchored on **Q4 FY2024** (RPO **$98B**, CapEx **$3.5B**/quarter, FY25 doubling narrative) — **two fiscal years** behind the June 13, 2026 catalyst. Only **6 claims, 3 sources, 2 cruxes, and zero agreement rows**. All six claims marked `high` confidence on obsolete data. Validation gates passed. Not usable for any downstream ORCL work without complete discard.

## Seven-Way Model Comparison (Shootout Summary)

| Dimension | Run 3 | Run 6 | Run 9 | Run 7 | Run 5 | Run 8 | Run 10 (`qwen3.7-plus`) |
|---|---|---|---|---|---|---|---|
| Timeliness | Q4 FY26 | Q4 FY26 | Q3+Q4 mix | Q3 FY26 | FY2025 | FY2025 | **FY2024** |
| RPO cited | $638B | $638B | $638B* | $552.6B | $138B | $138B | **$98B** |
| Claims | 36 | 16 | 17 | 16 | 14 | 5 | **6** |
| Sources | 15 | 9 | 8 | 8 | 7 | 3 | **3** |
| Agreements | 10 | 5 | 6 | 5 | 7 | 3 | **0** |
| Cruxes | 8 | 5 | 6 | 6 | 7 | 3 | **2** |
| Metrics on claims | 0/36 | 16/16 | 11/17 | 3/16 | 13/14 | 4/5 | **0/6** |
| Agent rounds | 27 | 27 | 12 | 21 | 27 | 27 | **13** |
| Cost | ~$0.10 | ~$0.63 | ~$0.14 | ~$0.08 | ~$0.08 | ~$0.06 | **~$0.05** |

\*Run 9 reaches $638B in agreements/cruxes but retains orphan $553B claims.

**Final ranking:** Run 3 ≈ Run 6 ≥ Run 9 >> Run 7 >> Run 5 >> Run 8 >> **Run 10**.

## What The Narratives Section Captures Well

Almost nothing relative to the June 2026 ORCL catalyst.

| Artifact | Count | Status |
|---|---|---|
| Sources | 3 | Q4 **FY2024** Motley Fool transcript + two bear commentaries |
| Claims | 6 | Bull 3 / Bear 3 |
| Agreements | **0** | `narrative_map_items` has no agreement rows; JSON `agreements` array empty |
| Cruxes | 2 | Generic margin and RPO conversion — both reference **$98B RPO** |
| Sections | orientation, business_model, why_now, narrative_map | Thin (525–2,435 chars) |

**Directionally true but obsolete themes:**

- OCI pivot from legacy database — correct arc, wrong era and magnitude.
- CapEx pressuring FCF — correct theme at **$3.5B/qtr FY24** scale, not **$55.7B FY26 / ~$70B FY27**.
- Margin compression from IaaS mix — valid structural point, undated.
- Multicloud Azure/GCP partnerships — real strategy, cited without current scale.

**`narrative_map` JSON** includes bull/bear/consensus/dominant text, but dominant narrative is Oracle's **"$50B+ bet"** and consensus cites **$98B RPO** — pre-Stargate, pre-Q4 FY2026 shock.

## Data Quality Findings

### Critical

- **`[Critical]` Two-year-stale board on June 13, 2026** (all claims, sections, cruxes): RPO **$98B (Q4 FY2024)** vs market reality **$638B (Q4 FY2026)** — **~6.5× understated**. CapEx **$3.5B in Q4 FY24, doubling in FY25** vs **$55.7B FY26 actual** and **~$70B FY27 guide**. OpenAI cited as signing **$12.5B in AI contracts in Q4 FY2024** — ancient context. `why_now` references **FY25 capacity build-out**; orientation time horizon tests **$98B RPO conversion**. Using this board would corrupt any scenario analysis.

- **`[Critical]` High-confidence claims on FY2024 data** (`claims`): All 6 claims marked `high` confidence. No hedging, no gap logged for missing FY2025/FY2026 results. False precision.

### High

- **`[High]` Thinnest substantive board in shootout** (tied with gemini on depth, worse on timeliness): 6 claims, 2 cruxes, **0 agreements**. `narrative_debate_present` gate **passed** with an empty agreements list in normalized rows.

- **`[High]` No Q4/Q3 FY2026 sources whatsoever** (`sources`): Newest source is **June 2024** Motley Fool Q4 FY2024 transcript. No 2025 or 2026 filings, press releases, or CNBC coverage. Run executed three days after Q4 FY2026 announcement.

- **`[High]` Dominant question misaligned** (orientation): "Can Oracle sustain its premium valuation by proving AI-driven OCI growth scales profitably without permanently diluting gross margins?" — generic FY24/FY25 framing. No $638B RPO, no -$23.7B FCF, no $40B financing, no post-earnings selloff.

### Medium

- **`[Medium]` Validation gates inadequate** (`quality_gate_results`): All `build_narrative_map` gates pass with 3 sources, 6 claims, 0 agreements, FY2024 numbers. Demonstrates same tooling failure as run 8.

- **`[Medium]` Zero metric hooks** (`claims`): 0/6 claims use `metric` column; no workspace SQL grounding.

- **`[Medium]` Minimal tool use:** 13 rounds = 13 tool calls — one call per round suggests shallow exploration vs 27–43 on competitive runs.

### Low

- **`[Low]` Init workspace unchanged:** FY2025 TTM fundamentals, no price/market cap. Agent ignored even Q3 workspace data ($552.6B RPO in `sec_raw_facts`).

- **`[Low]` `web_search_requests: 0`**.

- **`[Low]` Cheapest run (~$0.05)** — cost correlates with unusable output.

## Product Readiness

**Init workspace:** Partial pass (unchanged). Run 10 did not use available Q3 observations at all.

**Init + narrative (run 10):** **Fail.** Worse timeliness than runs 5 and 8 (FY2025-era); less depth than run 8 (6 vs 5 claims but 2 vs 3 cruxes and 0 agreements). Must be discarded entirely.

## Web Validation

| Field | Run 10 DB / Narrative | External (Jun 2026) | Source | Status |
|---|---|---|---|---|
| RPO | $98B (Q4 FY2024) | $638B (Q4 FY2026) | Q4 FY2026 release | **Wrong era — 6.5× understated** |
| Q4 CapEx | $3.5B (Q4 FY24) | Part of $55.7B FY26 | Q4 FY2026 release | **Wrong era** |
| FY26 FCF | Not cited | -$23.7B | Official release | **Missing** |
| FY27 capex | "$50B+ bet" dominant | ~$70B net | Official release | **Missing / wrong scale** |
| FY27 revenue | Not cited | ~$90B | Official release | **Missing** |
| OpenAI contracts | $12.5B (Q4 FY24) | $300B/5yr deal; $67B Q4 AI contracts | 2025–2026 sources | **Obsolete** |
| Q4 FY2026 revenue | Not cited | $19.2B | Official release | **Missing** |
| Stock catalyst | FY25 buildout | Post-Jun 10 selloff | Market | **Missing** |
| Workspace RPO (Q3) | Not used | $552.6B in `sec_raw_facts` | Workspace | **Ignored** |

## Big Ideas Missing

Essentially the entire 2025–2026 Oracle story. None of the following appear:

1. Stargate / $638B RPO / +363% YoY
2. Q4 FY2026 results ($19.2B revenue, 93% IaaS)
3. -$23.7B FY2026 FCF
4. $70B+ FY2027 capex and $40B financing
5. $75B BYOH/prepaid GPU
6. Abilene datacenter setback
7. Credit/CDS stress, S&P outlook
8. Cerner, litigation, Burry, Morningstar downgrade
9. Post-earnings ~50% drawdown
10. Any FY2025 or FY2026 primary source

## Judgment On `qwen/qwen3.7-plus` vs Siblings

### Strengths

- **Lowest cost in shootout** (~$0.05).
- **Completed lane quickly** (~125s).
- **Generic structural themes** (OCI pivot, margin mix, CapEx vs FCF) are not logically wrong — just two years old.

### Weaknesses

- **Worst timeliness:** FY2024 anchor beats gemini/mimo's FY2025 staleness.
- **High confidence on wrong data** — worse provenance hygiene than minimax (all inference) or gemini.
- **Ignored workspace:** Did not read $552.6B Q3 RPO from `sec_raw_facts` even without web Q4.
- **Thinnest debate structure:** 0 agreements, 2 cruxes.
- **No compensating depth:** Unlike glm (credit, nuclear, Cerner) or minimax (CDS, BYOH), qwen offers only obsolete generics.
- **Gate pass on empty agreements** — tooling bug.

## Recommendations

1. **Hard-fail boards** when newest source is >2 quarters behind run date for mega-cap post-earnings tickers.
2. **Minimum agreement count** in validation (≥3).
3. **Do not use `qwen/qwen3.7-plus` as narrative researcher** for catalyst-sensitive equity research without major guardrails.
4. **Confidence calibration** — disallow `high` on sources older than one fiscal year.
5. Shared init fixes (Q4 ingest, price/market cap) remain necessary but would not have saved this run — agent never reached Q3 workspace data.

## Summary

`qwen/qwen3.7-plus` on run 10 is the **weakest run in the ORCL model shootout**: FY2024 numbers ($98B RPO, $3.5B CapEx), six high-confidence claims, zero agreements, and no 2025–2026 sources on a June 13, 2026 run. Cheaper and faster than every other model, and the only output that is **two full fiscal years** behind the market. Discard entirely. Runs 3 and 6 remain the bar; glm-5.1 is the best mid-tier alternative if claim hygiene is fixed.
