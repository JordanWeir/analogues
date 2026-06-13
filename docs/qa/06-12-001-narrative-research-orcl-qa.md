# Narrative Research QA — ORCL (2026-06-13)

## Scope

(RUNNING "deepseek/deepseek-v4-flash")

QA inspection focused on the **narratives section** and `@src/agents/narrative_researcher/` tooling output for one Oracle run:

| Run | SQLite path | Focus |
|-----|-------------|-------|
| 1 | `reports/stock-narrative-research/ORCL-2026-06-13-3/run.sqlite` | Narrative map, claims, sources, sections, cruxes |

Web validation performed against official filings/press releases, CNBC, Morningstar, and secondary financial media (June 2026).

## Verdict

**Partial pass.** The narrative researcher produced a coherent, timely ORCL bull/bear map centered on the June 10, 2026 Q4 FY2026 inflection. Core financial claims check out against official and press sources. The work is strong on the AI-backlog-vs-capex debate, but it overstates database dominance, leaves a stale bull claim on FY2027 growth, and misses several material narrative threads (GPU prepayments, one-time EPS distortions, Cerner/TikTok, credit-market stress, litigation).

## What The Narratives Section Captures Well

The board is structurally complete and passed finalization:

| Artifact | Count | Status |
|---|---|---|
| Sources | 15 | Mix of official press release, transcript, CNBC, Morningstar, bull/bear commentary |
| Claims | 36 | Bull 16 / Bear 19 / Neutral 1 |
| Narrative sides | 5 | dominant, bull, bear, consensus, counter_narrative |
| Agreements | 10 | Well-framed shared ground |
| Cruxes | 8 | Backlog conversion, ROI, margins, moat, concentration, FCF, financing, share gains |
| Sections | orientation, business_model, why_now | Populated; downstream sections correctly left pending |

The **dominant narrative** is accurate: post-Q4 shift from backlog euphoria to balance-sheet skepticism. The agent correctly framed the central question — can $638B RPO convert without destroying value through debt, dilution, and margin compression?

**Crux quality is good.** The eight cruxes are falsifiable and map to what the market is actually debating. Orientation JSON (dominant question, base-rate warning, 12–24 month horizon) is thoughtful.

**Provenance discipline is decent:** 11 high-confidence claims tied to official sources; 25 marked inference. The agent logged an open gap for Q4 FY2026 not yet in `fundamental_observations` — good self-awareness given stale workspace fundamentals (still FY2025: revenue $57.4B, debt $134.6B, EPS $4.34).

**Tooling run succeeded:** 27 agent rounds, 43 tool calls, `finalize_narrative_research` passed validation gates in `validate.rs`.

## Data Quality Findings

### Critical

None. No headline numbers are materially wrong enough to corrupt downstream scenario work.

### High

- **`[High]` "~80%+ enterprise DB market share" is misstated** (consensus, agreement #2, bull claim #27): Web sources distinguish **Fortune 100 penetration** (~80% use Oracle) from **revenue market share** (~18% per industry reports; Gartner ranks AWS #1, Oracle #3). Treating penetration as share overstates the moat and weakens the database-disruption crux.

- **`[High]` Stale bull claim on FY2027 growth** (claim #3): Still says "~20% FY2027 revenue growth" from the Q3 FY2025 transcript. Q4 FY2026 confirmed **~$90B revenue (~34% growth)**. The narrative sides were updated; this orphaned claim was not.

- **`[High]` EPS growth framing understates management's adjusted story** (bear narrative, claim #36): Board says FY2027 non-GAAP EPS of $8.05 is "5–6% growth" vs $7.63. Oracle's press release says **18% growth vs $6.83** after stripping one-time Ampere/Bloom gains from FY2026. The margin-compression point still holds, but the headline EPS growth rate is wrong without that adjustment.

### Medium

- **`[Medium]` Debt figures lack a single reconciled view:** Claims cite $95B (Burry, Jan 2026), workspace fundamentals $134.6B (FY2025), narratives $162B+ (post-Q4). All can be true at different dates/definitions, but the board doesn't explain the bridge — a downstream agent could pick the wrong number.

- **`[Medium]` RPO quality under-explored:** Q4 disclosure that **$75B of the $638B RPO is prepaid/customer-supplied GPU hardware** is barely surfaced. That materially affects how "real" the backlog is and how much capex Oracle actually bears — central to both bull and bear cases.

- **`[Medium]` 17:1 committed-revenue-to-capex ratio** (bull claim #24, counter_narrative): Sourced from an investor letter (SVCP), not company filings. Reasonable heuristic, but presented too confidently relative to evidence tier.

- **`[Medium]` No claims linked to workspace metrics:** 0/36 claims use the `metric` column; narratives aren't grounded in `concept_catalog_entries` or `fundamental_observations` despite `workspace_sql` being available. The agent noted the ingestion gap but didn't bridge narratives to persisted fundamentals.

- **`[Medium]` Fundamentals snapshot in agent prompt is stale** (`agent.rs` hardcodes FY2025-era keys). The TODO in `fundamentals_summary` is real — the agent had to web-research around a workspace that hadn't caught up to the catalyst it was analyzing.

### Low

- **`[Low]` Source-type hygiene:** Q4 press release is "Official company source" but not tagged "Filing." Minor provenance nit.

- **`[Low]` `web_search_requests: 0` in worker telemetry** despite 15 sourced URLs — likely captured from model knowledge or indirect fetches, not counted web-search tool usage. Hard to audit discovery path.

- **`[Low]` `narrative_map` section duplicates agreements/cruxes as JSON blob (9,733 chars) alongside normalized `narrative_map_items` rows.

## Product Readiness (Narratives Specifically)

**Could a later agent build scenario work from this?** Mostly yes for the AI-infrastructure debate — agreements and cruxes are usable inputs for scenario assumptions and watch items.

**Gaps for downstream work:**

- No `crux_candidates` promoted yet (expected; that's `identify_crux_candidates` lane)
- No metric hooks from claims → catalog/fundamentals
- Pending sections (`financial_snapshot`, `watch_items`, `scenario_assumptions`) correctly empty
- Narratives ahead of ingested fundamentals — agent compensated via web sources but workspace SQL grounding is weak

## Web Validation

| Field | DB / Narrative | External | Source | Status |
|---|---|---|---|---|
| RPO | $638B | $638B (+363% YoY) | Oracle Q4 press release, CNBC | **Confirmed** |
| FY2026 revenue | $67.4B | $67.4B (+17%) | Official release | **Confirmed** |
| Q4 IaaS growth | 93% | 93% to $5.8B | CNBC, DCD | **Confirmed** |
| FY2026 FCF | -$23.7B | -$23.7B | CNBC, ERP Today | **Confirmed** |
| FY2027 net capex outlay | ~$70B | ~$70B (reported $90–95B incl. prepayments) | CNBC, DCD | **Confirmed** (net); reported capex higher |
| FY2027 financing | $40B debt+equity, $20B ATM | Same | CNBC, Reuters | **Confirmed** |
| FY2027 revenue guide | ~$90B (+34%) | $90B confirmed | Official release | **Confirmed** |
| FY2027 non-GAAP EPS | $8.05 | $8.05 (18% vs adj. FY2026) | Official release | **Confirmed**; growth rate mislabeled in bear text |
| OpenAI >50% of RPO | >50% / 57% | BofA >50% cited by CNBC | CNBC Jun 2026 | **Confirmed** (analyst estimate) |
| Stock reaction | ~$184, down ~50% from peak | Closed ~$184.10 Jun 11; ~47% off Sep peak | 24/7 Wall St., CNBC | **Confirmed** |
| Morningstar moat downgrade | Wide → Narrow | Wide → Narrow, FV $215 | Morningstar | **Confirmed** |
| Michael Burry puts | Disclosed | Jan 2026 Substack disclosure | Fortune, Yahoo | **Confirmed** |
| ~80% DB market share | Stated repeatedly | ~18% revenue share; ~80% Fortune 100 use | Industry reports | **Denied as market share** |
| FY2027 ~20% revenue growth | Claim #3 | 34% to $90B | Q4 FY2026 guidance | **Stale / wrong** |

## Big Ideas Missing From The Narrative Board

These are live in market commentary but absent or thin on the board:

1. **GPU prepayment / bring-your-own-hardware structure ($75B)** — Changes how to read RPO, capex burden, and bull "17:1" math. Oracle is partly an infrastructure operator, not pure capital owner.

2. **One-time investment gains (Ampere, Bloom warrants)** — Distorts FY2026 EPS; without adjustment the "EPS growth collapse" narrative is overstated (though margin mix shift is real).

3. **OpenAI credit / IPO overhang** — Motley Fool and CNBC analysts flag OpenAI profitability and IPO timing as backlog risk. Board mentions concentration but not counterparty solvency.

4. **Credit-market stress** — CDS at multi-year highs, Morgan Stanley ~$100B+ funding gap estimates, oversubscribed but expensive debt raises. Relevant to crux #7 (capital markets access).

5. **Cerner / Oracle Health** — $28B acquisition, VA contract delays, clinician-burnout rebuild, data-breach litigation. Material non-AI segment and execution drag.

6. **TikTok US JV (15% stake, cloud host)** — Jan 2026 resolution; large durable cloud customer diversifying away from pure OpenAI dependence.

7. **Other AI customers (Meta, xAI)** — Morningstar cites them; board treats AI demand as OpenAI-centric.

8. **Securities litigation** — Class actions alleging misstatements on backlog-to-revenue conversion speed (Barrows v. Oracle). Fits bear narrative on deployment gap.

9. **Dividend sustainability** — $0.50/qtr declared while FCF deeply negative; not discussed.

10. **Long-term FY2030 targets** — Management reconfirmed ~31% revenue CAGR / ~28% EPS CAGR through FY2030 on the Q4 call. Important bull anchor the board omits.

11. **"SaaS apocalypse" / agentic AI** — Ellison's Q3 rhetoric vs mid-teens apps growth deserves a crux on whether Oracle apps are immune or lagging.

12. **Reported vs net capex framing** — New CFO reporting convention ($70B net vs $90–95B reported) affects how scary the capex headline looks.

## Judgment On `src/agents/narrative_researcher/` Tooling

### What works well

- Incremental capture model (reuse board, don't wipe) is the right design for durable research
- Validation gates enforce minimum substance without being trivial
- Five-sided map + agreements/cruxes + orientation is a strong schema for scenario prep
- Research-gap capture connects narrative work back to init pipeline deficiencies
- 27-round run produced genuinely usable content, not placeholder text

### What to improve

1. **`fundamentals_summary`** — Pull recent time series from catalog-selected metrics, not six fixed FY2025 columns. The agent fought stale data on the biggest catalyst day.
2. **Claim hygiene pass** — Finalize should flag claims contradicted by newer captures (claim #3 vs Q4 guidance).
3. **Metric linking** — Encourage `metric` on claims tied to workspace observations; golden path should require at least a few SQL-grounded claims.
4. **Moat/share fact-checking** — Preamble or validation could nudge distinction between penetration vs share.
5. **RPO composition** — Add explicit prompt guidance to decompose RPO (prepaid GPU, recognition schedule, non-traditional components).
6. **Source freshness tiers** — Weight official Q4 release over March Q3 transcript when both exist.
7. **Telemetry** — `web_search_requests: 0` makes QA harder; ensure discovery path is observable.

## Recommendations

1. Re-ingest Q4 FY2026 into `fundamental_observations` (gap already logged) and re-run narrative researcher to reconcile claims #3, debt, and EPS framing.
2. Add a crux on **RPO composition quality** (prepaid GPU vs recurring cloud revenue) and **OpenAI counterparty risk**.
3. Patch consensus/agreement #2 to "dominant enterprise DB vendor; ~80% Fortune 100 penetration, ~high-teens revenue share."
4. Expand bear/bull with **Cerner, TikTok, credit spreads, litigation** as secondary narrative threads.
5. Wire `fundamentals_summary` to catalog manager outputs before the next ORCL run.

## Summary

The narrative researcher did credible work on the **central** ORCL story (Stargate backlog vs capex/financing shock). It reads like a solid first-draft research memo, not a complete picture of what sophisticated investors are debating in June 2026. The tooling architecture is sound; grounding, claim reconciliation, and second-order themes are the main gaps.
