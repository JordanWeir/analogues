# Data Quality Report

Version Note:
- This is after we adopted the "capture all SEC fact concepts"
- This is during the transition from "deterministically label core concepts" to "use an agent to promote concepts to canon table"
- Things like the Headline Debt issue come from a bad deterministic process, and should be fixed soon
- Things like the Period typing need to be investigated

## Verdict
Partial pass. reports/stock-narrative-research/ORCL-2026-06-07-2/run.sqlite is a useful initialized workspace with a strong raw SEC facts layer, but I would not trust the current headline fundamentals table as deterministic analysis input yet.

The main reason: raw data is broad and well-provenanced, but canonical/headline selection has serious labeling and mapping gaps.

## What It Captures Well

Workspace metadata exists: run_metadata has ticker, run slug, workspace path, schema_version=2, and status initialized.
SEC ingestion is broad: 25,369 raw SEC facts, 516 raw concept/unit catalog rows, and coverage through Oracle’s FY26 Q3 filing on 2026-03-11.
Provenance is strong in sec_raw_facts: all raw rows had raw_json, accession, and filed_at.
Oracle-specific narrative signals are preserved, especially RevenueRemainingPerformanceObligation: latest DB value $552.6B, matching Oracle’s Q3 FY26 release rounded to $553B.
Raw cash-flow and capex data exists, including FY26 YTD operating cash flow $17.357B and PP&E purchases $39.17B.

## Data Quality Findings

Critical: Headline debt is materially wrong. fundamentals.total_debt is $7.271B, which is stale/current-only. Oracle’s Q3 FY26 release shows current notes/borrowings $9.887B and non-current notes/borrowings $124.718B. The raw DB has NotesPayableCurrent latest $9.887B, but the headline layer is not using it and does not capture latest non-current borrowings.

High: Period typing is misleading. FY26 Q3 YTD revenue $48.173B, net income $12.783B, EPS $4.38, and operating income $14.473B are labeled period_type='annual', even though they are nine-month YTD 10-Q facts. Q2 YTD duration facts are labeled instant. This can silently corrupt annual, TTM, and margin math.

High: Starter fundamentals are too sparse. The fundamentals table has only price, market cap, shares, cash, and total debt. It omits revenue, EPS, net income, operating income, margins, OCF, capex, and FCF despite many of those values existing in fundamental_observations or sec_raw_facts.

Medium: Company profile is incomplete. stock_info has ticker/name/currency, but exchange, sector, industry, and CIK are null/not represented. External profile sources identify ORCL as NYSE-listed, USD-reporting, CIK 0001341439, technology/software infrastructure.

Medium: Canonical mappings are still heuristic seeds. Mappings include useful concepts, but no high-confidence company-reviewed precedence. debt_noncurrent has a definition but no active mapping. Shares also mixes common shares outstanding and weighted-average diluted shares under one canonical key.

## Web Validation

Oracle Q3 FY26 revenue: DB quarterly revenue $17.190B; Oracle release says Q3 total revenue $17.2B. Good.
Oracle Q3 FY26 GAAP operating income: DB $5.464B; Oracle release says $5.5B. Good.
Oracle Q3 FY26 GAAP net income: DB $3.721B; Oracle release says $3.7B. Good.
Oracle Q3 FY26 diluted EPS: DB $1.27; Oracle release says $1.27. Good.
Cash at Feb. 28, 2026: DB $38.455B; Oracle release balance sheet says $38.455B. Good.
Market quote: DB price $213.68; search result for Jun. 5, 2026 close shows about $213.67/$213.68. Good, but market cap should carry timestamp/method notes.

## Recommendations

Fix duration classification: distinguish quarter, ytd, annual, ttm, and instant; do not label 9-month 10-Q facts as annual.
Add debt canonical mappings for Oracle-style NotesPayableCurrent, LongTermNotesPayable/borrowings equivalents, and a derived total debt with inputs persisted.
Promote deterministic starter fundamentals from existing observations: latest quarter revenue, YTD/annual revenue, net income, operating income, EPS, OCF, capex, FCF, cash, current debt, noncurrent debt, total debt, net debt.
Persist company identity fields: CIK, exchange, sector, industry, fiscal year convention, and quote timestamp/source.
Upgrade quality flags from info to actionable severities when headline fields are missing, stale, mixed-period, or derived from incomplete inputs.