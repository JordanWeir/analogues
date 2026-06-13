# Flow Metrics Gate Failure — Error Reporting and ORCL initWorkspace Block

Date: 2026-06-11

Invocation: `cargo loco task initWorkspace ticker:ORCL`

## Verdict

**initWorkspace fails with a generic CLI error** even though the underlying quality gate produces a detailed, actionable rejection message. The failure is real for ORCL (mixed income-statement period types in `fundamental_observations`), but operators must read SQL debug logs or query `quality_gate_results` to learn why.

## Error Report Summary

CLI output:

```text
Error: Message("build_catalog failed a quality gate")
```

What actually failed (from `quality_gate_results`, visible only in SQL logging at default log levels):

| Lane | Gate | Status | Message |
|------|------|--------|---------|
| `build_catalog` | `catalog_materialized` | pass | — |
| `build_catalog` | `core_fundamentals_traceable` | pass | — |
| `build_catalog` | `flow_metrics_period_labeled` | **reject** | flow observations mix period types without normalization: diluted_shares (annual, quarter, ytd); eps (annual, quarter, ytd); gross_profit (annual, quarter, ytd); net_income (annual, quarter, ytd); operating_income (annual, quarter, ytd); revenue (annual, quarter, ytd) |

Context from the same run:

- 25,369 `sec_raw_facts` ingested
- 516 `concept_catalog_entries` materialized
- 1,310 `fundamental_observations` derived
- 9 active canonical mappings (revenue traceable to `RevenueFromContractWithCustomerExcludingAssessedTax`)
- Starter financial fetch partial (missing current share price and market cap in `data_gaps`)

The lane itself completed successfully (`build_catalog` returned `LaneStatus::Success`). The pipeline stopped only because a post-lane quality gate rejected.

## Where the Error Is Repeated (and Where It Is Not)

### Generic message surfaces here

1. **`src/lanes/runner.rs`** — on blocking gate failure, `stop_reason` is set to `"{lane_name} failed a quality gate"` with no gate name or gate message, even though `gate_results` on the `LaneResult` already contain the detail.

2. **`src/tasks/init_workspace.rs`** — `initialize_workspace` returns `Err(Error::string(&reason))` using only `report.stop_reason`. The `LinearRunReport` also holds `lane_results` with full `gate_results`, but that structure is discarded before the error propagates to the CLI.

### Detailed message is recorded but not shown to the operator

3. **`src/services/quality_gate_store.rs`** — gate outcomes are persisted to `quality_gate_results` before the runner checks for blocking failures. The full rejection text is in the database.

4. **SQL debug logs** — at `INFO` level, `sqlx::query` logs the `INSERT INTO quality_gate_results` statement including the `message` column. This is how the rejection was discovered during manual debugging; it is easy to miss without SQL logging enabled or a DB query.

5. **`src/lanes/build_catalog/gate.rs`** — `FlowMetricsPeriodLabeledGate` constructs the detailed rejection in `GateResult::reject(...)`. The message is never lost upstream of the runner; the runner simply does not forward it.

### Related prior QA that did not catch this

- [06-08-001-data-quality-report.md](./06-08-001-data-quality-report.md) inspected ORCL init runs that completed and noted correct period typing in the observation layer. Those runs predated or bypassed the blocking `flow_metrics_period_labeled` gate behavior now enforced in `build_catalog`.
- [06-06-002-phase-5-sqlite3-orcl-analysis.md](./06-06-002-phase-5-sqlite3-orcl-analysis.md) documents period normalization as the main pain point for SQL-only research on ORCL, but does not cover initWorkspace gate failure or CLI error surfacing.

## Why It Happens

Two separate issues combine into one confusing operator experience.

### 1. Substantive gate failure (expected for real SEC data)

`FlowMetricsPeriodLabeledGate` rejects when any income-statement flow metric has observations tagged with more than one of `quarter`, `ytd`, or `annual` in `fundamental_observations`.

For ORCL this is expected. SEC Company Facts for core income-statement concepts include:

- **Annual** periods from 10-K filings (~300–390 day spans)
- **Quarter** periods from 10-Q filings (~60–120 day spans)
- **YTD** cumulative periods from 10-Q filings (121–299 day spans or quarterly filing heuristics)

`derive_starter_fundamentals_on_workspace` → `canonical_sec_observations` in `src/services/fundamental_deriver.rs` emits one observation per raw fact, classifying each row via `fact_period_type`. It does **not** normalize to a single comparable series per metric. All three period shapes coexist under the same `metric_key`.

The gate enforces pipeline plan intent from `docs/01-pipeline-plan.md`:

> Flow metrics do not mix quarterly, year-to-date, and annual observations without labeling or normalization.

Labeling exists per row (`period_type` column), but the gate treats co-mingled shapes under one metric key as "without normalization" and blocks downstream lanes.

### 2. Error reporting gap (unexpected, fixable independently)

`LinearRunner` persists gate results, then sets a generic `stop_reason` and breaks. `initWorkspace` surfaces only that string. The operator sees a failure with no gate name, no rejection message, and no pointer to `quality_gate_results`.

## Suggested Fix

### A. Surface gate details in CLI errors (small, high value)

In `src/lanes/runner.rs`, when `has_blocking_gate_failure()` is true, build `stop_reason` from the first blocking `GateResult`:

```text
build_catalog gate flow_metrics_period_labeled rejected: flow observations mix period types without normalization: revenue (annual, quarter, ytd); ...
```

Optionally log the same at `WARN` or `ERROR` before returning from `initialize_workspace`, and mention that full history lives in `quality_gate_results`.

Add a unit test in `src/lanes/tests.rs` (alongside `linear_runner_stops_on_gate_reject`) asserting `report.stop_reason` includes the gate name and rejection message.

This does not change gate policy; it makes every gate failure self-explanatory at the terminal.

### B. Period normalization for flow metrics (substantive, larger)

To make ORCL init pass the gate without weakening it:

1. **Choose a canonical period shape per flow metric** for the observation timeline (likely `quarter` for time-series work, with explicit `ytd` and `annual` rows kept in separate views or tagged series keys).
2. **Normalize in `fundamental_deriver` or a dedicated post-derive step** before persisting `fundamental_observations` — e.g. derive quarter-only values from YTD diffs where needed, dedupe amended filings by latest `filed_at`, and avoid mixing shapes under the same `metric_key` + `period_end` comparison set.
3. **Keep exotic multi-shape concepts in `concept_catalog_entries`** without promoting them to canonical flow fundamentals until normalized.

See [06-06-002-phase-5-sqlite3-orcl-analysis.md](./06-06-002-phase-5-sqlite3-orcl-analysis.md) § "Period normalization is the main pain point" for ORCL-specific patterns (YTD subtraction, `period_end` vs `fiscal_year` grouping).

### C. Interim operator workaround

Query the workspace DB after a failed init:

```sql
SELECT lane_name, gate_name, status, message, created_at
FROM quality_gate_results
WHERE status IN ('reject', 'quarantine')
ORDER BY created_at DESC;
```

The workspace directory and `run.sqlite` are created before the gate failure, so this query works on the partial run.

## Recommended priority

1. **Fix A** — unblocks debugging immediately; low risk.
2. **Fix B** — required for ORCL (and most filers with 10-Q history) to complete init without gate failure; aligns with existing pipeline plan and prior ORCL QA notes.
