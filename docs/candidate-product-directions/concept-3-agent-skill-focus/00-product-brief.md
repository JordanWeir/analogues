# Product Brief: 90-Minute Stock Narrative Research Skill

## 1. Product Concept

The **90-Minute Stock Narrative Research Skill** is a portable AI Skill that helps users go from cold to conversationally fluent on a stock in roughly 90 minutes.

The Skill guides an AI assistant through a structured research workflow that produces a stock narrative report covering:

* company and business model basics
* why the stock matters now
* current market narratives
* bull, bear, consensus, and counter-narrative views
* key financial math and assumptions
* conditional scenario projections
* implied price bands under different scenarios
* industry and ecosystem context
* historical analogues and failure modes
* what to watch next
* final conversational talk tracks

The Skill is designed to be distributed directly to early users as a functional research workflow, allowing fast feedback before committing to a full application.

It is also designed as a **product taste calibration tool** for the builder. Running full-depth reports against real portfolio and watchlist names should reveal which sections feel valuable, which sections get skipped, which outputs feel shareable, and which missing pieces can be added quickly through Skill updates.

The core promise:

> Use this Skill to research a narrative-driven stock and produce a structured report that helps you understand the story, the math, the scenarios, and the historical context.

---

## 2. Target User / Persona

### Primary User

The initial target user is:

**Finance writers, independent analysts, and sophisticated retail investors who already use AI tools for research.**

These users are comfortable with:

* ChatGPT / Claude / Perplexity-style research workflows
* asking iterative follow-up questions
* reading long-form outputs
* manually checking sources
* editing and reusing generated research
* publishing or discussing market narratives

They may not need a polished app at first. They need a powerful repeatable workflow.

### Secondary User

The secondary user is:

**A serious investor or market-curious professional who wants a structured research assistant rather than a generic stock summary.**

They may not publish their work, but they want to understand:

* what the company does
* why the stock is moving
* what bulls and bears believe
* what assumptions drive the stock
* what could create upside/downside
* what historical analogues are relevant

---

## 3. Product Philosophy

The Skill implementation is built around six principles.

### 1. The Skill is the fastest way to validate the workflow

A full app requires:

* data ingestion
* source storage
* financial data integration
* scenario modeling UI
* charts
* accounts
* saved reports
* watchlists
* alerting
* historical analogue database

A Skill can validate the core research workflow much earlier.

The Skill should answer:

> Do users actually value this kind of structured narrative research?

It should also answer:

> When the builder uses this on real stocks they care about, which parts feel like the future product?

### 2. The Skill should be useful even without proprietary infrastructure

The Skill should produce useful reports with:

* web search
* user-provided sources
* public filings
* public transcripts
* public news
* accessible market commentary
* manually entered financial data
* user-provided assumptions

It should not depend on a proprietary backend to be valuable.

### 3. The full report shape is necessary for product taste

The Skill should preserve the full 90-minute report shape during early testing.

The point of the Skill is not only to test whether a short AI summary is useful. It is to let the builder and early users experience the whole research arc:

* orientation
* business model
* why now
* narrative map
* financial math
* scenarios
* charts
* historical analogues
* watch items
* talk track

Depth is required to discover:

* which sections users skip
* which sections they revisit
* which sections they want to share
* which sections feel generic
* which sections feel like product hooks
* which missing pieces can be fixed with a quick Skill update

### 4. The Skill should expose product gaps honestly

The Skill should reveal what works well in a prompt/agent workflow and what requires a real application.

Likely strengths:

* structured report generation
* narrative mapping
* source synthesis
* scenario reasoning
* explainable assumptions
* charted scenario outputs
* writer-friendly exports

Likely weaknesses:

* persistent data
* repeatability
* financial data accuracy
* automated market data
* watchlists
* alerts
* historical analogue library
* source quality control
* user interface

### 5. The Skill should be source-disciplined

The Skill must enforce:

* source-backed claims
* separation of facts, interpretations, and projections
* clear uncertainty labels
* no buy/sell/hold recommendations
* no price target framing
* explicit assumptions
* historical analogue caveats

### 6. The Skill should be a distribution wedge

A useful Skill can be distributed to early users who already trust AI workflows.

The Skill can serve as:

* prototype
* research assistant
* user interview artifact
* competitive benchmark
* early distribution channel
* future companion to a full app

---

## 4. Golden Workflow

The user invokes the Skill with a ticker, company, or market narrative.

Example prompts:

```text
Use the Stock Narrative Research Skill to analyze SMCI.
```

```text
Use the 90-minute research workflow on NVDA.
```

```text
Research the current narrative around a stock like Super Micro and produce scenario-conditioned projections.
```

```text
I want to understand the bull/bear story, financial assumptions, scenarios, and historical analogues for [ticker].
```

The Skill then guides the assistant through a structured workflow.

### Skill Workflow

```text
1. Clarify target and scope if necessary.
2. Gather source pack.
3. Extract business model and recent events.
4. Build current narrative map.
5. Extract key claims and assumptions.
6. Build financial snapshot.
7. Construct conditional scenarios.
8. Build scenario projection assumptions.
9. Calculate illustrative revenue/EPS/valuation outputs where possible.
10. Identify industry context.
11. Find historical analogue candidates.
12. Generate final report.
13. Run quality checklist.
14. Produce optional talk-track and export variants.
```

### Ideal User Outcome

After using the Skill, the user should be able to explain:

* what the company does
* why the stock matters now
* what the current market narrative is
* what bulls believe
* what bears believe
* what assumptions actually matter
* what financial variables drive the stock
* how different scenarios could evolve
* what might lead to big upside or downside
* what historical analogues are relevant
* why those analogies may be misleading
* what to watch next

---

## 5. Skill Package Structure

The Skill should be structured as a folder that can be distributed, installed, copied, or adapted.

Possible structure:

```text
stock-narrative-research-skill/
  SKILL.md
  README.md
  glossary.md
  source-and-framing-rules.md
  source-policy.md

  templates/
    90-minute-report.md
    report-shell.html
    source-pack.md
    claim-table.md
    narrative-map.md
    scenario-projection.md
    historical-analogue-card.md
    final-talk-track.md
    writer-export.md

  rubrics/
    narrative-quality-rubric.md
    source-quality-rubric.md
    analogue-quality-rubric.md
    projection-quality-rubric.md
    final-report-checklist.md

  schemas/
    report.schema.json
    scenario-data.schema.json

  examples/
    smci-example-outline.md
    ai-infrastructure-example.md
    biotech-catalyst-example.md
    accounting-risk-example.md

  scripts/
    fetch_financials.py
    projection_model.py
    render_report.py
    clean_financial_table.py
    source_dedupe.py
    extract_claims.py

  outputs/
    report.html
    report.json
    scenario-data.json
```

The first functional version may only require:

```text
SKILL.md
templates/90-minute-report.md
templates/report-shell.html
templates/scenario-projection.md
rubrics/final-report-checklist.md
source-policy.md
source-and-framing-rules.md
scripts/fetch_financials.py
scripts/projection_model.py
schemas/report.schema.json
schemas/scenario-data.schema.json
```

---

## 6. Skill Invocation Rules

The Skill should be invoked when a user asks to:

* analyze a stock
* understand a stock narrative
* produce a 90-minute stock research report
* compare bull and bear narratives
* build scenario projections
* understand what could move a stock up or down
* identify historical analogues for a stock
* prepare a stock research memo
* prepare to discuss a company intelligently
* write a finance article or outline about a stock

The Skill should not be invoked for:

* casual price checks
* direct buy/sell advice
* portfolio allocation
* tax advice
* legal advice
* high-frequency trading signals
* options trading recommendations
* requests for guaranteed predictions

---

## 7. Skill Output: 90-Minute Research Report

The primary Skill output is a structured report.

### Report Sections

```text
# 90-Minute Stock Narrative Research Report: [Ticker / Company]

## 1. Five-Minute Orientation
## 2. Business Model Primer
## 3. Why Now?
## 4. Narrative Map
## 5. Financial Math and Market Assumptions
## 6. Conditional Scenario Projections
## 7. Industry and Ecosystem Context
## 8. Historical Analogues
## 9. What to Watch Next
## 10. Final Talk Track
## 11. Source Notes and Limitations
```

The report should be readable as a standalone memo, while also supporting iterative follow-up.

---

## 8. Five-Minute Orientation

Purpose:

> Give the user a fast, memorable understanding of the company and stock setup.

### Required Output

```text
Company:
[Plain-English description]

Why it matters:
[Why investors are paying attention now]

Current narrative:
[Dominant stock story]

Main debate:
[Central bull/bear disagreement]

Key numbers:
[Revenue growth, margins, valuation, stock move, market cap, or other relevant metrics]

Key pivots:
[Events or metrics that could change the story]
```

### Skill Instructions

The Skill should:

* avoid jargon in this section
* explain the company in simple terms
* state why the stock is being debated now
* identify the central crux, not just list pros and cons
* provide a mental hook the user can remember

---

## 9. Business Model Primer

Purpose:

> Help the user understand the economic engine.

### Required Questions

The Skill should answer:

* What does the company sell?
* Who buys it?
* How does the company make money?
* Where does it sit in the value chain?
* Is revenue recurring, transactional, cyclical, project-based, regulated, or event-driven?
* What are the margin characteristics?
* What creates pricing power?
* What are the main dependencies?
* What business-quality classification is the market debating?

### Output Format

```text
Business type:
[...]

Customers:
[...]

Revenue model:
[...]

Margin structure:
[...]

Value chain position:
[...]

Main dependencies:
[...]

Core business-quality debate:
[...]
```

### Skill Instructions

The Skill should explicitly distinguish between:

* business growth
* business quality
* stock valuation
* narrative framing

For many stocks, the key debate is not whether the company is growing. It is what kind of business the market should treat it as.

---

## 10. Why Now?

Purpose:

> Explain the recent catalyst or context that made the stock interesting.

### Required Output

| Layer          | What Changed? | Why It Matters |
| -------------- | ------------- | -------------- |
| Business       |               |                |
| Stock          |               |                |
| Narrative      |               |                |
| Credibility    |               |                |
| Sector / Theme |               |                |

### Skill Instructions

The Skill should search for and summarize:

* recent earnings
* recent guidance
* recent stock move
* recent controversy
* major news event
* sector/theme change
* valuation re-rating or de-rating
* change in management language
* external catalysts

The Skill should separate:

```text
What happened to the company?
What happened to the stock?
What happened to the narrative?
```

---

## 11. Narrative Map

Purpose:

> Show the live debate around the stock.

### Required Output

```text
Dominant narrative:
[...]

Bull narrative:
[...]

Bear narrative:
[...]

Consensus / base narrative:
[...]

Emerging counter-narrative:
[...]

What bulls and bears agree on:
- [...]

What bulls and bears actually disagree about:
- [...]
```

### Skill Instructions

The Skill should extract and organize claims into:

* bull claims
* bear claims
* neutral facts
* contested assumptions
* emerging concerns
* management framing
* market commentary

It should avoid making the bull case a strawman or the bear case a strawman.

The highest-value output is identifying the true cruxes.

Examples of cruxes:

* Is growth durable?
* Are margins temporary or structural?
* Is demand cyclical or secular?
* Is the company differentiated or commoditized?
* Is the valuation discount deserved?
* Is credibility repair likely?
* Is the market over-extrapolating?
* Is the company exposed to a real trend but capturing poor economics?

---

## 12. Financial Math and Market Assumptions

Purpose:

> Translate the narrative into numbers.

### Required Outputs

#### A. Financial Snapshot

The Skill should gather or ask the user for:

* current share price
* market cap
* revenue
* revenue growth
* gross margin
* operating margin
* net income
* EPS
* diluted share count
* cash/debt
* P/S
* P/E
* EV/Sales
* EV/EBITDA, where relevant

If exact data is unavailable, the Skill should say so and either:

* use clearly labeled approximate values
* ask the user to provide numbers
* produce a qualitative version without projection calculations

#### B. Key Math Drivers

The Skill should identify the 3–7 drivers that most affect the stock.

Examples:

* revenue growth
* gross margin
* operating margin
* EPS
* share count
* cash conversion
* P/S multiple
* P/E multiple
* approval probability
* peak sales
* commodity price
* debt paydown

#### C. Market-Implied Assumption Map

The Skill should explain what the stock appears to depend on.

Example:

```text
The stock appears to depend on:
- continued demand growth
- margin stability
- credibility repair
- valuation multiple normalization
```

#### D. Sensitivity Table

| Assumption | Bull Version | Bear Version | Why It Matters |
| ---------- | ------------ | ------------ | -------------- |

#### E. Upside / Downside Mechanics

```text
Large upside could happen if:
- [...]

Large downside could happen if:
- [...]
```

### Skill Instructions

The Skill should not pretend to have precise institutional data if it does not.

It should clearly label:

* exact sourced data
* approximate public data
* user-provided data
* model assumptions
* missing data

---

## 13. Conditional Scenario Projections

Purpose:

> Convert each narrative scenario into explicit assumptions and illustrative implied price bands.

This is the Skill’s most important analytical feature.

### Definition

A **Conditional Scenario Projection** says:

> If this scenario plays out, and if the market values the company within this range of multiples, these would be the implied financial outputs and price bands.

It is not a prediction or a price target.

### Required Scenarios

The Skill should usually produce 4–5 scenarios:

1. Bull narrative validates
2. Good business, bad stock
3. Credibility repair / multiple normalization
4. Bear narrative takes control
5. Sector or theme de-rating

The exact scenarios can vary by company type.

### Required Scenario Output

For each scenario:

```text
Scenario name:
[...]

Narrative description:
[...]

Operating assumptions:
- revenue growth
- gross margin
- operating margin / net margin
- share count
- EPS path

Valuation assumptions:
- P/S low / median / high
- P/E low / median / high
- blend weighting, if used

Implied outputs:
- revenue per share
- EPS
- P/S implied price range
- P/E implied price range
- blended implied price range

Key sensitivities:
- [...]

What would confirm this scenario:
- [...]

What would break this scenario:
- [...]
```

---

## 14. Projection Math

The Skill should use simple, transparent math.

### Revenue Projection

```text
Revenue_t = Revenue_(t-1) × (1 + Revenue Growth Rate_t)
```

### Revenue Per Share

```text
Revenue Per Share_t = Revenue_t / Diluted Shares_t
```

### Net Income Projection

Simple model:

```text
Net Income_t = Revenue_t × Net Margin_t
```

Optional detailed model:

```text
Gross Profit_t = Revenue_t × Gross Margin_t
Operating Income_t = Revenue_t × Operating Margin_t
Net Income_t = Operating Income_t × (1 - Tax Rate_t)
```

### EPS Projection

```text
EPS_t = Net Income_t / Diluted Shares_t
```

### P/S Implied Price

```text
P/S Implied Price_t = Revenue Per Share_t × P/S Multiple_t
```

### P/E Implied Price

```text
P/E Implied Price_t = EPS_t × P/E Multiple_t
```

### Blended Implied Price

```text
Blended Price_t =
(P/S Implied Price_t × P/S Weight_t) +
(P/E Implied Price_t × P/E Weight_t)
```

Where:

```text
P/S Weight_t + P/E Weight_t = 1
```

### Low / Median / High Bands

For each period, calculate:

```text
Low implied price
Median implied price
High implied price
```

for:

* P/S valuation
* P/E valuation
* blended valuation, if used

### Projection Periods

Default periods:

```text
Current
+6 months
+12 months
+24 months
+36 months
```

The Skill may adjust for event-driven companies.

### Negative or Distorted EPS

If EPS is negative, near-zero, or distorted:

* do not rely primarily on P/E
* use P/S, EV/Sales, gross profit multiple, EBITDA multiple, book value, NAV, or domain-specific model if appropriate
* clearly label limitations

### Skill Instructions

The Skill should:

* show the math
* show assumptions
* show when numbers are approximate
* avoid false precision
* avoid price-target language
* invite user overrides
* explain which assumptions matter most

---

## 15. Skill Projection Modes

The Skill should support three levels of projection depending on available data.

### Mode 1: Qualitative Projection

Used when data is insufficient.

Output:

* scenario description
* financial drivers
* likely direction of revenue/EPS/multiple
* no numeric implied price bands

### Mode 2: Simple Numeric Projection

Used when basic public financial data is available.

Output:

* revenue growth assumptions
* margin assumptions
* revenue per share
* EPS
* P/S implied price
* P/E implied price
* low/median/high ranges

### Mode 3: User-Provided Model Projection

Used when the user provides data or assumptions.

Output:

* more accurate scenario tables
* user-controlled assumptions
* model calculations
* optional chart-ready table

This mode is useful because early Skill users may be willing to paste:

* revenue
* EPS
* share count
* current multiples
* peer multiples
* management guidance
* their own assumptions

---

## 16. Charting and Visualization Constraints

A Skill may not have a native product UI, but charted scenario output is still a core requirement.

The Skill should close the loop from:

```text
scenario description
→ assumptions
→ basic financial model
→ JSON data
→ rendered HTML report with charts
```

The HTML report should use a reusable template rather than custom HTML generated for each invocation.

The preferred approach is:

* the agent generates or updates structured JSON files
* scripts fetch or normalize basic financial data where possible
* scripts calculate scenario outputs
* a static HTML report shell renders the output from JSON
* charts are rendered with `d3.js` loaded from a CDN

This allows the Skill to test the product feel of charts and scenario visualization without requiring a full web application.

### Chart-Ready Output

For each scenario, the Skill should produce both human-readable tables and structured JSON.

Example table:

| Period | Low Price | Median Price | High Price | Revenue / Share | EPS | P/S Median | P/E Median |
| ------ | --------: | -----------: | ---------: | --------------: | --: | ---------: | ---------: |

Required machine-readable outputs:

```text
outputs/report.json
outputs/scenario-data.json
outputs/report.html
```

The JSON files should include:

* company and ticker metadata
* report section text
* source notes
* financial snapshot
* scenario list
* scenario assumptions
* scenario period outputs
* low / median / high price bands
* key sensitivities
* watch items

### Reusable HTML Report Shell

The HTML report should be a template that renders from JSON.

It should include the overall report structure:

* orientation
* business model
* why now
* narrative map
* financial math
* scenario projections
* charts
* historical analogues
* what to watch
* final talk track
* source notes

The HTML file should not require custom per-invocation HTML authoring. Per-run variation should come from the JSON data.

The template may load D3 from a CDN:

```html
<script src="https://cdn.jsdelivr.net/npm/d3@7"></script>
```

The charting code should render:

* scenario low / median / high bands over time
* current price reference line, where available
* revenue per share or EPS paths, where available
* assumption tables
* sensitivity tables

### Financial Data and Projection Scripts

The Skill should include basic scripts to reduce manual work and improve repeatability.

Initial scripts may include:

* `fetch_financials.py`: fetch basic public financial data from a source such as Yahoo Finance / `yfinance`
* `projection_model.py`: calculate revenue, EPS, multiple-based implied ranges, and scenario tables
* `render_report.py`: combine report JSON and scenario JSON into the reusable HTML shell

The scripts should be treated as pragmatic pre-MVP utilities, not institutional-grade data infrastructure.

### Skill Limitation

Unlike a full product, the Skill may not provide:

* interactive sliders
* persistent assumptions
* saved scenarios
* dynamic chart updates
* user-friendly editing UI

This is a major comparison point against the app version.

---

## 17. Industry and Ecosystem Context

Purpose:

> Explain the broader system around the company.

### Required Output

```text
Broader theme:
[...]

Value chain:
[...]

Where the company sits:
[...]

Who captures the economics:
[...]

Key peers / adjacent companies:
[...]

External indicators to watch:
[...]

Why this matters for the stock:
[...]
```

### Skill Instructions

The Skill should help users avoid simplistic theme exposure.

Example:

Bad:

```text
This company is exposed to AI, so it benefits from AI growth.
```

Better:

```text
The company is exposed to AI infrastructure spending, but the key question is whether it captures durable profit pool economics or primarily high-volume, lower-margin implementation revenue.
```

---

## 18. Historical Analogues

Purpose:

> Give the user historical context and pattern recognition.

### Required Output

The Skill should produce 3–5 historical analogue candidates.

For each:

```text
Analogue:
[Company / period / narrative type]

Why it is similar:
[...]

How it played out:
[...]

Financial pattern:
- revenue
- margins
- EPS
- multiple
- stock performance, if available

Key pivots:
[...]

Lesson:
[...]

Why the analogy may be misleading:
[...]
```

### Analogue Types

The Skill should search for analogues by narrative pattern, not just sector.

Common patterns:

* infrastructure buildout winner
* picks-and-shovels theme beneficiary
* high-growth hardware supplier with margin pressure
* product launch ramp
* regulatory overhang
* credibility/accounting-risk growth company
* capex cycle beneficiary
* commodity windfall
* turnaround with operating leverage
* platform transition
* category disruption

### Skill Instructions

The Skill should avoid shallow analogies.

Bad:

```text
This is like Cisco because both are infrastructure.
```

Better:

```text
This resembles Cisco-era infrastructure narratives in that both involved suppliers tied to a major computing buildout. The analogy may be misleading because margin structure, customer concentration, supply constraints, competitive dynamics, and valuation context differ materially.
```

### Historical Data Limitation

The Skill should clearly state when historical analogue coverage is weak.

It should not pretend to have a curated historical narrative database unless one is provided.

This is one of the largest differences between the Skill and full application.

---

## 19. What to Watch Next

Purpose:

> Turn the research into a monitoring plan.

### Required Output

| Signal | Why It Matters | Scenario Affected | Bull Signal | Bear Signal |
| ------ | -------------- | ----------------- | ----------- | ----------- |

Examples:

* revenue guidance
* gross margin
* EPS
* free cash flow
* filing/audit status
* product launch metrics
* regulatory decisions
* customer concentration
* sector capex commentary
* management language
* peer results
* multiple compression/expansion

### Skill Instructions

Each watch item should connect to:

* a narrative
* an assumption
* a scenario
* a projection variable

This makes the output actionable without becoming investment advice.

---

## 20. Final Talk Track

Purpose:

> Help the user sound intelligent after the research block.

### Required Outputs

#### 30-Second Explanation

A concise summary.

#### 2-Minute Explanation

A fuller conversational explanation.

#### Smart Nuance

A point that avoids shallow thinking.

#### Common Misconception to Avoid

A warning against a simplistic narrative.

#### One-Sentence Thesis Map

```text
This is a [narrative type] story where the stock depends on [key assumptions], and the main pivots are [key pivots].
```

### Skill Instructions

This section should be memorable and conversational.

It should directly support the user’s desired outcome:

> I can talk intelligently to friends and colleagues about what is going on with the stock.

---

## 21. Source Pack Workflow

The Skill should build a source pack before writing the report.

### Current Narrative Sources

The Skill should look for:

* company filings
* earnings releases
* earnings call transcripts
* investor presentations
* press releases
* reputable financial news
* market commentary
* public investor letters
* short-seller reports, if relevant
* regulatory documents, if relevant

### Source Pack Table

| Source | Type | Date | Why It Matters | Claims Supported |
| ------ | ---- | ---- | -------------- | ---------------- |

### Source Types

The Skill should label sources as:

```text
Official company source
Filing
Transcript
Financial news
Market commentary
Investor letter
Short-seller / adversarial
Regulatory / legal
Social / retail narrative
Other
```

### Source Rules

The Skill should:

* prefer primary sources for facts
* use commentary sources for interpretation
* label adversarial sources clearly
* avoid treating allegations as facts
* avoid relying on low-quality SEO summaries
* cite major claims
* identify source gaps

---

## 22. Claim Extraction Workflow

Before writing the report, the Skill should extract claims.

### Claim Table

| Claim | Source | Date | Type | Side | Confidence | Related Metric |
| ----- | ------ | ---- | ---- | ---- | ---------- | -------------- |

### Claim Types

Possible claim types:

* demand
* revenue growth
* margin
* earnings
* valuation
* competitive position
* product
* regulatory
* credibility
* accounting
* customer concentration
* supplier dependency
* macro/sector
* management quality
* capital allocation

### Sides

Possible sides:

* bull
* bear
* neutral
* consensus
* counter-narrative
* adversarial

### Skill Instructions

The Skill should write the report from extracted claims rather than jumping directly from search results to prose.

This improves trust and reduces hallucination.

---

## 23. Quality Checklist

Before finalizing the report, the Skill should run a checklist.

### Required Checks

```text
- Does the report explain the company clearly?
- Does it explain why the stock matters now?
- Does it identify the dominant narrative?
- Does it steelman both bull and bear cases?
- Does it identify what bulls and bears actually disagree about?
- Does it connect narratives to financial assumptions?
- Are scenario projections explicitly assumption-driven?
- Are implied price bands framed as illustrative, not predictive?
- Are major claims source-backed?
- Are allegations labeled as allegations?
- Are historical analogues caveated?
- Are analogy breakers included?
- Are uncertainties and source gaps disclosed?
- Does the report avoid buy/sell/hold recommendations?
- Does it avoid price-target language?
- Does it include a final talk track?
```

### Output If Quality Is Weak

If the Skill lacks enough evidence, it should say:

```text
Source coverage is insufficient for a high-confidence report. Here is a partial report and the specific sources or data needed to improve it.
```

---

## 24. Source, Framing, and Quality Rules

The Skill should avoid sloppy analysis, unsupported claims, stale data, and misleading certainty.

This section is not intended to make the Skill feel institutional or compliance-heavy. It exists to keep the research useful, legible, and trustworthy.

### Avoid

The Skill should not provide:

* buy recommendations
* sell recommendations
* hold recommendations
* personalized investment advice
* portfolio allocation advice
* guaranteed returns
* definitive price targets
* instructions to trade
* claims that a scenario will happen

### Preferred Framing

Use language like:

```text
scenario-conditioned implied range
illustrative price band
assumption-driven model
if this scenario plays out
not a forecast
not investment advice
```

Avoid language like:

```text
target price
fair value
expected return
should trade at
will go to
guaranteed upside
```

### Projection Note

Every report with projections should include a short plain-English note:

```text
These scenario projections are illustrative and assumption-driven. They are not predictions, price targets, or investment advice. They show how different narrative outcomes could translate into financial assumptions and valuation ranges.
```

The note should be concise and should not dominate the report.

---

## 25. User Interaction Model

The Skill should support both one-shot and interactive workflows.

### One-Shot Mode

User asks:

```text
Analyze SMCI using the 90-minute stock narrative skill.
```

The Skill produces the full report.

### Guided Mode

The Skill asks the user to choose:

* ticker
* time horizon
* desired depth
* whether to include scenario projections
* whether to use approximate public data or user-provided numbers
* whether to optimize for investor understanding or writer output

### Iterative Mode

The user can ask:

```text
Make the bear case stronger.
```

```text
Show me the assumptions behind Scenario B.
```

```text
Change revenue growth to 15% and the P/E range to 10x–16x.
```

```text
Turn this into a Substack outline.
```

```text
Find better historical analogues.
```

```text
Make the final talk track more conversational.
```

### Recommended Default

For early distribution, default to one-shot mode with optional follow-ups.

This reduces friction for testers.

---

## 26. Skill Extensions

The Skill can be extended through specialized modes.

### 1. Historical Analogue Finder Mode

Input:

```text
Find historical analogues for this narrative.
```

Output:

* candidate analogues
* similarity dimensions
* differences
* outcomes
* lessons
* analogy breakers

### 2. Scenario Projection Builder Mode

Input:

```text
Build scenario projections from these assumptions.
```

Output:

* revenue/EPS model
* P/S and P/E implied price bands
* chart-ready table
* sensitivity analysis

### 3. Earnings Update Mode

Input:

```text
Update this narrative after the latest earnings call.
```

Output:

* what changed
* which narrative strengthened
* which scenario became more plausible
* which assumptions changed
* updated watch items

### 4. Writer Export Mode

Input:

```text
Turn this into a finance newsletter outline.
```

Output:

* headline ideas
* article outline
* opening paragraph
* bull/bear section
* chart/table suggestions
* final takeaway
* source appendix

### 5. Talk Track Mode

Input:

```text
Give me a 30-second and 2-minute explanation.
```

Output:

* concise conversational summary
* smart nuance
* misconception to avoid

### 6. Source Audit Mode

Input:

```text
Audit this report for weak sourcing and overclaims.
```

Output:

* weak claims
* missing sources
* unsupported projections
* hindsight leakage risks
* recommendation-language risks

### 7. Domain Packs

Specialized add-ons for:

* AI infrastructure
* biotech/pharma catalysts
* accounting-risk stories
* turnaround stories
* commodity cycles
* regulatory overhangs
* consumer adoption stories
* platform transitions

Each domain pack could include:

* common narrative patterns
* typical financial drivers
* relevant projection model
* common historical analogues
* failure modes
* source checklist

---

## 27. MVP Scope

### MVP Skill v0.1

Includes:

* SKILL.md
* 90-minute report template
* reusable HTML report shell
* report JSON schema
* scenario-data JSON schema
* source pack workflow
* narrative map workflow
* scenario projection template
* simple projection formulas
* basic financial data script
* chart rendering with D3 from CDN
* historical analogue template
* quality checklist
* source, framing, and quality rules

Goal:

A user can run the Skill on a ticker and receive a useful structured report plus a rendered HTML artifact with basic scenario charts.

### MVP Skill v0.2

Adds:

* examples
* improved rubrics
* writer export template
* guided follow-up prompts
* improved chart layouts
* improved projection model script
* better financial data normalization

Goal:

Early users can iterate on outputs and use them in real research/writing.

### MVP Skill v0.3

Adds:

* domain packs
* source audit mode
* analogue finder mode
* earnings update mode
* richer structured JSON output mode
* alternate chart/report views

Goal:

The Skill becomes a more complete research workflow and benchmark against the future app.

---

## 28. Success Criteria

### User Outcome

A user can run the Skill on a stock they do not know and afterward explain:

* what the company does
* why the stock matters now
* what the current narrative is
* what bulls and bears believe
* what assumptions matter
* what could produce upside or downside
* what historical analogues are relevant
* what to watch next

### Report Quality

A generated report should include:

* orientation
* business model primer
* why now
* narrative map
* financial math
* conditional scenario projections
* industry context
* historical analogues
* watchlist
* final talk track
* sources and limitations

### Projection Quality

The Skill should:

* show assumptions
* use transparent formulas
* produce scenario-conditioned ranges
* produce JSON-backed scenario tables
* render basic charts in the HTML report
* avoid false precision
* invite user overrides
* avoid price-target language

### Product Taste Quality

The Skill is successful for internal product taste calibration if repeated use across real portfolio or watchlist stocks reveals:

* which sections the builder reads closely
* which sections the builder skips
* which sections feel shareable
* which sections feel generic
* which charts clarify the scenario
* which charts feel unnecessary
* what is missing from the report
* what can be improved through a quick Skill update
* what clearly requires a full application

### Feedback Quality

The Skill is successful if early users can clearly say:

* which sections they loved
* which sections they ignored
* whether projections changed their understanding
* whether historical analogues felt useful
* whether the talk track helped
* whether they would use this again
* whether this should be an app, a Skill, a newsletter, or a hybrid

---

## 29. Skill vs Full Application Comparison

### Skill Strengths

The Skill is strong for:

* fast validation
* low build cost
* flexible research
* product taste calibration
* early distribution
* finance-writer workflows
* one-off reports
* interactive follow-up
* testing report structure
* testing scenario projections
* testing charted scenario outputs
* testing historical analogue usefulness

### Skill Weaknesses

The Skill is weak for:

* persistent storage
* saved reports
* watchlists
* alerts
* interactive charts
* automatic data refresh
* source normalization
* historical narrative database
* similarity search across indexed episodes
* user-friendly assumption editing
* consistent financial data
* quality evaluation at scale
* team collaboration
* SEO/public narrative pages

### Full Application Strengths

The full app can provide:

* structured database
* source-backed narrative episode library
* historical analogue engine
* interactive scenario charts
* saved assumptions
* scenario comparison UI
* financial data integration
* watchlist monitoring
* alerts
* exports
* team workflows
* public distribution pages
* quality/evaluation pipeline

### Key Strategic Insight

The Skill can validate the product’s research format, report depth, scenario charts, and product taste.

The full app can compound the product’s data, workflow, and distribution advantages.

---

## 30. Distribution Plan

The Skill is useful as an early distribution wedge.

### Early Distribution

Send to:

* finance writers
* independent analysts
* serious retail investors
* market-curious friends
* people who already use AI tools
* Substack/Seeking Alpha-style contributors

Ask them to test:

```text
Run this on one stock you know well.
Run this on one stock you know nothing about.
Tell me which sections were useful.
Tell me where it was wrong or shallow.
Tell me whether the projections helped.
Tell me whether the charts clarified the scenarios.
Tell me whether historical analogues helped.
Tell me whether you would use this again.
```

### Feedback Questions

Ask testers:

1. Did this help you understand the stock faster?
2. Which section was most valuable?
3. Which section was least valuable?
4. Did the scenario projections feel useful or misleading?
5. Did the math feel transparent enough?
6. Did the charts make the report easier to understand or share?
7. Were the historical analogues useful?
8. Did the final talk track help?
9. Would you prefer this as a Skill, web app, newsletter, or hybrid?
10. Would you pay for this?
11. What would make it 10x more useful?

### Distribution Goal

Use the Skill to learn whether the strongest product is:

* a standalone app
* a paid Skill
* a companion Skill + app
* a finance-writer research tool
* a newsletter/research product
* a data/API product
* a hybrid workflow

---

## 31. Risks and Mitigations

### Risk: Skill output feels like a generic AI stock report

Mitigation:

Make the structure distinctive: narrative map, math drivers, conditional scenario projections, historical analogues, final talk track.

### Risk: Financial data is inaccurate or stale

Mitigation:

Require data source labels, approximate-data warnings, and user-provided override mode.

### Risk: Scenario projections feel like price targets

Mitigation:

Use scenario-conditioned language and show assumptions clearly.

### Risk: Historical analogues are shallow

Mitigation:

Require analogy breakers, financial pattern, outcome, and lesson for each analogue.

### Risk: Users do not want long reports

Mitigation:

Include a short orientation and final talk track. Use progressive detail. Offer “summary-first” mode.

### Risk: Users want interactive charts

Mitigation:

Make charted output a v0.1 requirement. Produce a reusable HTML report backed by JSON data, with D3 loaded from a CDN. Treat requests for sliders, saved assumptions, and dynamic updates as evidence for building the app.

### Risk: Skill depends too heavily on web search quality

Mitigation:

Allow user-provided source packs and financial data. Show source limitations.

### Risk: Basic financial data scripts are brittle

Mitigation:

Treat scripts like `fetch_financials.py` as convenience utilities. Clearly label fetched, approximate, stale, or missing data. Allow user-provided overrides when script output is incomplete or questionable.

### Risk: Data-rendered HTML adds implementation overhead

Mitigation:

Keep the first HTML report shell simple. It should render from `report.json` and `scenario-data.json`, include only the most important charts, and avoid per-run custom HTML generation.

### Risk: Skill is too easy to copy

Mitigation:

Use it as validation and distribution, not the long-term moat. The moat becomes data, workflows, historical episodes, and application UX.

---

## 32. Non-Goals / Anti-Scope

The Skill is not:

* a stock-picking bot
* a trading signal
* a portfolio advisor
* a real-time market terminal
* a replacement for professional due diligence
* a fully automated investment model
* a guaranteed source of accurate financial data
* a persistent research database

The Skill should not initially attempt:

* automatic watchlist monitoring
* full historical narrative indexing
* institutional data integration
* real-time price alerts
* complex DCF modeling
* personalized investment advice
* fully interactive UI

---

## 33. Acceptance Criteria

The Skill v0.1 is successful if:

* it can be distributed to early users as a standalone workflow
* users can run it without a custom backend
* it produces a useful stock narrative report
* it produces structured JSON outputs for the report and scenarios
* it renders a reusable HTML report with basic charts
* it includes conditional scenario projections or clearly explains why data is insufficient
* it includes historical analogues with caveats
* it creates a strong final talk track
* repeated use on real portfolio/watchlist stocks reveals product taste signals
* users can identify clear likes/dislikes
* feedback helps decide whether to build a full app

The Skill v0.1 fails if:

* users perceive it as just a generic AI summary
* projections are confusing or misleading
* charted outputs are missing, broken, or not useful
* the HTML report requires custom per-run authoring instead of rendering from data
* historical analogues feel superficial
* source quality is too weak
* users do not understand how to use the output
* the report is too long without a clear payoff
* the workflow does not reveal strong app opportunities

---

## 34. Summary

The 90-Minute Stock Narrative Research Skill is a fast, portable implementation of the broader product idea.

It tests the core promise:

> Can a structured AI research workflow help users understand a stock’s story, math, scenarios, and historical context quickly enough to feel meaningfully smarter?

The Skill’s biggest advantage is speed of validation.

It can be distributed immediately to early users and used to learn:

* which sections matter
* which sections the builder personally reads, skips, revisits, or wants to share
* whether scenario projections are compelling
* whether charted scenario outputs clarify the report
* whether historical analogues are useful
* whether finance writers want exports
* whether the workflow should become an app
* where AI Skill workflows are sufficient
* where a dedicated product is clearly better

The Skill is not the final moat.

The full application can win by adding:

* persistent data
* interactive projections
* source-backed narrative episode library
* historical analogue search
* watchlists
* alerts
* exports
* public narrative pages
* repeatable financial data
* quality control

But the Skill is likely the best first implementation path.

It allows the product thesis to be tested directly:

> If people like the Skill but complain about persistence, charts, data accuracy, saved assumptions, historical analogues, and monitoring, those complaints become the roadmap for the full product.

The Skill should therefore include enough depth and visualization to make those complaints meaningful. A reusable HTML report rendered from JSON, with D3-powered charts and basic financial-data scripts, is part of the pre-MVP test rather than a later nice-to-have.
