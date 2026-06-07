# SEC Facts Insight Patterns

This note captures what the SEC Company Facts data can contribute to the Analogues narrative-scenario workflow. It uses the ORCL workspace below as an illustrative example, but the goal is reusable product guidance rather than an Oracle-specific investment memo.

Example workspace:

```text
reports/stock-narrative-research/ORCL-2026-06-07-4/run.sqlite
```

## Why SEC Facts Can Be Useful

SEC Facts are not just a source for headline fundamentals like revenue, EPS, cash, and debt. A broad raw-facts ingest can expose time series that are hard to find in standard market-data APIs:

- Contract backlog and backlog conversion.
- Capex intensity and asset-base growth.
- Lease liabilities and future lease payments.
- Purchase obligations and other off-balance-sheet-style commitments.
- Debt issuance, interest expense, and financing mix.
- Working-capital pressure from receivables, deferred revenue, and payables.
- Capital allocation regime changes, such as buybacks slowing while capex rises.

These concepts help a later research agent move from "what is the current market narrative?" to "what financial mechanics would confirm or break the narrative?"

For ORCL, the useful insight path was not a hidden Oracle-specific KPI like OCI revenue. The raw concepts were mostly `us-gaap` and `dei`. The value came from connecting a visible AI/cloud backlog story to less-discussed mechanics: RPO conversion, data center capex, debt issuance, leases, interest expense, and cash-flow timing.

## Common Concept Shapes

### 1. Instant Balance Sheet Series

These are point-in-time concepts with `period_start = NULL` and a `period_end` that acts like the as-of date. They are usually straightforward to plot directly.

Examples:

- `RevenueRemainingPerformanceObligation`
- `PropertyPlantAndEquipmentNet`
- `OperatingLeaseLiability`
- `FinanceLeaseLiability`
- `ContractWithCustomerLiability`
- `AccountsReceivableNetCurrent`

Use them to answer:

- Is an obligation, asset base, backlog, or working-capital balance growing or shrinking?
- Is a balance growing faster than revenue, operating cash flow, or market expectations?
- Has a company crossed into a different business model, such as asset-light software becoming infrastructure-heavy?

Plotting guidance:

- Use `period_end` as the x-axis.
- Deduplicate amended or re-reported facts by preferring the latest `filed_at` for a given `period_end`, concept, and unit.
- Treat restated prior periods as useful but make sure the plot does not double count them.

### 2. Duration Flow Series

These have both `period_start` and `period_end`. They can represent a quarter, year-to-date period, annual period, or occasionally an irregular duration. They are plot-ready, but only after choosing a consistent period basis.

Examples:

- `PaymentsToAcquirePropertyPlantAndEquipment`
- `InterestExpense`
- `NetCashProvidedByUsedInOperatingActivities`
- `NetCashProvidedByUsedInInvestingActivities`
- `PaymentsForRepurchaseOfCommonStock`
- `ResearchAndDevelopmentExpense`

Use them to answer:

- Is spending accelerating?
- Is operating cash flow keeping up with investment needs?
- Are financing costs becoming a material drag?
- Has capital allocation shifted from buybacks to reinvestment?

Plotting guidance:

- Do not mix quarterly, year-to-date, and annual values in one line without normalization.
- For quarterly plots, derive quarter-only values by subtracting prior YTD values when the filing reports cumulative YTD facts.
- For annual plots, prefer fiscal-year values from 10-K filings.
- Preserve `form`, `fiscal_year`, `fiscal_period`, and `filed_at` so chart labels can explain what basis is being shown.

### 3. Backlog And Conversion Series

These concepts connect demand claims to revenue recognition and cash conversion.

Examples:

- `RevenueRemainingPerformanceObligation`
- `ContractWithCustomerLiability`
- `ContractWithCustomerLiabilityRevenueRecognized`
- `RevenueFromContractWithCustomerExcludingAssessedTax`
- `AccountsReceivableNetCurrent`
- `IncreaseDecreaseInAccountsReceivable`

Use them to answer:

- Is reported backlog converting to revenue?
- Is backlog backed by near-term deferred revenue or mostly long-dated commitments?
- Is revenue growth showing up as cash, receivables, or only future obligations?

ORCL example:

- `RevenueRemainingPerformanceObligation` had 31 periods from 2018-08-31 to 2026-02-28.
- Latest RPO was about `$552.6B`.
- `ContractWithCustomerLiability` was much smaller, about `$11.18B`.
- That contrast suggests a useful narrative question: how much of the huge RPO balance is near-cash versus long-dated infrastructure commitment?

### 4. Commitment And Obligation Series

These are often more interesting than headline debt because they show future cash or capacity obligations not always captured by simple leverage screens.

Examples:

- `LesseeOperatingLeaseLiabilityPaymentsDue`
- `OperatingLeaseLiability`
- `FinanceLeaseLiability`
- `UnrecordedUnconditionalPurchaseObligationBalanceSheetAmount`
- `CapitalExpendituresIncurredButNotYetPaid`
- `RightOfUseAssetObtainedInExchangeForOperatingLeaseLiability`

Use them to answer:

- Is the company taking on contractual obligations ahead of recognized revenue?
- Are lease and purchase commitments growing faster than cash flow?
- Is capex being incurred before cash payment, creating future funding pressure?

ORCL example:

- `LesseeOperatingLeaseLiabilityPaymentsDue` had 10 periods from 2019-08-31 to 2026-02-28.
- Latest undiscounted operating lease payments due were about `$28.7B`.
- `UnrecordedUnconditionalPurchaseObligationBalanceSheetAmount` had 15 periods from 2013-05-31 to 2026-02-28.
- Latest unrecorded unconditional purchase obligations were about `$11.0B`.

These are the types of facts that can turn a generic "AI cloud growth" narrative into a more precise crux: utilization and pricing must cover not only capex, but also lease and purchase commitments.

### 5. Financing And Capital Structure Series

These concepts reveal how a growth narrative is being funded.

Examples:

- `ProceedsFromIssuanceOfSeniorLongTermDebt`
- `SeniorNotes`
- `DebtInstrumentCarryingAmount`
- `NotesPayableCurrent`
- `InterestExpense`
- `ProceedsFromIssuanceOfConvertiblePreferredStock`
- `PreferredStockValue`

Use them to answer:

- Is growth self-funded or externally funded?
- Are interest costs rising fast enough to pressure EPS?
- Is the company using hybrid financing, preferred stock, or other instruments that may affect common shareholders?

ORCL example:

- `ProceedsFromIssuanceOfSeniorLongTermDebt` had 17 distinct periods from 2021-05-31 to 2026-02-28.
- Latest FY2026 YTD senior long-term debt issuance was about `$44.5B`.
- `InterestExpense` had 72 distinct periods from 2008-05-31 to 2026-02-28.
- Latest FY2026 YTD interest expense was about `$3.16B`.
- `ProceedsFromIssuanceOfConvertiblePreferredStock` had only 2 periods, so it is more of an event flag than a full time series.

### 6. Capital Allocation Series

These concepts can reveal shifts in management priorities.

Examples:

- `PaymentsForRepurchaseOfCommonStock`
- `StockRepurchaseProgramRemainingAuthorizedRepurchaseAmount1`
- `PaymentsOfDividendsCommonStock`
- `CommonStockDividendsPerShareCashPaid`
- `CommonStockSharesOutstanding`
- `WeightedAverageNumberOfDilutedSharesOutstanding`

Use them to answer:

- Are buybacks slowing while capex or debt rises?
- Is dilution becoming a larger part of the story?
- Is the dividend consuming cash that might otherwise fund growth?

ORCL example:

- `PaymentsForRepurchaseOfCommonStock` had 72 periods from 2008-05-31 to 2026-02-28.
- Latest FY2026 YTD repurchases were about `$95M`, far below the historical scale of Oracle's buyback program.
- This is useful as a narrative clue: cash may be shifting away from buybacks toward infrastructure investment.

## Plot Readiness From The ORCL Example

The top 20 ORCL narrative concepts were mostly plot-ready:

- 11 of 20 had long histories, typically 30 to 70 distinct periods.
- 7 of 20 had medium or sparse histories, still usable for directional plots.
- 1 of 20 was a new/sparse concept with only 6 periods.
- 1 of 20 was essentially an event concept with 2 periods.

Long-history examples:

- `RevenueRemainingPerformanceObligation`: 31 periods, 2018-08-31 to 2026-02-28.
- `PaymentsToAcquirePropertyPlantAndEquipment`: 64 periods, 2010-05-31 to 2026-02-28.
- `PropertyPlantAndEquipmentNet`: 68 periods, 2009-05-31 to 2026-02-28.
- `InterestExpense`: 72 periods, 2008-05-31 to 2026-02-28.
- `NetCashProvidedByUsedInOperatingActivities`: 58 periods, 2008-05-31 to 2026-02-28.
- `PaymentsForRepurchaseOfCommonStock`: 72 periods, 2008-05-31 to 2026-02-28.
- `ResearchAndDevelopmentExpense`: 65 periods, 2009-05-31 to 2026-02-28.

Medium-history examples:

- `ProceedsFromIssuanceOfSeniorLongTermDebt`: 17 periods, 2021-05-31 to 2026-02-28.
- `SeniorNotes`: 12 periods, 2013-07-10 to 2026-02-28.
- `OperatingLeaseLiability`: 13 periods, 2019-08-31 to 2026-02-28.
- `UnrecordedUnconditionalPurchaseObligationBalanceSheetAmount`: 15 periods, 2013-05-31 to 2026-02-28.
- `CapitalExpendituresIncurredButNotYetPaid`: 13 periods, 2022-05-31 to 2026-02-28.

Sparse/event examples:

- `FinanceLeaseLiability`: 6 periods, 2024-05-31 to 2026-02-28.
- `ProceedsFromIssuanceOfConvertiblePreferredStock`: 2 periods, best treated as an event flag.

## Useful Derived Views

A raw fact table is not enough. The product should build derived views that make these concepts easy for downstream agents to inspect.

### Concept Catalog

One row per concept/unit:

- taxonomy
- concept name
- label
- description
- unit
- fact count
- distinct period count
- earliest period
- latest period
- latest filed date
- min and max value

This helps identify broad, recent, and potentially interesting concepts.

### Plot Readiness

Classify each concept/unit:

- `long_history`: 20 or more distinct periods.
- `medium_history`: 8 to 19 distinct periods.
- `sparse`: 3 to 7 distinct periods.
- `event_or_point`: 1 to 2 distinct periods.
- `stale`: latest period is old relative to the latest filing.

This lets agents distinguish "plot this" from "treat this as an event clue."

### Period Shape

Classify observations by shape:

- `instant`: no `period_start`, has `period_end`.
- `quarter`: duration roughly one fiscal quarter.
- `ytd`: starts at fiscal year start and ends at Q1/Q2/Q3.
- `annual`: full fiscal year.
- `irregular`: unusual duration.

This is essential for flow metrics. A chart that mixes YTD and quarterly values can mislead.

### Narrative Candidate Tags

Attach lightweight tags to concepts:

- `backlog`
- `conversion`
- `capex`
- `asset_base`
- `lease`
- `purchase_obligation`
- `debt`
- `interest`
- `cash_flow`
- `working_capital`
- `capital_return`
- `dilution`
- `margin`
- `tax`

The tagger can start heuristic, then agents can promote or reject candidates during research.

## Product Implications

The init workflow should preserve and surface broad SEC Facts, not only canonical fundamentals. Core canonical metrics are necessary, but many narrative insights come from "exotic" concepts that are not part of a standard financial snapshot.

Recommended workflow:

1. Ingest all available SEC Facts with provenance.
2. Build a raw concept catalog with plot-readiness and period-shape metadata.
3. Seed canonical fundamentals separately for revenue, income, EPS, cash, debt, shares, and cash flow.
4. Run a narrative concept triage step that tags broad themes and selects high-value time series.
5. Normalize selected flow concepts into annual and quarterly series before plotting.
6. Let scenario generation choose the few concepts that matter for each specific narrative crux.

For a company like Oracle, the narrative work should not stop at "RPO is growing." It should connect:

- RPO growth.
- Revenue and deferred revenue conversion.
- Capex and PPE growth.
- Lease and purchase obligations.
- Debt issuance and interest expense.
- Operating cash flow versus investing cash flow.
- Buyback slowdown or dilution risk.

That connection is where SEC Facts can create non-obvious, hard-to-find research value.
