# Design: Multi-Strategy Scenario Projection

**Status:** Draft  
**Date:** 2026-06-18  
**Authors:** Pipeline architecture  
**Related:**
- [scenario-calculator.md](../contracts/scenario-calculator.md)
- [monte-carlo-engine.md](../contracts/monte-carlo-engine.md)
- [01-pipeline-plan.md](../01-pipeline-plan.md) (Worker Lane 6–7)
- [02-orchestration-styles-comparison.md](../02-orchestration-styles-comparison.md)
- [03-system-map.md](../03-system-map.md)

---

## Summary

Scenario projection today asks a single LLM workflow to fill a fixed period grid (+6m, +12m, +24m, +36m) with independent assumptions for revenue growth, margins, valuation multiples, and P/S vs P/E blend weights. That shape is transparent and easy to chart, but it fights how narratives evolve, invites false precision, and leaves several judgment calls (blend weights, period-to-period growth) without a defensible rule.

This design proposes a **two-layer model**:

1. **Authoring layer** — one of several projection strategies (manual periods, epochs, epochs + shocks, drivers, analogues, …), each produced by a specialized scenario agent when appropriate.
2. **Canonical layer** — a deterministic `ScenarioCompiler` that expands strategy-specific assumptions into the existing `scenario_periods` shape, after which [`ScenarioCalculator`](../contracts/scenario-calculator.md) performs roll-forward math unchanged.

Valuation blend weights move from LLM discretion into calculator policy wherever possible. The report, Monte Carlo engine, and HTML artifacts continue to consume compiled period outputs.

---

## Context

### What works today

- `scenario_periods` is a clear, auditable contract for report tables and charts.
- `ScenarioCalculator` / `projection_model.py` / `generate_report.rs` perform deterministic roll-forward and valuation-band math.
- Scenario envelopes (name, stance, probability, crux assumptions, sensitivities, signals) map well to narrative research outputs.
- `FinancialModelExplorer` already demonstrates **mode-specific workers** (crux triage vs mechanics experiments) behind a shared tool loop — a precedent for scenario specialists.

### What does not work well

| Issue | Symptom |
|-------|---------|
| **Period independence** | Each horizon gets its own growth rate with no enforced narrative coherence across the path. |
| **Narrative regimes** | Stories naturally read as “hypergrowth → normalization” or “constraint → unlock,” not four arbitrary quarterly guesses. |
| **Arbitrary blend weights** | LLM picks P/S vs P/E weights without grounding in earnings quality, horizon, or multiple consistency. |
| **One-size-fits-all** | Growth pre-profit names, mature compounders, event-driven names, and multi-segment conglomerates share the same template. |
| **Lost provenance** | Users cannot see whether a path was built manually, from epochs, or from drivers — only the final period cells. |

Product docs already acknowledge alternative period structures for event-driven companies and describe stocks moving through **narrative regimes** rather than a single static story. This design formalizes that intuition.

---

## Goals

1. Support **multiple projection strategies** while preserving one downstream contract (`scenario_periods` → `ScenarioCalculator`).
2. Let a **planner** assign the best strategy **per scenario** (not necessarily per company run).
3. Persist **strategy-specific assumptions** for audit, user edit, and recompilation.
4. Move **valuation blend policy** into deterministic code where feasible.
5. Keep **Phase 1 implementable** with only `manual` and `epochs` compilers; other strategies are extension points.
6. Maintain compatibility with existing Monte Carlo, report payload, and workspace schema patterns.

## Non-goals (for this design)

- Replacing `ScenarioCalculator` roll-forward math or Monte Carlo sampling semantics.
- Building a full embedded expression language / RHAI runtime for scenarios (SQLite-first remains preferred).
- User-facing strategy picker UI (future); initial routing is agent- or rule-driven.
- Perfect forecast accuracy; projections remain illustrative and assumption-driven.

---

## Architectural overview

```text
┌─────────────────────────────────────────────────────────────────┐
│  ScenarioPlanner (per run)                                       │
│  Inputs: narrative map, cruxes, experiments, analogues, baseline │
│  Output: scenario_plan[] with strategy + rationale per scenario  │
└───────────────────────────┬─────────────────────────────────────┘
                            │ fan-out (parallel)
        ┌───────────────────┼───────────────────┐
        ▼                   ▼                   ▼
 ManualScenarioAgent  EpochsScenarioAgent  DriverScenarioAgent …
        │                   │                   │
        └───────────────────┼───────────────────┘
                            ▼
              ┌─────────────────────────┐
              │  Strategy validation     │
              │  (per strategy rules)    │
              └────────────┬────────────┘
                           ▼
              ┌─────────────────────────┐
              │  ScenarioCompiler        │
              │  strategy_payload →      │
              │  scenario_periods[]      │
              └────────────┬────────────┘
                           ▼
              ┌─────────────────────────┐
              │  Compile validator       │
              │  (shared exit gate)      │
              └────────────┬────────────┘
                           ▼
              ┌─────────────────────────┐
              │  ScenarioCalculator      │
              │  (unchanged contract)    │
              └────────────┬────────────┘
                           ▼
              Monte Carlo + ReportPayloadCompiler
```

**Key boundary:** workers and compilers produce **assumptions**; `ScenarioCalculator` produces **derived financial paths and implied price bands**. Orchestration (skill, linear lane, blackboard) remains swappable as long as workers return structured workspace writes.

---

## Projection strategies

Each strategy is an authoring model. All strategies compile to the same period grid for calculation and display.

### Strategy catalog

| Strategy | ID | Best when | Agent authors | Compiler expands |
|----------|-----|-----------|---------------|------------------|
| **Manual** | `manual` | Simple paths, user overrides, fallback | Period-level growth, margin, multiples | Pass-through (+ optional consistency checks) |
| **Epochs** | `epochs` | Regime shifts (growth → mature) | 2–4 epochs with stable parameters | Monthly/quarterly internal grid → sample report periods |
| **Epochs + shocks** | `epochs_shocks` | Event-driven narratives | Epochs + 0–2 discrete shocks | Apply shocks on internal grid, then sample periods |
| **Driver** | `driver` | Crux is units, ASP, penetration, capacity | Drivers + constraints | Derive revenue path, then standard valuation fields |
| **Analogue** | `analogue` | Strong historical comp with clear phase shape | Analogue ref + scale + deltas | Warp historical path to baseline, sample periods |
| **Segment sum** | `segment_sum` | Multi-business conglomerates | Per-segment sub-strategies | Sum consolidated revenue/margins, then value |
| **Milestone gates** | `milestones` | Binary unlock paths | Gates + run-rate branches | Piecewise path from gate outcomes |

**v1 scope:** implement `manual` and `epochs` (shocks optional within `epochs` payload). Register other IDs in schema but return “unsupported” until implemented.

### Manual (`manual`)

Current behavior, formalized. The agent (or user) specifies assumptions directly at each report period label.

**Use as:** universal fallback when routing confidence is low or no specialist applies.

### Epochs (`epochs`)

An **epoch** is a time window with stable dynamics:

```text
Epoch k:  [t_start, t_end]
  revenue_growth_annual   (or CAGR)
  net_margin              (flat, or linear ramp start→end)
  diluted_shares_drift    (optional annual rate)
  valuation multiples     (P/S required; P/E optional)
  narrative_label
  cruxes_settled[]        (optional links)
```

**Within epoch** (compiler math):

```text
Revenue(t) = Revenue(t_start) × (1 + g_k)^Δt           [annualized compounding]
Margin(t)  = margin_start + (margin_end - margin_start) × progress(t)
```

**Between epochs:** parameters change only at boundaries — not every display period.

**Internal resolution:** expand at **monthly** granularity, then aggregate/sample to default report periods (`Current`, `+6 months`, `+12 months`, `+24 months`, `+36 months`).

### Epochs + shocks (`epochs_shocks`)

Extends epochs with discrete **shocks**:

| Shock type | Effect | Example |
|------------|--------|---------|
| `level_shift` | One-time revenue multiplier | Backlog conversion beat |
| `growth_reset` | New CAGR from t onward | Demand normalization |
| `margin_shock` | Temporary or permanent margin delta | Tariff, mix shift |
| `multiple_rerate` | Step change in valuation band | Narrative premium expands |
| `share_event` | Step or drift in share count | Equity raise, accelerated buyback |

```text
Shock j at t_j:
  type, magnitude, duration (instant | decaying | permanent)
  trigger_label (ties to watch item / crux)
  optional probability (for Monte Carlo extensions later)
```

**Composition:**

```text
Path = Epoch₁ compound → Shock_A → Epoch₂ compound → Shock_B → Epoch₃ …
```

**Guardrails:** max 2 shocks per scenario in v1; each shock must reference a crux or watch item in validation.

### Driver (`driver`) — future

Project from economic drivers instead of revenue growth:

```text
Revenue = Units × ASP × AttachRate
```

or capacity-limited:

```text
Revenue = min(Demand, Capacity) × Price
```

Compiler derives revenue per period; valuation fields attach at epoch or terminal period.

### Analogue (`analogue`) — future

```text
revenue(t) = baseline_revenue × (analogue_revenue(t) / analogue_revenue(0)) × scale_factor
```

Requires a persisted `historical_analogues` row and documented scale rationale.

---

## Scenario agents

### Pattern

Mirror `FinancialModelExplorerAgent`:

- Shared **tool loop** shell (`workspace_sql`, submit tool, validation, step budget).
- Strategy-specific **preamble**, **golden path**, and **output schema**.
- Shared **scenario envelope** types (crux assumptions, signals, probability, sensitivities).

### Agent types (v1)

| Worker name | Strategy | Submit tool |
|-------------|----------|-------------|
| `manual_scenario` | `manual` | `submit_scenario_assumptions` |
| `epochs_scenario` | `epochs` / `epochs_shocks` | `submit_scenario_assumptions` |

All agents submit the same top-level shape; `projection_strategy` and `strategy_payload` differ.

### Scenario envelope (shared)

Every strategy must produce:

```json
{
  "name": "…",
  "stance": "bullish | neutral | bearish | mixed",
  "probability": 0.25,
  "description": "…",
  "assumption_summary": "…",
  "projection_strategy": "epochs",
  "strategy_rationale": "Two regime shifts: scarcity then normalization.",
  "strategy_payload": { },
  "crux_assumptions": [ ],
  "sensitivities": [ ],
  "confirming_signals": [ ],
  "breaking_signals": [ ]
}
```

Strategy-specific assumptions live in `strategy_payload`. Compiled periods are **not** agent output in the primary path — the compiler generates them.

### ScenarioPlanner

Lightweight planner runs once per scenario-generation lane. Inputs:

- Narrative map and crux candidates
- Financial mechanics experiments (purpose: `forward_projection`, `scenario_validation`)
- Historical analogues
- Baseline fundamentals and data gaps
- Watch items

Output: `scenario_plan[]`:

```json
{
  "scenarios": [
    {
      "name_hint": "Capacity Unlock Supercycle",
      "stance": "bullish",
      "strategy": "epochs",
      "rationale": "Crux resolution implies two distinct growth regimes."
    },
    {
      "name_hint": "Financing Stress",
      "stance": "bearish",
      "strategy": "manual",
      "rationale": "Simple downside grid sufficient; event path is one-dimensional."
    }
  ]
}
```

**Routing heuristics (deterministic first, LLM override second):**

```text
IF cruxes emphasize units / capacity / penetration / ASP
  → driver (or manual until driver ships)

ELIF high-confidence analogue with documented phase shape
  → analogue (or epochs informed by analogue phase lengths)

ELIF ≥2 crux phase shifts OR regime language in narrative map
  → epochs

ELIF event-driven with discrete watch-item triggers
  → epochs_shocks

ELSE
  → manual (default fallback)
```

Planner does not need a heavy model; rules + short LLM justification is enough. **Route per scenario**, not per ticker — one report may mix strategies.

### Fan-out

Same pattern as financial mechanics fan-out:

1. Planner writes `scenario_plan` to workspace.
2. Lane dispatches N scenario agents in parallel (bounded concurrency).
3. Each agent writes one `scenario_assumptions` row + `strategy_payload`.
4. Compiler job expands all scenarios to `scenario_periods`.
5. `generate_report` / Lane 7 runs `ScenarioCalculator` as today.

---

## ScenarioCompiler

### Responsibility

Convert `(projection_strategy, strategy_payload, baseline, compile_options)` → ordered `ScenarioPeriodInput[]`.

### Interface (conceptual)

```rust
trait ScenarioCompiler {
    fn strategy_id(&self) -> &'static str;
    fn validate_payload(&self, payload: &Value, ctx: &CompileContext) -> Result<(), CompileError>;
    fn compile(&self, payload: &Value, ctx: &CompileContext) -> Result<Vec<ScenarioPeriodInput>, CompileError>;
}
```

`CompileContext` includes:

- Baseline revenue, shares, margin, EPS
- Default period labels and offsets
- Internal grid resolution (default: monthly)
- Optional analogue rows, experiment outputs
- Compiler version string (for recompile on rule changes)

### Compile options

```json
{
  "period_labels": ["Current", "+6 months", "+12 months", "+24 months", "+36 months"],
  "internal_resolution": "monthly",
  "compiler_version": "1.0.0"
}
```

### Shared post-compile steps

After strategy-specific expansion, all paths run:

1. **Valuation blend policy** (see below) — populate `blend_ps_weight` / `blend_pe_weight` if not set in payload.
2. **Multiple consistency check** — warn or adjust if `P/E` and `P/S` imply divergent prices given projected margin.
3. **Shared compile validator** — enforce `ScenarioCalculator` input requirements.

### Recompilation

When compiler rules change, re-run compile from persisted `strategy_payload` without re-invoking LLM agents. Store `compiled_at` and `compiler_version` on each scenario.

---

## Valuation blend policy

Blend weights should not be a free LLM choice. The calculator (or compile step) applies a **tiered policy**:

### Tier 0 — Hard gates

```text
IF EPS ≤ 0 OR EPS below floor
  → P/S only (w_pe = 0)
```

Matches existing `projection_model.py` behavior for non-positive EPS.

### Tier 1 — Earnings reliability score

Deterministic score from workspace facts:

| Signal | Effect on P/E weight |
|--------|----------------------|
| EPS ≤ 0 | 0 |
| Few recent positive EPS quarters | Lower |
| High net margin volatility | Lower |
| Fast revenue growth + ramping margin | Lower |
| Stable positive EPS (8+ quarters) | Higher |
| Known distorted TTM (one-times) | Lower |

```text
w_pe = clamp(earnings_reliability, 0, 1)
w_ps = 1 - w_pe
```

### Tier 2 — Horizon adjustment

P/E weight increases toward terminal report period:

```text
w_pe(t) = w_pe_base × (t / T)^α
```

Early periods weight revenue multiples; later periods weight earnings multiples for growth names transitioning to profitability.

### Tier 3 — Multiple consistency (optional)

Algebraic link:

```text
P/E_implied ≈ P/S / net_margin
```

If agent-supplied P/E and P/S diverge beyond tolerance, compiler may:

- Derive secondary multiple from primary + margin, or
- Emit `quality_flag: multiple_inconsistency` for the report limitations section.

### Tier 4 — Inverse-variance (future)

Weight by historical multiple dispersion from `fundamental_observations` (quarterly P/S and P/E bands). Higher volatility → lower weight.

**Division of labor:** agents set **multiple bands**; calculator sets **blend weights** unless user explicitly overrides in manual strategy.

---

## Data model

### `scenario_assumptions` extensions

```sql
ALTER TABLE scenario_assumptions ADD COLUMN projection_strategy TEXT NOT NULL DEFAULT 'manual';
ALTER TABLE scenario_assumptions ADD COLUMN strategy_payload TEXT;  -- JSON
ALTER TABLE scenario_assumptions ADD COLUMN strategy_rationale TEXT;
ALTER TABLE scenario_assumptions ADD COLUMN compiler_version TEXT;
ALTER TABLE scenario_assumptions ADD COLUMN compiled_at TEXT;
```

`strategy_payload` holds epochs, shocks, drivers, etc. `scenario_periods` remains the compiled grid.

### `scenario_periods` semantics

- **Authoring strategies:** rows are **compiler output**, marked `source_note` or a future `row_kind = 'compiled'`.
- **Manual strategy:** rows may be authored directly; compiler is pass-through.
- Derived calculation outputs (post-`ScenarioCalculator`) should remain separate from assumptions per [database structure analysis](../qa/06-06-003-database-structure-analysis.md).

### Optional: `scenario_plan` table

```sql
CREATE TABLE scenario_plan (
  id INTEGER PRIMARY KEY,
  scenario_order INTEGER NOT NULL,
  name_hint TEXT,
  stance TEXT,
  projection_strategy TEXT NOT NULL,
  rationale TEXT,
  assigned_worker TEXT,
  status TEXT  -- planned | drafted | compiled | failed
);
```

Enables fan-out tracking and partial reruns.

### Report payload

Add to each scenario in compiled report JSON:

```json
{
  "projection_strategy": "epochs",
  "strategy_rationale": "…",
  "strategy_summary": {
    "epoch_count": 3,
    "shock_count": 1
  },
  "blend_policy": "earnings_reliability_v1"
}
```

HTML report may show a badge: “3-epoch model” vs “Manual assumptions.”

---

## Validation

### Layer 1 — Strategy validation (per compiler)

**Epochs:**

- 1–4 epochs covering the projection window without gaps
- Each epoch has `revenue_growth_annual` and P/S median
- Shocks ≤ 2; each references crux or watch item
- Epoch boundaries snap to allowed anchors (month offsets from current)

**Manual:**

- At least one period
- Growth jumps > threshold require `assumption_summary` justification flag

**Driver (future):**

- `units × price` reconciles to revenue within tolerance

### Layer 2 — Compile validation (shared)

- Baseline revenue and diluted shares present
- Every compiled period has revenue or revenue growth
- P/S median present per current contract
- Blend weights sum to positive value after normalization
- Valuation bands ordered: low ≤ median ≤ high

### Layer 3 — ScenarioCalculator validation (unchanged)

Per [scenario-calculator.md](../contracts/scenario-calculator.md).

### Agent resubmit loop

Same pattern as `submit_crux_triage` / `submit_mechanics_experiments`: validation errors return structured messages; agent fixes and resubmits within step budget.

---

## Integration with existing pipeline

| Component | Change |
|-----------|--------|
| **Worker Lane 6** | Add planner + scenario fan-out; persist `strategy_payload` |
| **Worker Lane 7** | Run compiler before `ScenarioCalculator` if periods not yet compiled |
| **`generate_report.rs`** | Invoke compiler; apply blend policy; existing period load path otherwise unchanged |
| **`projection_model.py`** | Add `compile_strategy()` entry point; Python parity for skill workflow |
| **Skills** | May continue using `manual` via JSON; optional epochs payload in `scenario-assumptions.json` |
| **Monte Carlo** | No change if terminal bands come from same period outputs |
| **Report HTML** | Display strategy metadata; optional epoch timeline chart (future) |

---

## Phased rollout

### Phase 1 — Contract and compiler (no new agents)

- Define `projection_strategy` enum and JSON schemas for `manual` and `epochs`
- Implement `ScenarioCompiler` in Rust (and Python wrapper for skills)
- Extend `scenario_assumptions` schema
- Deterministic blend policy in compile step
- Tests: epoch expansion, shock application, period sampling, blend gates

### Phase 2 — Scenario agents

- `ScenarioPlanner` worker (rules + light LLM)
- `ManualScenarioAgent` — largely current prompt, structured submit
- `EpochsScenarioAgent` — epoch/shock authoring
- Fan-out lane wiring + `scenario_plan` persistence

### Phase 3 — Extensions

- `DriverScenarioAgent`, `AnalogueScenarioAgent`
- User edit: modify `strategy_payload` → recompile
- Optional epoch timeline visualization in report
- Probabilistic shocks in Monte Carlo (branching within scenario)

### Phase 4 — Quality and refresh

- Stale scenario flags when cruxes or baseline change
- Recompile-all without LLM when compiler version bumps
- QA gates: epoch count, driver reconciliation, multiple consistency flags

---

## Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Planner picks wrong strategy | `manual` fallback; log `strategy_rationale`; QA review of strategy distribution |
| Prompt fragmentation | Shared envelope types; one submit tool; one compiler module |
| Over-engineered epoch models | Cap epochs at 4, shocks at 2; require crux linkage |
| Incomparable scenarios in Monte Carlo | All strategies compile to identical period schema |
| Compiler/agent drift | Version `compiler_version`; recompile from payload |
| Lost user editability | Persist `strategy_payload`; support edit → recompile |
| False sophistication | Quality flags; limitations section surfaces inconsistency |

---

## Open questions

1. **Should manual strategy allow direct period edits after compile?** Recommendation: yes for manual; epoch edits should mutate `strategy_payload` then recompile.
2. **Monthly vs quarterly internal grid?** Recommendation: monthly default; configurable.
3. **Where does blend policy live — compile step or ScenarioCalculator?** Recommendation: compile step sets weights on period inputs; calculator normalizes as today.
4. **Probabilistic shocks in Monte Carlo v1?** Recommendation: defer; shocks deterministic in compiled path first.
5. **Segment-sum as separate strategy or epochs per segment?** Recommendation: defer; use `segment_sum` ID when needed.
6. **User-facing strategy override in v1?** Recommendation: no; agent/planner only.

---

## Example: epochs payload → compiled periods

**Strategy payload (agent output):**

```json
{
  "epochs": [
    {
      "label": "Scarcity premium",
      "start_offset_months": 0,
      "end_offset_months": 18,
      "revenue_growth_annual": 0.32,
      "net_margin": { "start": 0.18, "end": 0.18 },
      "ps_multiple": { "low": 10, "median": 13, "high": 16 }
    },
    {
      "label": "Normalization",
      "start_offset_months": 18,
      "end_offset_months": 36,
      "revenue_growth_annual": 0.16,
      "net_margin": { "start": 0.18, "end": 0.24 },
      "ps_multiple": { "low": 7, "median": 9, "high": 11 },
      "pe_multiple": { "low": 22, "median": 26, "high": 32 }
    }
  ],
  "shocks": [
    {
      "label": "Hyperscaler capacity contract",
      "offset_months": 9,
      "type": "level_shift",
      "revenue_multiplier": 1.10,
      "watch_item_ref": "Major cloud capex commitments"
    }
  ]
}
```

**Compiler output:** `scenario_periods` rows at `+6m`, `+12m`, `+24m`, `+36m` with interpolated revenue, margins, multiples, and calculator-computed blend weights — fed to `ScenarioCalculator` unchanged.

---

## Success criteria

- [ ] At least two strategies (`manual`, `epochs`) compile to valid `scenario_periods` and pass `generate_report` gates.
- [ ] Blend weights are computed deterministically for non-manual overrides.
- [ ] Report shows `projection_strategy` and `strategy_rationale` per scenario.
- [ ] Recompile from `strategy_payload` reproduces periods without LLM.
- [ ] Mixed-strategy reports (e.g. bull=epochs, bear=manual) generate valid Monte Carlo output.
- [ ] Unit tests cover epoch gaps, shock timing, EPS≤0 blend gate, and compile validator errors.

---

## References

- Current period roll-forward: `src/tasks/generate_report.rs`, `.agents/skills/stock-narrative-research-skill/scripts/projection_model.py`
- Worker mode precedent: `src/agents/financial_model_explorer/`
- Orchestration boundary: `docs/02-orchestration-styles-comparison.md` — “Workers should not know whether they are being called by a skill, a linear phase, or a blackboard loop.”
- Narrative regimes: `docs/candidate-product-directions/concept-1-narrative-historical-focus/00-product-brief.md`
- Event-driven period alternatives: `docs/candidate-product-directions/concept-2-narrative-scenario-model-focus/00-product-brief.md`
