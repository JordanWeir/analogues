# Contract Spec: ScenarioCalculator

## Overview

`ScenarioCalculator` converts persisted scenario assumptions and baseline financial facts into scenario-conditioned financial paths and valuation bands.

The seam is deterministic. It does not choose scenarios, probabilities, or cruxes. It reads approved baseline metrics and scenario period assumptions, validates them, performs roll-forward math, and returns structured outputs that can be persisted, sampled by Monte Carlo, and compiled into reports.

## Public Types / Traits

- `ScenarioCalculator`: service or module responsible for projection math.
- `ScenarioCalculationRequest`: stock identity, currency, baseline metrics, scenarios, and calculation options.
- `BaselineFinancials`: revenue, diluted shares, optional net margin, optional EPS, current price, period, source notes.
- `ScenarioInput`: scenario ID, name, stance, probability, description, assumption summary, crux assumptions, sensitivities, confirming signals, breaking signals, and ordered periods.
- `ScenarioPeriodInput`: label, revenue or revenue growth, diluted shares, margins, net income, EPS, valuation multiples, blend weights, source note.
- `ScenarioPath`: scenario identity plus ordered calculated period outputs.
- `ScenarioPeriodOutput`: calculated revenue, growth, shares, margins, net income, EPS, valuation multiples, implied price bands, blend weights, and source metadata.
- `ValuationBand`: low, median, high.
- `ScenarioCalculationError`: missing baseline, missing period input, invalid weights, invalid multiple, invalid ordering, unit mismatch.

## Operations

- `calculate(request) -> ScenarioCalculationResult`
- `validate_baseline(baseline)`
- `validate_scenario_inputs(scenarios)`
- `calculate_period(previous_period, period_input) -> ScenarioPeriodOutput`
- `apply_multiple(base_value, valuation_band) -> ValuationBand`
- `blend_bands(ps_band, pe_band, weights) -> ValuationBand`
- `summarize_terminal_bands(paths) -> Vec<ScenarioTerminalOutput>`

## Behavioral Rules

- Each scenario must contain at least one ordered period.
- Each period must provide either absolute revenue or revenue growth.
- Revenue growth is calculated from previous revenue when absolute revenue is supplied.
- Diluted shares carry forward from the prior period when omitted.
- Net margin carries forward when omitted.
- Net income may be supplied directly or derived from revenue and net margin.
- EPS may be supplied directly, derived from net income and diluted shares, or carried forward only when the inputs do not support a new value.
- P/S median is required for price projection unless another explicitly supported valuation method is introduced.
- P/E bands are optional and only used when EPS and P/E assumptions are available.
- Blend weights are normalized and must sum to a positive value before normalization.
- If only one valuation band is available, it is used directly.
- Low/median/high bands should remain ordered or be rejected before downstream Monte Carlo.
- Outputs preserve enough source notes and assumptions for report limitations and calculation entries.
- The calculator does not persist by itself unless wrapped by a repository or service that records calculation outputs.

## Error Semantics

- Missing baseline revenue returns a hard validation error.
- Missing baseline diluted shares returns a hard validation error.
- A period with neither revenue nor revenue growth returns a scenario-period validation error.
- Missing P/S median returns a scenario-period validation error.
- Zero or negative total blend weight returns an invalid-weights error.
- Negative shares, invalid margins, or nonsensical multiples should be rejected unless explicitly marked as an allowed stress case.
- A missing optional P/E band does not fail calculation if P/S valuation is valid.
- Errors identify scenario name/ID and period label where possible.

## Valid Examples

- A base period starts with baseline revenue and share count, then a scenario period supplies `revenue_growth = 0.08` and `ps_median = 6.0`.
- A scenario period supplies absolute revenue, so revenue growth is derived against the previous period.
- A period omits diluted shares, so previous diluted shares carry forward.
- P/S and P/E implied price bands are blended with normalized weights when both are available.
- P/S implied price is used directly when EPS is unavailable and no P/E band can be calculated.

## Invalid Examples

- A scenario period has no revenue and no revenue growth.
- A period has `blend_ps_weight = 0` and `blend_pe_weight = 0`.
- A report compiler recalculates scenario periods independently instead of consuming `ScenarioCalculator` output.
- A model worker writes final scenario price bands without deterministic recalculation.
- A calculator silently samples Monte Carlo distributions; that belongs to `MonteCarloEngine`.

## Required Tests

- Requires baseline revenue and diluted shares.
- Requires at least one period per scenario.
- Requires revenue or revenue growth for every period.
- Carries forward shares and margin when omitted.
- Derives revenue growth from absolute revenue.
- Derives net income and EPS when inputs allow.
- Requires P/S median under the current valuation method.
- Normalizes blend weights.
- Rejects non-positive total blend weights.
- Uses P/S-only bands when P/E cannot be calculated.
- Blends P/S and P/E bands when both are available.
- Returns errors with scenario and period context.

