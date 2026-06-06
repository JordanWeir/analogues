---
name: stock-agent-2
description: Produces 90-minute stock narrative research reports using Rust tasks, a per-run SQLite database, source packs, bull/bear narrative maps, scenario-conditioned projections, historical analogues, and chart-ready HTML artifacts. Use when analyzing a stock, comparing bull and bear narratives, building stock scenario projections, identifying historical analogues, or preparing a stock research memo.
---

# Stock Narrative Research

Use this skill to help a user understand a public company stock narrative without turning the output into investment advice.

## Use When

- The user asks to analyze a stock, ticker, or public company narrative.
- The user wants a 90-minute research report, stock memo, talk track, bull/bear debate, scenario projection, or historical analogue.
- The user asks what could move a stock up or down, or what assumptions the stock appears to depend on.

Do not use this skill for casual price checks, buy/sell/hold recommendations, portfolio allocation, tax/legal advice, options recommendations, high-frequency signals, or guaranteed predictions.

## Core Workflow (Rust Tooling)

Use a ticker-specific working directory and treat the stock-specific SQLite file as the canonical work product. The agent supplies research judgment and assumptions; Rust tasks scaffold, calculate, validate, compile, and render.

Run task commands from the workspace root. New runs default to `reports/stock-narrative-research/`.

Important: the first data-ingestion stage must be run outside the sandbox. In practice, run `initWorkspace` with unrestricted network access, because its starter quote and SEC fetches can fail inside the default sandboxed network policy even when the task logic is otherwise correct.

1. Identify the ticker, company, user goal, and time horizon
2. Initialize a working directory: `cargo loco task initWorkspace ticker:ORCL`
3. The working directory will be initialized with a SQLite file. It comes with schema tables for the full run and initializes with available starter fundamentals.
4. Examine the existing data in the tables.  You should see values for:
- Stock Info
- Fundamentals
5. Use the web search tool to find more information about the stock, populating these tables in order.  As you find key sources or claims online, also add those source or claim data to the sources and claims tables.
- Orientation
- Business Model
- Why Now
- Narrative Map
- Financial Snapshot
- Financial Math
- Industry Context
- Watch Items
- Historical Analogue
- Final Talk Track
6. Think deeply about the "scenario assumptions" table. This is the highest-impact judgment step. Use fetched baseline values, source-backed assumptions, dynamic crux-derived scenarios, conditional probabilities, and explicit scenario narratives.
7. Run the scenario calculator as an internal arithmetic step. It also runs a probability-weighted Monte Carlo simulation from the terminal scenario bands: `cargo loco task generateReport ticker:MSFT date:2026-06-04 index:1`


## Required Outputs

Each run should produce a directory like `reports/stock-narrative-research/MSFT-YYYY-MM-DD-INDEX/`:

- `run.sqlite`: canonical run database containing source, claim, section, scenario, calculator, and artifact tables.
- `generated/report.html`: final rendered HTML artifact.

Do not write intermediate section files outside the SQLite database. Markdown is optional. Do not make it the default final artifact.

## Projection Framing

Use phrases like "scenario-conditioned implied range", "illustrative price band", "assumption-driven model", and "if this scenario plays out". Avoid "target price", "fair value", "expected return", "should trade at", "will go to", and "guaranteed upside".

## Scenario Construction

Do not default to the same scenario names across every report. Build 4-6 scenarios from the report's narrative cruxes, watch items, claim table, and historical analogues. Include at least one bullish, one neutral, and one bearish scenario, but let the remaining scenarios reflect the company-specific debate.

For each scenario in `scenario-assumptions`:

- Set `stance` to `bullish`, `neutral`, `bearish`, or `mixed`.
- Set `probability` to the scenario's conditional likelihood as a decimal, such as `0.25`. Probabilities should usually sum to 1.0; the calculator normalizes them if needed.
- Use a company-specific `name` that describes the narrative path, not a generic bucket.
- Explain which cruxes are settled and how in `crux_assumptions`.
- Make revenue, margin, EPS, P/S, P/E, and blend-weight assumptions visible in each period.
- Use `confirming_signals` and `breaking_signals` that map to the watch items.

The scenario calculator runs 10,000 Monte Carlo draws by default. For each draw, it selects a scenario according to normalized scenario probabilities, treats that scenario's terminal low / median / high implied price band as an approximate P10 / P50 / P90 normal distribution, and samples one non-negative terminal price. Override defaults with `iterations`, `seed`, or `bins` in `monte_carlo_config` only when the user asks for a different simulation shape.

Keep a hidden mental taxonomy for coverage: upside validation, durable-but-priced-in, company-specific constraint, downside failure, and sector/theme multiple risk. Do not expose that taxonomy as repetitive scenario names unless it is genuinely the clearest label.

## Financial Math Construction

Treat `financial-math` as a company-specific economic bridge, not a generic valuation-method note. Prefer ordered content blocks in the database over fixed prose fields. The section should identify the 2-4 economic engines that most determine the stock narrative and connect each engine to current facts, growth, margin structure, TAM/penetration implications, and scenario assumptions.

Before writing financial math, answer for each key engine:

- What is current revenue and profit/margin, if disclosed?
- What is current or recent growth?
- What future scale is implied by the bull case or major scenario?
- What TAM, penetration, unit, ASP, attach-rate, or utilization assumptions are required?
- Is the assumption an extrapolation from disclosed history or a narrative break into a new market?
- Which watch items would confirm or break this engine?

Use block types such as `section`, `metric_grid`, `table`, `economic_engine`, `segment_mix`, `tam_bridge`, `scenario_implication`, and `callout`. For example, a company like TSLA should not stop at aggregate revenue growth and P/S math; it should break out automotive, energy/storage, services/software, FSD/Robotaxi, and Optimus/robotics where relevant, clearly labeling which are disclosed businesses and which are speculative future revenue pools.

SQLite does not have a strict native JSON column type. When a content block needs a flexible nested shape, store stable fields as columns and put the flexible payload in `TEXT CHECK (json_valid(payload))`; validate richer payload schemas in Rust before writing. Prefer relational child tables for frequently queried structures such as metrics, assumptions, rows, and source links.

Rendering contract: `templates/report.html.j2` expects `generateReport` to compile database rows into a report payload where `sections.financial_math` is a string, object, or content-block array. Content-block array items should hydrate to objects with `type`, optional `title`, `body`, `metrics`, `items`, `assumptions`, `columns`, `rows`, and `source_note`. The template gives `table` and `metric_grid` special renderers; other block types render as generic content block articles using the `type` as a CSS class.

When populating this section manually, write SQL that makes the rendered block obvious. Use one `content_blocks` row per visual block, ordered by `block_order`. Put `type`, `title`, `body`, and `source_note` in stable columns; put renderer-specific arrays such as `metrics`, `items`, `assumptions`, `columns`, and `rows` in validated payload text unless there is a dedicated child table.

```sql
BEGIN;

INSERT INTO content_blocks (
  section_key,
  block_order,
  block_type,
  title,
  body,
  payload
) VALUES (
  'financial_math',
  1,
  'metric_grid',
  'Current revenue mix',
  'Summarize disclosed segment revenue and profit mix, including source gaps.',
  json_object(
    'metrics',
    json_array(
      json_object('label', 'Energy revenue', 'value', '$X', 'note', 'Y% YoY; source/date'),
      json_object('label', 'Automotive revenue', 'value', '$X', 'note', 'Y% YoY; source/date')
    )
  )
);

INSERT INTO content_blocks (
  section_key,
  block_order,
  block_type,
  title,
  body,
  payload
) VALUES (
  'financial_math',
  2,
  'tam_bridge',
  'Robotics TAM bridge',
  'Explain what market size, penetration, unit volume, ASP, and margin would be required for the bull case.',
  json_object(
    'assumptions',
    json_array('Assumption 1 with source context', 'Assumption 2 with source context')
  )
);

COMMIT;
```

Example content block rows:

| section_key | block_order | block_type | title | body | payload |
| --- | ---: | --- | --- | --- | --- |
| financial_math | 1 | segment_mix | Current revenue mix | Explain disclosed segment revenue and profit mix, including source gaps. | Optional validated payload for extra metrics. |
| financial_math | 2 | tam_bridge | Robotics TAM bridge | Explain market size, penetration, unit volume, ASP, and margin assumptions. | Optional validated payload for assumption details. |

Keep `scenario-assumptions` as the calculation input. Use `financial-math` to explain why the aggregate assumptions are economically plausible or implausible.

## Data and Task Usage

Prefer Rust tasks:

```bash
# Run this first step outside the sandbox / with unrestricted network access.
cargo run task initWorkspace ticker:MSFT
cargo run task generateReport ticker:MSFT date:2026-06-04 index:1
```

`initWorkspace` creates the run directory, initializes `run.sqlite`, applies the run schema, and seeds available stock info and fundamentals. `generateReport` reads from `run.sqlite`, calculates scenario outputs, validates required tables, and renders HTML. It is a deterministic calculator and renderer, not a forecasting model.

If task-backed financial fetching fails, explain the data gap and continue with user-provided numbers or a qualitative projection written into the database.

## Supporting Files

- `templates/report.html.j2`: final static HTML report template.
- `templates/90-minute-report.md`: legacy markdown report template.
- `templates/scenario-projection.md`: scenario formulas and input shape.
- `templates/historical-analogue-card.md`: analogue candidate template.
- `templates/report-shell.html`: legacy reusable D3 HTML report shell.
