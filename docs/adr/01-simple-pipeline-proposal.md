# Research Pipeline Planning

## Summary 

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

## Phases

### Phase 1: Gather Data, Tag Cannon, Validate Cannon

Get the workspace to the point where it's in a durable and well validated state.

Responsibilities:
- Get data from SEC Facts
- Identify Cannon concepts and transition them to be properly cannonized
- Validate Cannon facts with web search, and reconcile any issues


### Phase 2: Generate Narratives

Use Web Search + Known data to write out a list of candidate narratives.  These aren't finalized, but these candidate narratives may help guide us to insights later.

### Phase 3: Generate Insights

Use Web Search + Time Series to search for key insights around the narratives.  
- Are there useful but exotic time series from the SEC Facts?
- Is there recent news that impacts narrative viability?
- Are there macro-economic factors at play?

Output:
- High value concepts are tagged/triaged
- Any key news articles, macro-factors, etc are discovered, enumerated, with links to sources where possible
- Each high value concept has been inspected and insights surfaced to an insights table
- Important or useful ideas from the news articles, etc, are also captured and saved to insights table.
- Key Narrative Cruxes are identified


### Phase 4: Financial Mechanics Experiments

Use scripting tools (RHAI) and the datasets above to experiment with some simple financial models.

We should be ambitious here; writing 50 different scripts that experiment with different financial patterns is a great thing!

These script experiments could represent scenarios like:
- If margins increase dramatically on just this one product segment, but everything else holds to the existing 3 year trend, what does earnings look like in 3 years?
- If the new intense debt financing continues for a couple years, how does the increase in interest expenses impact future EPS?
- If market interest rates overall increase, how does that impact EPS going forward?
- A narrative look at Backlogs and Conversions surfaced this question: how much of the huge RPO balance is near-cash versus long-dated infrastructure commitment?
    - What do future projections look like with a Low / Medium / High Perspective on that?

As the agents write scripts to do simple financial modelling analysis on the data, they should save important or interesting modelling results as insights in the insights table.

### Phase 5: Generate Scenarios + Projections

Decide on what the primary likely scenarios are, and then have an independent agent draft the scenario + projections + cruxes for each.

Lean heavily on the 'insights' table above, as well as the tagged canonical and high value time series.

If useful, write short scripts to execute with RHAI to calculate more fine-grained projections and help with accurate math in the scenario period outputs.


### Phase 6: Draft Report

Final report assembly, done in a context-aware way to ensure overall coherence is achieved


