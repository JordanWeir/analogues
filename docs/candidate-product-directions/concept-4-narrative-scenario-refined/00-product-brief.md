# Analogues

Analogues is a stock research tool that analyzes a stock, producing:
- A description of the business
- A description of the current narratives driving stock activity
- A scenario analysis, showing business fundamentals projections of what might occur with ~5 different scenario paths
- Possible stock prices + outcome distributions given the scenarios and their assessed probabilities


## Reference work

### Pre-Proof of Concept Work - Agent Skills

Initial experiments proved a rough version of this is possible.  2 skills were produced that executed this rough workflow.  They did have issues with cost and reliabilty though, and a true product surface would need more durable access to many elements of the data.

Aspects of the agent skills idea have begun to be abstracted into loco tasks, so we can run certain stages deterministically and manage the agent skills context window a little more effectively, while executing certain things a bit more deterministically.

### Stateful Swarms

Irys has open sourced a "Stateful Swarms" repo, https://github.com/dl1683/irys-stateful-swarms/tree/master.

They are a more open ended research agent architecture, and have shown extremely positive results on Harvey's open sourced legal benchmarks, hitting around 18% full-task success rate vs. ~8% for Opus in Claude, and at dramatically reduced cost.

I've taken a look at their code base (cloned in ~/apps/irys-stateful-swarms), and extracted out a possible interface we could build out if we want to use a similar overall stateful swarm strategy. (docs/references/blackboard-concept-design.md)


## Target Output

We want to generate a report for a stock with 3 goals:
- User feels "wowed" by the report, and has a much better idea of what they should think about when considering investing in the stock
- The "Scenario Conditioned Paths" with Monte Carlo price distribution are a first class artifact and are shown very prominently
- Users with more time can optionally read the full report to understand the wider business context

Key Sections
- Current Narratives
- Scenario-Conditioned Price Projections
- Executive Summary
- Business Model
- Industry Context
- Financial Math
- Talk Track / Conclusions
- Signals to Watch For
- Historical Analogues (??)
- Citations

## Research Flow Overview

Overall stages are:

Simple Stages:
1. Extract Deterministic Data
2. Link Canonical Concepts from Facts API to Core Financial Metrics
3. Web Search for insights and general 'vibe' based on citeable sources
4. Web Search for their past Annual Reports, and convert to readable form
5. Identify early "Interesting Concepts" from the Facts API
6. Produce Business Summary Artifacts
7. Produce Scenario Ideas
8. For Each Scenario...
    i. Identify additional "Interesting Concepts" from the Facts API
    ii. Review historical earnings growth / revenue growth / other time series from facts API
    iii. Decide on a set of fundamental time series for this specific analysis
    iv. Identify key scenario assumptions
    v. Project all time series forward with the scenario assumptions in mind
    vi. Calculate derived columns as needed
    vii. Save out Crux Assumptions + Key Sensitivities
    viii. Draft other scenario content
9. Assign Scenario Conditional Probabilities, Generate Monte Carlo Histogram
10. Draft Sections
    - Current Narratives
    - Executive Summary
    - Business Model
    - Industry Context
    - Financial Math
    - Talk Track / Conclusions
    - Signals to Watch for
    - Historical Analogues
    - Citations

## Data Strategy

The SEC Facts API provides numerous unstructured time series for a specific stock.

These are unstructured because businesses have discretion in terms of what data they want to highlight as important or influential.

This is a headache for most stock market products; they need to normalize this data into clean revenue / EPS / long term debt / etc.

This is an opportunity for us, since there will often be extremely useful but non-standard time series here.  Things like revenue by businses segment may be present here, allowing much stronger narrative analysis and projections of what's really going on, and how things might play out.

Some notes about this:
- To display core fields like Revenue, we need to persist a DB field linking the common "Revenue" concept to whatever they've called it in their facts/concepts API, different per company
- Scenario-specific fields can be selected and used, allowing for dynamic tables and projections visible to the user
- The SEC Facts API is based on company submissions, which can lag up to 40 days after the earnings report.  
- Analysis should include data from:
    - Company annual reports, investor relations website and scraped data
    - SEC Facts API
    - Financial Media Analysis
- When projecting, especially projecting concepts that might be unavailable due to late filing, we should be particularly careful to annotate Projected vs. Historical
- In practice, each time period in our scenarios is either: Historical | Projected | Mixed (eg: some data available, some not)

## Distribution Strategy

The main unit of usage is stock reports.

We'll maintain a 'long term' and 'weekly' rotation of stock reports.
- Long term reports are rotated quarterly, and represent a couple big names everyone is watching (NVDA, MU)
- Weekly reports focus on stocks that are recently in play and have a lot of volume (SMCI, NOK)

Eventually, we'll support an alerting functionality that lets you know if any of the key Crux Factors for stocks your following have changed positively or negatively, and how that influences the monte carlo projection.

## Pricing Strategy

Three user tiers.

Not Signed In: Long Term + Weekly Rotation reports available
Free Tier: ??
Paid Tier: Full S&P 500 stock reports available
Premium Paid Tier: Follow individual stocks and get Alerts when crux conditions change

## Data Population Strategy

Things to decide:
- Initial Data Population
- Stock Report Refresh Policy
- Earnings Release Report Refresh Strategy

We're leaving these undecided for now, conditional on understanding how much generating reports will cost.

Right now, generating reports seems to be in the $1.00 - $1.50 range, and so even a monthly report on 500 stocks is a bit expensive before we have users.

## Product Hooks

- Scenario Analysis feels cool and sophisticated
- Plenty of "Snippet Level" shareable artifacts
    - Share Scenario Paths + Simluation Graph
    - Share a specific Scenario Analysis
- Both the Scenario Paths and Scenario Analysis components need to:
    - Feel smooth, cool, and sophisticated
    - Have an analysis that seems smart and non-trivial
    - Lean on a combo of "familiar data" and "exotic data" series, in a way that feels way better then what they've seen elsewhere
- People should feel they "need" to know if the Crux Conditions change; changes their should feel like an investment opportunity or actionable info.  IF X THEN BUY is implied, although not stated.  The closest we want to get to stating "BUY" is changing the scenario probabilities, and hence the implied Expected Value of the stock.



