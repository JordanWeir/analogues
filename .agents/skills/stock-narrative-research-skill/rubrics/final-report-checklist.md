# Final Report Checklist

Run this checklist before giving the user the final report or rendered artifact.

## Core Understanding

- [ ] Does the report explain the company clearly in plain English?
- [ ] Does it explain why the stock matters now?
- [ ] Does it identify the dominant narrative?
- [ ] Does it distinguish business growth, business quality, valuation, and narrative framing?

## Narrative Quality

- [ ] Does it steelman both bull and bear cases?
- [ ] Does it identify what bulls and bears actually disagree about?
- [ ] Does it avoid generic pro/con lists when a deeper crux is available?
- [ ] Does it explain what would change the narrative?

## Projection Quality

- [ ] Does it connect narratives to financial assumptions?
- [ ] Does financial math identify the key economic engines instead of relying only on aggregate revenue growth?
- [ ] Does it show current revenue, profit/margin, and recent growth for important disclosed segments where available?
- [ ] Does it label TAM, penetration, unit, ASP, attach-rate, utilization, or new-market assumptions behind major upside scenarios?
- [ ] Are scenario projections explicitly assumption-driven?
- [ ] Are scenario probabilities explicit, normalized or explained, and connected to the narrative cruxes?
- [ ] Does the Monte Carlo distribution use the terminal low/median/high bands and show histogram bins in the rendered artifact?
- [ ] Are implied price bands framed as illustrative, not predictive?
- [ ] Are formulas, assumptions, and approximate numbers visible?
- [ ] Does it avoid false precision?
- [ ] Does it avoid price-target language?

## Source Quality

- [ ] Are major claims source-backed?
- [ ] Are primary sources used for factual claims?
- [ ] Are commentary sources treated as interpretation?
- [ ] Are allegations labeled as allegations?
- [ ] Are stale, low-quality, or missing sources disclosed?

## Analogue Quality

- [ ] Are historical analogues based on narrative pattern, not shallow sector matching?
- [ ] Are historical analogues caveated?
- [ ] Are analogy breakers included?
- [ ] Does the report say when analogue coverage is weak?

## Final Output

- [ ] Does it include a final talk track?
- [ ] Does it include source notes and limitations?
- [ ] Does it avoid buy/sell/hold recommendations?
- [ ] Does it avoid personalized investment advice?
- [ ] Does `generated/report-data.json` contain the compiled canonical report data?
- [ ] Does `generated/scenario-data.json` contain calculator output from explicit scenario assumptions?
- [ ] Does `generated/report.html` render as a self-contained final artifact?

## If Quality Is Weak

Use this language:

```text
Source coverage is insufficient for a high-confidence report. Here is a partial report and the specific sources or data needed to improve it.
```
