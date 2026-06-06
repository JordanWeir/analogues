# Fundamentals Data: Build / Buy / Hybrid Analysis

## Context

The stock narrative report workflow currently initializes a per-run SQLite database by fetching market data and SEC Company Facts, then stores normalized baseline fundamentals such as revenue, margins, share count, cash, debt, and valuation multiples.

The recent `INTC` investigation exposed a class of data-quality problem that is broader than one ticker. The SEC data was available, but the ingestion layer selected and labeled it incorrectly:

- `fundamentals.total_debt` was stored as roughly `$2.0B`.
- That value matched Intel's latest `DebtCurrent` fact only.
- Intel's noncurrent debt existed in SEC Company Facts under `LongTermDebtNoncurrent`, but the current concept whitelist only looked for `LongTermDebtAndFinanceLeaseObligationsNoncurrent` and `LongTermDebtAndCapitalLeaseObligations`.
- As a result, the system silently reported current debt as total debt.
- The row was also stamped with the shared income-statement period `2025-12-27`, even though the selected current-debt value came from a later Q1 2026 balance-sheet fact.

This was not a network failure or a missing SEC filing. It was a normalization and reconciliation failure.

That matters because the report generator uses these metrics as factual anchors for narrative, scenario construction, and valuation math. A wrong but plausible-looking number is more dangerous than an explicitly missing number.

## The Core Problem

SEC Company Facts is authoritative but not product-ready. It exposes raw XBRL facts by taxonomy concept, unit, period, and filing metadata. It does not guarantee that a stable business metric like `total_debt`, `revenue`, `operating_income`, or `shares_outstanding` maps cleanly to one concept across companies, years, industries, or accounting standards.

The same economic metric may vary because:

- Different companies choose different GAAP tags for similar line items.
- A company may change tags over time as taxonomy standards evolve.
- Some industries report structurally different statements, especially banks, insurers, REITs, funds, and energy companies.
- Some concepts are direct totals, while others require composition from current and noncurrent components.
- Some reported facts are instant balance-sheet values, while others are duration income-statement or cash-flow values.
- Vendor or SEC APIs can return multiple facts for the same period from original filings, amended filings, or later filings carrying prior-period comparative values.
- Extension concepts may be absent from the SEC Company Facts API if the current ingestion only reads standard `us-gaap` concepts.

The Intel debt issue is a concise example:

- The economic metric was "total debt."
- The code implemented it as current debt plus a small set of noncurrent debt aliases.
- Intel used `LongTermDebtNoncurrent`, which was not in the alias list.
- The code therefore computed a partial aggregate without knowing it was partial.
- The database stored the result as a finished metric with no warning.

## Option 1: Build

Building means continuing to use SEC Company Facts as the canonical input and investing in a real fundamentals normalization layer inside this project.

This is not just adding more concept names. It becomes a mini-data product.

### What The Build Would Need

The system should represent each normalized metric as a recipe, not as a lookup. A recipe should know:

- Candidate SEC concepts.
- Direct-total concepts.
- Component concepts.
- Preferred units.
- Period semantics, such as instant, annual duration, quarterly duration, or TTM-derived.
- Industry applicability.
- Fallback order.
- Reconciliation checks.
- Confidence status.
- Provenance for every selected and rejected input.

For example, `total_debt` could be modeled as:

- Prefer direct total debt if available and period-aligned.
- Otherwise compute `current_debt + noncurrent_debt`.
- Accept concept aliases such as `DebtCurrent`, `LongTermDebtCurrent`, `LongTermDebtNoncurrent`, `LongTermDebt`, and finance-lease variants where appropriate.
- Flag partial debt if current debt exists but no noncurrent component or direct total exists.
- Flag mismatches if direct total and computed total differ beyond a tolerance.
- Store selected concept names, form, filed date, fact end date, value, and calculation method.

### Data Model Changes

The current schema has `fundamentals(metric_key, metric_value, period, source_note)`, which is too compressed for confidence work.

A stronger model would add or derive tables like:

- `normalized_metrics`: canonical metric value, value type, as-of period, confidence state, calculation method.
- `normalized_metric_inputs`: raw facts used in the final value.
- `normalized_metric_candidates`: rejected but relevant candidate facts.
- `normalization_warnings`: semantic warnings and validation failures.
- `metric_recipes`: recipe version, concept aliases, industry applicability.

The important design rule is that every number should answer:

- What value did we select?
- From which concept or calculation?
- From which filing and period?
- What alternatives existed?
- What checks did it pass or fail?
- How confident should downstream code be?

### Validation Rules

A build approach needs validations that are metric-specific and statement-aware.

Examples:

- Balance-sheet facts must use instant periods.
- Income-statement and cash-flow facts must use duration periods.
- TTM values must be either derived from aligned quarters or explicitly marked as annual fallback.
- Aggregates should reconcile to component totals when both are present.
- A metric should not inherit a period from another metric family.
- A mature industrial company with current debt but no noncurrent debt should trigger a suspicious-partial-debt warning.
- Direct total debt and current-plus-noncurrent debt should be within tolerance.
- Market cap derived from price and shares should use compatible share-count timing or be labeled approximate.

### Test Corpus

The build path needs a curated issuer corpus, not only unit tests with hand-authored fake data.

The corpus should include:

- Semiconductors: Intel, Nvidia, AMD.
- Software: Microsoft, Salesforce, Adobe.
- Banks: JPMorgan, Bank of America.
- Insurers: Progressive, Chubb.
- REITs: Prologis, Realty Income.
- Energy: Exxon, Chevron.
- Retail: Walmart, Costco.
- Autos: Tesla, Ford, GM.
- Industrials: GE, Caterpillar.
- Healthcare and biotech: Johnson & Johnson, Pfizer, Moderna.
- Companies with unusual capital structures, restatements, divestitures, or major financing events.

Each corpus case should record expected values for a small set of canonical metrics and why those values are expected.

### Operational Burden

Building creates ongoing responsibilities:

- Maintain concept aliases as SEC taxonomy usage changes.
- Add company-specific or industry-specific exceptions.
- Re-run coverage tests when normalization logic changes.
- Store diagnostic output so future failures are explainable.
- Decide how aggressive to be with inferred values.
- Handle restatements, amended filings, and prior-period comparatives.

The upside is control, auditability, and deep integration with the report pipeline. The downside is that this becomes a persistent data engineering surface area.

## Option 2: Buy

Buying means using a normalized fundamentals provider as the primary source for statements and baseline metrics.

Candidate provider categories:

- Low-cost normalized fundamentals: Financial Modeling Prep, SimFin, Alpha Vantage, EODHD.
- SEC-focused normalized statement APIs: SEC API, Fundamentals API, similar vendors.
- Higher-end institutional data: Intrinio and similar providers.

Databento is less relevant for this specific problem. It is strong for market data, corporate actions, security master data, historical prices, and adjustment factors, but it does not currently solve normalized company financial statements.

### What Buying Solves

A good vendor can reduce work on:

- GAAP and IFRS concept normalization.
- Cross-company standardized statement lines.
- Common historical taxonomy transitions.
- Basic statement history.
- Ratios and derived metrics.
- Faster implementation.

For this project, a vendor API could supply normalized fields such as:

- Total revenue.
- Gross profit.
- Operating income.
- Net income.
- Cash and equivalents.
- Total debt.
- Shares outstanding.
- Operating cash flow.
- Capital expenditures.
- Free cash flow.

### What Buying Does Not Solve

Buying does not eliminate judgment.

Vendors still make mapping decisions. They may:

- Choose different concepts than we would.
- Infer missing values.
- Revise historical data.
- Collapse as-reported detail into standardized lines.
- Handle industry-specific statements differently.
- Have licensing restrictions.
- Have coverage gaps or stale data.
- Be opaque about calculation provenance.

For a narrative research product, blind trust in a vendor number can still produce a confident but wrong report. A vendor should reduce risk, not remove verification.

### Vendor Evaluation Questions

Before choosing a provider, evaluate:

- Does it provide standardized statements and as-reported statements?
- Does it expose source filing metadata?
- Does it expose concept-level provenance or only normalized fields?
- How quickly does it update after earnings?
- Does it cover the target universe?
- Does it handle banks, insurers, REITs, and foreign filers well?
- Does it support point-in-time history or only latest-restated history?
- What are the redistribution and commercial-use terms?
- What happens when a value is inferred?
- Can we reproduce or audit key numbers?

## Option 3: Hybrid

The hybrid approach uses SEC Company Facts as the auditable source layer and a paid or free normalized provider as a confidence oracle.

This is the most attractive path for the current report system.

### How Hybrid Would Work

The run database would store:

- SEC-derived normalized values.
- Vendor-normalized values.
- Differences between them.
- Confidence states.
- Source notes and warnings.

For example:

| metric | SEC-derived value | vendor value | delta | confidence |
| --- | ---: | ---: | ---: | --- |
| total debt | `$2.0B` | `$45.0B` | huge | fail |
| revenue | `$52.9B` | `$52.9B` | small | high |
| cash | `$17.2B` | `$17.2B` | small | high |

In the Intel case, a hybrid system would likely have caught the issue immediately because vendor `totalDebt` would have been near `$45B`, while the SEC-derived value was only current debt.

### Hybrid Confidence States

Useful states:

- `high`: SEC and vendor agree within tolerance.
- `sec_only`: no vendor value, but SEC recipe passed validations.
- `vendor_only`: SEC recipe failed, but vendor value exists.
- `mismatch`: SEC and vendor both exist but differ materially.
- `partial`: only part of a component recipe was found.
- `missing`: no credible value.
- `industry_inappropriate`: metric should not be used for this company type.

The report generator can then decide whether to use, caveat, or suppress a metric.

### Why Hybrid Fits The Report Product

The report does not need millisecond-grade data. It needs good enough fundamentals, transparent provenance, and warnings when the foundation is weak.

Hybrid gives:

- Better confidence than SEC-only.
- More transparency than vendor-only.
- A path to improve the internal normalization layer over time.
- A way to generate regression tests from real mismatches.
- Vendor optionality.

## Bakeoff Plan

Do not choose immediately. Run a bakeoff.

### Provider Candidates

Start with:

- Financial Modeling Prep.
- SimFin.
- Alpha Vantage.
- EODHD.
- One SEC-focused normalized statement API if pricing and access are acceptable.

Databento can be evaluated separately for prices, security master, splits, dividends, and corporate actions.

### Ticker Set

Use 25-50 tickers across industries:

- `INTC`, `NVDA`, `AMD`, `TSM`
- `MSFT`, `AAPL`, `ORCL`, `CRM`
- `JPM`, `BAC`, `GS`
- `PGR`, `CB`
- `PLD`, `O`
- `XOM`, `CVX`
- `WMT`, `COST`, `HD`
- `TSLA`, `F`, `GM`
- `GE`, `CAT`, `BA`
- `JNJ`, `PFE`, `MRNA`

Include tickers with known issues:

- Debt concept variation.
- Segment-heavy companies.
- Major divestitures.
- Negative earnings.
- Multiple share classes.
- Financial companies.
- REIT metrics where generic GAAP metrics are less useful.

### Metrics To Compare

Compare at least:

- Revenue.
- Gross profit.
- Operating income.
- Net income.
- EPS.
- Diluted shares.
- Cash and equivalents.
- Short-term debt.
- Long-term debt.
- Total debt.
- Operating cash flow.
- Capital expenditures.
- Free cash flow.
- Market cap.
- Price.

For each metric, collect:

- SEC selected value.
- SEC selected concept or recipe.
- SEC period end.
- SEC filing date.
- Vendor value.
- Vendor period end.
- Difference.
- Confidence state.
- Notes.

### Bakeoff Outputs

Create a SQLite table or CSV like:

```text
ticker
industry
metric_key
sec_value
sec_method
sec_concepts
sec_period_end
sec_filed_at
vendor
vendor_value
vendor_period_end
delta_abs
delta_pct
confidence_state
warning
```

Then review:

- Which provider agrees most often with SEC-derived values?
- Which provider catches known SEC-ingestion mistakes?
- Which provider has the best industry coverage?
- Which provider exposes enough provenance?
- Which provider updates fastest after earnings?
- Which provider has acceptable licensing?
- Which mismatches are vendor errors versus internal errors?

### Decision Criteria

Choose based on:

- Accuracy on the corpus.
- Explainability.
- Coverage.
- Cost.
- Update latency.
- API reliability.
- Licensing.
- Ease of integration.
- Ability to support report-quality caveats.

Do not optimize only for price. A cheap provider that silently changes historical data or hides provenance may not be cheap if it erodes report trust.

## Recommended Direction

The best near-term path is hybrid:

1. Patch obvious SEC normalization bugs, including Intel debt.
2. Add metric-level provenance and confidence states.
3. Add a diagnostic view for selected metrics and candidate facts.
4. Run a bakeoff across low-cost normalized providers.
5. Use the winning provider as a confidence oracle.
6. Keep SEC facts in the database as the auditable foundation.

The system should not try to become Bloomberg. But it should become honest about what it knows, how it knows it, and when a number is fragile.

## Near-Term Engineering Checklist

- Add `LongTermDebtNoncurrent` and `LongTermDebtCurrent` to the debt recipe.
- Add direct `LongTermDebt` as a possible total-debt fallback.
- Stop assigning balance-sheet metrics the income-statement bundle period.
- Store selected fact end dates for cash, shares, debt, and price-derived metrics.
- Add warnings for partial aggregate metrics.
- Add warnings for period mismatches.
- Add a `fundamentals_diagnostics` query or task.
- Add Intel as a regression fixture.
- Add at least one bank, insurer, REIT, and energy company fixture.
- Compare SEC-derived values against one vendor before trusting a report run.

## Open Questions

- Should reports use vendor values when SEC-derived values fail, or only use vendors as warnings?
- Should the pipeline block report generation on high-severity metric mismatches?
- Which metrics are critical enough to require cross-source agreement?
- Should industry classification happen before fundamentals normalization?
- Should we maintain point-in-time data or latest-restated data for reports?
- How much provenance should appear in the final HTML versus remain in SQLite?

## Working Principle

The report pipeline should treat fundamentals as evidence, not decoration.

Every important number should be selected through a recipe, reconciled against alternatives, stored with provenance, and assigned a confidence state. Build, buy, and hybrid are not only sourcing choices; they are choices about how much of that responsibility lives inside this repo versus with a vendor.
