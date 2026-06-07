# Financial Math Cheatsheet

This note is a practical guide for weaker models doing Phase 5-style financial mechanics work directly from SQLite.

Assumption: stages 1-4 have already done a better cleanup pass than the ORCL workspace inspected during QA. In particular, assume:

- canonical and supporting metrics have already been selected reasonably well
- obvious duplicate/restated rows are either removed or easy to dedupe
- period shapes are labeled or inferable
- company baseline metadata is present
- raw facts still exist for deeper inspection when needed

The goal is not to produce a full valuation model. The goal is to answer focused questions like:

- Is a growth narrative converting into revenue and cash?
- Is capex rising faster than revenue or cash flow?
- Is leverage or interest becoming a constraint?
- Are obligations building ahead of visible monetization?
- Has capital allocation shifted?

## Core Idea

Weak models do much better when the task is framed as:

1. pick one financial question
2. pick 2-5 metrics that answer it
3. use one consistent period basis
4. compute a small number of ratios or bridges
5. state what the arithmetic says
6. separately state what it might mean

Do not start with "analyze the company." Start with one mechanic.

## What Is A Bridge?

In this context, a `bridge` is a short chain of arithmetic that connects a business narrative to a financial consequence.

It is more than a single ratio, but much smaller than a full model.

A good bridge usually has:

- a starting business driver
- one or two intermediate mechanics
- an ending financial effect

You can think of it as:

`story -> mechanism -> financial result`

Examples:

- backlog growth -> deferred revenue / receivables / revenue conversion -> cash timing
- capex growth -> PPE growth and cash consumption -> margin or funding pressure
- debt issuance -> interest expense -> EPS or coverage pressure
- buyback slowdown -> cash reallocation -> investment regime change

The point of a bridge is to make the narrative testable. Instead of saying "AI demand is strong," the bridge asks "what math would need to happen for that demand to turn into revenue, cash, and acceptable returns?"

### Concrete Example: Backlog To Funding Pressure

A weak narrative:

- "Backlog is growing, so the business is getting stronger."

A better bridge:

1. `RevenueRemainingPerformanceObligation` is growing quickly.
2. `ContractWithCustomerLiability` and `AccountsReceivableNetCurrent` are much smaller than backlog.
3. `PaymentsToAcquirePropertyPlantAndEquipment` is rising faster than `NetCashProvidedByUsedInOperatingActivities`.
4. Therefore the real financial question is whether backlog will convert fast enough to fund the heavier investment base.

This can be expressed with a small set of outputs:

- `RPO / deferred_revenue`
- `RPO / receivables`
- `capex / revenue`
- `capex / operating_cash_flow`
- `operating_cash_flow - capex`

That is a bridge because it links:

- demand visibility
- conversion timing
- investment intensity
- funding pressure

### Concrete Example: Debt To EPS Pressure

1. `ProceedsFromIssuanceOfSeniorLongTermDebt` rises.
2. `InterestExpense` rises after the debt issuance.
3. `OperatingIncomeLoss` does not rise enough to offset the added financing cost.
4. Therefore the company may face future EPS or coverage pressure even if revenue is still growing.

Useful outputs:

- debt issuance trend
- interest expense trend
- `interest_expense / operating_income`
- EPS trend

### Rule Of Thumb

If the analysis naturally produces a sentence like "therefore the crux is..." or "so the key question becomes...", you probably have a real bridge rather than just a disconnected metric.

## Common Bridge Archetypes

These are reusable bridge shapes. A weaker model does not need to invent a new structure each time. It can choose the archetype that matches the narrative and then fill in the relevant metrics.

### 1. Backlog To Cash-Conversion Bridge

Use when the story is about bookings, demand visibility, cloud backlog, or long-term contracts.

Shape:

- backlog grows
- near-term conversion buckets stay smaller or grow more slowly
- therefore the crux becomes conversion timing, not just demand

Typical metrics:

- `RevenueRemainingPerformanceObligation`
- `ContractWithCustomerLiability`
- `AccountsReceivableNetCurrent`
- `RevenueFromContractWithCustomerExcludingAssessedTax`

Useful outputs:

- `RPO / deferred_revenue`
- `RPO / receivables`
- backlog growth vs revenue growth

### 2. Capex To Funding-Pressure Bridge

Use when the story is about infrastructure buildout, manufacturing capacity, AI spend, or data-center scaling.

Shape:

- capex rises
- asset base rises
- cash flow does not keep up
- therefore the crux becomes whether the buildout is self-funded or requires heavier financing

Typical metrics:

- `PaymentsToAcquirePropertyPlantAndEquipment`
- `PropertyPlantAndEquipmentNet`
- `NetCashProvidedByUsedInOperatingActivities`
- `RevenueFromContractWithCustomerExcludingAssessedTax`

Useful outputs:

- `capex / revenue`
- `capex / operating_cash_flow`
- `operating_cash_flow - capex`
- PPE growth

### 3. Debt To EPS / Coverage Bridge

Use when the company appears to be leaning on financing.

Shape:

- debt issuance rises
- interest expense rises
- operating income does not improve enough
- therefore financing cost may pressure EPS or interest coverage

Typical metrics:

- `ProceedsFromIssuanceOfSeniorLongTermDebt`
- `InterestExpense`
- `OperatingIncomeLoss`
- `EarningsPerShareDiluted`

Useful outputs:

- debt issuance trend
- interest expense trend
- `interest_expense / operating_income`
- EPS trend

### 4. Buyback Slowdown To Capital-Reallocation Bridge

Use when management behavior is part of the narrative.

Shape:

- buybacks fall
- capex or other investment rises
- therefore management may be shifting from shareholder return toward reinvestment

Typical metrics:

- `PaymentsForRepurchaseOfCommonStock`
- `PaymentsToAcquirePropertyPlantAndEquipment`
- `NetCashProvidedByUsedInOperatingActivities`
- `ProceedsFromIssuanceOfSeniorLongTermDebt`

Useful outputs:

- buybacks trend
- buybacks vs capex
- buybacks vs OCF
- capex plus debt issuance trend

### 5. Obligation-Build To Future-Cash-Constraint Bridge

Use when debt alone understates the future fixed burden.

Shape:

- lease or purchase obligations rise
- current cash generation does not rise proportionally
- therefore future cash demands may tighten flexibility even before classic leverage ratios look extreme

Typical metrics:

- `OperatingLeaseLiability`
- `LesseeOperatingLeaseLiabilityPaymentsDue`
- `UnrecordedUnconditionalPurchaseObligationBalanceSheetAmount`
- `NetCashProvidedByUsedInOperatingActivities`

Useful outputs:

- obligations trend
- obligations vs OCF
- obligations vs revenue

### 6. Working-Capital Pressure Bridge

Use when revenue growth may not be converting cleanly into cash.

Shape:

- revenue grows
- receivables or other working-capital balances rise faster
- therefore cash conversion may be weakening

Typical metrics:

- `RevenueFromContractWithCustomerExcludingAssessedTax`
- `AccountsReceivableNetCurrent`
- `IncreaseDecreaseInAccountsReceivable`
- `NetCashProvidedByUsedInOperatingActivities`

Useful outputs:

- receivables growth vs revenue growth
- receivables as a share of revenue
- OCF trend vs revenue trend

### 7. Asset-Base To Margin-Risk Bridge

Use when a company may be moving from a lighter model to a heavier one.

Shape:

- PPE or other fixed assets rise
- depreciation, maintenance needs, or utilization risk likely rise
- therefore future margins may depend more on utilization and pricing

Typical metrics:

- `PropertyPlantAndEquipmentNet`
- `PaymentsToAcquirePropertyPlantAndEquipment`
- `OperatingIncomeLoss`
- `RevenueFromContractWithCustomerExcludingAssessedTax`

Useful outputs:

- PPE growth
- capex trend
- operating margin trend
- asset growth vs revenue growth

### 8. Scenario-Crux Bridge

Use when the task is explicitly to identify what must go right or wrong in a scenario.

Shape:

- one visible narrative metric looks strong
- one less-visible cost, obligation, or funding metric is also rising
- therefore the scenario depends on whether the visible strength outruns the hidden burden

Typical metric pairings:

- backlog vs capex
- revenue growth vs receivables growth
- debt issuance vs interest expense
- buybacks vs capex
- PPE growth vs operating margin

Useful output:

- one sentence in the form:
  - "This scenario works if `X` grows faster than `Y`."
  - "This scenario breaks if `Y` rises faster than `X`."

## Most Useful Patterns

These were the most reusable patterns from the ORCL SQL-only pass.

### 1. Pair a narrative metric with a cash-conversion metric

This is often the highest-value move.

Examples:

- backlog with deferred revenue and receivables
- capex with operating cash flow
- debt issuance with interest expense
- PPE growth with revenue growth
- buybacks with capex and debt issuance

This keeps the analysis grounded in mechanisms instead of isolated facts.

### 2. Split metrics into two shapes

Most useful metrics fall into one of two buckets:

- `instant` metrics: balance-sheet-like, point-in-time
- `duration` metrics: income statement / cash flow style, over a period

Examples:

- instant: `cash`, `debt`, `RPO`, `deferred_revenue`, `receivables`, `PPE`, `lease_liability`
- duration: `revenue`, `operating_cash_flow`, `capex`, `interest_expense`, `buybacks`

Weak models should almost never mix these blindly. Compare instant with instant, and duration with duration, unless the point is explicitly a bridge like `RPO / revenue` or `PPE / revenue`.

### 3. Use one period basis per query

A lot of bad analysis comes from mixing:

- annual
- YTD
- quarter-only
- instant

For a single query, pick one basis:

- annual for medium-term regime shifts
- YTD for current-year buildout or pressure
- quarter-only for inflection checks
- instant for balance-sheet and obligation snapshots

### 4. Use ratios that reveal stress, not just size

Weak models can compute useful math if the formulas are simple and the interpretation is constrained.

High-value examples:

- `capex / revenue`
- `capex / operating_cash_flow`
- `(operating_cash_flow - capex)` as a free-cash-flow proxy
- `RPO / deferred_revenue`
- `RPO / receivables`
- `interest_expense / operating_income`
- `debt_issued / capex`
- `buybacks / operating_cash_flow`

### 5. Treat arithmetic and interpretation as separate steps

Always write:

- arithmetic result
- what it suggests
- what it does not prove

Example:

- Arithmetic: `capex / OCF = 2.26x`
- Suggests: current investment is outrunning internal cash generation
- Does not prove: the spend is bad or unproductive

That separation helps weaker models avoid overclaiming.

## Default Workflow

Use this workflow unless the task explicitly asks for something else.

### Step 1. Pick one question

Good examples:

- Is backlog converting into near-term cash?
- Is capex intensity stepping up?
- Is financing replacing internal funding?
- Are lease and purchase obligations building?
- Has capital allocation shifted away from buybacks?

Bad examples:

- How does the whole business work?
- What is the stock worth?
- Analyze everything important.

### Step 2. Pick the minimum metric set

Use only the metrics needed for the question.

Examples:

- backlog conversion:
  - `RevenueRemainingPerformanceObligation`
  - `ContractWithCustomerLiability`
  - `AccountsReceivableNetCurrent`
  - `RevenueFromContractWithCustomerExcludingAssessedTax`

- capex intensity:
  - `PaymentsToAcquirePropertyPlantAndEquipment`
  - `NetCashProvidedByUsedInOperatingActivities`
  - `RevenueFromContractWithCustomerExcludingAssessedTax`

- financing pressure:
  - `ProceedsFromIssuanceOfSeniorLongTermDebt`
  - `InterestExpense`
  - `OperatingIncomeLoss`
  - `NetCashProvidedByUsedInOperatingActivities`

- obligation build:
  - `OperatingLeaseLiability`
  - `LesseeOperatingLeaseLiabilityPaymentsDue`
  - `UnrecordedUnconditionalPurchaseObligationBalanceSheetAmount`
  - `PropertyPlantAndEquipmentNet`

### Step 3. Normalize period shape

Before doing math, determine whether each metric is:

- instant
- annual
- YTD
- quarter-only

If the data is already cleaned, this may come from a column like `period_type`.
If not, infer it:

- `period_start IS NULL` -> usually instant
- duration around 80-100 days -> usually quarter
- duration around 170-290 days -> usually YTD
- duration around 300-380 days -> usually annual

### Step 4. Dedupe

Keep the latest filed version for the same concept, unit, and period boundaries.

Preferred partition key:

- concept
- unit
- period_start
- period_end

Preferred tie-breaker:

- latest `filed_at`
- latest `id` if needed

### Step 5. Compute only a few outputs

Weak models should usually stop at:

- 1-3 ratios
- 1-2 growth comparisons
- 1 short bridge

Too many outputs increases the chance of mistakes.

### Step 6. Write the finding in a fixed shape

Use this format:

1. Question
2. Inputs used
3. Arithmetic
4. Interpretation
5. Caveat

## Recommended Metric Families

These families are the easiest way to organize Phase 5 work.

### Backlog And Conversion

Use when the narrative is about demand, bookings, cloud growth, long-term contracts, or "visibility."

Preferred metrics:

- `RevenueRemainingPerformanceObligation`
- `ContractWithCustomerLiability`
- `AccountsReceivableNetCurrent`
- `RevenueFromContractWithCustomerExcludingAssessedTax`

Good questions:

- Is backlog much larger than near-term conversion buckets?
- Is revenue growth showing up as receivables or deferred revenue?
- Is backlog growing faster than monetization?

Useful outputs:

- latest backlog snapshot
- backlog vs deferred revenue
- backlog vs receivables
- revenue growth alongside backlog growth

### Capex And Asset-Base Growth

Use when the narrative involves infrastructure, manufacturing, AI buildout, capacity expansion, or data-center scaling.

Preferred metrics:

- `PaymentsToAcquirePropertyPlantAndEquipment`
- `PropertyPlantAndEquipmentNet`
- `NetCashProvidedByUsedInOperatingActivities`
- `RevenueFromContractWithCustomerExcludingAssessedTax`

Good questions:

- Is capex accelerating?
- Is the asset base growing faster than revenue?
- Is internal cash flow funding the buildout?

Useful outputs:

- `capex / revenue`
- `capex / OCF`
- `OCF - capex`
- PPE growth over time

### Financing And Leverage

Use when the story might depend on external funding.

Preferred metrics:

- `ProceedsFromIssuanceOfSeniorLongTermDebt`
- `InterestExpense`
- `OperatingIncomeLoss`
- `NetCashProvidedByUsedInOperatingActivities`

Good questions:

- Is growth being financed externally?
- Is interest becoming a drag?
- Is debt issuance rising during a capex build?

Useful outputs:

- debt issuance trend
- interest expense trend
- `interest_expense / operating_income`
- debt issuance vs capex

### Capital Allocation

Use when management behavior is part of the story.

Preferred metrics:

- `PaymentsForRepurchaseOfCommonStock`
- `PaymentsOfDividendsCommonStock`
- `PaymentsToAcquirePropertyPlantAndEquipment`
- `ProceedsFromIssuanceOfSeniorLongTermDebt`

Good questions:

- Are buybacks slowing while investment rises?
- Is shareholder return being replaced by infrastructure spending?
- Is debt funding capex while buybacks continue?

Useful outputs:

- buyback trend
- buybacks vs capex
- buybacks vs OCF
- buybacks before and after regime change

### Obligations And Commitments

Use when future cash obligations matter more than plain debt.

Preferred metrics:

- `OperatingLeaseLiability`
- `LesseeOperatingLeaseLiabilityPaymentsDue`
- `UnrecordedUnconditionalPurchaseObligationBalanceSheetAmount`
- `CapitalExpendituresIncurredButNotYetPaid`

Good questions:

- Are obligations building ahead of revenue conversion?
- Is the company taking on fixed commitments?
- Are future cash calls rising faster than visible monetization?

Useful outputs:

- latest balance snapshot
- obligation growth over time
- obligations relative to OCF
- obligations relative to revenue or backlog

## Query Templates

These are intentionally simple. Replace names and period filters as needed.

### 1. Latest snapshot for instant metrics

```sql
WITH ranked AS (
  SELECT
    concept_name,
    unit,
    period_end,
    metric_value,
    ROW_NUMBER() OVER (
      PARTITION BY concept_name, unit, period_end
      ORDER BY filed_at DESC, id DESC
    ) AS rn
  FROM sec_raw_facts
  WHERE period_start IS NULL
    AND concept_name IN (
      'RevenueRemainingPerformanceObligation',
      'ContractWithCustomerLiability',
      'AccountsReceivableNetCurrent'
    )
)
SELECT concept_name, period_end, metric_value
FROM ranked
WHERE rn = 1
ORDER BY concept_name, period_end DESC;
```

### 2. Annual trend for duration metrics

```sql
WITH annual AS (
  SELECT
    concept_name,
    period_start,
    period_end,
    metric_value,
    ROW_NUMBER() OVER (
      PARTITION BY concept_name, period_start, period_end
      ORDER BY filed_at DESC, id DESC
    ) AS rn
  FROM sec_raw_facts
  WHERE form = '10-K'
    AND period_start IS NOT NULL
    AND CAST(julianday(period_end) - julianday(period_start) AS INT) >= 300
    AND concept_name IN (
      'RevenueFromContractWithCustomerExcludingAssessedTax',
      'NetCashProvidedByUsedInOperatingActivities',
      'PaymentsToAcquirePropertyPlantAndEquipment'
    )
)
SELECT
  period_end,
  MAX(CASE WHEN concept_name = 'RevenueFromContractWithCustomerExcludingAssessedTax' THEN metric_value END) AS revenue,
  MAX(CASE WHEN concept_name = 'NetCashProvidedByUsedInOperatingActivities' THEN metric_value END) AS ocf,
  MAX(CASE WHEN concept_name = 'PaymentsToAcquirePropertyPlantAndEquipment' THEN metric_value END) AS capex
FROM annual
WHERE rn = 1
GROUP BY period_end
ORDER BY period_end DESC;
```

### 3. Q3-only from YTD filings

```sql
WITH ytd AS (
  SELECT
    concept_name,
    period_end,
    metric_value,
    ROW_NUMBER() OVER (
      PARTITION BY concept_name, period_start, period_end
      ORDER BY filed_at DESC, id DESC
    ) AS rn
  FROM sec_raw_facts
  WHERE form = '10-Q'
    AND CAST(julianday(period_end) - julianday(period_start) AS INT) BETWEEN 170 AND 290
    AND concept_name IN (
      'RevenueFromContractWithCustomerExcludingAssessedTax',
      'NetCashProvidedByUsedInOperatingActivities',
      'PaymentsToAcquirePropertyPlantAndEquipment'
    )
    AND period_end IN ('2026-02-28', '2025-11-30')
)
SELECT
  a.concept_name,
  a.metric_value - b.metric_value AS q3_only_value
FROM ytd a
JOIN ytd b
  ON a.concept_name = b.concept_name
WHERE a.rn = 1
  AND b.rn = 1
  AND a.period_end = '2026-02-28'
  AND b.period_end = '2025-11-30';
```

### 4. Simple mechanics ratios

```sql
SELECT
  revenue,
  ocf,
  capex,
  ROUND(1.0 * capex / revenue, 3) AS capex_to_revenue,
  ROUND(1.0 * capex / ocf, 3) AS capex_to_ocf,
  ROUND((ocf - capex) / 1000000000.0, 3) AS fcf_proxy_b
FROM some_prepared_view;
```

### 5. Simple bull / base / bear scenario in SQLite

This is the smallest useful "what if" pattern I tested successfully in practice.

Use it when:

- you already have a trusted historical base year
- the scenario only needs a few explicit assumptions
- you want 1-3 projected years
- you care more about auditability than elegance

```sql
WITH base AS (
  SELECT
    57399000000.0 AS revenue_0,
    3578000000.0 AS interest_0,
    2866000000.0 AS shares_0
),
assumptions(
  scenario,
  year_num,
  revenue_growth,
  ocf_margin,
  capex_ratio,
  op_margin,
  existing_interest_growth,
  new_debt_rate,
  share_growth
) AS (
  VALUES
    ('bull', 1, 0.18, 0.37, 0.34, 0.31, 0.03, 0.070, 0.000),
    ('bull', 2, 0.16, 0.38, 0.30, 0.32, 0.03, 0.070, 0.000),
    ('base', 1, 0.15, 0.35, 0.38, 0.30, 0.05, 0.075, 0.002),
    ('base', 2, 0.13, 0.35, 0.34, 0.30, 0.05, 0.075, 0.002),
    ('bear', 1, 0.10, 0.32, 0.42, 0.27, 0.08, 0.085, 0.005),
    ('bear', 2, 0.08, 0.31, 0.40, 0.26, 0.08, 0.085, 0.005)
),
recursive_proj AS (
  SELECT
    a.scenario,
    a.year_num,
    b.revenue_0 * (1 + a.revenue_growth) AS revenue,
    b.revenue_0 * a.ocf_margin AS ocf,
    b.revenue_0 * a.capex_ratio AS capex,
    b.revenue_0 * a.op_margin AS operating_income,
    b.interest_0 * (1 + a.existing_interest_growth) AS interest_expense,
    MAX((b.revenue_0 * a.capex_ratio) - (b.revenue_0 * a.ocf_margin), 0) AS incremental_debt,
    b.shares_0 * (1 + a.share_growth) AS diluted_shares
  FROM assumptions a
  CROSS JOIN base b
  WHERE a.year_num = 1

  UNION ALL

  SELECT
    a.scenario,
    a.year_num,
    p.revenue * (1 + a.revenue_growth) AS revenue,
    p.revenue * a.ocf_margin AS ocf,
    p.revenue * a.capex_ratio AS capex,
    p.revenue * a.op_margin AS operating_income,
    (p.interest_expense * (1 + a.existing_interest_growth)) + (p.incremental_debt * a.new_debt_rate) AS interest_expense,
    p.incremental_debt + MAX((p.revenue * a.capex_ratio) - (p.revenue * a.ocf_margin), 0) AS incremental_debt,
    p.diluted_shares * (1 + a.share_growth) AS diluted_shares
  FROM recursive_proj p
  JOIN assumptions a
    ON a.scenario = p.scenario
   AND a.year_num = p.year_num + 1
)
SELECT
  scenario,
  year_num,
  ROUND(revenue / 1000000000.0, 2) AS revenue_b,
  ROUND(ocf / 1000000000.0, 2) AS ocf_b,
  ROUND(capex / 1000000000.0, 2) AS capex_b,
  ROUND((ocf - capex) / 1000000000.0, 2) AS fcf_proxy_b,
  ROUND(interest_expense / 1000000000.0, 2) AS interest_b,
  ROUND(incremental_debt / 1000000000.0, 2) AS cumulative_incremental_debt_b
FROM recursive_proj
ORDER BY scenario, year_num;
```

This pattern is good enough for:

- explicit scenario assumptions
- deterministic roll-forwards
- simple FCF proxies
- financing-need accumulation
- first-pass EPS or coverage proxies

It is not ideal for:

- large numbers of interacting assumptions
- complex balance-sheet roll-forwards
- probabilistic modeling
- hard-to-audit formula stacks

### 6. Small sensitivity grid in SQLite

Sensitivity grids were also easier than expected when kept low-dimensional.

```sql
WITH base AS (
  SELECT 57399000000.0 AS revenue_0
),
growth(g) AS (
  VALUES (0.10), (0.15), (0.20)
),
capex_ratio(c) AS (
  VALUES (0.30), (0.35), (0.40)
),
ocf_margin(o) AS (
  VALUES (0.33)
)
SELECT
  printf('growth_%.0f%%', g * 100) AS revenue_growth_case,
  printf('capex_%.0f%%', c * 100) AS capex_case,
  ROUND((base.revenue_0 * (1 + g)) / 1000000000.0, 2) AS revenue_b,
  ROUND((base.revenue_0 * (1 + g) * o) / 1000000000.0, 2) AS ocf_b,
  ROUND((base.revenue_0 * (1 + g) * c) / 1000000000.0, 2) AS capex_b,
  ROUND((base.revenue_0 * (1 + g) * (o - c)) / 1000000000.0, 2) AS fcf_proxy_b
FROM base
CROSS JOIN growth
CROSS JOIN capex_ratio
CROSS JOIN ocf_margin
ORDER BY g, c;
```

This is a strong fit for questions like:

- "Does capex intensity matter more than top-line growth?"
- "At what capex ratio does FCF turn negative?"
- "How sensitive is the next year to a small change in margin?"

## Prebuilt Questions Weaker Models Can Reuse

These prompts work well when the data is already cleaned.

### Prompt: backlog conversion

Use SQL to answer whether backlog is converting into near-term monetization. Compare the latest and prior periods for backlog, deferred revenue, receivables, and revenue. Report the arithmetic first, then interpretation, then caveats.

### Prompt: capex stress

Use annual and latest YTD SQL to test whether capex intensity has stepped up. Compute capex as a share of revenue and operating cash flow, plus a simple `OCF - capex` proxy. Report whether the current period looks like a regime shift.

### Prompt: financing pressure

Use SQL to compare debt issuance, interest expense, and operating income over time. Report whether external financing appears to be rising faster than the business's ability to absorb interest costs.

### Prompt: capital-allocation shift

Use SQL to compare buybacks, capex, and debt issuance across recent annual periods. Report whether the company appears to be reallocating cash away from shareholder return and toward investment.

### Prompt: obligations build

Use SQL to compare lease liabilities, lease payments due, purchase obligations, and cash flow. Report whether fixed obligations are growing into a meaningful future cash constraint.

### Prompt: simple what-if scenario

Use SQL to build a 3-case deterministic scenario model from the latest annual base year. Explicitly assume revenue growth, OCF margin, capex intensity, and interest growth. Project 1-3 years of revenue, OCF, capex, FCF proxy, and incremental debt need. Report the arithmetic first, then say which assumption matters most.

## Guardrails For Weaker Models

These rules prevent most bad outputs.

### 1. Never mix period types in one arithmetic statement

Bad:

- annual capex divided by quarterly revenue
- instant debt compared directly to YTD cash flow without saying why

Good:

- annual capex / annual revenue
- latest instant debt compared with latest instant cash

### 2. Prefer `period_end` over a reported fiscal label

If there is any ambiguity, trust the actual date range more than the filing's year label.

### 3. Dedupe before aggregating

Do not aggregate raw rows first and hope duplicates cancel out.

### 4. Stop at arithmetic that is easy to audit

Good:

- ratios
- differences
- simple growth comparisons
- quarter-from-YTD subtraction
- small deterministic scenario roll-forwards
- low-dimensional sensitivity grids

Avoid for weaker models unless already prepared upstream:

- multi-step rolling TTM rebuilds from messy raw facts
- heavy valuation logic
- scenario trees with many interacting assumptions
- probabilistic models or Monte Carlo
- embedded free-form expression systems unless truly necessary

### 5. Use a fixed reporting template

For every finding, write:

- `Question`
- `Data used`
- `Computation`
- `Result`
- `Interpretation`
- `Caveat`

## Best Upstream Prep For Weak Models

If stages 1-4 are already cleaning data, these are the most valuable artifacts to provide before Phase 5:

- a deduped fact view keyed by concept, unit, `period_start`, `period_end`
- a period-shape label such as `instant`, `quarter`, `ytd`, `annual`
- a small canonical metric table for core metrics
- a supporting-metric shortlist by theme: backlog, capex, debt, obligations, capital return
- a concept catalog with history length and latest period
- a small set of verified ratio recipes

With those in place, weaker models can do useful Phase 5 work with SQL alone.

## Practical Recommendation On Runtime Choice

After testing both historical bridge work and a real forward-looking scenario model in plain SQLite, the practical takeaway is:

- start with SQLite and explicit assumptions tables
- prefer prepared views for historical base metrics
- use recursive CTEs for small deterministic projections
- use sensitivity grids only when they stay low-dimensional

Only add a separate embedded math or expression runtime if real product needs clearly exceed what audited SQL can express comfortably.

That is especially relevant when considering tools like Rhai. If the main need is straightforward scenario math, SQLite appears good enough for much longer than you might expect, and it avoids introducing extra runtime surface area and security concerns too early.

## Bottom Line

The cheat code is not better financial theory. It is better task framing.

If a weaker model is given:

- clean period labels
- deduped rows
- a small metric shortlist
- a narrow question
- a fixed reporting template

then it can produce useful Phase 5 mechanics work directly in SQLite without needing a separate math runtime.
