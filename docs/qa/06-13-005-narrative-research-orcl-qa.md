# Init Workspace & Narrative Research QA — ORCL (2026-06-13)

## Scope

(RUNNING "z-ai/glm-5.1")

QA inspection of **init workspace substrate** and **narrative researcher output** for one Oracle run, compared against prior ORCL QA reports:

| Run | SQLite path | Model | Prior QA |
|-----|-------------|-------|----------|
| 9 | `reports/stock-narrative-research/ORCL-2026-06-13-9/run.sqlite` | `z-ai/glm-5.1` | This report |
| 3 | `reports/stock-narrative-research/ORCL-2026-06-13-3/run.sqlite` | `deepseek/deepseek-v4-flash` | `06-12-001` |
| 5 | `reports/stock-narrative-research/ORCL-2026-06-13-5/run.sqlite` | `xiaomi/mimo-v2.5-pro` | `06-13-001` |
| 6 | `reports/stock-narrative-research/ORCL-2026-06-13-6/run.sqlite` | `deepseek/deepseek-v4-pro` | `06-13-002` |
| 7 | `reports/stock-narrative-research/ORCL-2026-06-13-7/run.sqlite` | `minimax/minimax-m3` | `06-13-003` |
| 8 | `reports/stock-narrative-research/ORCL-2026-06-13-8/run.sqlite` | `google/gemini-3-flash-preview` | `06-13-004` |

Web validation performed against official filings/press releases, CNBC, Futurum, Morningstar, and secondary financial media (June 2026).

Worker telemetry (run 9): 12 agent rounds, 27 client tool calls, 0 web search requests, ~$0.14 cost, ~168s latency.

## Verdict

**Partial pass — Q4-aware but internally inconsistent.** `z-ai/glm-5.1` produced a broader, more theme-rich board than runs 6–8: $638B RPO, Q4 revenue/OCI figures, -$23.7B FCF, $70B capex, Cerner, S&P BBB negative outlook, nuclear/SMR regulatory risk, and two thoughtful custom gaps. However, **orphaned Q3 claims** ($553B RPO, Q3 OCI $4.9B) coexist with Q4-updated agreements/cruxes without reconciliation. BYOH is mislabeled "Bring Your Own **Hadoop**." FY2027 growth math is wrong (~40% from FY25 vs ~34% from FY26). No official Q4 press release in the source pack. Usable for scenario prep with manual cleanup; not as clean as run 6, richer than run 7 on Q4 themes.

## Six-Way Model Comparison

| Dimension | Run 3 | Run 5 | Run 6 | Run 7 | Run 8 | Run 9 (`glm-5.1`) |
|---|---|---|---|---|---|---|
| Timeliness | Q4 FY2026 | FY2025 | Q4 FY2026 | Q3 FY2026 | FY2025 | **Q3+Q4 mixed** |
| RPO in cruxes/agreements | $638B | $138B | $638B | $552.6B | $138B | **$638B** |
| Claims | 36 | 14 | 16 | 16 | 5 | **17** |
| Sources | 15 | 7 | 9 | 8 | 3 | **8** |
| Agreements / cruxes | 10/8 | 7/7 | 5/5 | 5/6 | 3/3 | **6/6** |
| Claims with `metric` | 0/36 | 13/14 | 16/16 | 3/16 | 4/5 | **11/17** |
| High-confidence claims | Many | 13/14 | 13/16 | 0/16 | 3/5 | **9/17** |
| Agent rounds | 27 | 27 | 27 | 21 | 27 | **12** |
| Cost | ~$0.10 | ~$0.08 | ~$0.63 | ~$0.08 | ~$0.06 | **~$0.14** |

**Ranking for this ORCL catalyst:** Run 3 ≈ Run 6 ≥ **Run 9** >> Run 7 >> Run 5 >> Run 8.

## What The Narratives Section Captures Well

The board passed all narrative validation gates and covers more secondary threads than most runs:

| Artifact | Count | Status |
|---|---|---|
| Sources | 8 | Q3 8-K/transcript/CNBC + Q4 Futurum + Morningstar + bear commentary |
| Claims | 17 | Bull 9 / Bear 8 |
| Agreements | 6 | OCI hypergrowth, $638B RPO, negative FCF through FY27, DB moat floor, drawdown pricing, margin dilution |
| Cruxes | 6 | OCI margins, RPO conversion, OpenAI concentration, credit rating, multicloud halo, nuclear power timeline |
| Custom gaps | 2 | OpenAI RPO concentration; OCI training vs inference mix |

**Q4 FY2026 headline numbers appear in the right places:**

- Agreement #2: **$638B RPO**
- Bull claim #12: Q4 revenue **$19.18B**, OCI **$5.79B (+93%)**, RPO **$638B**
- Bear claims: **-$23.7B FY26 FCF**, **~$70B FY27 capex**
- Orientation: 58% drawdown **$346 → ~$185**, $638B RPO, -$23.7B FCF

**Themes run 3/6 under-weighted that run 9 surfaces:**

- **S&P BBB negative outlook** and total obligations framing ($134.6B debt + ~$135B leases)
- **Cerner** ($28B acquisition, margin drag) in business model
- **Morningstar** fair-value source in pack
- **Nuclear/SMR/Three Mile Island** regulatory crux (#6) — unique in shootout
- **GPU utilization 97.5%** (Q4) — confirmed in earnings commentary
- **$75B prepaid/BYOH** theme present (mislabeled — see findings)
- **Stock level specificity** ($346 high, ~$185 current)
- **Thoughtful gaps:** OpenAI RPO share unverified; OCI training vs inference mix unknown

**Crux quality is strong.** Six falsifiable cruxes span margins, conversion, concentration, credit downgrade, multicloud halo, and power delivery — broader than run 6's five.

## Data Quality Findings

### Critical

None at the level of run 5/8 (year-stale board). Q4 numbers are present in agreements, cruxes, and at least one bull claim.

### High

- **`[High]` Q3 and Q4 claims coexist without reconciliation** (`claims`): Bull claim #2 cites **$553B RPO (Q3)** at high confidence while claim #12 and all agreements/cruxes use **$638B (Q4)**. Bear OpenAI concentration claim divides **$300B / $553B** while crux #3 uses **$638B** base. A downstream agent could pick the wrong RPO depending on which claim it reads.

- **`[High]` BYOH mislabeled** (bull claim #4, business model): "Bring Your Own **Hadoop**" — should be Bring Your Own **Hardware**. The concept is correct; the expansion corrupts auditability and suggests sloppy source reading.

- **`[High]` FY2027 growth math wrong** (bull claim #3): "~40%+ total revenue growth from FY25's $57.4B" to $90B. Correct: **~34%** from FY2026 actual **$67.4B**; ~57% from FY25 TTM if using stale workspace base. Overstates near-term growth from the relevant comparator.

- **`[High]` No official Q4 FY2026 source in pack** (`sources`): Q4 data appears via Futurum commentary (#5) and embedded in claims, not Oracle investor release or CNBC Q4 article. Weaker provenance than runs 3/6 for the catalyst document.

### Medium

- **`[Medium]` Lease obligations ~$135B** (bear claim #7, narrative_map bear text): Presented as additive to $134.6B debt (~$250B total obligations). Directionally plausible for hyperscale lessor model but not reconciled to a specific filing line; could double-count or mis-classify operating vs finance leases.

- **`[Medium]` Mixed-period FCF claim** (bear claim #8): Correctly cites -$23.7B FY26 but also -$13.2B "Q3 FY26" in one sentence — conflates partial-period and full-year without clear labels.

- **`[Medium]` No Q4 workspace ingestion gap** (`data_gaps`): Two company-specific narrative gaps logged (good) but no `narrative_q4_fy2026_workspace_ingestion` flag like run 3.

- **`[Medium]` Database moat claim unqualified** (bull claim #14): "Wide moat" without Fortune 100 penetration vs revenue-share distinction — run 3's 80% error is avoided but moat strength may be overstated.

- **`[Medium]` AI layoffs / code-gen productivity** (bear claim #11): Interesting thread but thin sourcing at medium confidence.

### Low

- **`[Low]` Init workspace unchanged:** FY2025 TTM fundamentals, no price/market cap, observations through Q3 FY2026.

- **`[Low]` Only 12 agent rounds** — efficient (27 tool calls) but less exhaustive than 27-round DeepSeek runs.

- **`[Low]` `web_search_requests: 0`** — discovery path not observable.

## Product Readiness

**Init workspace:** Partial pass (unchanged). Agent partially grounded debt claim to Q3 observations ($9.9B + $124.7B).

**Init + narrative (run 9):** Partial pass. Usable for scenario work on AI-infrastructure debate if an agent reconciles Q3/Q4 claim conflicts first. Richest secondary-theme coverage in the shootout after run 3; cleaner Q4 headline treatment than run 7; far more complete than runs 5/8.

**Gaps for downstream work:**

- Claim hygiene pass needed (retire $553B RPO claim #2 or mark superseded)
- Pending sections correctly empty
- Missing: Abilene cancellation, $40B financing detail, one-time Ampere/Bloom EPS, securities litigation, explicit $75B BYOH dollar figure in claims

## Web Validation

| Field | Run 9 DB / Narrative | External (Jun 2026) | Source | Status |
|---|---|---|---|---|
| RPO (agreements/crux) | $638B | $638B (+363% YoY) | Q4 press release | **Confirmed** |
| RPO (orphan claim #2) | $553B (Q3) | Superseded by Q4 | Q3 vs Q4 | **Stale — should retire** |
| Q4 revenue | $19.18B (+21%) | $19.2B | Official release | **Confirmed** |
| Q4 OCI / IaaS | $5.79B (+93%) | $5.8B (+93%) | Official release | **Confirmed** |
| FY2026 FCF | -$23.7B | -$23.7B | Official release | **Confirmed** |
| FY2027 capex | ~$70B | ~$70B net | CNBC, official | **Confirmed** |
| FY2027 revenue guide | ~$90B | $90B | Official release | **Confirmed**; growth rate mislabeled |
| GPU utilization | 97.5% (Q4) | 97.5% | Earnings call / Futurum | **Confirmed** |
| BYOH / prepaid GPU | "Hadoop" label | BYOH = bring your own **hardware**; $75B cumulative | Q4 release | **Theme confirmed; label wrong** |
| OpenAI ~$300B | Cited vs $553B or $638B | WSJ-reported deal | Secondary | **Confirmed**; denominator inconsistent |
| S&P BBB negative | Cited | Plausible; verify exact date | Credit agencies | **Plausible** |
| Stock $346 → ~$185 | Cited | ~50% off Sep 2025 peak | Market data | **Confirmed** (approximate) |
| Q3 OCI $4.9B (+84%) | Claim #1 | Matches Q3 filing | Q3 8-K | **Confirmed** (but not catalyst) |
| Total debt Feb 2026 | $134.6B | Matches workspace Q3 obs | Observations | **Confirmed** for Q3 period |

## Big Ideas Missing From The Narrative Board

1. **Abilene datacenter expansion cancellation** (run 6 captured).
2. **$40B FY2027 financing plan** (mandatory convert, $20B ATM) — capex cited, financing structure thin.
3. **$75B prepaid/BYOH dollar figure** in claims — theme present, number absent.
4. **One-time Ampere/Bloom EPS adjustments** for FY2027 comparables.
5. **Morningstar moat downgrade** (Wide → Narrow) — source in pack but downgrade not claimed.
6. **Michael Burry short** — absent.
7. **Securities litigation** — absent.
8. **RPO 12-month conversion (~12% / ~$77B)** — not in claims despite crux on conversion.
9. **Official Q4 press release** — not in source pack.

## Judgment On `z-ai/glm-5.1` vs Siblings

### Strengths

- **Q4 capture with breadth:** Reached $638B, Q4 revenue/OCI, FCF, and capex while also retaining useful Q3 trajectory data.
- **Best secondary-theme coverage** in the mid-tier: credit rating, leases, Cerner, nuclear regulatory risk, GPU utilization, drawdown levels.
- **Strong custom gaps** — OpenAI RPO disaggregation and OCI mix are actionable research requests.
- **Efficient:** 12 rounds, ~$0.14, ~168s — more output than gemini at 27 rounds.
- **11/17 metric-tagged claims** with workspace-aligned values (debt, OCI Q3, RPO Q3).
- **Six cruxes** — tied with minimax, more than DeepSeek pro.

### Weaknesses

- **Claim hygiene failure:** Did not retire or supersede Q3 claims after adding Q4 claim #12 — same class of bug as run 3's stale FY2027 growth claim.
- **BYOH typo** undermines trust in technical literacy.
- **No official Q4 source** — relies on Futurum secondary for catalyst quarter.
- **Growth math error** on FY2027 guide.
- **Fewer rounds** than top DeepSeek runs — may explain missing financing/litigation threads run 3 had.

## Recommendations

1. **Finalize claim reconciliation pass** — auto-flag or retire claims superseded by newer quarter captures (claim #2 vs #12).
2. **Require official catalyst-quarter source** when earnings date precedes run date.
3. **Validate acronym expansions** in claims (BYOH ≠ Hadoop).
4. **Re-ingest Q4 FY2026** — shared init constraint; glm partially compensated but left Q3 orphans.
5. **Model selection:** `glm-5.1` is a credible **second-pass analyst** (credit, nuclear, Cerner, lease structure) after a flash/pro run establishes Q4-clean claims; risky as sole researcher without hygiene gates.

## Summary

`z-ai/glm-5.1` on run 9 delivers a **partial pass**: the most theme-diverse mid-cost board in the shootout, Q4-aware in agreements and cruxes, but marred by unreconciled Q3 claims and a BYOH typo. Ranks **third** behind runs 3 and 6 — ahead of minimax (Q3-stale), mimo (FY2025-stale), and gemini (hollow FY2025 stub). Fix claim reconciliation and source freshness, and glm could close the gap with DeepSeek flash on the next refreshed workspace.
