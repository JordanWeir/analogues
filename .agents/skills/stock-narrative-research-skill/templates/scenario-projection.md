# Scenario Projection Template

Use this template to turn narrative scenarios into explicit assumptions, illustrative implied price bands, and probability-weighted Monte Carlo distribution data.

The calculator chooses scenarios according to normalized scenario probabilities, treats each scenario's terminal low / median / high implied price band as an approximate P10 / P50 / P90 normal distribution, and samples a non-negative terminal price.

## Scenario Set

Use 4-6 scenarios derived from the company's narrative cruxes. Include at least one bullish, one neutral, and one bearish scenario, then add company-specific mixed or event-path scenarios as needed.

Do not reuse the same scenario names across every report. Use labels that describe the actual narrative path, such as "HBM Scarcity Supercycle", "Copilot Becomes Table Stakes", or "Working Capital Crunch".

## Scenario Card

Scenario name:
[Name]

Stance:
[bullish / neutral / bearish / mixed]

Conditional probability:
[Decimal probability such as 0.25. Scenario probabilities should usually sum to 1.0 before normalization.]

Narrative description:
[If this scenario plays out, what changes?]

Crux assumptions:

- Crux: [Narrative crux from narrative_map.cruxes]
  - Assumption: [How this crux is settled in this scenario]
  - Impact: [How it affects revenue, earnings, margin, or multiples]

Operating assumptions:

- Revenue growth: [low/median/high or period path]
- Gross margin: [assumption]
- Operating margin / net margin: [assumption]
- Diluted share count: [assumption]
- EPS path: [assumption]

Valuation assumptions:

- P/S low / median / high: [values]
- P/E low / median / high: [values]
- Blend weighting, if used: [P/S weight + P/E weight = 1]

Implied outputs:

- Revenue per share: [value]
- EPS: [value]
- P/S implied price range: [low / median / high]
- P/E implied price range: [low / median / high]
- Blended implied price range: [low / median / high]

Key sensitivities:

- [Variable that most changes the output]

What would confirm this scenario:

- [Signal]

What would break this scenario:

- [Signal]

## Simple Projection Math

```text
Revenue_t = Revenue_(t-1) * (1 + Revenue Growth Rate_t)
Revenue Per Share_t = Revenue_t / Diluted Shares_t
Net Income_t = Revenue_t * Net Margin_t
EPS_t = Net Income_t / Diluted Shares_t
P/S Implied Price_t = Revenue Per Share_t * P/S Multiple_t
P/E Implied Price_t = EPS_t * P/E Multiple_t
Blended Price_t = (P/S Implied Price_t * P/S Weight_t) + (P/E Implied Price_t * P/E Weight_t)
```

Default periods:

- Current
- +6 months
- +12 months
- +24 months
- +36 months

If EPS is negative, near-zero, or distorted, do not rely primarily on P/E. Use P/S, EV/Sales, gross profit multiple, EBITDA multiple, book value, NAV, or a domain-specific model if appropriate, and label the limitation.

## Scenario Assumptions JSON Shape

`scripts/scenario_calculator.py` accepts this minimal shape. `scripts/projection_model.py` is retained as a compatibility wrapper, but new workflow docs should use `scenario_calculator.py` or `report_pipeline.py project`.

```json
{
  "company": "Example Co.",
  "ticker": "EXM",
  "currency": "USD",
  "current_price": 100.0,
  "base_year": "Current",
  "baseline": {
    "revenue": 1000000000,
    "diluted_shares": 100000000,
    "net_margin": 0.1,
    "eps": 1.0
  },
  "monte_carlo": {
    "iterations": 10000,
    "seed": 42,
    "bins": 30
  },
  "scenarios": [
    {
      "name": "Product Adoption Flywheel",
      "stance": "bullish",
      "probability": 0.35,
      "description": "Demand durability and margins both improve.",
      "assumption_summary": "Revenue growth, net margin, and multiples all improve because the main adoption and margin cruxes settle favorably.",
      "crux_assumptions": [
        {
          "crux": "Does demand stay durable?",
          "assumption": "Customer adoption remains strong through the scenario window.",
          "impact": "Revenue growth stays above the baseline trend."
        },
        {
          "crux": "Can margins expand?",
          "assumption": "Scale and mix improve net margin.",
          "impact": "EPS grows faster than revenue."
        }
      ],
      "periods": [
        {
          "label": "+12 months",
          "revenue_growth": 0.2,
          "net_margin": 0.12,
          "diluted_shares": 100000000,
          "ps_multiple": { "low": 5, "median": 7, "high": 9 },
          "pe_multiple": { "low": 20, "median": 25, "high": 30 },
          "blend_weights": { "ps": 0.5, "pe": 0.5 }
        }
      ],
      "confirming_signals": ["Revenue guide raises"],
      "breaking_signals": ["Gross margin compression"],
      "sensitivities": ["Revenue growth", "Net margin", "P/S multiple"]
    }
  ]
}
```
