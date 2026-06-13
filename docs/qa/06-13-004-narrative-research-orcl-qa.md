# Init Workspace & Narrative Research QA — ORCL (2026-06-13)

## Scope

(RUNNING "google/gemini-3-flash-preview")

QA inspection of **init workspace substrate** and **narrative researcher output** for one Oracle run, compared against prior ORCL QA reports:

| Run | SQLite path | Model | Prior QA |
|-----|-------------|-------|----------|
| 8 | `reports/stock-narrative-research/ORCL-2026-06-13-8/run.sqlite` | `google/gemini-3-flash-preview` | This report |
| 3 | `reports/stock-narrative-research/ORCL-2026-06-13-3/run.sqlite` | `deepseek/deepseek-v4-flash` | `06-12-001` |
| 5 | `reports/stock-narrative-research/ORCL-2026-06-13-5/run.sqlite` | `xiaomi/mimo-v2.5-pro` | `06-13-001` |
| 6 | `reports/stock-narrative-research/ORCL-2026-06-13-6/run.sqlite` | `deepseek/deepseek-v4-pro` | `06-13-002` |
| 7 | `reports/stock-narrative-research/ORCL-2026-06-13-7/run.sqlite` | `minimax/minimax-m3` | `06-13-003` |

Web validation performed against official filings/press releases, CNBC, and secondary financial media (June 2026).

Worker telemetry (run 8): 27 agent rounds, 27 client tool calls, 0 web search requests, ~$0.06 cost, ~56s latency.

## Verdict

**Fail — worst run in the model shootout.** `google/gemini-3-flash-preview` produced a minimal, year-stale narrative board anchored on **Q4 FY2025** (RPO $138B, June 2025 earnings framing) with only **5 claims, 3 sources, 3 agreements, and 3 cruxes**. Two of three sources use **placeholder URLs** (`no-url-placeholder`). The `why_now` section references June 2025 and "late 2025"; the logged research gap cites capex guidance of "$15B+." The run passed all validation gates in under 60 seconds. Not usable for downstream scenario work on the June 2026 ORCL catalyst.

## Five-Way Model Comparison

| Dimension | Run 3 (`v4-flash`) | Run 5 (`mimo`) | Run 6 (`v4-pro`) | Run 7 (`minimax-m3`) | Run 8 (`gemini-3-flash`) |
|---|---|---|---|---|---|
| Narrative timeliness | Q4 FY2026 | Q4 FY2025 | Q4 FY2026 | Q3 FY2026 | **Q4 FY2025** |
| RPO cited | $638B | $138B | $638B | $552.6B | **$138B** |
| Claims | 36 | 14 | 16 | 16 | **5** |
| Sources | 15 | 7 | 9 | 8 | **3** |
| Agreements / cruxes | 10 / 8 | 7 / 7 | 5 / 5 | 5 / 6 | **3 / 3** |
| Claims with `metric` | 0/36 | 13/14 | 16/16 | 3/16 | **4/5** |
| Placeholder source URLs | No | No | No | No | **2/3** |
| Q4 ingestion gap | Yes | No | No | No | No |
| Latency | ~299s+ | ~299s | ~326s | ~364s | **~56s** |
| Cost | — | ~$0.08 | ~$0.63 | ~$0.08 | **~$0.06** |

**Ranking for this ORCL catalyst:** Run 3 ≈ Run 6 >> Run 7 >> Run 5 >> **Run 8**.

## What The Narratives Section Captures Well

Very little relative to the June 2026 catalyst. The board is structurally minimal:

| Artifact | Count | Status |
|---|---|---|
| Sources | 3 | 1× Q4 FY2025 official; 2× commentary with **invalid placeholder URLs** |
| Claims | 5 | Bull 3 / Bear 2 |
| Agreements | 3 | Generic OCI growth, record RPO, AI capex catalyst |
| Cruxes | 3 | OCI price/perf vs hyperscalers, RPO conversion vs capex, GenAI sustainability |
| Sections | orientation, business_model, why_now, narrative_map | Drafted but thin (625–2,885 chars) |

**Generic themes that are directionally true but not catalyst-specific:**

- OCI growing faster than legacy software — correct in principle, stale in magnitude.
- AI data-center capex pressuring FCF — correct theme, no Q4 FY2026 figures (-$23.7B FCF, $55.7B capex).
- Database moat / enterprise migration to OCI — reasonable but shallow; no Fortune 100 vs revenue-share distinction (run 3's error is at least absent here because the claim is vague).

**`narrative_map` section JSON** includes bull/bear/consensus/dominant text, but consensus and dominant still cite **$138B+ RPO** and pre-Q4 framing. `counter_narrative` is empty.

## Data Quality Findings

### Critical

- **`[Critical]` Year-stale board on June 13, 2026 catalyst** (claims, sections, narrative_map): Primary bull claim: RPO **$138B (+44% YoY) at end of FY2025**. `why_now` opens with "Following the **June 2025** earnings which revealed a staggering $138B RPO" and "as of **late 2025**." Market debate on June 13 is Q4 FY2026: **$638B RPO**, -$23.7B FCF, $70B+ FY2027 capex, $40B financing. Using this board would misstate backlog by **~4.6×** and miss the post-earnings selloff entirely.

- **`[Critical]` Placeholder source URLs** (`sources` #2, #3): `https://www.trefis.com/no-url-placeholder-trefis-orcl-2025-12-08` and `https://profitvisionlab.com/no-url-placeholder-orcl-2026`. Claims cite these sources at high/medium confidence but URLs are not auditable. `claims_source_custody` gate **passed** — validation gap in tooling.

### High

- **`[High]` Insufficient board depth despite successful finalize** (all narrative tables): 5 claims and 3 cruxes vs 36/8 on run 3. Validation minimums are too low for a mega-cap post-earnings catalyst. Board reads like an early stub, not a research memo.

- **`[High]` Research gap cites obsolete capex scale** (`narrative_capex_fcf_granularity`): "Management guidance suggests significant increases in capex to **$15B+** range." Q4 FY2026 reported **$55.7B FY26 capex** and **~$70B net FY27**. Gap logging is actively misleading.

- **`[High]` No Q4 FY2026 sources** (source pack): Only official source is Q4 **FY2025** press release (June 11, 2025). No Q3/Q4 FY2026 8-K, investor release, or CNBC Q4 coverage despite run executing three days after Q4 FY2026 announcement.

- **`[High]` Dominant question misaligned** (orientation JSON): "Can Oracle scale its OCI capacity fast enough to capture current AI demand without compromising its investment-grade balance sheet?" — generic pre-Q4 framing. Missing $638B RPO conversion, $40B financing, and balance-sheet shock from Q4 guide.

### Medium

- **`[Medium]` Validation gates passed on inadequate artifact** (`quality_gate_results`): All `build_narrative_map` gates pass with 3 sources (2 broken URLs), 5 claims, 3 cruxes. Suggests gates enforce presence, not substance or freshness.

- **`[Medium]` `metric` labels without workspace grounding:** 4/5 claims have semantic metrics (`RPO`, `capex`, `OCI revenue growth`, `database market share`) but values are FY2025-era, not tied to `fundamental_observations` rows.

- **`[Medium]` High-confidence claims on weak sources:** 3/5 claims marked `high` confidence while resting on FY2025 release and placeholder URLs.

### Low

- **`[Low]` Init workspace unchanged:** FY2025 TTM fundamentals, 1,310 observations through Q3 FY2026, no price/market cap. Same as all prior runs.

- **`[Low]` Extremely fast completion (~56s)** for 27 rounds — suggests minimal tool exploration vs 300–360s on other models.

- **`[Low]` `web_search_requests: 0`** — no observable discovery path.

## Product Readiness

**Init workspace:** Partial pass (unchanged). Not the binding failure mode for this run.

**Init + narrative (run 8):** **Fail.** A later agent could not build scenario work without discarding the entire narrative layer and re-researching from scratch. Worse than run 5 (`mimo`) on depth (5 vs 14 claims) while matching its FY2025 staleness.

**Tooling concern:** This run demonstrates that **gate pass ≠ research quality**. Flash speed and low cost correlate with hollow output when freshness and source-custody checks are weak.

## Web Validation

| Field | Run 8 DB / Narrative | External (Jun 2026) | Source | Status |
|---|---|---|---|---|
| RPO | $138B (+44% YoY, FY2025) | $638B (Q4 FY2026) | Q4 FY2026 release | **Wrong era — 4.6× understated** |
| FY2026 revenue | Not cited | $67.4B | Official release | **Missing** |
| FY2026 FCF | "Pressuring FCF" (generic) | -$23.7B | Official release | **Missing** |
| FY2026 capex | Gap: "$15B+" | $55.7B | Official release | **Wrong by order of magnitude** |
| FY2027 revenue guide | Not cited | ~$90B (+34%) | Official release | **Missing** |
| Q4 IaaS growth | Not cited | 93% to $5.8B | Official release | **Missing** |
| Stock / catalyst | "Late 2025 valuation reset" | ~50% off peak post-Jun 10 Q4 | Market data | **Stale** |
| OCI faster than legacy | Stated | True but underspecified | — | **Directionally true** |
| Source URLs #2, #3 | Placeholder paths | N/A | — | **Invalid / unauditable** |
| Workspace revenue TTM | $57.4B @ May 2025 | Superseded by $67.4B FY26 | Workspace vs official | **Stale in DB** |

## Big Ideas Missing From The Narrative Board

Essentially the entire June 2026 debate. None of the following appear:

1. Q4 FY2026 results ($19.2B Q4 revenue, FY2026 totals).
2. $638B RPO and +$85B sequential jump.
3. $75B prepaid/customer-supplied GPU (BYOH).
4. $40B FY2027 financing (convert, ATM).
5. ~$70B FY2027 net capex.
6. OpenAI concentration / Stargate / $300B deal.
7. Abilene datacenter setback.
8. Credit/CDS stress, Moody's project-finance framing.
9. Cerner, litigation, Burry, Morningstar moat downgrade.
10. One-time Ampere/Bloom EPS adjustments.
11. Post-Q4 ~50% drawdown and analyst downgrades.

## Judgment On `google/gemini-3-flash-preview` vs Siblings

### Strengths

- **Lowest cost and latency** in the shootout (~$0.06, ~56s).
- **Passed validation** — technically completes the lane.
- **No egregious 80% DB market share error** (run 3) — because database moat claim is vague.
- **Basic OCI-vs-legacy structure** is not wrong, just obsolete.

### Weaknesses

- **Shallowest output:** 5 claims / 3 sources — minimum viable by a wide margin.
- **Worst timeliness tie with run 5:** FY2025 anchor on a Q4 FY2026 catalyst day.
- **Fabricated/placeholder provenance:** Two of three source URLs are not real links.
- **False confidence:** High-confidence claims on year-old data and broken citations.
- **Misleading gap log:** $15B+ capex vs $55.7B actual.
- **Speed as red flag:** Fast completion likely indicates insufficient tool use, not efficiency.
- **No compensating depth:** Unlike run 7 (credit/BYOH/Cerner) or run 5 (14 claims), run 8 offers neither timeliness nor thematic richness.

## Recommendations

1. **Reject boards with placeholder or invalid source URLs** — hard fail `claims_source_custody`.
2. **Freshness gate** — fail finalize if headline metrics (RPO, revenue guide) lag known earnings by >1 quarter.
3. **Minimum depth floors** — e.g. ≥10 claims, ≥5 sources, ≥5 cruxes for mega-cap post-earnings runs.
4. **Capex/FCF sanity check** — flag gaps or claims citing an order-of-magnitude wrong capex vs public guidance.
5. **Do not use `gemini-3-flash-preview` as primary narrative researcher** for catalyst-sensitive equity research without major prompt/guardrail changes.
6. **Re-ingest Q4 FY2026** — still the shared init constraint; flash models amplify workspace staleness when they don't web-compensate.

## Summary

`google/gemini-3-flash-preview` on run 8 is the **clearest fail** in the ORCL model comparison: year-stale numbers, placeholder URLs, five claims, and a `$15B` capex gap on a day when the market is digesting **$55.7B** actual capex and **$638B** RPO. Fast and cheap, but actively harmful as research input. Runs 3 and 6 remain the bar; run 8 shows validation gates can pass while output is unusable.
