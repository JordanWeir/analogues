---
name: stock-narrative-research
description: Produces 90-minute stock narrative research reports with source packs, bull/bear narrative maps, scenario-conditioned projections, historical analogues, and chart-ready JSON/HTML artifacts. Use when analyzing a stock, comparing bull and bear narratives, building stock scenario projections, identifying historical analogues, or preparing a stock research memo.
---

# Stock Narrative Research

Use this skill to help a user understand a public company stock narrative without turning the output into investment advice.

## Use When

- The user asks to analyze a stock, ticker, or public company narrative.
- The user wants a 90-minute research report, stock memo, talk track, bull/bear debate, scenario projection, or historical analogue.
- The user asks what could move a stock up or down, or what assumptions the stock appears to depend on.

Do not use this skill for casual price checks, buy/sell/hold recommendations, portfolio allocation, tax/legal advice, options recommendations, high-frequency signals, or guaranteed predictions.

## Core Workflow

Use a ticker-specific working directory and treat section JSON files as the main work product. The agent supplies research judgment and assumptions; scripts scaffold, calculate, validate, compile, and render.

Run pipeline commands from the workspace root with the script path shown below. New runs default to `reports/stock-narrative-research/`; existing files under this skill's `outputs/` directory are legacy/reference artifacts, not the default place for new reports.

1. Identify the ticker, company, user goal, time horizon, and whether the user wants one-shot or guided mode.
2. Initialize a working directory:

```bash
python .agents/skills/stock-narrative-research-skill/scripts/report_pipeline.py MSFT init
```

3. Fetch starter public-data inputs:

```bash
python .agents/skills/stock-narrative-research-skill/scripts/report_pipeline.py MSFT fetch --workdir reports/stock-narrative-research/MSFT-YYYYMMDD-HHMMSS
```

4. If you edit or replace `raw/financials.json`, reseed baseline scenario scaffolding:

```bash
python .agents/skills/stock-narrative-research-skill/scripts/report_pipeline.py MSFT seed-scenarios --workdir reports/stock-narrative-research/MSFT-YYYYMMDD-HHMMSS
```

5. Build the source pack and claim table in `sections/sources.json` and `sections/claims.json`. Prefer primary sources for facts and commentary sources for interpretation.
6. Fill the remaining section JSON files: orientation, business model, why now, narrative map, financial snapshot, financial math, industry context, watch items, historical analogues, final talk track, and limitations.
7. Think deeply about `sections/scenario-assumptions.json`. This is the highest-impact judgment step. Use fetched baseline values, source-backed assumptions, dynamic crux-derived scenarios, conditional probabilities, and explicit scenario narratives.
8. Run the scenario calculator as an internal arithmetic step. It also runs a probability-weighted Monte Carlo simulation from the terminal scenario bands:

```bash
python .agents/skills/stock-narrative-research-skill/scripts/report_pipeline.py MSFT project --workdir reports/stock-narrative-research/MSFT-YYYYMMDD-HHMMSS
```

9. Validate, compile, and render the final HTML artifact:

```bash
python .agents/skills/stock-narrative-research-skill/scripts/report_pipeline.py MSFT validate --workdir reports/stock-narrative-research/MSFT-YYYYMMDD-HHMMSS
python .agents/skills/stock-narrative-research-skill/scripts/report_pipeline.py MSFT compile --workdir reports/stock-narrative-research/MSFT-YYYYMMDD-HHMMSS
python .agents/skills/stock-narrative-research-skill/scripts/report_pipeline.py MSFT render --workdir reports/stock-narrative-research/MSFT-YYYYMMDD-HHMMSS
```

10. Run `rubrics/final-report-checklist.md` before finalizing.

## Required Outputs

Each run should produce a directory like `reports/stock-narrative-research/MSFT-YYYYMMDD-HHMMSS/`:

- `raw/financials.json`: fetched starter data and gaps.
- `sections/*.json`: agent-authored section files.
- `generated/scenario-data.json`: calculator output from scenario assumptions, including Monte Carlo distribution data.
- `generated/report-data.json`: compiled canonical report data.
- `generated/report.html`: final rendered HTML artifact.

Markdown is optional. Do not make it the default final artifact.

## Projection Framing

Every report with projections must include this note:

```text
These scenario projections are illustrative and assumption-driven. They are not predictions, price targets, or investment advice. They show how different narrative outcomes could translate into financial assumptions and valuation ranges.
```

Use phrases like "scenario-conditioned implied range", "illustrative price band", "assumption-driven model", and "if this scenario plays out". Avoid "target price", "fair value", "expected return", "should trade at", "will go to", and "guaranteed upside".

## Scenario Construction

Do not default to the same scenario names across every report. Build 4-6 scenarios from the report's narrative cruxes, watch items, claim table, and historical analogues. Include at least one bullish, one neutral, and one bearish scenario, but let the remaining scenarios reflect the company-specific debate.

For each scenario in `sections/scenario-assumptions.json`:

- Set `stance` to `bullish`, `neutral`, `bearish`, or `mixed`.
- Set `probability` to the scenario's conditional likelihood as a decimal, such as `0.25`. Probabilities should usually sum to 1.0; the calculator normalizes them if needed.
- Use a company-specific `name` that describes the narrative path, not a generic bucket.
- Explain which cruxes are settled and how in `crux_assumptions`.
- Make revenue, margin, EPS, P/S, P/E, and blend-weight assumptions visible in each period.
- Use `confirming_signals` and `breaking_signals` that map to the watch items.

The scenario calculator runs 10,000 Monte Carlo draws by default. For each draw, it selects a scenario according to normalized scenario probabilities, treats that scenario's terminal low / median / high implied price band as an approximate P10 / P50 / P90 normal distribution, and samples one non-negative terminal price. Override defaults with `monte_carlo.iterations`, `monte_carlo.seed`, or `monte_carlo.bins` in `sections/scenario-assumptions.json` only when the user asks for a different simulation shape.

Keep a hidden mental taxonomy for coverage: upside validation, durable-but-priced-in, company-specific constraint, downside failure, and sector/theme multiple risk. Do not expose that taxonomy as repetitive scenario names unless it is genuinely the clearest label.

## Financial Math Construction

Treat `sections/financial-math.json` as a company-specific economic bridge, not a generic valuation-method note. Prefer a structured array of render blocks over fixed prose keys. The section should identify the 2-4 economic engines that most determine the stock narrative and connect each engine to current facts, growth, margin structure, TAM/penetration implications, and scenario assumptions.

Before writing financial math, answer for each key engine:

- What is current revenue and profit/margin, if disclosed?
- What is current or recent growth?
- What future scale is implied by the bull case or major scenario?
- What TAM, penetration, unit, ASP, attach-rate, or utilization assumptions are required?
- Is the assumption an extrapolation from disclosed history or a narrative break into a new market?
- Which watch items would confirm or break this engine?

Use block types such as `section`, `metric_grid`, `table`, `economic_engine`, `segment_mix`, `tam_bridge`, `scenario_implication`, and `callout`. For example, a company like TSLA should not stop at aggregate revenue growth and P/S math; it should break out automotive, energy/storage, services/software, FSD/Robotaxi, and Optimus/robotics where relevant, clearly labeling which are disclosed businesses and which are speculative future revenue pools.

Example block shape:

```json
[
  {
    "type": "segment_mix",
    "title": "Current revenue mix",
    "body": "Explain disclosed segment revenue and profit mix, including source gaps.",
    "metrics": [
      { "label": "Energy revenue", "value": "$X", "note": "Y% YoY; source/date" }
    ]
  },
  {
    "type": "tam_bridge",
    "title": "Robotics TAM bridge",
    "body": "Explain what market size, penetration, unit volume, ASP, and margin would be required for the bull case.",
    "assumptions": ["Assumption 1", "Assumption 2"]
  }
]
```

Keep `sections/scenario-assumptions.json` as the calculation input. Use `financial-math.json` to explain why the aggregate assumptions are economically plausible or implausible.

## Data and Script Usage

Prefer the pipeline script:

```bash
python .agents/skills/stock-narrative-research-skill/scripts/report_pipeline.py MSFT init
python .agents/skills/stock-narrative-research-skill/scripts/report_pipeline.py MSFT fetch --workdir reports/stock-narrative-research/MSFT-YYYYMMDD-HHMMSS
python .agents/skills/stock-narrative-research-skill/scripts/report_pipeline.py MSFT seed-scenarios --workdir reports/stock-narrative-research/MSFT-YYYYMMDD-HHMMSS
python .agents/skills/stock-narrative-research-skill/scripts/report_pipeline.py MSFT project --workdir reports/stock-narrative-research/MSFT-YYYYMMDD-HHMMSS
python .agents/skills/stock-narrative-research-skill/scripts/report_pipeline.py MSFT validate --workdir reports/stock-narrative-research/MSFT-YYYYMMDD-HHMMSS
python .agents/skills/stock-narrative-research-skill/scripts/report_pipeline.py MSFT compile --workdir reports/stock-narrative-research/MSFT-YYYYMMDD-HHMMSS
python .agents/skills/stock-narrative-research-skill/scripts/report_pipeline.py MSFT render --workdir reports/stock-narrative-research/MSFT-YYYYMMDD-HHMMSS
```

`scripts/fetch_financials.py` remains a standalone helper for starter public-data snapshots. `scripts/scenario_calculator.py` converts explicit scenario assumptions into derived scenario data and a probability-weighted Monte Carlo distribution. It is a deterministic calculator, not a forecasting model.

The scripts are pragmatic v0.1 helpers. If they fail, explain the data gap and continue with user-provided numbers or a qualitative projection.

## Supporting Files

- `source-policy.md`: source hierarchy and acceptable source use.
- `source-and-framing-rules.md`: language, advice, and evidence boundaries.
- `scripts/report_pipeline.py`: orchestrates run directory setup, fetching, validation, scenario calculation, compile, and render.
- `scripts/scenario_calculator.py`: deterministic scenario calculator. `scripts/projection_model.py` is retained for compatibility.
- `templates/report.html.j2`: final static HTML report template.
- `templates/90-minute-report.md`: legacy markdown report template.
- `templates/scenario-projection.md`: scenario formulas and input shape.
- `templates/historical-analogue-card.md`: analogue candidate template.
- `templates/report-shell.html`: legacy reusable D3 HTML report shell.
- `schemas/report.schema.json`: report JSON contract.
- `schemas/scenario-data.schema.json`: scenario JSON contract.
- `rubrics/final-report-checklist.md`: final quality gate.
