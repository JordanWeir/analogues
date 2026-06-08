# Fixture-Based Automated Canonical Metric Checks

**Status: back burner.** This document describes a practical plan for a 50-stock golden regression corpus. It is not scheduled work. Implement after higher-priority initWorkspace fixes land (debt mapping, period typing, headline alignment, LLM review reliability).

This follows manual QA findings in [`06-06-001-data-quality-report.md`](06-06-001-data-quality-report.md) and scoring work motivated by Oracle debt mis-mapping. The goal is to stop re-discovering the same canonical-mapping and headline-fundamental bugs by hand.

---

## Problem

Manual workspace QA (see `stock-init-workspace-qa` skill) is effective but slow. Each initWorkspace change requires re-opening sqlite files, re-checking mappings, and re-validating headline values against filings.

We need a **repeatable regression suite** that answers:

- Did canonical concept selection regress for company X?
- Are headline fundamentals within tolerance of filing-grounded truth at an explicit `period_end`?
- Did forbidden concepts (e.g. debt maturity schedules) get selected again?

---

## Approach

Build a **golden fixture corpus**: frozen `run.sqlite` inputs plus human-reviewed YAML annotations. Tests load both and assert derived outputs — not whole-database equality.

Do **not** golden entire `sec_raw_facts` (tens of thousands of rows per ticker). Golden **decisions the product must get right**.

---

## Directory layout

```
tests/fixtures/workspace_golden/
  manifest.yaml                 # ticker list, archetypes, freeze metadata
  annotations/
    ORCL.yaml
    MSFT.yaml
    ...
  sqlite/
    ORCL/run.sqlite
    MSFT/run.sqlite
    ...
  README.md                     # regenerate + approve workflow
```

Keep annotations **separate** from sqlite. Sqlite is the artifact under test; YAML is the reviewed truth.

---

## Ticker selection (50 stocks)

Choose tickers to stress different failure modes (from init-workspace QA cross-company guidance):

| Archetype | Count (target) | Examples | Stress |
| --- | --- | --- | --- |
| Cloud / software | ~8 | ORCL, MSFT, CRM | RPO, deferred revenue, notes-payable debt lines |
| Semi / industrial | ~8 | NVDA, CAT, DE | Inventory, capex, segments |
| Bank / insurer | ~8 | JPM, BAC, BRK.B | Non-standard revenue/debt presentation |
| Retail / consumer | ~8 | WMT, COST, NKE | Leases, fiscal calendar quirks |
| Energy / materials | ~8 | XOM, COP | Reserves, production metrics |
| Edge cases | ~10 | Recent IPO, ADR, REIT, sparse filer | Thin SEC coverage |

`manifest.yaml` records per ticker: `archetype`, `cik`, `fiscal_year_end`, `freeze_date`, `schema_version`, `mapping_strategy`, `sqlite_path`, `annotation_path`, `approved_by`, `approved_at`.

---

## Annotation schema (per ticker)

Example `annotations/ORCL.yaml`:

```yaml
ticker: ORCL
as_of_filing: "2026-03-11"
profile:
  exchange: NYSE
  cik: "0001341439"
  currency: USD

canonical_mappings:
  revenue: RevenueFromContractWithCustomerExcludingAssessedTax
  net_income: NetIncomeLoss
  operating_income: OperatingIncomeLoss
  eps: EarningsPerShareDiluted
  cash: CashAndCashEquivalentsAtCarryingValue
  debt_current: NotesPayableCurrent
  debt_noncurrent: LongTermNotesAndLoans
  shares_outstanding: WeightedAverageNumberOfDilutedSharesOutstanding

headlines:
  - metric_key: debt_current
    period_end: "2026-02-28"
    value: 9.887e9
    tolerance_pct: 0.5
    source: "Q3 FY26 10-Q balance sheet"
  - metric_key: debt_noncurrent
    period_end: "2026-02-28"
    value: 124.718e9
    tolerance_pct: 0.5
  - metric_key: total_debt
    period_end: "2026-02-28"
    value: 134.605e9
    tolerance_pct: 1.0
    derived_from: [debt_current, debt_noncurrent]

forbidden_mappings:
  debt_noncurrent:
    - LongTermDebtMaturitiesRepaymentsOfPrincipalInYearThree
    - LongTermDebtMaturitiesRepaymentsOfPrincipalAfterYearFive

quality_expectations:
  min_raw_concepts: 400
  min_observations: 1000
```

Notes:

- Banks and insurers may use `unavailable` or `calculated_from_components` instead of forcing a bad direct revenue/debt map.
- `shares_outstanding` may carry an explicit ambiguity note (WADSO vs basic outstanding).
- Extend later with `interesting_concepts`, `period_type` expectations, and LLM-strategy fixtures.

---

## Generating sqlite fixtures

### Phase 1 (recommended): full frozen workspace

Run `initWorkspace` once per ticker with pinned parameters, copy `run.sqlite` into the fixture tree:

```bash
cargo run -- initWorkspace \
  ticker:ORCL \
  date:2026-06-07 \
  fetch_financials:true \
  mapping_strategy:candidate_scoring

cp reports/stock-narrative-research/ORCL-2026-06-07-N/run.sqlite \
   tests/fixtures/workspace_golden/sqlite/ORCL/run.sqlite
```

Pin in `manifest.yaml`:

- `freeze_date` — when the fixture was generated
- `schema_version` — from `run_metadata`
- `mapping_strategy` — `candidate_scoring` initially; add `llm_reviewed` corpus later if needed

**Pros:** Tests replay exactly what QA inspects; catches persistence and headline-layer bugs.

**Cons:** ~15–25 MB per ticker × 50 ≈ large repo footprint. Mitigate with Git LFS, a compressed tarball, or CI download with checksum verification.

### Phase 2 (optional): slim raw-facts fixture

Store only `sec_raw_facts` + minimal metadata; run mapping logic in tests. Smaller files, but does not catch headline assembly or sqlite persistence issues. Use as a supplement, not a replacement.

---

## Annotation workflow

1. **Generate** sqlite batch (same date, same `schema_version`, same `mapping_strategy`).
2. **Auto-draft** YAML from sqlite + filing cross-check (script queries `canonical_metric_mappings`, `fundamentals`, latest observation per concept).
3. **Human review** against 10-K/10-Q or earnings release; set `approved_by` / `approved_at`.
4. **Commit** sqlite + approved YAML together per ticker.

Optional helper: `cargo run --example draft_fixture_annotation -- ORCL` prints draft YAML from a fixture sqlite path.

---

## Test harness (Rust)

Add integration tests under `tests/fixtures/` (or `tests/workspace_golden/`):

```rust
// Pseudocode — not implemented
for entry in manifest.tickers {
    let db = open_readonly(&entry.sqlite_path);
    let expected = load_yaml(&entry.annotation_path);

    assert_canonical_mappings(&db, &expected.canonical_mappings);
    assert_forbidden_mappings_absent(&db, &expected.forbidden_mappings);
    assert_headlines_within_tolerance(&db, &expected.headlines);
    assert_quality_expectations(&db, &expected.quality_expectations);
}
```

Assertion categories:

| Check | Source tables |
| --- | --- |
| Canonical concept names | `canonical_metric_mappings` |
| Forbidden concepts absent | `canonical_metric_mappings` |
| Headline values + periods | `fundamentals`, optionally `fundamental_observations` |
| Critical quality flags absent | `data_quality_flags` |
| Minimum breadth | `sec_raw_facts`, `fundamental_observations` counts |

Run modes:

- **PR subset (fast):** 5 tickers — ORCL, JPM, WMT, XOM, one sparse/edge filer
- **Nightly:** all 50

---

## What to automate vs hand-annotate

| Field | Auto-draft from sqlite | Human approval required |
| --- | --- | --- |
| Canonical concept names | yes | yes |
| Headline values at `period_end` | yes | yes (vs filing) |
| `forbidden_mappings` | partial (from known bug list) | yes |
| Profile (CIK, exchange) | partial | yes |
| Bank/insurer revenue/debt | no | yes |

---

## Rollout phases (when prioritized)

1. **Bootstrap:** `manifest.yaml` + harness + 5 tickers (ORCL, MSFT, JPM, WMT, NVDA). Assertions: canonical mappings + forbidden concepts only.
2. **Headlines:** Add `fundamentals` / observation tolerance checks; expand to ~15 tickers.
3. **Scale:** Reach 50 tickers; nightly CI job; annotation draft tool.
4. **LLM path:** Separate manifest or fixture flag for `mapping_strategy:llm_reviewed` with recorded model responses (mock or frozen), so LLM regressions are testable without live API calls.

---

## Prerequisites before starting

These should be stable enough that fixtures do not churn weekly:

- [ ] Debt scoring seeds and negative penalties (in progress / landed)
- [ ] Headline period alignment for balance-sheet metrics
- [ ] Period typing on observations (quarter vs YTD vs annual vs instant)
- [ ] Decision on whether PR tests use `candidate_scoring` only or also LLM-reviewed fixtures

---

## Maintenance

- When `schema_version` bumps, regenerate all fixtures and re-approve annotations in one batch.
- When SEC filings update a ticker, regenerate that ticker’s sqlite intentionally; do not let live SEC fetches cause CI flakes.
- Document regeneration in `tests/fixtures/workspace_golden/README.md`.
- Track fixture size; move to LFS or external storage before committing all 50 full sqlites to main.

---

## Success criteria

When implemented, a developer should be able to:

1. Change `concept_catalog.rs` or headline assembly logic
2. Run `cargo test workspace_golden` (or PR subset)
3. See which tickers regressed with ticker, metric, expected concept/value, and actual — without manual sqlite inspection

Until then, continue manual QA via `stock-init-workspace-qa` and `examples/playground.rs`.
