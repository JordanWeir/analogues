# ADR 03: Concept Freshness Tiers for Experiment Filtering

**Status:** Draft  
**Date:** 2026-06-09  
**Deciders:** Product / pipeline architecture  
**Related:** [02-canonical-metric-tiering.md](./02-canonical-metric-tiering.md), [01-pipeline-plan.md](../01-pipeline-plan.md) (Worker Lanes 4–5), [concept_catalog.rs](../../src/services/concept_catalog.rs)

---

## Context

`concept_catalog_entries` already carries several classification fields:

| Field | What it measures today | Freshness? |
|-------|------------------------|------------|
| `series_usability` | Fact count buckets (`long_history`, `medium_history`, `sparse`, `event_point`) | No — `stale` is referenced in agent prompts but **never assigned** in code |
| `plot_readiness` | Dominant period-shape consistency | No — 94 ORCL concepts were `plot_ready` yet >1 year behind workspace anchor |
| `dominant_period_shape` | `instant`, `quarter`, `annual`, `ytd`, etc. | Orthogonal — required for SQL experiments but not recency |
| `latest_period_end` / `latest_filed_at` | Raw recency signals | Present but agents/heuristics must infer usefulness ad hoc |
| `narrative_tags` | Thematic relevance (backlog, debt, capex, …) | No — 107 stale ORCL concepts still carried narrative tags |

Empirical analysis of `reports/stock-narrative-research/ORCL-2026-06-08-4/run.sqlite` (516 catalog rows):

- **43%** of concepts had no fact within 1 year of the workspace anchor period.
- **38%** were >4 years behind anchor.
- **29%** of `long_history` concepts were stale — high fact count does not imply usable series.
- **69** concepts had `Deprecated` in the label; **98.6%** of those were stale.
- Heuristic canonical mapping promoted **`GrossProfit` (2018)** as `high` confidence while eight other metrics were fresh — freshness is the missing gate.

Worker Lanes 4–5 need a fast, deterministic filter: “which concepts are worth SQLite trend experiments?” without re-deriving staleness from `sec_raw_facts` every time.

---

## Problem statement

We lack a **freshness / experiment-usability** dimension that answers:

> Can a financial mechanics agent treat this concept as a time series input for historical investigation or lightweight projection?

This is distinct from:

- **Canonical tiering (ADR 02)** — whether a concept maps to a product metric.
- **Narrative relevance (Lane 4)** — whether a concept matters to a crux (stale backlog can still be narratively important as a one-off snapshot).

---

## Options considered

### Option A — Binary `fresh` flag

Tag `fresh = true` if the concept has **any** observation with `period_end` in the last 3 calendar years (relative to workspace anchor or wall clock).

| Pros | Cons |
|------|------|
| Simple to implement and query | Too coarse for Lane 5 — one 2024 datapoint qualifies |
| Easy agent filter | Collapses “recent snapshot” vs “plottable history” |
| | Still allows superseded concepts (`Revenues` at FY2025 while ASC 606 tag is fresher) |
| | Does not help rank 150+ “fresh enough” concepts |

**Assessment:** Useful as a **minimum bar** inside a richer scheme, not sufficient alone.

### Option B — Three-tier freshness code (`bad` \| `ok` \| `great`)

Proposed semantics:

| Tier | Rule (as proposed) |
|------|---------------------|
| **bad** | No data in the last 3 years |
| **ok** | Something in the last 3 years |
| **great** | `instant` **or** continuous series for at least 3 years |

| Pros | Cons |
|------|------|
| Matches agent mental model (ignore / maybe / default pool) | “Something in 3 years” still allows single-point series |
| Separates experiment-grade series from legacy catalog noise | “Instant OR continuous” needs precise definitions per `dominant_period_shape` |
| Aligns with Lane 5 quality gates (sparse/event labeling) | Merging into one code hides period-shape constraints |

**Assessment:** **Preferred direction**, with tightened tier rules and companion fields (below).

### Option C — Keep inferring from existing tags

Continue relying on `series_usability`, `plot_readiness`, and agent SQL.

**Rejected.** ORCL proved existing tags are insufficient; agents waste rounds rediscovering staleness; heuristics map high-confidence dead concepts.

---

## Recommendation (draft decision)

Adopt **Option B** as `freshness_tier` on `concept_catalog_entries`, materialized deterministically at catalog build time, with explicit rules per period shape and supporting metadata columns for SQL filters.

Do **not** overload `series_usability` — keep history-length classification separate from recency/experiment grade.

### Tier definitions (refined)

Anchor date: **`workspace_fresh_period_end`** — max `latest_period_end` across catalog entries, excluding sentinel / far-future dates (e.g. `> anchor + 1 year`). Optionally also compute `workspace_fresh_filed_at`.

Lookback window: **`FRESHNESS_LOOKBACK_YEARS = 3`** (constant; revisit via QA fixtures).

**Pre-check caps (apply before tier assignment):**

- `label` contains `Deprecated` (case-insensitive) → cap at **`ok`** maximum; often **`bad`** if also outside lookback.
- `latest_period_end` is null → **`bad`** (already `event_point`-like).
- Sentinel `period_end` beyond sanity threshold → exclude from anchor computation; concept likely **`bad`**.

**bad**

- No `period_end` within lookback window of anchor, **or**
- Fewer than **`MIN_OBSERVATIONS_OK = 2`** distinct period ends in lookback window (prevents single-point “ok”).

**ok**

- At least **`MIN_OBSERVATIONS_OK`** distinct `period_end` values in lookback window, **or**
- Exactly **1** recent observation but `latest_period_end` within **`RECENT_SNAPSHOT_DAYS = 95`** of anchor (recent point-in-time / event disclosure — usable for snapshot comparisons, not trends).

**great**

Must satisfy **ok** plus shape-specific continuity (distinct period ends spanning ≥ lookback years):

| `dominant_period_shape` | Great criteria (draft) |
|-------------------------|-------------------------|
| **instant** (balance sheet) | ≥ **8** instant `period_end` values spanning ≥ 3 years within lookback (≈ quarterly balance sheet cadence) |
| **quarter** | ≥ **8** quarterly duration facts spanning ≥ 3 years; YTD-dominant series capped at **ok** unless YTD share < 50% |
| **annual** | ≥ **3** annual duration facts spanning ≥ 3 years |
| **ytd** | **ok** at best by default; great only if normalized quarterly series exists (future work) |
| **mixed / unknown** | **ok** at best unless shape resolves cleanly |

Instant and flow series use different “continuity” notions — **do not** treat a single recent instant as great.

### Companion fields (adjacent idea — recommended)

Tier alone is not enough for experiment SQL. Materialize alongside `freshness_tier`:

| Field | Purpose |
|-------|---------|
| `freshness_tier` | `bad` \| `ok` \| `great` |
| `observations_in_lookback` | Count of distinct period ends in window — SQL `WHERE observations_in_lookback >= 8` |
| `years_spanned_in_lookback` | Float — quick span check |
| `days_behind_anchor` | Integer — workspace-relative staleness (heuristic/agent canonical picks) |
| `is_deprecated_label` | Boolean — cheap dirty signal |

Optional later: `superseded_by_concept` when a fresher same-unit sibling exists in catalog (e.g. `Revenues` vs `RevenueFromContractWithCustomer…`).

### Agent / lane usage

| Lane | Filter guidance |
|------|-----------------|
| **Lane 2 canonical mapping** | Tier A metrics: require `days_behind_anchor` ≤ 380; reject **`bad`** for promotion. Tier B: **`bad`** → `unavailable`; **`ok`** → snapshot only; **`great`** → preferred for mapping. |
| **Lane 4 triage** | Narrative search may include **`ok`** stale-adjacent concepts with explicit “snapshot only” flag; do not promote to crux mechanics without freshness note. |
| **Lane 5 experiments** | Default pool: `freshness_tier = 'great'` + relevant `dominant_period_shape`; exploratory: include **`ok`** with `observations_in_lookback < MIN_GREAT` logged as low-confidence. |
| **Lane 6 scenarios** | Scenario drivers should prefer **`great`** series; **`ok`** for level inputs only. |

Golden path SQL recipes should replace ineffective `series_usability NOT IN ('stale', …)` with `freshness_tier IN ('ok', 'great')` or `'great'` for experiment templates.

---

## Adjacent ideas considered (not in initial scope)

1. **Workspace-relative vs wall-clock lookback** — Prefer anchor-relative `days_behind_anchor` for filers with delayed filings; keep calendar lookback for cross-company fixture QA.

2. **Filing recency (`latest_filed_at`)** — Secondary signal; period_end remains primary for financial math. A concept filed recently but with old period_end is still **`bad`** for forward-looking trends.

3. **Separate `experiment_usability` from canonical freshness** — A stale RPO snapshot may be **`ok`** for “latest backlog level” experiments but never **`great`**. If tier semantics blur, split into `freshness_tier` (recency) and `series_continuity_tier` (plottability).

4. **Negative signals beyond Deprecated** — Maturity schedules, pro forma acquisition EPS, `ProceedsFrom*` / `RepaymentsOf*` flows: narrative_tags + name heuristics cap at **`ok`** or force **`bad`** for balance-sheet canonical contexts (overlap with debt scoring).

5. **Observation-layer parity** — Recompute or validate tier against `sec_raw_facts` periodically; catalog aggregates can drift if facts are merged/restated.

6. **UI / report vocabulary** — Use “experiment-grade” / “snapshot-only” / “legacy” in user-facing copy instead of Bad/Ok/Great.

7. **Configurable thresholds per industry** — Banks, REITs, recent IPOs may need fixture-driven overrides; ship one global constant first, tune via [fixture annotations](../qa/06-07-003-fixture-based-automated-canon-metric-checks.md).

---

## Consequences

### Positive

- Lane 5 agents get a deterministic shortlist (`freshness_tier = 'great'`) instead of 50+ ad hoc SQL probes.
- Heuristics stop preferring `long_history` stale aliases (ORCL `GrossProfit`).
- Clear product language for “useful vs low value” on the **time-series** dimension, complementing ADR 02’s canonical **required vs supplementary** split.

### Negative / tradeoffs

- Schema migration + backfill on existing workspaces.
- Tier rules require tuning on MSFT, JPM, REIT, and sparse filers — expect iteration on YTD and mixed-shape edge cases.
- **`ok`** bucket may remain large; agents still need `dominant_period_shape` and narrative filters to pick experiment topics.

### Implementation sketch (non-binding)

1. Add columns to `concept_catalog_entries` in [schema.rs](../../src/workspace/schema.rs).
2. Implement `classify_freshness_tier(entry, workspace_anchor, all_entries)` in [concept_catalog.rs](../../src/services/concept_catalog.rs) at materialize time.
3. Update [review_workspace.rs](../../src/services/review_workspace.rs) golden path filters.
4. Add ORCL fixture assertions: ~38% `bad`, promoted canonicals none `bad`, `GrossProfit` is `bad`.
5. Document tier semantics in agent preambles for Lane 5 query library templates.

---

## Open questions

1. Should **`ok`** require ≥2 observations always, or allow 1 if within one quarter of anchor (recent snapshot use case)?
2. Is **`great`** threshold (8 quarterly points / 3 annual) too strict for sparse filers or recent IPOs?
3. Do we cap Deprecated labels to **`bad`** unconditionally, or **`ok`** when one recent restatement exists?
4. Single column `freshness_tier` vs split `recency_tier` + `continuity_tier`?

---

## References

- ORCL sqlite analysis (2026-06-09): 516 concepts; 43% >1yr stale; 0 rows with `series_usability = 'stale'`; 45 `long_history` + `plot_ready` + stale.
- [ADR 02: Canonical Metric Tiering](./02-canonical-metric-tiering.md) — required vs supplementary metrics; freshness gates promotion differently by tier.
