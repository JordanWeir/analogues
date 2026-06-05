#!/usr/bin/env python3
"""Calculate simple scenario-conditioned projection tables.

Input assumptions are intentionally explicit. The model is transparent v0.1
math, not a forecast engine.
"""

from __future__ import annotations

import argparse
import json
import random
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


PROJECTION_NOTE = (
    "These scenario projections are illustrative and assumption-driven. They are not "
    "predictions, price targets, or investment advice. They show how different narrative "
    "outcomes could translate into financial assumptions and valuation ranges."
)
DEFAULT_MONTE_CARLO_ITERATIONS = 10_000
DEFAULT_MONTE_CARLO_BINS = 30
DEFAULT_MONTE_CARLO_SEED = 42
P10_P90_Z_SCORE = 1.2815515655446004


def main() -> int:
    parser = argparse.ArgumentParser(description="Build scenario projection output from assumptions JSON.")
    parser.add_argument("input", nargs="?", help="Assumptions JSON file")
    parser.add_argument("--output", "-o", help="Write scenario-data JSON to this path instead of stdout")
    parser.add_argument("--sample", action="store_true", help="Print a sample assumptions file")
    args = parser.parse_args()

    if args.sample:
        write_json(sample_assumptions(), args.output)
        return 0

    if not args.input:
        parser.error("input is required unless --sample is used")

    assumptions = json.loads(Path(args.input).read_text(encoding="utf-8"))
    result = build_projection(assumptions)
    write_json(result, args.output)
    return 0


def build_projection(data: dict[str, Any]) -> dict[str, Any]:
    baseline = data.get("baseline") or {}
    baseline_revenue = require_number(baseline, "revenue")
    baseline_shares = require_number(baseline, "diluted_shares")
    baseline_margin = optional_number(baseline.get("net_margin"))
    baseline_eps = optional_number(baseline.get("eps"))

    scenarios = [build_scenario(item, baseline_revenue, baseline_shares, baseline_margin, baseline_eps) for item in data.get("scenarios", [])]
    if not scenarios:
        raise ValueError("At least one scenario is required.")
    monte_carlo = build_monte_carlo(data, scenarios)

    return {
        "company": data.get("company") or "",
        "ticker": data.get("ticker") or "",
        "currency": data.get("currency") or "USD",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "projection_note": PROJECTION_NOTE,
        "base_year": data.get("base_year") or "Current",
        "current_price": optional_number(data.get("current_price")),
        "baseline": {
            "revenue": baseline_revenue,
            "diluted_shares": baseline_shares,
            "net_margin": baseline_margin,
            "eps": baseline_eps,
        },
        "scenarios": scenarios,
        "monte_carlo": monte_carlo,
        "source_notes": data.get("source_notes", []),
    }


def build_scenario(
    scenario: dict[str, Any],
    baseline_revenue: float,
    baseline_shares: float,
    baseline_margin: float | None,
    baseline_eps: float | None,
) -> dict[str, Any]:
    periods = []
    previous_revenue = baseline_revenue
    previous_shares = baseline_shares
    previous_margin = baseline_margin
    previous_eps = baseline_eps

    for period in scenario.get("periods", []):
        revenue_growth = optional_number(period.get("revenue_growth"))
        revenue = optional_number(period.get("revenue"))
        if revenue is None:
            if revenue_growth is None:
                raise ValueError(f"Scenario '{scenario.get('name', '')}' period '{period.get('label', '')}' needs revenue or revenue_growth.")
            revenue = previous_revenue * (1 + revenue_growth)
        elif revenue_growth is None and previous_revenue:
            revenue_growth = (revenue / previous_revenue) - 1

        diluted_shares = optional_number(period.get("diluted_shares")) or previous_shares
        gross_margin = optional_number(period.get("gross_margin"))
        operating_margin = optional_number(period.get("operating_margin"))
        net_margin = optional_number(period.get("net_margin"))
        if net_margin is None:
            net_margin = previous_margin

        net_income = optional_number(period.get("net_income"))
        if net_income is None and net_margin is not None:
            net_income = revenue * net_margin

        eps = optional_number(period.get("eps"))
        if eps is None and net_income is not None:
            eps = net_income / diluted_shares
        if eps is None:
            eps = previous_eps

        net_income_growth = growth_rate(net_income, None)
        if previous_margin is not None:
            previous_net_income = previous_revenue * previous_margin
            net_income_growth = growth_rate(net_income, previous_net_income)
        eps_growth = growth_rate(eps, previous_eps)
        revenue_per_share = revenue / diluted_shares
        ps_multiple = band(period.get("ps_multiple"))
        pe_multiple = nullable_band(period.get("pe_multiple"))
        blend_weights = weights(period.get("blend_weights"))
        ps_price = apply_multiple(revenue_per_share, ps_multiple)
        pe_price = apply_multiple(eps, pe_multiple) if eps is not None and eps > 0 and pe_multiple else None
        blended_price = blend_bands(ps_price, pe_price, blend_weights)

        periods.append(
            {
                "label": period.get("label") or f"Period {len(periods) + 1}",
                "revenue_growth": round_optional(revenue_growth),
                "revenue": round_float(revenue),
                "diluted_shares": round_float(diluted_shares),
                "revenue_per_share": round_float(revenue_per_share),
                "gross_margin": gross_margin,
                "operating_margin": operating_margin,
                "net_margin": net_margin,
                "net_income": round_optional(net_income),
                "net_income_growth": round_optional(net_income_growth),
                "eps": round_optional(eps),
                "eps_growth": round_optional(eps_growth),
                "ps_multiple": ps_multiple,
                "pe_multiple": pe_multiple,
                "blend_weights": blend_weights,
                "ps_implied_price": ps_price,
                "pe_implied_price": pe_price,
                "blended_price": blended_price,
            }
        )

        previous_revenue = revenue
        previous_shares = diluted_shares
        previous_margin = net_margin
        previous_eps = eps

    if not periods:
        raise ValueError(f"Scenario '{scenario.get('name', '')}' needs at least one period.")

    return {
        "name": scenario.get("name") or "Unnamed scenario",
        "stance": scenario.get("stance") or "",
        "probability": optional_number(scenario.get("probability")),
        "description": scenario.get("description") or "",
        "crux_assumptions": scenario.get("crux_assumptions", []),
        "assumption_summary": scenario.get("assumption_summary") or "",
        "sensitivities": scenario.get("sensitivities", []),
        "confirming_signals": scenario.get("confirming_signals", []),
        "breaking_signals": scenario.get("breaking_signals", []),
        "periods": periods,
    }


def build_monte_carlo(data: dict[str, Any], scenarios: list[dict[str, Any]]) -> dict[str, Any]:
    config = data.get("monte_carlo") if isinstance(data.get("monte_carlo"), dict) else {}
    iterations = positive_int(config.get("iterations"), DEFAULT_MONTE_CARLO_ITERATIONS)
    bins = positive_int(config.get("bins"), DEFAULT_MONTE_CARLO_BINS)
    seed = positive_int(config.get("seed"), DEFAULT_MONTE_CARLO_SEED, allow_zero=True)
    specs = scenario_sampling_specs(scenarios)
    if not specs:
        return {
            "iterations": iterations,
            "seed": seed,
            "bins": bins,
            "histogram": [],
            "scenario_probabilities": [],
            "summary": {},
            "methodology": "No terminal price bands were available for Monte Carlo sampling.",
        }

    rng = random.Random(seed)
    cumulative: list[tuple[float, dict[str, Any]]] = []
    running = 0.0
    for spec in specs:
        running += spec["probability"]
        cumulative.append((running, spec))

    samples: list[float] = []
    counts = {spec["name"]: 0 for spec in specs}
    for _ in range(iterations):
        pick = rng.random()
        spec = cumulative[-1][1]
        for boundary, candidate in cumulative:
            if pick <= boundary:
                spec = candidate
                break
        price = sample_from_price_band(rng, spec["band"])
        samples.append(price)
        counts[spec["name"]] += 1

    return {
        "iterations": iterations,
        "seed": seed,
        "bins": bins,
        "price_field": "terminal blended price, falling back to P/S or P/E implied price",
        "probability_basis": "Scenario probabilities are normalized across scenarios with terminal price bands; equal weights are used only when no positive probabilities are supplied.",
        "normal_distribution_basis": "Each scenario's low / median / high terminal band is treated as an approximate P10 / P50 / P90 normal distribution, floored at zero.",
        "summary": distribution_summary(samples),
        "histogram": histogram(samples, bins),
        "scenario_probabilities": [
            {
                "name": spec["name"],
                "input_probability": round_optional(spec.get("input_probability")),
                "normalized_probability": round_float(spec["probability"]),
                "sample_count": counts[spec["name"]],
                "observed_probability": round_float(counts[spec["name"]] / iterations),
            }
            for spec in specs
        ],
    }


def scenario_sampling_specs(scenarios: list[dict[str, Any]]) -> list[dict[str, Any]]:
    specs: list[dict[str, Any]] = []
    for scenario in scenarios:
        band_value = terminal_price_band(scenario)
        if not usable_band(band_value):
            continue
        input_probability = optional_number(scenario.get("probability"))
        specs.append(
            {
                "name": scenario.get("name") or "Unnamed scenario",
                "input_probability": input_probability,
                "raw_probability": input_probability if input_probability is not None and input_probability > 0 else 0.0,
                "band": band_value,
            }
        )
    if not specs:
        return []

    total_probability = sum(spec["raw_probability"] for spec in specs)
    if total_probability <= 0:
        equal_probability = 1 / len(specs)
        for spec in specs:
            spec["probability"] = equal_probability
        return specs

    for spec in specs:
        spec["probability"] = spec["raw_probability"] / total_probability
    return specs


def terminal_price_band(scenario: dict[str, Any]) -> dict[str, float | None] | None:
    periods = scenario.get("periods") or []
    if not periods:
        return None
    terminal = periods[-1]
    return terminal.get("blended_price") or terminal.get("ps_implied_price") or terminal.get("pe_implied_price")


def usable_band(value: Any) -> bool:
    return (
        isinstance(value, dict)
        and optional_number(value.get("low")) is not None
        and optional_number(value.get("median")) is not None
        and optional_number(value.get("high")) is not None
    )


def sample_from_price_band(rng: random.Random, value: dict[str, float | None]) -> float:
    low = require_number(value, "low")
    median = require_number(value, "median")
    high = require_number(value, "high")
    spread = max(abs(median - low), abs(high - median))
    if spread == 0:
        return max(0.0, round_float(median))
    sigma = spread / P10_P90_Z_SCORE
    return max(0.0, round_float(rng.gauss(median, sigma)))


def distribution_summary(samples: list[float]) -> dict[str, float]:
    sorted_samples = sorted(samples)
    mean_value = sum(samples) / len(samples)
    variance = sum((sample - mean_value) ** 2 for sample in samples) / len(samples)
    return {
        "min": round_float(sorted_samples[0]),
        "p10": round_float(percentile(sorted_samples, 0.10)),
        "p25": round_float(percentile(sorted_samples, 0.25)),
        "median": round_float(percentile(sorted_samples, 0.50)),
        "mean": round_float(mean_value),
        "p75": round_float(percentile(sorted_samples, 0.75)),
        "p90": round_float(percentile(sorted_samples, 0.90)),
        "max": round_float(sorted_samples[-1]),
        "stdev": round_float(variance**0.5),
    }


def percentile(sorted_values: list[float], fraction: float) -> float:
    if len(sorted_values) == 1:
        return sorted_values[0]
    position = (len(sorted_values) - 1) * fraction
    lower_index = int(position)
    upper_index = min(lower_index + 1, len(sorted_values) - 1)
    weight = position - lower_index
    return sorted_values[lower_index] * (1 - weight) + sorted_values[upper_index] * weight


def histogram(samples: list[float], bins: int) -> list[dict[str, float | int]]:
    minimum = min(samples)
    maximum = max(samples)
    if minimum == maximum:
        return [
            {
                "low": round_float(minimum),
                "high": round_float(maximum),
                "midpoint": round_float(minimum),
                "count": len(samples),
                "probability": 1.0,
            }
        ]
    width = (maximum - minimum) / bins
    counts = [0] * bins
    for sample in samples:
        index = min(int((sample - minimum) / width), bins - 1)
        counts[index] += 1
    return [
        {
            "low": round_float(minimum + index * width),
            "high": round_float(minimum + (index + 1) * width),
            "midpoint": round_float(minimum + (index + 0.5) * width),
            "count": count,
            "probability": round_float(count / len(samples)),
        }
        for index, count in enumerate(counts)
    ]


def positive_int(value: Any, default: int, *, allow_zero: bool = False) -> int:
    number = optional_number(value)
    if number is None:
        return default
    integer = int(number)
    if allow_zero and integer >= 0:
        return integer
    return integer if integer > 0 else default


def band(value: Any) -> dict[str, float]:
    parsed = nullable_band(value)
    if parsed is None:
        raise ValueError("P/S multiple band is required for each period.")
    return parsed


def nullable_band(value: Any) -> dict[str, float] | None:
    if value is None:
        return None
    if isinstance(value, (int, float)):
        number = float(value)
        return {"low": number, "median": number, "high": number}
    if not isinstance(value, dict):
        raise ValueError(f"Expected band object, got {type(value).__name__}")
    median = require_number(value, "median")
    return {
        "low": optional_number(value.get("low")) if value.get("low") is not None else median,
        "median": median,
        "high": optional_number(value.get("high")) if value.get("high") is not None else median,
    }


def weights(value: Any) -> dict[str, float]:
    if value is None:
        return {"ps": 1.0, "pe": 0.0}
    ps_weight = optional_number(value.get("ps")) if isinstance(value, dict) else None
    pe_weight = optional_number(value.get("pe")) if isinstance(value, dict) else None
    if ps_weight is None and pe_weight is None:
        return {"ps": 1.0, "pe": 0.0}
    if ps_weight is None:
        ps_weight = 1 - pe_weight
    if pe_weight is None:
        pe_weight = 1 - ps_weight
    total = ps_weight + pe_weight
    if total <= 0:
        raise ValueError("Blend weights must sum to a positive value.")
    return {"ps": ps_weight / total, "pe": pe_weight / total}


def apply_multiple(base_value: float | None, multiple: dict[str, float] | None) -> dict[str, float | None] | None:
    if base_value is None or multiple is None:
        return None
    return {
        "low": round_float(base_value * multiple["low"]),
        "median": round_float(base_value * multiple["median"]),
        "high": round_float(base_value * multiple["high"]),
    }


def blend_bands(
    ps_price: dict[str, float | None] | None,
    pe_price: dict[str, float | None] | None,
    blend_weights: dict[str, float],
) -> dict[str, float | None] | None:
    if not ps_price and not pe_price:
        return None
    if ps_price and not pe_price:
        return ps_price
    if pe_price and not ps_price:
        return pe_price

    assert ps_price is not None
    assert pe_price is not None
    return {
        key: round_optional((ps_price[key] or 0) * blend_weights["ps"] + (pe_price[key] or 0) * blend_weights["pe"])
        for key in ("low", "median", "high")
    }


def require_number(data: dict[str, Any], key: str) -> float:
    value = optional_number(data.get(key))
    if value is None:
        raise ValueError(f"Missing required numeric field: {key}")
    return value


def optional_number(value: Any) -> float | None:
    if isinstance(value, bool) or value is None:
        return None
    if isinstance(value, (int, float)):
        return float(value)
    return None


def growth_rate(value: float | None, previous_value: float | None) -> float | None:
    if value is None or previous_value is None or previous_value == 0:
        return None
    return (value / previous_value) - 1


def round_optional(value: float | None) -> float | None:
    return round_float(value) if value is not None else None


def round_float(value: float) -> float:
    return round(float(value), 6)


def write_json(payload: dict[str, Any], output_path: str | None) -> None:
    text = json.dumps(payload, indent=2, sort_keys=True)
    if output_path:
        path = Path(output_path)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text + "\n", encoding="utf-8")
    else:
        print(text)


def sample_assumptions() -> dict[str, Any]:
    return {
        "company": "Example Co.",
        "ticker": "EXM",
        "currency": "USD",
        "current_price": 100.0,
        "base_year": "Current",
        "baseline": {
            "revenue": 1000000000,
            "diluted_shares": 100000000,
            "net_margin": 0.1,
            "eps": 1.0,
        },
        "monte_carlo": {"iterations": 10000, "seed": 42, "bins": 30},
        "scenarios": [
            {
                "name": "Product Adoption Flywheel",
                "stance": "bullish",
                "probability": 0.35,
                "description": "The highest-impact adoption and margin cruxes settle positively together.",
                "crux_assumptions": [
                    {
                        "crux": "Does demand stay durable?",
                        "assumption": "Customer adoption remains strong through the scenario window.",
                        "impact": "Revenue growth stays above the baseline trend.",
                    },
                    {
                        "crux": "Can margins expand?",
                        "assumption": "Scale and mix improve net margin.",
                        "impact": "EPS grows faster than revenue.",
                    },
                ],
                "assumption_summary": "Revenue growth, net margin, and valuation multiples all improve because two major cruxes resolve favorably.",
                "periods": [
                    {
                        "label": "+12 months",
                        "revenue_growth": 0.2,
                        "net_margin": 0.12,
                        "diluted_shares": 100000000,
                        "ps_multiple": {"low": 5, "median": 7, "high": 9},
                        "pe_multiple": {"low": 20, "median": 25, "high": 30},
                        "blend_weights": {"ps": 0.5, "pe": 0.5},
                    },
                    {
                        "label": "+24 months",
                        "revenue_growth": 0.18,
                        "net_margin": 0.14,
                        "diluted_shares": 100000000,
                        "ps_multiple": {"low": 5, "median": 7.5, "high": 10},
                        "pe_multiple": {"low": 20, "median": 27, "high": 34},
                        "blend_weights": {"ps": 0.45, "pe": 0.55},
                    },
                ],
                "confirming_signals": ["Revenue guide raises", "Margins expand"],
                "breaking_signals": ["Customer demand slows", "Gross margin compresses"],
                "sensitivities": ["Revenue growth", "Net margin", "P/S multiple"],
            }
        ],
        "source_notes": ["Sample assumptions only."],
    }


if __name__ == "__main__":
    sys.exit(main())
