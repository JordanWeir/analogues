# Contract Spec: MonteCarloEngine

## Overview

`MonteCarloEngine` samples scenario-conditioned terminal price distributions from calculated scenario outputs.

The seam is deterministic for a given seed, scenario set, probability set, and sampling configuration. It does not create scenarios or valuation bands. It consumes terminal price bands, normalizes scenario probabilities, samples prices, summarizes the distribution, and returns persistable histogram and probability records.

## Public Types / Traits

- `MonteCarloEngine`: service or module responsible for deterministic distribution sampling.
- `MonteCarloConfig`: iterations, seed, bins, price field, and sampling method.
- `ScenarioTerminalOutput`: scenario ID, name, optional input probability, terminal `ValuationBand`.
- `MonteCarloResult`: config, methodology metadata, summary statistics, histogram bins, scenario probabilities.
- `DistributionSummary`: min, p10, p25, median, mean, p75, p90, max, standard deviation.
- `HistogramBin`: bin order, low, high, midpoint, count, probability.
- `ScenarioProbabilityOutput`: scenario ID, name, input probability, normalized probability, sample count, observed probability.
- `MonteCarloError`: invalid config, no sampleable scenarios, invalid probability, invalid band, persistence failure.

## Operations

- `run(config, scenario_outputs) -> MonteCarloResult`
- `build_sampling_specs(scenario_outputs) -> Vec<SamplingSpec>`
- `normalize_probabilities(specs) -> Vec<SamplingSpec>`
- `sample_terminal_prices(config, specs) -> Vec<Sample>`
- `distribution_summary(samples) -> DistributionSummary`
- `histogram(samples, bins) -> Vec<HistogramBin>`
- `persist_result(store, result)`

## Behavioral Rules

- Iterations must be greater than zero.
- Bin count must be greater than zero.
- Seeded sampling must be deterministic.
- Only scenarios with terminal price bands are sampleable.
- Positive scenario probabilities are normalized across sampleable scenarios.
- Equal weights are used only when no sampleable scenario has a positive probability.
- Input probabilities are preserved alongside normalized probabilities.
- Each scenario's low, median, and high band is treated as an approximate P10, P50, and P90 normal distribution unless the method is changed explicitly.
- Sampled prices are floored at zero.
- Histograms cover the full sampled range.
- Single-price distributions produce one histogram bin.
- Summary statistics are computed from the sampled distribution, not from scenario medians.
- The engine records methodology fields so reports can disclose sampling assumptions.
- Persistence replaces prior Monte Carlo output for the run unless the caller requests versioned results.

## Error Semantics

- `iterations <= 0` returns invalid config.
- `bins <= 0` returns invalid config.
- A scenario band with missing low/median/high is not sampleable.
- A scenario band with nonsensical ordering should be rejected before sampling.
- Negative probabilities are treated as invalid input or ignored only if the contract explicitly marks that behavior; the preferred behavior is validation error.
- No sampleable scenarios returns an empty result only if the caller is compiling a limitation; otherwise it should be a readiness error.
- Persistence errors identify the target table or artifact.

## Valid Examples

- Three scenarios with probabilities `0.2`, `0.5`, and `0.3` sample according to those normalized weights.
- Three scenarios with missing or zero probabilities sample equally.
- A terminal band of `low = 50`, `median = 70`, `high = 100` is sampled as an approximate P10/P50/P90 distribution.
- Re-running the same config and scenario outputs with seed `42` produces the same summary and histogram.
- Persisting a new result clears old histogram and scenario-probability rows for that run.

## Invalid Examples

- Monte Carlo code deriving scenario revenue or EPS before sampling.
- Sampling from scenarios with no terminal price band.
- Using unseeded process randomness in report generation.
- Treating scenario probabilities as already normalized when some scenarios lack terminal bands.
- Hiding sampling methodology from the report payload.

## Required Tests

- Rejects zero iterations.
- Rejects zero bins.
- Produces deterministic results for the same seed.
- Normalizes positive probabilities across sampleable scenarios.
- Uses equal weighting when all probabilities are missing or zero.
- Excludes scenarios without terminal bands.
- Floors sampled prices at zero.
- Produces a single histogram bin for identical samples.
- Produces summary percentiles from sorted samples.
- Records scenario input probability, normalized probability, sample count, and observed probability.
- Replaces prior persisted Monte Carlo rows on persist.

