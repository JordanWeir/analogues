# Scenario Traceability & Data-Pathing — ORCL `ORCL-2026-06-15-1`

## Scope

Deep inspection of **how scenario numbers were produced**, what traceability artifacts exist between financial experiments and `scenario_periods`, and why persisted projections and Monte Carlo output look materially wrong relative to spot (~$184/share, Jun 2026).

Companion to [06-15-001-data-quality-report-orcl.md](./06-15-001-data-quality-report-orcl.md) (workspace-wide QA) and [06-14-001-worker-telemetry-orcl-qa.md](./06-14-001-worker-telemetry-orcl-qa.md) (worker failures on prior run).

| Field | Value |
|-------|-------|
| SQLite | `reports/stock-narrative-research/ORCL-2026-06-15-1/run.sqlite` |
| Scenarios | 5 (`scenario_assumptions`) |
| Quarterly periods | 92 (`scenario_periods`) |
| Promoted experiments | 20 (`analysis_experiments`) |
| Experiment SQL runs | 25 (`analysis_runs`; 23 success, 2 error) |
| Scenario workers | 6 (`scenario_builder`: 1 blueprint + 5 detail) |
| Monte Carlo median terminal implied price | **~$47** (no spot anchor in `fundamentals`) |

---

## Verdict

**Narrative scaffolding is usable; quantitative pathing is broken.** Scenarios are company-specific, crux-linked, and cite experiments by name — but **quarterly projections are LLM-interpolated**, not derived from `analysis_runs.result_json`. Terminal valuation bands often contradict scenario prose. Monte Carlo mechanically samples those terminal bands and is heavily skewed by one bear scenario’s distressed 0.4× P/S multiple. The “obviously bad” numbers are largely a **data-pathing and validation problem**, not random noise.

---

## How Scenarios Are Built (Actual Pipeline)

Scenarios are **not** the output of a deterministic model that runs experiments forward quarter-by-quarter.

```
sec_raw_facts + av_raw_facts + analysis_experiments + crux_candidates + claims
        │
        ▼
scenario_builder (blueprint) ──► scenario_assumptions (5 scenarios, probabilities)
        │
        ▼
scenario_builder (detail) × 5 ──► submit_scenario_detail JSON
        │                              ├── scenario_periods (revenue, margins, EPS, shares)
        │                              ├── scenario_crux_assumptions (experiment_key strings)
        │                              ├── scenario_signals / scenario_sensitivities
        │                              └── source_note prose per period
        ▼
compute_and_persist_monte_carlo ──► samples terminal P/E × EPS and P/S × rev/share only
```

Code references:

- Blueprint/detail preambles: `src/agents/scenario_builder/agent.rs` (`SCENARIO_BLUEPRINT_PREAMBLE`, `SCENARIO_DETAIL_PREAMBLE`).
- Detail submit schema: `src/agents/tools/scenario_detail_submit.rs`.
- Monte Carlo: `src/services/scenario_projection.rs` (`build_monte_carlo`, `build_scenario_json`).
- Context injected into detail workers: `src/agents/scenario_builder/context.rs` (summaries only — not full experiment JSON).

**Detail-mode instructions (paraphrased):** anchor ~4 historical quarters on Alpha Vantage `av_raw_facts`; project 12–20 forward quarters; use experiments and claims to *shape* assumptions; **interpolate where needed**; set valuation bands on the **terminal quarter only**.

---

## Traceability Artifacts

### What exists

| Artifact | Links | Strength |
|----------|-------|----------|
| `scenario_crux_assumptions.experiment_key` | Scenario crux → experiment by string | **Citation only** — 35/36 rows; no FK |
| `scenario_crux_assumptions.source_id` | Assumption → `sources` | **Moderate** — 33/36 cite earnings/8-K/news |
| `scenario_periods.source_note` | Quarter → agent prose | **Weak** — not machine-verified |
| `analysis_experiments` | `sql_body`, `outputs_json`, `bridge_json`, `worker_run_id` | **Strong** for experiments themselves |
| `analysis_runs` | `executed_sql`, `result_json`, `execution_status` | **Strong** — 2/25 failed |
| `supporting_metric_selections` | Crux → SEC concepts | **Crux-level** — `scenario_id` always null |
| `worker_runs.metadata_json` | Worker → `focus_scenario_key`, `mode`, rounds | **Who wrote which scenario** |
| Monte Carlo | Terminal `pe_median`/`ps_median` × terminal EPS/revenue | **Mechanical** from last period only |

### What does not exist

- Period-level FK from `scenario_periods` → `analysis_runs` or `outputs_json` rows.
- Reproducible formula chain: SQL result → quarterly grid.
- OCF, CapEx, FCF, debt, interest, or RPO columns in `scenario_periods` (central crux metrics are prose-only).
- Validation that terminal `source_note` implied prices match stored `pe_median`/`ps_median`.
- `current_price` in `fundamentals` to anchor Monte Carlo as return-from-today.

---

## What the Scenario Builder Saw

Each detail worker (`worker_runs.metadata_json`) received a prompt section built by `load_scenario_context`:

- Promoted crux board (9 cruxes, titles, archetypes).
- **At most 12** experiments where `purpose IN ('forward_projection', 'sensitivity', 'scenario_validation')` — key + purpose + crux_key only.
- AV quarterly `totalRevenue` coverage summary.
- Sources board (7 entries with ids).
- Blueprint summary (scenario keys, stances, probabilities).

**Not injected:** full `result_json`, `outputs_json`, or `bridge_json`. The agent must re-query `workspace_sql` or rely on truncated summaries, then hand-fill 16–20 quarters.

### Worker runs (scenario lane)

| Mode | `focus_scenario_key` | Rounds | Latency |
|------|----------------------|-------:|--------:|
| `scenario_blueprint` | — | 4 | ~94s |
| `scenario_detail` | `rpo_acceleration_fcf_inflection_bull` | 12 | ~378s |
| `scenario_detail` | `gradual_optimization_neutral` | 9 | ~402s |
| `scenario_detail` | `buildout_on_track_financing_digestible_neutral_bull` | 13 | ~423s |
| `scenario_detail` | `openai_concentration_shock_bear` | 13 | ~460s |
| `scenario_detail` | `obligation_leverage_spiral_bear` | 14 | ~577s |

---

## Financial Experiments → Scenario Citations

### Experiment inventory (promoted)

| experiment_key | purpose | crux_id | Run status |
|----------------|---------|--------:|------------|
| `arch_moat_forward_sensitivity` | forward_projection | 5 | success |
| `buildout_funding_gap_forward` | forward_projection | 7 | success |
| `rpo_openai_concentration_sensitivity` | sensitivity | 2 | success |
| `financing_forward_projection_sensitivity` | forward_projection | 3 | success |
| `fcf_inflection_timing_sensitivity` | forward_projection | 4 | success |
| `obligation_interest_coverage_trajectory` | forward_projection | 8 | success |
| `ppne_placement_efficiency_sensitivity` | sensitivity | 7 | success |
| `capex_funding_gap_fy27_forward_projection` | forward_projection | 9 | success |
| `prepay_funding_gap_sensitivity_fy27` | forward_projection | 6 | success |
| `rpo_conversion_to_fund_fy27_capex` | forward_projection | 1 | success |
| `rpo_openai_forward_projection` | forward_projection | 2 | success |
| `obligation_eps_dilution_atm_sensitivity` | sensitivity | 8 | success |
| `obligation_stack_eps_sensitivity` | forward_projection | 8 | success |
| `capex_funding_gap_forward_projection` | — | — | **error** (`POWER()` not in SQLite) |
| `capex_ocf_ratio_ytd_fy2026` | — | — | **error** (bad `ORDER BY`) |

Historical experiments (`arch_moat_margin_trajectory`, `capex_ocf_ratio_trajectory`, etc.) are **not** in the 12-line prompt summary but can still appear in `scenario_crux_assumptions.experiment_key`.

### Citation frequency in `scenario_crux_assumptions`

| experiment_key | Scenarios citing |
|----------------|-----------------|
| `capex_funding_gap_fy27_forward_projection` | 1, 2, 4, 5 |
| `financing_forward_projection_sensitivity` | 1, 2, 3, 4, 5 |
| `fcf_inflection_timing_sensitivity` | 1, 2, 4, 5 |
| `buildout_funding_gap_forward` | 1, 3, 4, 5 |
| `obligation_interest_coverage_trajectory` | 1, 2, 3, 4, 5 |
| `rpo_openai_forward_projection` | 1, 3, 5 |
| `arch_moat_forward_sensitivity` | 1, 3, 5 |
| `obligation_eps_dilution_atm_sensitivity` | 4 |
| `rpo_openai_concentration_sensitivity` | 2 |
| `prepay_funding_gap_sensitivity_fy27` | 1, 5 |

---

## Key Experiment Outputs (What They Actually Say)

### `obligation_eps_dilution_atm_sensitivity` — primary driver of bear EPS

SQL computes FY2027 EPS as `( $90B revenue × margin − interest ) / shares`:

| Scenario | Assumptions | FY2027 EPS |
|----------|-------------|------------|
| A | 15% margin, $4.5B interest, 3.5B shares | **$2.57** |
| B | 15% margin, $5.0B interest, 3.5B shares | **$2.43** |
| C | 12% margin, $4.5B interest, 3.5B shares | **$1.80** |
| D | 12% margin, $5.0B interest, 3.7B shares | **$1.57** |

Interpretation in `outputs_json`: *"Even optimistic $90B / 15% margin yields $2.57 — 41% below FY2025 $4.34. Worst case $1.57 — 64% decline."*

**Scenario 4 FY2027 quarterly EPS** ($0.45–0.61 on ~$20–24B/qtr revenue) is directionally consistent with this bridge, then extrapolated further down to terminal **$0.15**.

### `capex_funding_gap_fy27_forward_projection` — funding gap at guided CapEx

Static scenario table at **$70B FY2027 CapEx**:

| Scenario | OCF | OCF/CapEx | Implied growth from FY2025 OCF |
|----------|----:|----------:|-------------------------------:|
| FY2025 actual | $20.8B | 0.98× | — |
| FY2026 YTD Q3 | $17.4B | 0.44× | — |
| FY2027 @ 0.5× ratio | $35.0B | 0.50× | 34% |
| FY2027 @ 0.75× ratio | $52.5B | 0.75× | 76% |
| FY2027 @ 1.0× ratio | $70.0B | 1.00× | 118% |
| FY2027 @ 1.5× ratio | $105.0B | 1.50× | 202% |

Used in scenario prose (OCF growth 10–30%) but **not written into period columns**.

### `financing_forward_projection_sensitivity` — dilution + interest

- ATM at $170/sh adds ~118M shares on 2.876B base.
- Interest coverage falls from 4.94× (FY2025) to ~3.70× (FY2027 pro forma at 5%).
- **Flaw:** FY2027 pro forma rows hold **operating income flat at $19.3B** while revenue scales to $90B in scenarios — interest-coverage math ignores revenue scale-up.

### `fcf_inflection_timing_sensitivity`

- FCF negative through FY2027 at guided CapEx (~$70B).
- Turns positive FY2028 only if CapEx drops to ~$40B and OCF grows 15–20%.
- Notes SEC staleness: claims cite $32B FY2026 OCF vs YTD annualized ~$23B.

### `rpo_openai_forward_projection`

- OpenAI 50–60% of $638B RPO → **$38–46B recognized revenue in FY2027 alone** (57–69% of estimated FY2026 total revenue).
- Forward concentration risk; no SEC customer breakdown.

---

## Scenario-by-Scenario: Numbers vs Lineage

| # | Scenario | P | FY2027 rev (Σ quarters) | Terminal EPS | Stored terminal implied price | Experiment alignment |
|---|----------|---|--------------------------|--------------|------------------------------|----------------------|
| 1 | RPO acceleration bull | 20% | ~$90B | $3.01 | ~$60–82 | Revenue matches guidance; EPS above obligation bridge (bull margins) |
| 2 | OpenAI shock bear | 15% | ~$63B | $0.82 | ~$15–22 | Revenue matches narrative; concentration experiment |
| 3 | Build-out on track bull | 25% | ~$90B | $2.96 | **~$65** (bands) vs **~$233** (source_note) | **Prose/band mismatch** |
| 4 | Leverage spiral bear | 15% | ~$90B | $0.15 | **~$2–3** (0.4× P/S) | EPS path from obligation experiment; terminal punitive beyond experiment |
| 5 | Gradual optimization neutral | 25% | ~$87B | $1.11 | ~$33–55 | Slight guidance miss; normalized Q2 called out in notes |

### Scenario 4 walk-through (the “obviously bad” case)

| Layer | Content |
|-------|---------|
| Experiments | `obligation_eps_dilution_atm_sensitivity` → FY2027 EPS $1.57–2.57 at $90B rev |
| | `financing_forward_projection_sensitivity` → coverage ~3.7× |
| | `capex_funding_gap_fy27_forward_projection` → needs 76%+ OCF growth for 0.75× at $70B CapEx |
| FY2027 quarters | Rev ~$90B annualized; EPS $0.45–0.61; shares 2.95–3.12B |
| Terminal | EPS $0.15; P/S 0.4× → ~$2.60 implied |
| Monte Carlo | 15% weight on ~$2–15 range pulls distribution far below spot |

Quarterly EPS is internally consistent: `eps ≈ revenue × net_margin / diluted_shares` (verified for FY2027 Q1–Q4).

---

## Why Results Look Bad in Practice

### 1. No mechanical bridge from experiments → quarters

Experiments produce point estimates and sensitivity tables. The scenario builder **interpolates** YoY growth quarter-by-quarter. Revenue paths can match headline anchors ($63B / $87B / $90B) while EPS/margin paths drift from experiment assumptions.

### 2. Terminal valuation bands contradict scenario prose

Example — scenario 3 terminal `source_note`:

> Blend ~$233/sh, ~$727B market cap vs current $579B

Stored bands: P/E 22×, P/S 5.5× on terminal quarter → **~$65/share**. Monte Carlo uses bands, not prose.

### 3. Monte Carlo samples terminal-only, bear-heavy distribution

- 10k iterations, seed 42; P10/P50/P90 normal from terminal low/median/high bands.
- Median **~$47**, mean **~$45** — vs **~$184** spot (not in DB).
- Scenario 4’s 0.4× P/S (~$2.60) dominates left tail despite 15% probability.

### 4. Bear EPS collapse is partly grounded, then over-extended

`obligation_eps_dilution_atm_sensitivity` legitimately shows EPS compression at $90B revenue under debt + dilution. Agent extended to terminal $0.15 and distressed multiples beyond experiment FY2027 points.

### 5. Historical anchor poisoned (FY2026 Q2)

AV Q2 FY2026: **38.2% net margin, $2.10 EPS** — one-time Ampere/Bloom gains. Scenario 5 normalizes in `source_note`; scenarios 1–2 anchor raw AV without adjustment.

### 6. Crux metrics absent from period grid

RPO conversion, OCF/CapEx coverage, and funding gap are analyzed in experiments and discussed in `source_note` but **not persisted as columns** — cannot audit whether quarterly revenue growth is consistent with RPO math.

### 7. Failed experiments alongside successful siblings

`capex_funding_gap_forward_projection` failed (`POWER()`); promoted sibling `capex_funding_gap_fy27_forward_projection` succeeded. Scenarios cite the successful key, but the failure pattern shows SQL fragility in the experiment lane.

---

## Data-Pathing Failure Modes (for investigation)

| Failure mode | Where it breaks | Symptom in ORCL run |
|--------------|-----------------|---------------------|
| **Prompt summary truncation** | `context.rs` LIMIT 12 experiments, no `outputs_json` | Agent re-guesses numbers instead of copying SQL results |
| **LLM interpolation gap** | `scenario_detail` submit | Quarterly grid not reproducible from `analysis_runs` |
| **No period–experiment FK** | Schema | Cannot query "which experiment row produced FY2027 Q3 EPS?" |
| **Terminal band validation missing** | `validate_detail_output` | Bull scenario claims $233/sh, stores $65/sh |
| **Monte Carlo without spot** | `scenario_projection.rs` + missing `current_price` | Median ~$47 reads like price target, isn't |
| **Flat OI in financing experiment** | `financing_forward_projection_sensitivity` SQL | Coverage math inconsistent with $90B revenue scenarios |
| **AV anchor contamination** | Detail mode uses raw AV Q2 | Elevated margin/EPS seed |
| **Missing funding columns** | `scenario_periods` schema | CapEx/OCF claims in prose only |
| **SQLite SQL errors in experiments** | `analysis_runs` errors | Partial experiment lane; agent may not know what failed |

---

## Recommendations

### Traceability (highest leverage)

1. **Persist experiment → period links** — e.g. `scenario_period_derivations(period_id, experiment_key, output_kind, value)` or store `inputs_json`/`outputs_json` snapshot on terminal period.
2. **Inject full `outputs_json` + `bridge_json`** for cited experiments into detail-worker prompt (or require `workspace_sql` read of `analysis_experiments` before submit).
3. **Validate terminal bands vs `source_note`** — reject submit when implied price from bands differs >X% from prose claim.

### Projection mechanics

4. **Shared projection function** — derive quarterly paths from experiment bridges (EPS bridge, OCF/CapEx table) with agent filling gaps only where experiments are silent.
5. **Add OCF, CapEx, FCF, net_debt, RPO** to `scenario_periods` or a child table.
6. **Normalize AV anchors** — strip one-time gain quarters before forward projection.

### Monte Carlo

7. **Require `current_price`** before Monte Carlo, or store distribution as **return from spot**.
8. **Cap distressed multiples** or require explicit `distressed: true` flag when P/S < 1×.

### Experiment lane

9. **Ban non-SQLite functions** in experiment SQL (`POWER`, etc.) or pre-validate against workspace SQL dialect.
10. **Fix financing projection** — scale operating income with revenue in forward pro forma rows.

---

## Summary

ORCL scenarios are **strong conditional narratives** with crux links, experiment citations, and falsifiable signals. They are **weak forecasts** because quarterly numbers are agent-interpolated, terminal multiples contradict prose, and Monte Carlo amplifies the most extreme bear terminal without a spot anchor.

The obligation-stack experiment (`obligation_eps_dilution_atm_sensitivity`) explains much of the bear EPS story; the **0.4× P/S terminal and ~$47 Monte Carlo median** are pathing artifacts, not direct experiment outputs. Fixing this requires structural traceability and validation — not better scenario writing alone.
