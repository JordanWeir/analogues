# Worker Telemetry, Failures & Run Counts — ORCL `ORCL-2026-06-14-2`

## Scope

QA inspection of **worker run telemetry** for one full `initWorkspace` invocation on Oracle (`ORCL-2026-06-14-2`). Focus: how many workers ran, agent round counts, failures, and what the persisted `worker_runs` table actually captures.

| Field | Value |
|-------|-------|
| SQLite | `reports/stock-narrative-research/ORCL-2026-06-14-2/run.sqlite` |
| Command | `initWorkspace ticker:ORCL mapping_strategy:heuristic build_narrative_map:true build_financial_analysis:true` |
| Model | `deepseek/deepseek-v4-flash` (all workers) |
| Exit code | **1** — `scenario_generation failed: not every scenario has persisted quarterly periods` |
| `run_metadata.status` | `initialized` (partial; scenario lane did not complete) |
| Wall-clock (worker latency sum) | **~7,773s (~129 min)** of recorded worker time (parallel fan-out; not sequential) |

Compared to prior ORCL QA: `06-13-015` (run 25, 24 financial workers, all gates pass). This run adds **scenario generation** and records **29** total workers.

---

## Verdict

**Partial pass on upstream lanes; fail on scenario generation.** Twenty-six quality gates pass through `financial_fan_out`. Two of 29 workers failed with the same root pattern: **OpenRouter returned empty assistant text after exhausting the agent round budget** without calling the submit tool. One failure blocked the entire `initWorkspace` run because scenario detail is a hard completeness gate (4/5 scenarios have `scenario_periods`).

Worker telemetry is **usable for successful runs** (rounds, tool calls, cost, latency) but **misleading for failures** — failed workers persist `agent_rounds = 0` and null token/cost fields even though the error message records the real step count.

---

## Run Outcome Summary

| Dimension | Count |
|-----------|------:|
| Quality gates passed | 26 / 26 (through `financial_fan_out`; `scenario_generation` gates never recorded) |
| `worker_runs` total | 29 |
| Successful workers | 27 |
| Failed workers | 2 |
| Total `agent_rounds` (persisted) | 400 |
| Estimated actual rounds incl. failures | **~468** (400 + 36 + 32 from error messages) |
| Total client tool calls | 800 |
| Total cost (`worker_runs.cost_usd`) | **$0.66** |
| Promoted cruxes | 11 |
| Promoted experiments | 24 |
| Scenario blueprints | 5 |
| Scenarios with quarterly periods | **4 / 5** |
| `monte_carlo_summary` rows | 0 |

---

## Worker Counts by Lane

| Worker | Lane | Mode | Runs | OK | Err | Total rounds | Avg rounds | Round range | Tool calls | Cost | Latency (sum) |
|--------|------|------|-----:|---:|----:|-------------:|-----------:|------------:|-----------:|-----:|--------------:|
| `narrative_researcher` | `build_narrative_map` | — | 1 | 1 | 0 | 11 | 11.0 | 11 | 23 | $0.026 | 144s |
| `financial_model_explorer` | `identify_crux_candidates` | `crux_triage` | 10 | 10 | 0 | 125 | 12.5 | 8–18 | 320 | $0.155 | 2,523s |
| `financial_model_explorer` | `financial_mechanics_experiments` | `mechanics_experiment` | 12 | 11 | 1 | 200* | 16.7* | 0–31 | 305 | $0.270 | 3,262s |
| `scenario_builder` | `scenario_generation` | `scenario_blueprint` | 1 | 1 | 0 | 16 | 16.0 | 16 | 43 | $0.076 | 450s |
| `scenario_builder` | `scenario_generation` | `scenario_detail` | 5 | 4 | 1 | 48* | 9.6* | 0–15 | 109 | $0.132 | 1,394s |
| **Total** | | | **29** | **27** | **2** | **400** | **13.8** | | **800** | **$0.66** | **7,773s** |

\*Failed runs contribute `0` to persisted round totals; see failure section below.

### Workers not spawned this run

| Worker | Why absent | Configured max rounds |
|--------|------------|----------------------:|
| `fundamental_catalog_manager` | `mapping_strategy:heuristic` (no LLM catalog review) | 20 |

### Configured max agent rounds (code)

| Worker / mode | `MAX_AGENT_ROUNDS` constant |
|---------------|----------------------------:|
| `narrative_researcher` | 28 |
| `financial_model_explorer` (triage) | 24 |
| `financial_model_explorer` (mechanics) | 36 |
| `scenario_builder` (blueprint) | 20 |
| `scenario_builder` (detail) | 32 |
| `fundamental_catalog_manager` | 20 |

---

## Per-Worker Round Counts (all 29 runs)

| ID | Worker | Lane | Mode | Status | Rounds | Tool calls | Cost | Latency |
|---:|--------|------|------|--------|-------:|-----------:|-----:|--------:|
| 1 | narrative_researcher | build_narrative_map | — | success | 11 | 23 | $0.026 | 144s |
| 2 | financial_model_explorer | identify_crux_candidates | crux_triage | success | 8 | 23 | $0.008 | 155s |
| 3 | financial_model_explorer | identify_crux_candidates | crux_triage | success | 12 | 45 | $0.023 | 171s |
| 4 | financial_model_explorer | identify_crux_candidates | crux_triage | success | 10 | 25 | $0.011 | 172s |
| 5 | financial_model_explorer | identify_crux_candidates | crux_triage | success | 11 | 27 | $0.008 | 198s |
| 6 | financial_model_explorer | identify_crux_candidates | crux_triage | success | 13 | 39 | $0.021 | 233s |
| 7 | financial_model_explorer | identify_crux_candidates | crux_triage | success | 18 | 40 | $0.019 | 242s |
| 8 | financial_model_explorer | identify_crux_candidates | crux_triage | success | 14 | 35 | $0.018 | 284s |
| 9 | financial_model_explorer | identify_crux_candidates | crux_triage | success | 17 | 32 | $0.019 | 289s |
| 10 | financial_model_explorer | identify_crux_candidates | crux_triage | success | 14 | 33 | $0.020 | 291s |
| 11 | financial_model_explorer | identify_crux_candidates | crux_triage | success | 8 | 21 | $0.008 | 489s |
| 12 | financial_model_explorer | financial_mechanics_experiments | mechanics_experiment | success | 9 | 19 | $0.009 | 159s |
| 13 | financial_model_explorer | financial_mechanics_experiments | mechanics_experiment | success | 13 | 18 | $0.013 | 169s |
| 14 | financial_model_explorer | financial_mechanics_experiments | mechanics_experiment | success | 15 | 24 | $0.016 | 176s |
| **15** | **financial_model_explorer** | **financial_mechanics_experiments** | **mechanics_experiment** | **error** | **0**† | **0**† | — | 193s |
| 16 | financial_model_explorer | financial_mechanics_experiments | mechanics_experiment | success | 13 | 23 | $0.018 | 204s |
| 17 | financial_model_explorer | financial_mechanics_experiments | mechanics_experiment | success | 24 | 35 | $0.043 | 265s |
| 18 | financial_model_explorer | financial_mechanics_experiments | mechanics_experiment | success | 12 | 17 | $0.015 | 275s |
| 19 | financial_model_explorer | financial_mechanics_experiments | mechanics_experiment | success | 16 | 23 | $0.023 | 301s |
| 20 | financial_model_explorer | financial_mechanics_experiments | mechanics_experiment | success | 23 | 29 | $0.024 | 315s |
| 21 | financial_model_explorer | financial_mechanics_experiments | mechanics_experiment | success | 27 | 44 | $0.046 | 356s |
| 22 | financial_model_explorer | financial_mechanics_experiments | mechanics_experiment | success | 17 | 24 | $0.015 | 391s |
| 23 | financial_model_explorer | financial_mechanics_experiments | mechanics_experiment | success | 31 | 49 | $0.049 | 458s |
| 24 | scenario_builder | scenario_generation | scenario_blueprint | success | 16 | 43 | $0.076 | 450s |
| **25** | **scenario_builder** | **scenario_generation** | **scenario_detail** | **error** | **0**† | **0**† | — | 21s |
| 26 | scenario_builder | scenario_generation | scenario_detail | success | 13 | 32 | $0.017 | 216s |
| 27 | scenario_builder | scenario_generation | scenario_detail | success | 10 | 25 | $0.033 | 223s |
| 28 | scenario_builder | scenario_generation | scenario_detail | success | 10 | 24 | $0.036 | 397s |
| 29 | scenario_builder | scenario_generation | scenario_detail | success | 15 | 28 | $0.047 | 538s |

†Persisted as zero on error path; actual counts embedded in `error_message` only.

---

## Worker Failures

### Summary

| ID | Worker | Lane / mode | Persisted rounds | Actual steps (from error) | Client tool calls (from error) | Latency | Downstream impact |
|---:|--------|-------------|-----------------:|--------------------------:|-------------------------------:|--------:|-------------------|
| 15 | `financial_model_explorer` | mechanics_experiment | 0 | **36** (max = 36) | 8 | 193s | **None** — lane continued; 11/12 mechanics workers succeeded |
| 25 | `scenario_builder` | scenario_detail | 0 | **32** (max = 32) | 3 | 21s | **Fatal** — `multicloud_buffer_neutral` has 0 periods; `initWorkspace` exit 1; Monte Carlo skipped |

### Worker #15 — mechanics experiment

```
OpenRouter returned no assistant text after 36 agent steps
(finish_reason=None, web_search_requests=0, client_tool_calls=8, preview=<empty>)
```

- Exhausted the mechanics round budget (`FINANCIAL_MECHANICS_MAX_AGENT_ROUNDS = 36`).
- Never called `submit_mechanics_experiments`.
- One of 12 parallel per-crux mechanics workers; financial fan-out lane still passed all gates.

### Worker #25 — scenario detail

```
OpenRouter returned no assistant text after 32 agent steps
(finish_reason=Some("stop"), web_search_requests=0, client_tool_calls=3, preview=<empty>)
```

- Exhausted the detail round budget (`SCENARIO_DETAIL_MAX_AGENT_ROUNDS = 32`).
- Never called `submit_scenario_detail`.
- Only **3 client tool calls** in ~21s vs 216–538s for the four successful detail workers — failed fast without meaningful submission attempt.
- Missing scenario: **`multicloud_buffer_neutral`** (scenario id 2, neutral stance, **28% probability** — highest-weight scenario).

### Common failure pattern

Both failures are `empty_completion_error` from `run_client_tool_loop` (`src/services/openrouter_chat.rs`): the model loop ran to the round limit without producing non-empty assistant text or a successful submit-tool completion. This is an **intermittent model/provider behavior**, not a schema or persist validation error on the successful siblings.

---

## Round Count Observations

### By phase (successful workers only)

| Phase | Workers | Total rounds | Mean rounds/worker | % of max budget used (mean) |
|-------|--------:|-------------:|-------------------:|----------------------------:|
| Narrative | 1 | 11 | 11.0 | 39% of 28 |
| Crux triage | 10 | 125 | 12.5 | 52% of 24 |
| Mechanics | 11 | 200 | 18.2 | 50% of 36 |
| Scenario blueprint | 1 | 16 | 16.0 | 80% of 20 |
| Scenario detail | 4 | 48 | 12.0 | 38% of 32 |

### Headroom vs budget

- **Triage** peaks at 18/24 rounds (worker #7) — comfortable margin.
- **Mechanics** peaks at 31/36 (worker #23); worker #15 hit the ceiling at 36/36 and failed.
- **Scenario detail** successful workers used only 10–15/32 rounds; the failure also hit 32/32, suggesting a different failure mode (empty responses) rather than simply "ran out of budget while making progress."

### Round accounting gap

Persisted total: **400 rounds**. If failure error messages are accurate, true total is **~468 rounds** (~17% under-counted) because `ToolLoopAgent` calls `empty_response()` on error and zeroes `agent_rounds`, usage, and tool-call counts before persisting (`src/agents/tool_loop_agent.rs`).

---

## Telemetry Gaps

| Gap | Severity | Detail |
|-----|----------|--------|
| Failed-run usage not persisted | High | `input_tokens`, `output_tokens`, `cost_usd` are NULL for workers #15 and #25 |
| Failed-run rounds zeroed | High | DB shows 0; true counts only in `error_message` string |
| No chat / tool transcript | High | Cannot inspect what SQL or prompts led to failure without re-running |
| No `focus_scenario_key` / `focus_crux_key` in metadata | Medium | Cannot directly map worker #25 → `multicloud_buffer_neutral` from `worker_runs` alone |
| `fundamental_catalog_manager` absent | Low (expected) | Heuristic mapping path does not spawn catalog LLM worker |

---

## Recommendations

1. **Persist loop telemetry on error** — pass `total_usage`, `agent_rounds`, and `client_tool_calls` from `run_client_tool_loop` into the error path instead of `empty_response()`.
2. **Persist `focus_*` keys in `metadata_json`** for fan-out workers (scenario key, crux key, scout flag).
3. **Add step-level or JSONL transcript** for post-mortem (see `docs/tasks/06-14-organizing-next-steps.md`).
4. **Retry policy for empty-completion failures** — especially scenario detail, where one failed worker blocks Monte Carlo.
5. **Re-run scenario detail for `multicloud_buffer_neutral` only** — four sibling scenarios are already persisted with 20 quarters each.

---

## Queries Used

```sql
-- Worker summary by lane/mode
SELECT worker_name, json_extract(metadata_json,'$.lane') AS lane,
       json_extract(metadata_json,'$.mode') AS mode,
       COUNT(*) AS runs, SUM(agent_rounds) AS total_rounds,
       MIN(agent_rounds), MAX(agent_rounds), SUM(client_tool_calls),
       ROUND(SUM(cost_usd), 4), SUM(latency_ms)/1000.0
FROM worker_runs GROUP BY 1, 2, 3;

-- Failures
SELECT id, worker_name, agent_rounds, error_message
FROM worker_runs WHERE status = 'error';
```
