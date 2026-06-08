# Phase 5 SQL-Only Research Check for ORCL

Database examined: `reports/stock-narrative-research/ORCL-2026-06-07-4/run.sqlite`

## Verdict

`sqlite3` alone is sufficient for a meaningful subset of Phase 5 calculation work in this ORCL workspace. I was able to use direct SQL against `sec_raw_facts`, `fundamental_observations`, and `raw_fact_metric_catalog` to discover non-trivial financial mechanics around backlog, capex intensity, free cash flow pressure, financing mix, lease obligations, and capital allocation without using a separate math notebook or custom execution runtime.

The limitation is not arithmetic. The limitation is data ergonomics. The SQL gets long and brittle once the work requires deduping amended filings, distinguishing annual vs. YTD vs. quarter periods, using `period_end` rather than unreliable `fiscal_year` labels from comparative disclosures, and promoting results into structured downstream artifacts.

## Workspace Shape Relevant To Phase 5

- `sec_raw_facts` has `25,369` rows, which is enough breadth for direct SQL exploration.
- `fundamental_observations` has `1,550` rows.
- `raw_fact_metric_catalog` already exposes concept-level summaries that help with SQL-first exploration.
- Phase-5-adjacent downstream tables are mostly empty in this run:
  - `supporting_metric_selections`: `0`
  - `narrative_map`: `0`
  - `narrative_map_items`: `0`
  - `content_blocks`: `0`
  - `content_block_metrics`: `0`
  - `scenario_assumptions`: `0`
  - `scenario_crux_assumptions`: `0`
  - `claims`: `0`
  - `sources`: `0`

That means SQL can discover mechanics, but this run does not yet give a structured place to store promoted/rejected experiment outputs inside the DB.

## What SQL Alone Can Discover

### 1. The raw SEC series are broad enough for mechanics work

Several of the concept families called out in `docs/references/sec-facts-insight-patterns.md` are present with enough history to analyze directly in SQL:

- `RevenueRemainingPerformanceObligation`: `31` periods, latest `2026-02-28`
- `ContractWithCustomerLiability`: `32` periods, latest `2026-02-28`
- `PaymentsToAcquirePropertyPlantAndEquipment`: `64` periods, latest `2026-02-28`
- `PropertyPlantAndEquipmentNet`: `68` periods, latest `2026-02-28`
- `NetCashProvidedByUsedInOperatingActivities`: `58` periods, latest `2026-02-28`
- `InterestExpense`: `72` periods, latest `2026-02-28`
- `PaymentsForRepurchaseOfCommonStock`: `72` periods, latest `2026-02-28`
- `ProceedsFromIssuanceOfSeniorLongTermDebt`: `17` periods, latest `2026-02-28`
- `OperatingLeaseLiability`: `13` periods, latest `2026-02-28`
- `LesseeOperatingLeaseLiabilityPaymentsDue`: `10` periods, latest `2026-02-28`
- `UnrecordedUnconditionalPurchaseObligationBalanceSheetAmount`: `15` periods, latest `2026-02-28`

This is enough coverage to support SQL-only versions of backlog, conversion, capex, leverage, obligations, and capital-allocation experiments.

### 2. Backlog and conversion mechanics are easy to explore in SQL

Latest instant values at `2026-02-28`:

- RPO: `$552.6B`
- Deferred revenue / contract liability: `$11.182B`
- Accounts receivable: `$10.719B`
- PPE: `$83.617B`
- Operating lease liability: `$21.31B`
- Lease payments due: `$28.726B`
- Unrecorded purchase obligations: `$11.0B`

Useful SQL-derived ratios:

- `RPO / deferred revenue = 49.4x`
- `RPO / accounts receivable = 51.6x`

Interpretation:

- The backlog signal is enormous relative to near-term balance-sheet conversion buckets.
- This supports a SQL-first narrative question very similar to the reference note: Oracle's reported demand pipeline is much larger than the amounts already sitting in deferred revenue or receivables, so the key Phase 5 issue becomes conversion timing and capital intensity rather than simply "is demand up?"

### 3. SQL can directly show a capex regime shift

Annual 10-K-based mechanics, keyed by `period_end`:

- FY ended `2025-05-31`:
  - Revenue: `$57.399B`
  - Operating cash flow: `$20.821B`
  - Capex: `$21.215B`
  - `Capex / revenue = 37.0%`
  - `Capex / OCF = 1.019x`
  - Approx. free cash flow proxy (`OCF - capex`) = `-$0.394B`

- FY ended `2024-05-31`:
  - Revenue: `$52.961B`
  - Operating cash flow: `$18.673B`
  - Capex: `$6.866B`
  - `Capex / revenue = 13.0%`
  - `Capex / OCF = 0.368x`
  - Free cash flow proxy = `$11.807B`

- FY ended `2023-05-31`:
  - Revenue: `$49.954B`
  - Operating cash flow: `$17.165B`
  - Capex: `$8.695B`
  - `Capex / revenue = 17.4%`
  - `Capex / OCF = 0.507x`
  - Free cash flow proxy = `$8.470B`

Interpretation:

- The SQL-only view shows a sharp break from the older Oracle pattern. Capex moved from a modest share of revenue and cash flow into a level that roughly consumed full-year operating cash flow by FY2025.
- This is exactly the kind of deterministic "financial mechanics" bridge Phase 5 is trying to produce.

### 4. The latest 9-month and quarter views show an even more extreme buildout

Nine-month YTD snapshots:

- `2026-02-28`:
  - Revenue YTD: `$48.173B`
  - OCF YTD: `$17.357B`
  - Capex YTD: `$39.170B`
  - `Capex / revenue = 81.3%`
  - `Capex / OCF = 2.257x`
  - Free cash flow proxy = `-$21.813B`
  - Interest expense YTD: `$3.160B`
  - Buybacks YTD: `$95M`
  - Senior debt issued YTD: `$44.544B`

- `2025-02-28`:
  - Revenue YTD: `$41.496B`
  - OCF YTD: `$14.664B`
  - Capex YTD: `$12.135B`
  - `Capex / revenue = 29.2%`
  - `Capex / OCF = 0.828x`
  - Free cash flow proxy = `$2.529B`
  - Interest expense YTD: `$2.600B`
  - Buybacks YTD: `$450M`
  - Senior debt issued YTD: `$19.548B`

I also derived Q3-only values in pure SQL by subtracting Q2 YTD from Q3 YTD:

- Quarter ended `2026-02-28`:
  - Revenue: `$17.190B`
  - OCF: `$7.151B`
  - Capex: `$18.635B`
  - `Capex / revenue = 1.084x`
  - `Capex / OCF = 2.606x`
  - Free cash flow proxy = `-$11.484B`
  - Interest expense: `$1.180B`
  - Senior debt issued: `$26.664B`

- Quarter ended `2025-02-28`:
  - Revenue: `$14.130B`
  - OCF: `$5.933B`
  - Capex: `$5.862B`
  - `Capex / revenue = 0.415x`
  - `Capex / OCF = 0.988x`
  - Free cash flow proxy = `$0.071B`
  - Interest expense: `$892M`
  - Senior debt issued: `$7.711B`

Interpretation:

- SQL-only analysis strongly supports a capital-intensity story: the latest quarter's capex exceeded quarterly revenue and more than doubled quarterly operating cash flow.
- That is a concrete Phase 5 mechanic, not just a prose narrative.

### 5. SQL also surfaces financing and capital-allocation changes

Annual buybacks from 10-K data:

- FY2025: `$0.6B`
- FY2024: `$1.202B`
- FY2023: `$1.3B`
- FY2022: `$16.248B`
- FY2021: `$20.934B`
- FY2020: `$19.240B`

Debt issuance:

- FY2025: `$19.548B`
- FY2023: `$33.494B`
- 9M FY2026 through `2026-02-28`: `$44.544B`
- Q3-only FY2026 quarter: `$26.664B`

Interest expense:

- FY2025: `$3.578B`
- FY2024: `$3.514B`
- FY2023: `$3.505B`
- 9M FY2026 through `2026-02-28`: `$3.160B`
- Q3-only FY2026 quarter: `$1.180B`

Interpretation:

- The SQL-only output suggests a major capital-allocation shift away from buybacks and toward infrastructure buildout and financing.
- Combined with the capex and obligation data, this gives a clear crux candidate: Oracle's AI/cloud growth narrative now depends on whether backlog conversion and unit economics can justify a much heavier financing and asset base.

## Difficulties With A SQL-Only Phase 5 Workflow

### 1. Period normalization is the main pain point

This was the hardest part by far.

- Some concepts are instant balance-sheet values.
- Some have annual 10-K durations.
- Some have YTD 10-Q durations.
- Some have quarter-only rows, while others require subtracting Q2 YTD from Q3 YTD.

This is all solvable in SQL, but the queries become verbose quickly. A separate math environment is not strictly required, but pre-modeled SQL views would help a lot.

### 2. `fiscal_year` is not always a safe grouping key

Comparative values in later filings can reuse earlier periods while carrying filing metadata from the newest report. I had better results grouping annual results by `period_end` plus concept and deduping by latest `filed_at`.

Implication:

- SQL-only research is feasible, but Phase 5 should not assume that naive `GROUP BY fiscal_year` logic is trustworthy.

### 3. Deduplication logic has to be repeated

For most serious queries I needed a pattern like:

- partition by concept + unit + period boundaries
- order by latest `filed_at` (and often `id`)
- keep `ROW_NUMBER() = 1`

This is okay for one-off analysis but annoying for iterative research. A reusable normalized view would remove a lot of friction.

### 4. The DB supports discovery better than experiment tracking

The raw facts are rich enough to do the math, but the tables that would hold promoted/rejected experiment outputs are empty in this run:

- no supporting metric selections
- no narrative map entries
- no content blocks or content block metrics
- no scenario assumptions
- no claims or sources persisted for later review

So the issue is not "can SQL find interesting mechanics?" It can. The issue is "where do the results live after discovery?"

### 5. Canonical starter fields are still incomplete

`run_metadata` says `financial_fetch_status = partial` with `missing fields: revenue, net margin, EPS`.

Even though `fundamental_observations` contains usable EPS and raw revenue histories, the top-level `fundamentals` table only held:

- current price
- market cap
- shares outstanding
- cash
- total debt

This means SQL-first analysis can often recover the needed mechanics from the raw layer, but the curated layer is not yet reliable enough to make those experiments easy.

### 6. Company baseline metadata is still sparse

`stock_info` has the ticker and company name, but `exchange`, `sector`, and `industry` are blank in this run.

That does not block direct SQL math, but it does weaken downstream interpretation and report assembly.

## What-If Modeling Attempt In Plain SQLite

I also attempted a real forward-looking scenario model directly in SQLite to test whether the workflow can move beyond historical bridges.

### What I Modeled

I built a 3-year bull / base / bear projection starting from the latest annual period ending `2025-05-31`.

Base historical starting point:

- Revenue: `$57.399B`
- Operating cash flow: `$20.821B`
- Capex: `$21.215B`
- Interest expense: `$3.578B`
- Operating income: `$17.678B`
- Diluted shares: `2.866B`

I then used explicit scenario assumptions for each projected year:

- revenue growth
- OCF margin
- capex as a percent of revenue
- operating margin
- growth in existing interest expense
- interest rate on incremental debt needed to fund any FCF deficit
- share growth / dilution

The model itself was a recursive CTE, not external code.

### Example Outputs

Projected results from the SQLite-only scenario run:

- `Bull`
  - 2026 revenue: `$67.73B`, FCF proxy: `$1.72B`, EPS proxy: `$3.89`
  - 2027 revenue: `$78.57B`, FCF proxy: `$5.42B`, EPS proxy: `$4.93`
  - 2028 revenue: `$89.57B`, FCF proxy: `$9.43B`, EPS proxy: `$6.07`
  - incremental debt needed: `$0.0B`

- `Base`
  - 2026 revenue: `$66.01B`, FCF proxy: `-$1.72B`, EPS proxy: `$3.70`
  - 2027 revenue: `$74.59B`, FCF proxy: `$0.66B`, EPS proxy: `$4.32`
  - 2028 revenue: `$82.79B`, FCF proxy: `$3.73B`, EPS proxy: `$5.13`
  - cumulative incremental debt needed: `$1.72B`

- `Bear`
  - 2026 revenue: `$63.14B`, FCF proxy: `-$5.74B`, EPS proxy: `$3.19`
  - 2027 revenue: `$68.19B`, FCF proxy: `-$5.68B`, EPS proxy: `$3.21`
  - 2028 revenue: `$72.96B`, FCF proxy: `-$4.77B`, EPS proxy: `$3.18`
  - cumulative incremental debt needed: `$16.2B`

I also ran a one-year sensitivity grid to test ergonomics:

- at `10%` revenue growth and `30%` capex intensity, FCF proxy was about `$1.89B`
- at `10%` revenue growth and `40%` capex intensity, FCF proxy was about `-$4.42B`
- at `20%` revenue growth and `30%` capex intensity, FCF proxy was about `$2.07B`
- at `20%` revenue growth and `40%` capex intensity, FCF proxy was about `-$4.82B`

The strongest practical lesson from that grid is that capex intensity mattered more than modest differences in top-line growth.

### What Felt Easier Than Expected

- Simple bull / base / bear scenarios were straightforward to encode with `VALUES` CTEs.
- Recursive CTEs were sufficient for a 3-year roll-forward.
- Deterministic formulas like:
  - revenue growth
  - margin assumptions
  - capex intensity
  - FCF proxy
  - incremental debt accumulation
  - interest carry on new debt
  worked fine in pure SQL.
- Sensitivity tables were also easy enough for low-dimensional cases.

This means SQLite is not limited to point-in-time analysis. It can support honest "what if" work as long as the model stays explicit and compact.

### What Felt Harder Than Expected

- The query became long quickly once assumptions varied by scenario and by year.
- Readability degraded faster than the math complexity justified.
- It is easy to lose confidence in the model if assumptions are only embedded in a large SQL statement instead of being stored in a small structured assumptions table.
- EPS-style outputs already started to feel less clean than revenue / OCF / capex / FCF outputs because they require more layered assumptions.
- A richer model would soon want:
  - scenario tables persisted in the DB
  - reusable prepared views for historical base metrics
  - named formulas or calculation blocks
  - a cleaner way to inspect intermediate steps

So the friction moved from "can SQLite do the arithmetic?" to "can humans comfortably author and audit the SQL?"

### Security And Runtime Implication

This practical test makes me more confident that a separate embedded expression runtime is not necessary for the first useful version of Phase 5.

For deterministic scenario work like:

- small numbers of scenarios
- a few explicit assumptions
- 1-3 projected years
- straightforward bridges into revenue, OCF, capex, FCF, interest, and simple EPS proxies

plain SQLite appears sufficient.

That matters because it reduces pressure to introduce an embedded scripting engine such as Rhai just to unlock basic scenario math. Given the security concerns around embedded evaluators, the bias should probably be:

- prefer SQLite + explicit assumptions tables + prepared views first
- add a more expressive math layer only if real scenario needs exceed what audited SQL can comfortably express

I would not say SQLite fully replaces a richer modeling layer forever. But this attempt suggests it is good enough to defer that complexity and security risk much longer than I initially expected.

## Bottom Line

For this ORCL workspace, `sqlite3` is strong enough to perform a real first pass of Phase 5 work without a separate code environment. I was able to produce:

- backlog/conversion checks
- capex intensity analysis
- free cash flow proxy calculations
- debt issuance versus interest pressure views
- lease and purchase obligation context
- capital-allocation shift evidence
- quarter-only derivations from YTD facts
- a real 3-year bull / base / bear what-if scenario model
- a simple sensitivity grid

What `sqlite3` alone does not give comfortably is a smooth research workflow. The missing pieces are:

- prebuilt normalized views for instant / annual / YTD / quarter shapes
- reusable deduped concept views
- a structured place to persist promoted and rejected experiments
- better curated/canonical headline fundamentals

If the product goal is "can the database itself support Phase 5 math?", the answer is yes.

If the product goal is "can analysts do Phase 5 comfortably with only ad hoc SQL and no extra modeling layer?", the answer is still only partially yes. But after attempting an actual forward model, I think the gap is more about authoring ergonomics than raw computational capability. That makes "SQLite first, no embedded scripting runtime yet" look like a plausible product path.
