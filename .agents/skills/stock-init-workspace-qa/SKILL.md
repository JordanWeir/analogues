---
name: stock-init-workspace-qa
description: Performs exploratory data quality QA for a user-provided SQLite file from stock research workspace initialization and initWorkspace outputs. Use when validating generated stock run databases, SEC Facts ingestion, canonical concept selection, starter fundamentals, narrative/scenario readiness, or whether initialization works well across different public companies. If the user does not provide a SQLite file path, ask for it before starting.
---

# Stock Init Workspace QA

Use this skill to evaluate whether `initWorkspace` produced a useful, trustworthy starting point for the Analogues stock research product. The goal is not to follow one fixed query path. Explore the generated artifacts, infer the schema in front of you, and test whether the workspace contains enough durable data for narrative research, scenario-conditioned projections, and later agent work.

Do not run the full stock research workflow unless the user asks. Prefer readonly inspection and targeted web validation.

## Required Input

Start from a specific SQLite database path supplied by the user, such as `reports/stock-narrative-research/<TICKER>-<DATE>-<INDEX>/run.sqlite`.

If the user asks for this QA workflow but does not provide a SQLite file path, stop and ask them for the path. Do not search broadly for candidate run databases unless the user explicitly asks you to find one.

Once the SQLite path is known, you may infer the surrounding run directory and inspect nearby generated artifacts when useful.

## Product Context

Analogues depends on initialization to create a durable research substrate, not just a few headline fundamentals. A good initialized workspace should help later stages:

- Extract deterministic stock/company data.
- Link SEC Facts concepts to familiar core metrics like revenue, EPS, cash, debt, and shares.
- Preserve a broad enough SEC Facts universe to discover company-specific or "exotic" time series.
- Support narrative, scenario, crux, and watch-item analysis.
- Distinguish historical, projected, and mixed periods.
- Preserve provenance so claims and calculations can be audited.

Treat QA as answering: "Could a later research agent build a smart, company-specific scenario report from this workspace without silently making up or re-fetching everything?"

## Exploration Posture

Start broad, then narrow. Let the database and generated files tell you what exists.

1. Confirm the user-provided SQLite path exists and open it readonly.
2. Inspect the SQLite schema, views, table counts, and representative rows.
3. Look for data families, not just table names: company profile, market quote, raw facts, curated fundamentals, concept mappings, observations/time series, quality flags, gaps, sources, claims, sections, scenarios, artifacts.
4. Compare expected product needs against actual persisted data.
5. Validate the highest-impact values online.
6. Report what is present, what is missing, what is stale or misleading, and what should be improved in `initWorkspace`.

Current schemas may include tables such as `stock_info`, `fundamentals`, `fundamental_observations`, `sec_raw_facts`, `canonical_metric_mappings`, `data_gaps`, or `data_quality_flags`, but do not assume those exact names are the only valid structure. If the schema changes, adapt.

## Discovery Techniques

Use readonly SQLite. Start with schema discovery:

```sql
SELECT name, type
FROM sqlite_master
WHERE type IN ('table', 'view')
ORDER BY type, name;
```

For unfamiliar schemas, inspect columns dynamically:

```sql
SELECT m.name AS object_name, p.cid, p.name AS column_name, p.type, p.pk
FROM sqlite_master m
JOIN pragma_table_info(m.name) p
WHERE m.type IN ('table', 'view')
ORDER BY m.name, p.cid;
```

Then count rows for all meaningful tables. If writing a one-off helper query or shell loop is faster, do that, but keep it readonly.

Search for columns that imply important data families:

- Identity/profile: `ticker`, `company`, `exchange`, `sector`, `industry`, `currency`, `cik`.
- Market data: `price`, `market_cap`, `shares`, `quote`, `as_of`, `timestamp`.
- SEC facts/concepts: `taxonomy`, `concept`, `label`, `description`, `unit`, `frame`, `accession`, `filed`, `period`.
- Canonicalization: `canonical`, `metric_key`, `mapping`, `confidence`, `rationale`, `selected_by`.
- Time series: `period_start`, `period_end`, `fiscal_year`, `fiscal_period`, `value`, `source`.
- Provenance: `source`, `url`, `claim`, `citation`, `raw_json`, `fetched_at`.
- Readiness: `gap`, `flag`, `quality`, `status`, `section`, `scenario`, `artifact`.

Sample representative rows instead of relying only on counts. Empty tables can be fine if they are meant for later workflow stages; they are a problem if initialization is supposed to populate them.

## What To Evaluate

### 1. Workspace Shape

Check whether the provided SQLite file appears to belong to a coherent initialized workspace. When the surrounding run directory is available, also check generated output directories, schema metadata, required placeholder sections, and any version/run metadata. Flag missing artifacts or artifacts that are present but unusable.

Ask:

- Can a future task locate the run deterministically?
- Is the schema version or run metadata recorded?
- Are empty placeholders clearly distinguished from failed ingestion?
- Are gaps and quality concerns persisted in a way downstream agents will see?

### 2. Company And Market Baseline

Verify that the workspace identifies the company and market context well enough for research.

Look for ticker, legal/company name, exchange, currency, sector, industry, fiscal year convention, CIK or filing identity, quote timestamp, current price, shares, and market cap.

Common issues:

- Company profile fields are blank with no gap logged.
- Quote data lacks timestamp or source.
- Market cap is derived from stale or mismatched shares.
- Currency is assumed but not sourced.
- Ticker maps to the wrong share class, exchange, or company.

### 3. SEC Facts Breadth

The product needs both familiar metrics and company-specific time series. For a mature SEC filer, raw facts should often include dozens to hundreds of unique concepts. A tiny concept set can be correct for a constrained curated layer, but suspicious for the raw ingestion layer.

Look for:

- Raw concept count and fact count.
- Concept labels, descriptions, units, taxonomy, accession, form, period, filing date, and raw payload custody.
- Coverage across income statement, balance sheet, cash flow, segment metrics, obligations/backlog, and company-specific disclosures.
- Whether concepts are preserved even if they are not selected as canonical fundamentals.

Flag:

- Raw facts are absent while curated observations exist.
- Only seed/core concepts are persisted.
- Facts have values but no provenance.
- Labels/descriptions/units are missing, making concept selection hard.
- Filing lag is not represented, especially around recent earnings.

### 4. Canonical Concept Linking

Core fundamentals need stable canonical links, but those links should be auditable and not over-normalized.

Evaluate:

- Which canonical metrics exist and why.
- Which raw concepts feed each canonical metric.
- Whether mappings include confidence/rationale/source.
- Whether canonical metrics distinguish annual, quarterly, YTD, TTM, and instant balance sheet facts.
- Whether canonical choices are company-appropriate rather than blindly generic.

Common issues:

- A canonical metric exists but no observations support it.
- Observations have canonical keys without definitions.
- Mappings are missing, so canonicalization cannot be audited.
- Semantically different concepts are merged, such as common shares outstanding vs. weighted average diluted shares.
- `total_debt` is actually only current debt, only long-term debt, or stale debt.
- Revenue maps to multiple concepts without a clear precedence rule.

### 5. Starter Fundamentals

Starter fundamentals should be useful but not falsely complete. Compare headline fields to the richer observation/fact layer.

High-priority fields:

- Price, shares outstanding, market cap.
- Latest annual and latest quarter revenue.
- Net income, operating income, EPS, margins.
- Cash and equivalents.
- Current debt, non-current debt, total debt, net debt.
- Operating cash flow, capex, free cash flow when available.

Flag:

- Headline/starter values are missing even though raw facts contain them.
- Values are stale relative to newer filings.
- YTD values are labeled as annual or TTM.
- Instant balance sheet values are mixed with income-statement periods without explanation.
- Derived ratios lack formula/source inputs.
- Data gaps remain open but are easy to close from persisted observations.

### 6. Narrative And Scenario Readiness

Initialization does not need to write the final report, but it should leave enough structured data for later narrative and scenario work.

Look for:

- A broad concept catalog that later agents can mine for "interesting concepts."
- Company-specific time series that could matter for scenarios, such as RPO, backlog, cloud revenue, segment revenue, subscribers, units, production, capex, free cash flow, customer counts, or reserves.
- Fields or placeholders for sources, claims, scenarios, crux assumptions, signals/watch items, and artifacts.
- Explicit handling of historical vs. projected vs. mixed periods.

Flag:

- The workspace forces all later work to re-fetch source data.
- Company-specific facts are discarded during initialization.
- There is no place to persist scenario-specific metric selections.
- Downstream sections exist only as empty text with no data hooks, source hooks, or gap records.

## Web Validation

Validate important fields against external sources. Prefer official sources for filed financials:

- SEC 10-K/10-Q filings.
- Company investor relations earnings releases.
- Company profile / exchange pages for listing metadata.
- Market data providers for price and market cap, with timestamp awareness.
- Reputable financial media only as secondary context.

Always web-check the values most likely to corrupt downstream analysis:

- Company identity, exchange, sector/industry, reporting currency.
- Current price, quote timestamp, shares, and market cap.
- Latest annual and latest quarter revenue, operating income, net income, EPS.
- Cash, current debt, non-current debt, and total debt.
- Any company-specific metric likely to drive the narrative or scenarios.

When sources disagree, explain the likely reason: filing date, market timestamp, after-hours price, basic vs. diluted shares, current shares vs. weighted-average shares, annual vs. YTD, GAAP vs. non-GAAP, or provider methodology.

Do not just say "looks right." Record the DB value, external value, source type, and whether the difference matters.

## Cross-Company QA

When validating the workflow across a range of companies, choose tickers that stress different failure modes:

- Mega-cap software/cloud company with many segment and backlog concepts.
- Manufacturer or semiconductor company with inventory, capex, and segment data.
- Bank or insurer where standard revenue/debt concepts may be misleading.
- Retailer with same-store sales, inventory, leases, and fiscal calendar quirks.
- Energy/materials company with reserves, production, commodity exposure, and non-standard metrics.
- Recent IPO or foreign issuer where SEC Facts coverage may be sparse.

Compare not just pass/fail, but how useful each initialized workspace would be for narrative-scenario research.

## Reporting

Keep the report exploratory and evidence-based. Lead with the verdict, then explain what you found.

Suggested structure:

```markdown
## Verdict
[Pass / partial / fail with the main reason.]

## What The Workspace Captures Well
- [Evidence-backed strengths.]

## Data Quality Findings
- [Severity] [Finding]: [where observed, why it matters, suggested fix.]

## Product Readiness
[Whether the initialized data supports deterministic fundamentals, canonical concepts, interesting concept discovery, provenance, and scenario work.]

## Web Validation
[Important checked fields with DB value, external value, source type, and status.]

## Recommendations
- [Concrete improvements to initWorkspace, schema, source retention, or QA tests.]
```

Severity guide:

- `Critical`: Later workflow cannot rely on the workspace, or a headline value is materially wrong.
- `High`: Core data is missing, stale, mislabeled, or not auditable.
- `Medium`: Data is present but ambiguous, hard to query, or likely to confuse scenario work.
- `Low`: Naming, formatting, source note, or ergonomics issue.

Do not provide investment advice. The QA target is data quality and product readiness.
