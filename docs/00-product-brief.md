# Product Brief: Stock Narratives

## 1. Product Concept

**Stock Narratives** is a narrative research assistant for serious market observers.

The product helps independent analysts, finance writers, and serious retail investors understand the current story driving a stock, compare that story to similar current and historical market narratives, and identify the pivots that would validate, fade, transform, or break the thesis.

Instead of organizing research only around charts, financial metrics, news feeds, or analyst estimates, Stock Narratives organizes research around **market stories**:

* Why is this stock moving?
* What story does the market appear to be pricing in?
* What are bulls saying?
* What are bears saying?
* What would prove either side right or wrong?
* What historical narratives looked similar?
* How did those narratives play out?
* What made the winners different from the losers?

The product does not aim to predict stock prices or provide buy/sell recommendations. It helps users reason more clearly about narrative-driven stock performance.

---

## 2. Target User / Persona

### Primary user

The initial target user is:

**Independent analysts and finance writers who need to quickly understand, explain, compare, and track market narratives.**

These users include:

* finance newsletter writers
* Substack-style market commentators
* independent equity researchers
* independent analysts
* small research teams
* serious retail investors who publish or share investment analysis

They are not looking for a generic “stock summary.” They are looking for a faster way to understand the story behind a stock and how that story compares to historical patterns.

### Secondary user

The secondary user is:

**Serious retail investors who want better research tools, but do not need an institutional terminal.**

They may not write publicly, but they want to understand:

* why a stock is moving
* what narrative is currently dominant
* where the bull and bear cases differ
* whether similar stories have happened before
* what events could change the market’s perception

### Initial positioning

Stock Narratives should feel like:

> A narrative research workspace for market stories.

Not:

> A stock-picking bot.

Not:

> A generic AI finance chatbot.

Not:

> A Bloomberg clone.

---

## 3. Product Philosophy

The product is built around the idea that stocks are often driven by a combination of fundamentals, expectations, sentiment, positioning, and narrative.

A company may be performing well, but the stock may be driven by a much larger story. A company may be struggling, but the stock may be driven by a turnaround narrative. A company may be highly profitable, but the market may be debating whether those profits are temporary, cyclical, structural, or already priced in.

Stock Narratives helps users separate:

* the company
* the stock
* the current market narrative
* the assumptions behind that narrative
* the evidence supporting or weakening it
* the historical analogues that may or may not apply

The product’s philosophy is:

> Do not predict. Do not recommend. Help users reason.

### Core principles

#### 1. Narrative-first, not ticker-first

The stock is the entry point, but the core object is the **narrative episode**.

A single company can have many different narratives over time. For example, Tesla has had narratives around EV adoption, battery cost advantage, manufacturing scale, autonomy, robotaxis, energy storage, margin compression, AI optionality, and CEO/key-person risk.

The product should not treat a stock as having one permanent identity.

#### 2. Similarity must be explainable

The product should never say:

> “This stock is 87% similar to Cisco in 1999.”

without explaining what kind of similarity is being measured.

Narrative similarity may involve:

* sector
* valuation setup
* catalyst type
* market structure
* customer concentration
* margin profile
* hype cycle dynamics
* regulatory exposure
* revenue growth pattern
* earnings revision pattern
* capital cycle
* credibility risk
* product launch cycle

Every comparison should explain:

* why the analogy is useful
* where the analogy breaks
* what variables mattered in the historical case
* what variables matter now

#### 3. Historical analogues are tools, not forecasts

The product should help users learn from past narratives without implying that similar narratives must produce similar outcomes.

A good historical analogue should answer:

* What was the story at the time?
* What did market participants appear to believe?
* What evidence supported the story?
* What evidence challenged it?
* What happened next?
* What were the key pivots?
* Why did the narrative validate, fade, break, or transform?
* Why might this analogy be misleading today?

#### 4. Scenario thinking over price prediction

The product should avoid hard price targets, buy/sell labels, and one-path forecasts.

Instead, it should help users reason through scenarios:

* What happens if the bull narrative validates?
* What happens if fundamentals improve but valuation compresses?
* What happens if growth continues but margins disappoint?
* What happens if the narrative changes entirely?
* What events would force the market to update its view?

#### 5. Trust through sources and uncertainty

The product should distinguish between:

* sourced facts
* market narratives
* model interpretation
* historical analogy
* speculative scenario

Users should be able to inspect the evidence behind major claims.

When uncertainty is high, the product should say so.

---

## 4. Golden Workflow

### Core workflow

The primary user workflow is:

1. User searches for a ticker.
2. User sees the current narrative map for that stock.
3. User reviews the bull narrative, bear narrative, consensus narrative, and emerging counter-narratives.
4. User sees the key assumptions behind the current market story.
5. User sees a scenario tree for how the narrative may play out over the next 6–12 months.
6. User sees the key pivots that would validate, weaken, break, or transform the narrative.
7. User explores similar current narratives in other stocks.
8. User explores similar historical narrative episodes.
9. User reviews how those historical narratives resolved and why the analogies may or may not apply.
10. User leaves with a clearer understanding of the story, the risks, and the historical pattern space.

### 3-minute magic moment

Within three minutes, a user should be able to search a ticker and answer:

* What story is currently driving this stock?
* What is the strongest bull case?
* What is the strongest bear case?
* What would change the story?
* What past narratives looked similar?
* How did those past narratives play out?

### Example workflow

A user searches `SMCI`.

The product shows:

* Current narrative: AI infrastructure/server demand beneficiary with governance and margin credibility risk.
* Bull narrative: AI server demand remains structurally strong; customer demand continues; governance concerns fade; revenue growth remains exceptional.
* Bear narrative: company is a low-margin integrator in a competitive market; accounting/internal control issues impair credibility; growth is more cyclical than structural.
* Emerging counter-narrative: business demand remains real, but valuation and trust reset the stock into a lower-multiple hardware supplier.
* Key pivots:

  * audited financials
  * gross margin stabilization
  * customer concentration disclosures
  * AI server backlog
  * competitive pricing pressure
  * index/relisting/filing status
* Similar current narratives:

  * AI capex hardware beneficiaries
  * data center infrastructure suppliers
  * companies with growth plus credibility risk
* Historical analogues:

  * prior infrastructure buildout winners
  * past hardware cycle beneficiaries
  * past high-growth companies with accounting or governance overhangs
* Analogy breakers:

  * different margin structure
  * different customer concentration
  * different supply chain constraints
  * different macro and valuation environment

The output should feel like a structured research memo, not a chatbot response.

---

## 5. MVP Scope by Parts

The desired product is relatively complete early, but it should still be staged through coherent product slices.

### MVP Part 1: Current Stock Narrative Page

Users can search for a ticker and view a structured narrative map.

Core capabilities:

* identify the current dominant narrative
* identify bull, bear, consensus, and counter-narratives
* summarize recent narrative changes
* extract key assumptions
* extract key metrics and catalysts
* show source-backed evidence
* show uncertainty and disagreement

Output sections:

* Current Narrative
* Bull Case
* Bear Case
* Emerging Counter-Narrative
* What Changed Recently
* Key Assumptions
* Key Metrics to Watch
* Key Sources

This is the foundational page.

### MVP Part 2: Scenario and Pivot Tracker

For each stock narrative, the product generates a 6–12 month scenario tree.

Example scenarios:

* narrative validates
* fundamentals improve but valuation compresses
* growth continues but key metric disappoints
* bear case takes control
* narrative transforms into a different story
* external shock changes the framing

Each scenario should include:

* description
* confirming signals
* disconfirming signals
* likely narrative impact
* key dates or catalysts
* relevant metrics
* uncertainty level

The product should emphasize conditional thinking, not prediction.

### MVP Part 3: Historical Narrative Episodes

Users can view past narrative episodes that appear structurally similar to the current stock narrative.

Each historical narrative episode should include:

* company
* ticker
* time period
* narrative title
* narrative summary
* source-time evidence
* bull claims
* bear claims
* key assumptions
* key pivots
* outcome summary
* stock/relative return context, if available
* why the analogy is useful
* why the analogy may be misleading

The historical data strategy is central to the product, but the exact implementation details remain TBD. The brief should not hard-commit to a specific source archive, vendor, or historical web-search mechanism yet.

### MVP Part 4: Similar Current Narratives

Users can explore other stocks currently driven by similar narratives.

Examples:

* AI infrastructure winners
* product launch ramp stories
* regulatory overhang stories
* GLP-1 disruption stories
* commodity cycle windfall stories
* turnaround stories
* fraud/accounting credibility stories
* platform transition stories
* operating leverage stories

The goal is to help users discover comparable active narratives.

### MVP Part 5: Narrative Timeline

For each stock, users can see how the story has changed over time.

Example for a stock:

* “AI capex beneficiary”
* “margin concern emerges”
* “customer concentration risk”
* “accounting/internal control risk”
* “recovery and credibility repair”

The timeline should show that stocks often move through narrative regimes rather than having a single static story.

### MVP Part 6: Research Workspace

Users can save and track:

* tickers
* narratives
* historical analogues
* scenarios
* pivots
* watchlists
* alerts

This creates retention and makes the product useful beyond one-off lookup.

---

## 6. Core Objects / Domain Model

The product should be structured around narrative primitives rather than only around stocks.

### Stock

A publicly traded company/security.

Fields may include:

* ticker
* company name
* exchange
* sector
* industry
* market cap
* relevant peers
* current narrative status

### NarrativeEpisode

The core object.

A time-bounded market story about why a company’s business performance, valuation, risk profile, or strategic position may change.

Fields may include:

* company
* ticker
* start date
* end date
* narrative title
* narrative type
* narrative summary
* bull claims
* bear claims
* consensus view
* emerging counter-narrative
* key assumptions
* key metrics
* key catalysts
* source documents
* scenario paths
* historical outcome
* similarity embedding
* structured similarity attributes

### NarrativeClaim

A specific claim within a narrative.

Examples:

* “AI server demand will remain supply constrained.”
* “Margins will structurally expand.”
* “The company has a durable cost advantage.”
* “The product launch will create a new revenue base.”
* “Regulatory risk is overblown.”
* “The current profits are cyclical.”

Fields may include:

* claim text
* claim type
* supporting sources
* opposing sources
* confidence level
* related metrics
* date first observed
* date last observed

### SourceDocument

A document used as evidence.

Potential source types:

* company filings
* earnings call transcripts
* press releases
* investor presentations
* reputable financial news
* analyst commentary, if licensed
* financial blogs/newsletters, if allowed
* historical archives, if available

Fields may include:

* source type
* publisher
* title
* URL or document ID
* publication date
* retrieval date
* covered time period
* associated claims
* source reliability metadata

### Scenario

A possible future narrative path.

Fields may include:

* scenario title
* description
* time horizon
* narrative effect
* confirming signals
* disconfirming signals
* relevant metrics
* catalysts
* probability language, if used carefully
* uncertainty level

### Pivot

An observable event, metric, or development that could cause the narrative to validate, weaken, break, or transform.

Examples:

* FDA approval
* margin guidance
* backlog disclosure
* product launch metrics
* customer retention
* regulatory decision
* filing delay
* short report
* earnings revision
* large customer loss
* management change

### HistoricalOutcome

A structured summary of how a historical narrative episode resolved.

Fields may include:

* outcome window
* stock return
* relative return
* revenue growth change
* margin change
* valuation multiple change
* estimate revisions
* narrative resolution type
* key pivot events
* post-mortem summary

### NarrativeCluster

A group of current or historical narrative episodes with similar structure.

Examples:

* AI infrastructure buildout
* commodity supercycle
* category-defining drug launch
* consumer adoption curve
* regulatory overhang
* fraud/credibility crisis
* turnaround with operating leverage
* legacy company re-rated as tech/platform/AI

---

## 7. Key Features and Why They Matter

### 1. Current Narrative Map

Shows the dominant story currently driving a stock.

Why it matters:

Users do not just want to know what happened. They want to know what the market thinks is happening.

The current narrative map should answer:

* What is the stock’s current story?
* What do bulls believe?
* What do bears believe?
* What is the market debating?
* What changed recently?

### 2. Bull/Bear/Counter-Narrative Breakdown

Separates the competing interpretations of the same facts.

Why it matters:

A good stock narrative is often a disagreement over assumptions, not a disagreement over facts.

For example:

* Bulls and bears may agree revenue is growing.
* They may disagree on whether growth is durable.
* They may agree margins are falling.
* They may disagree on whether that is temporary or structural.

### 3. Key Assumptions

Identifies the assumptions that must be true for the narrative to work.

Why it matters:

A narrative becomes more useful when it is testable.

Examples:

* AI capex will keep growing.
* Gross margins will stabilize.
* Regulatory approval is likely.
* Product adoption will accelerate.
* Customers will not churn.
* Competition will not compress pricing.
* The company deserves a higher multiple.

### 4. Scenario Tree

Shows plausible narrative paths over the next 6–12 months.

Why it matters:

Users should not anchor on one forecast. They should understand the branching structure of the thesis.

### 5. Key Pivots

Identifies observable events or metrics that could change the story.

Why it matters:

The user needs to know what to watch.

A pivot can be:

* a metric
* a date
* a filing
* a product release
* a regulatory event
* a guidance update
* a customer announcement
* a competitor action
* a change in language from management or analysts

### 6. Historical Analogues

Finds past narrative episodes that resemble the current narrative.

Why it matters:

Users want to know:

* Has this kind of story happened before?
* What separated durable winners from temporary hype cycles?
* What usually breaks these narratives?
* What tends to be missed early?

### 7. Analogy Breakers

Explains why each historical comparison may be wrong.

Why it matters:

This is essential to trust.

The product should not merely say:

> “This looks like Cisco in 1999.”

It should say:

> “This resembles Cisco in 1999 in the sense that both involved infrastructure demand for a new computing paradigm. The analogy may be misleading because the competitive structure, margin profile, customer concentration, supply chain, and valuation backdrop are materially different.”

### 8. Similar Current Narratives

Shows other stocks driven by similar stories right now.

Why it matters:

This supports discovery and comparative research.

A user interested in one AI infrastructure name may want to see other AI capex beneficiaries. A user studying a regulatory overhang may want to compare other stocks under similar pressure.

### 9. Narrative Timeline

Shows how a stock’s story changed over time.

Why it matters:

Stocks often move through narrative regimes.

The timeline helps users see whether a stock is:

* early in a narrative
* late in a narrative
* transitioning between narratives
* recovering from a broken narrative
* being re-rated around a new story

### 10. Source Trail

Shows the sources behind the narrative summary.

Why it matters:

This reduces hallucination risk and gives users confidence.

Users should be able to inspect the evidence behind major claims.

---

## 8. Data, Integrations, and Retrieval Strategy

The data strategy is central to the product, especially for historical narrative comparison. However, many implementation details remain intentionally TBD at the Product Brief stage.

The product should be designed around a few conceptual layers.

### 1. Current source layer

The current source layer collects recent information used to describe active narratives.

Potential sources include:

* company filings
* earnings call transcripts
* press releases
* investor presentations
* reputable financial news
* market commentary
* analyst commentary, where licensed
* selected high-quality blogs/newsletters, where allowed

The product should avoid low-quality source pollution, especially for stocks with heavy retail chatter.

### 2. Historical source-time layer

The historical source-time layer attempts to reconstruct what was being said during a specific historical period.

Example:

> What was being said about this stock between January 2019 and January 2021?

This layer is important because historical narrative comparison only works if the system can avoid hindsight leakage.

The product should try to distinguish:

* what was knowable at the time
* what was being claimed at the time
* what the market appeared to believe at the time
* what only became obvious later

The exact approach is TBD.

Possible approaches may include:

* date-constrained search
* source-specific search
* archived pages
* historical news databases
* licensed financial news archives
* transcript and filing archives
* manually curated source packs
* hybrid human-reviewed historical episode creation

The Product Brief should not commit to a specific vendor or source mechanism yet.

### 3. Outcome layer

The outcome layer describes what happened after a narrative episode.

This may include:

* stock return
* relative return versus index/sector
* revenue growth
* margin trajectory
* valuation multiple changes
* estimate revisions
* product launch outcomes
* regulatory outcomes
* management changes
* narrative resolution

The outcome layer should be clearly separated from source-time evidence.

### 4. Modern interpretation layer

The modern interpretation layer is the product’s synthesis of:

* what the narrative was
* how it developed
* what evidence mattered
* what changed
* how it resolved
* why it may or may not be analogous to a current narrative

This is where LLMs can add significant value.

### 5. Vector search and narrative similarity

Vector search should power narrative similarity, but not be the only mechanism.

The system should index narrative episodes using:

* narrative summaries
* bull claims
* bear claims
* assumptions
* catalysts
* failure modes
* outcome summaries
* structured metadata

Similarity should combine embeddings with structured filters and reranking.

Possible similarity dimensions:

* narrative type
* sector
* business model
* catalyst type
* valuation setup
* financial setup
* margin profile
* revenue growth profile
* hype-cycle dynamics
* regulatory exposure
* credibility risk
* capital intensity
* customer concentration
* market structure
* time horizon

The product should return explainable similarities, not black-box matches.

---

## 9. UX / Primary Screens

### 1. Home / Search

The home screen should make the core workflow obvious.

Primary action:

> Search a ticker or narrative.

Examples:

* `NVDA`
* `SMCI`
* `AI infrastructure winners`
* `GLP-1 disruption`
* `regulatory overhang`
* `turnaround with operating leverage`

### 2. Stock Narrative Overview

The main page for a stock.

Sections:

* current narrative
* bull case
* bear case
* emerging counter-narrative
* what changed recently
* key assumptions
* key metrics
* key pivots
* source-backed evidence
* related current narratives
* historical analogues

This is the core product surface.

### 3. Narrative Timeline

A timeline view showing how the stock’s narrative changed over time.

Each timeline item should include:

* narrative title
* date range
* summary
* key events
* source evidence
* transition reason

### 4. Scenario Tree

A structured view of potential future narrative paths.

Each scenario should include:

* title
* summary
* what would validate it
* what would weaken it
* key metrics
* key dates/catalysts
* related historical examples

### 5. Historical Analogues

A comparison page showing past narrative episodes.

For each analogue:

* company
* period
* narrative title
* similarity summary
* outcome
* key pivots
* why useful
* why misleading
* source trail

### 6. Analogue Detail Page

A deep dive into a specific historical narrative episode.

Sections:

* original narrative
* source-time evidence
* bull case at the time
* bear case at the time
* key assumptions
* narrative timeline
* outcome summary
* key pivots
* comparison to current stock
* analogy breakers

### 7. Similar Current Narratives

A discovery screen for active narrative clusters.

Examples:

* AI infrastructure
* regulatory overhang
* product launch ramp
* consumer adoption
* operating leverage turnaround
* commodity windfall
* credibility repair

### 8. Watchlist

Users can save tickers and narratives.

Watchlist items should show:

* current narrative
* recent narrative changes
* upcoming pivots
* new sources
* similar narratives
* alerts

### 9. Source Explorer

A source inspection view.

Users can inspect:

* source documents
* claims extracted from each source
* which narrative claims each source supports
* which claims are contested
* publication dates
* source reliability metadata

### 10. Internal Research/Admin View

An internal tool for reviewing narrative episodes.

This may include:

* source ingestion status
* extracted claims
* proposed narrative episode summaries
* duplicate detection
* quality review
* historical outcome labeling
* manual correction tools

This may be important early if historical narrative episodes are semi-automated or manually reviewed.

---

## 10. Trust, Safety, and Quality Rules

Because the product is investment-adjacent, trust and safety are core product requirements.

### No recommendations

The product should not provide:

* buy recommendations
* sell recommendations
* hold recommendations
* price targets
* portfolio allocation advice
* personalized investment advice

It should present research context, not investment instructions.

### Separate narrative from fact

The product should distinguish:

* fact: “Revenue grew X%.”
* narrative: “The market appears to be pricing this as a durable growth story.”
* interpretation: “The key debate is whether margins are temporarily or structurally impaired.”
* scenario: “If margins stabilize, the bull narrative may strengthen.”
* analogy: “This resembles prior infrastructure buildout stories in specific ways.”

### Cite major claims

Major factual and narrative claims should be source-backed.

The user should be able to inspect where a claim came from.

### Include disagreement

The product should not collapse all evidence into a single summary.

It should show:

* bull view
* bear view
* consensus view
* emerging counter-view
* contested assumptions

### Show analogy limitations

Every historical analogue should include an explicit section explaining why the analogy may be misleading.

### Avoid fake precision

The product should avoid unjustified certainty.

Bad:

> “This narrative has an 82% chance of playing out like Nvidia 2016.”

Better:

> “This narrative shares several features with prior accelerator-cycle stories, especially around demand acceleration and supply constraints. The analogy is weaker on valuation, customer concentration, and margin structure.”

### Avoid hindsight leakage

Historical narrative research should separate:

* what people knew at the time
* what happened later
* how the product currently interprets the resolution

### Label uncertainty

The product should explicitly label uncertainty where appropriate.

Examples:

* insufficient source coverage
* conflicting evidence
* unclear narrative dominance
* sparse historical analogues
* high dependence on future catalysts

### Prefer transparent incompleteness over confident hallucination

If the system lacks enough evidence, it should say:

> “Not enough source coverage to confidently reconstruct this historical narrative.”

---

## 11. Non-Goals / Anti-Scope

### Not a trading signal product

Stock Narratives is not a real-time trading alert or signal generator.

### Not a recommendation engine

It does not tell users what to buy or sell.

### Not a full financial terminal

It should not try to replicate Bloomberg, FactSet, Koyfin, YCharts, or Capital IQ.

### Not a generic AI chatbot for stocks

The differentiated value is structured narrative research, historical analogy, and scenario/pivot analysis.

### Not a social sentiment firehose

Social data may eventually be useful, but v1 should avoid becoming a noisy sentiment aggregator.

### Not all stocks globally at launch

The product should begin with a focused coverage universe and expand over time.

### Not fully automated at the expense of quality

For historical narrative episodes especially, quality matters more than full automation in the early product.

### Not price-target driven

The product should avoid making the user experience revolve around target prices.

---

## 12. Milestones

### v0.1 — Curated Narrative Research Prototype

Goal:

Create a credible early version for a curated set of stocks and narrative categories.

Scope:

* ticker search for a small universe
* current narrative page
* bull/bear/counter-narrative summaries
* key assumptions
* key pivots
* basic source trail
* manually or semi-manually curated historical analogues
* simple outcome summaries

Initial universe:

* AI infrastructure and AI capex beneficiaries

Potential companies:

* Nvidia
* AMD
* Broadcom
* Marvell
* Super Micro
* Dell
* Vertiv
* Micron
* TSMC
* Arista
* Oracle
* selected data center infrastructure names

Historical analogue categories:

* dot-com infrastructure buildout
* cloud infrastructure buildout
* crypto GPU cycle
* memory supercycles
* hardware capex boom/bust cycles
* high-growth hardware companies with margin or credibility questions

Success criteria:

* users can search a covered ticker and quickly understand the current narrative
* each narrative page feels meaningfully better than a generic LLM summary
* historical analogues are useful and include analogy breakers
* source trail is sufficient to build trust

### v0.5 — Automated Narrative Extraction and Episode Library

Goal:

Move from curated prototype toward repeatable narrative indexing.

Scope:

* automated source ingestion for selected sources
* LLM-based claim extraction
* LLM-based narrative episode drafting
* vector indexing of narrative episodes
* structured narrative similarity
* scenario generation
* pivot detection
* human review workflow for important episodes
* expanded current narrative clusters

Success criteria:

* system can generate useful first drafts of narrative pages
* system can retrieve plausible historical analogues
* similarity explanations are transparent
* historical episode creation is reviewable and correctable
* users can browse current narrative clusters

### v1 — Narrative Research Workspace

Goal:

Deliver a polished user-facing research workspace.

Scope:

* robust ticker search
* current narrative overview
* narrative timeline
* scenario tree
* key pivots
* historical analogue explorer
* similar current narratives
* watchlists
* alerts
* source explorer
* shareable research pages or exports
* user accounts and saved research

Success criteria:

* users return to monitor narrative changes
* finance writers and analysts use it as part of their research process
* historical analogues are seen as differentiated and useful
* the product has a clear identity distinct from generic stock tools and AI chatbots

---

## 13. Risks and Mitigations

### Risk: The product becomes a generic stock summarizer

Mitigation:

Focus the UX around narrative episodes, historical analogues, scenario pivots, and analogy breakers.

### Risk: Historical analogies create false confidence

Mitigation:

Every analogue must include differences, limitations, and “why this may be misleading.”

### Risk: LLM hallucination

Mitigation:

Use constrained retrieval, source-backed claims, citations, claim extraction, and review workflows.

### Risk: Hindsight leakage

Mitigation:

Separate period-correct source evidence from retrospective outcome labeling and modern interpretation.

### Risk: News/data access is harder than expected

Mitigation:

Keep historical data implementation details TBD in the Product Brief. Start with a curated or semi-automated approach and expand source coverage over time.

### Risk: Users expect recommendations

Mitigation:

Position clearly as research and education. Avoid buy/sell labels, price targets, and personalized advice.

### Risk: Too much scope

Mitigation:

Design the v1 vision broadly, but start with one narrative universe: AI infrastructure.

### Risk: Similarity search is too shallow

Mitigation:

Use hybrid similarity: embeddings plus structured metadata, reranking, and LLM-generated explanations.

### Risk: Low trust in generated narratives

Mitigation:

Show sources, uncertainty, contested claims, and confidence boundaries.

### Risk: Users do not understand “narrative episode”

Mitigation:

Use plain-language UX labels like “market story,” “what changed,” “what to watch,” and “similar past stories,” while retaining the structured object model internally.

---

## 14. Acceptance Criteria

### Product usefulness

A user can search a covered ticker and understand the current market narrative in under three minutes.

### Narrative clarity

Each covered stock has:

* current narrative
* bull case
* bear case
* emerging counter-narrative, where applicable
* key assumptions
* key pivots
* source-backed evidence

### Historical analogue quality

Each historical analogue includes:

* company
* time period
* narrative summary
* why it is similar
* why it may be misleading
* outcome summary
* key pivots
* source trail

### Scenario quality

Each scenario includes:

* what would cause it
* what signals would confirm it
* what signals would weaken it
* how it would affect the narrative
* relevant metrics or catalysts

### Trust and safety

The product:

* does not provide buy/sell/hold recommendations
* does not provide personalized financial advice
* avoids unjustified price targets
* distinguishes facts from interpretation
* cites major claims
* labels uncertainty
* includes analogy limitations

### Differentiation

A user should feel the product is meaningfully different from:

* a financial news feed
* a stock screener
* a generic LLM stock summary
* a charting app
* a basic sentiment tracker

The differentiated value is:

> Structured narrative research, historical market-story comparison, and scenario/pivot reasoning.

---

## 15. Summary

Stock Narratives is a research assistant for understanding market stories.

The product helps users answer:

* What story is driving this stock?
* What assumptions does that story depend on?
* What would validate or break the story?
* What other stocks are driven by similar stories?
* What historical narratives looked similar?
* How did those historical narratives resolve?
* Why might those analogies be useful or misleading?

The strongest initial wedge is independent analysts and finance writers, with serious retail investors as a secondary audience.

The best first coverage universe is AI infrastructure, because the narratives are current, high-profile, volatile, and rich in historical comparisons.

The long-term opportunity is to build a searchable, source-backed, historically grounded library of market narratives.

The product should be careful, transparent, and research-oriented.

Its core promise is not:

> “We predict what happens next.”

Its core promise is:

> “We help you understand the story, compare it to history, and know what would change the thesis.”
