# ADR 02: Canonical Metric Tiering and Strategic Tagging

**Status:** Accepted  
**Date:** 2026-06-09  
**Deciders:** Product / pipeline architecture  
**Related:** [01-pipeline-plan.md](../01-pipeline-plan.md) (Worker Lanes 2, 4, 5, 6), [concept_catalog.rs](../../src/services/concept_catalog.rs), QA reports ORCL/MSFT (2026-06-08)

---

## Context

Worker Lane 2 currently seeds **nine** product-level canonical metrics in `canonical_metric_definitions`:

| Key | Label |
|-----|-------|
| `revenue` | Revenue |
| `net_income` | Net income |
| `gross_profit` | Gross profit |
| `operating_income` | Operating income |
| `shares_outstanding` | Diluted shares |
| `eps` | Diluted EPS |
| `cash` | Cash and equivalents |
| `debt_current` | Current debt |
| `debt_noncurrent` | Noncurrent debt |

The Fundamental Catalog Manager agent must emit a decision for **every** row. Promotion into `canonical_metric_mappings` requires a `direct_mapping` with high or medium confidence. Downstream derivation, headline fundamentals, and scenario scaffolding treat promoted canonical metrics as durable inputs.

Empirical QA and agent review runs (ORCL, MSFT, and related fixture work) show a split in XBRL reliability:

- **Tier A — near-universal for US operating filers:** `revenue`, `net_income`, `eps`. Tags exist and align with product intent in the vast majority of cases. Failures are edge cases (banks, pre-revenue, foreign structures), not the norm.
- **Tier B — usually available but semantically fragile:** `operating_income`, `cash`. Concepts are common; risk is definition choice, period shape (quarter vs YTD), or restricted-cash variants.
- **Tier C — often missing, stale, or ambiguous:** `gross_profit` (especially software/cloud filers that stop tagging consolidated gross profit), `shares_outstanding` (weighted-average diluted vs point-in-time outstanding — different economic meaning), `debt_current` / `debt_noncurrent` (fragmented concept families, maturity-schedule traps, lease accounting, filer-specific naming; MSFT heuristics mapped stale debt concepts while fresh tags existed).

Treating all nine as **hard requirements** creates false completeness: init and agents appear to succeed when optional metrics are mapped to stale concepts, while Worker Lanes 4–6 inherit bad or absent inputs without an explicit gap signal.

At the same time, the pipeline plan (Worker Lanes 4–6) depends on **more than three numbers**. Crux triage, financial mechanics experiments, and scenario construction need margins, balance-sheet risk, cash flow, backlog, capex, dilution, and company-specific mechanics. Those should be captured **early** when quality is high — but not conflated with guaranteed core fundamentals.

---

## Decision

### 1. Hard-required canonical metrics (Tier A)

Only three metrics are **required** for a workspace to be considered canonically mapped at the core layer:

| `canonical_key` | Requirement |
|-----------------|-------------|
| `revenue` | Must resolve to a fresh, company-appropriate total revenue concept or be explicitly flagged unavailable with rationale. |
| `net_income` | Must resolve to consolidated GAAP net income (`NetIncomeLoss` or equivalent) or be explicitly unavailable. |
| `eps` | Must resolve to diluted EPS (`EarningsPerShareDiluted` or equivalent) or be explicitly unavailable. |

**Quality bar:** Lane 2 quality gates for “core fundamentals traceable to SEC concepts” apply strictly to Tier A. Missing Tier A after agent + fallback is a **blocking data gap**, not a silent omission.

### 2. Downgraded canonical metrics (Tier B — supplementary)

The following remain in `canonical_metric_definitions` but are **supplementary**, not required for core completeness:

| `canonical_key` | Role |
|-----------------|------|
| `operating_income` | Useful P&L subtotal when tagged; not assumed present. |
| `gross_profit` | Best-effort; expect `unavailable` / `review_required` for many software and services filers. |
| `shares_outstanding` | Best-effort; distinguish diluted weighted average (EPS bridge) from point-in-time outstanding (market cap bridge). |
| `cash` | Best-effort balance-sheet liquidity; definition variants must be documented. |
| `debt_current` | Best-effort; high filer variance; freshness and balance-vs-flow validation mandatory. |
| `debt_noncurrent` | Best-effort; same constraints as current debt. |

**Behavior:**

- Agent/heuristic **may** promote Tier B mappings when confidence is high and `latest_period_end` is fresh.
- **Must not** fail init or block Lane 3+ solely because Tier B is unmapped.
- `unavailable`, `review_required`, and low-confidence outcomes are **expected and acceptable** for Tier B.
- Heuristics must not prefer stale seed aliases over fresh catalog candidates for Tier B (debt and gross profit in particular).

### 3. Strategic tagging (supporting metrics and catalog — not Tier A)

Worker Lanes 4–6 need rich concept surfaces beyond three headline metrics. We **will** continue early tagging of high-value concepts that **generally align** with known patterns but are not guaranteed:

- **Supporting metric selections** (Lane 2 writes, Lane 4 consumes): OCF, capex, FCF components, RPO/backlog, segment-adjacent revenue, lease liabilities, interest expense, working-capital drivers, buyback/dilution series, etc.
- **Concept catalog metadata** already produced at ingest: `series_usability`, `dominant_period_shape`, `narrative_tags`, plot readiness.
- **Lane 4 triage:** promote concepts into crux candidates when they confirm, complicate, or contradict narratives — with rationale, not because they appear on a fixed checklist.
- **Lane 5 experiments:** SQLite calculations over tagged concepts; results promoted, rejected, or left unresolved per experiment gates.
- **Lane 6 scenarios:** period assumptions may draw from supporting metrics and experiment outputs; scenarios must tolerate absent Tier B canonicals (e.g. gross margin path omitted or explicitly gated on a crux).

Strategic tags are **opportunistic assets**: high quality when present, never assumed universal. Sparse, stale, or period-ambiguous concepts get quality flags before projection (per Lane 4 gates).

### 4. Semantic split (future-friendly)

`shares_outstanding` conflates two product needs. This ADR does not mandate an immediate schema change, but new work should treat:

- **Diluted weighted-average shares** — EPS and per-share bridges (income statement period).
- **Common shares outstanding (point-in-time)** — market cap and per-share valuation (balance sheet instant).

as distinct supporting or Tier B targets when we extend definitions.

---

## Consequences

### Positive

- Init and agent review stop optimizing for false 9/9 completeness.
- Tier A failures surface as real gaps; Tier B gaps surface as labeled optional absences.
- Worker Lanes 4–6 can rely on a **stable trio** for baseline scenario math while mining the catalog for company-specific mechanics.
- Early supporting-metric tagging preserves downstream value described in Lanes 4–6 without over-promising XBRL coverage.

### Negative / tradeoffs

- Headline `fundamentals` and starter bundles may expose fewer rows until Tier B is explicitly promoted — UI and agents must read `data_gaps` and supporting metrics, not assume margins or debt in every workspace.
- Scenario templates that assume gross margin or total debt paths need crux-conditional branches or explicit “metric unavailable” handling.
- Fixture QA must distinguish **Tier A assertions** (must pass) from **Tier B assertions** (best-effort or ticker-specific).

### Implementation follow-ups (non-blocking for this ADR)

1. Add `tier` or `required_for_core` column to `canonical_metric_definitions` (`required` | `supplementary`).
2. Update agent preamble and golden path: Tier A must resolve; Tier B bail early when stale (e.g. gross profit per ORCL pattern).
3. Adjust promotion and init quality gates: blocking only on Tier A.
4. Extend fixture annotations (`expected: required | supplementary | supporting`) per [06-07-003-fixture-based-automated-canon-metric-checks.md](../qa/06-07-003-fixture-based-automated-canon-metric-checks.md).
5. Align [01-pipeline-plan.md](../01-pipeline-plan.md) Lane 2 wording: “core fundamentals” = Tier A; other listed measures = supplementary or supporting.

---

## References

- ORCL agent run (2026-06-09): 8/8 promoted mappings matched heuristics; `gross_profit` correctly `review_required` (stale `GrossProfit`, no cost components).
- [06-08-001-data-quality-report.md](../qa/06-08-001-data-quality-report.md) — ORCL gross profit excluded from bundle; debt/cash period mismatch in headlines.
- [06-08-002-data-quality-microsoft.md](../qa/06-08-002-data-quality-microsoft.md) — MSFT P&L canonicals strong; debt canonicals mapped to deprecated concepts.
- Pipeline Worker Lanes 4–6: triage, financial mechanics experiments, and scenario construction consume supporting metrics and catalog tags, not only canonical mappings.
